//! ztmux-original crash diagnostics — no upstream tmux counterpart.
//!
//! Provides the `~/.ztmux` output directory, a best-effort crash backtrace
//! dump, and fatal-signal handlers so an unexpected server death always leaves
//! a `server-crash-<pid>.txt` behind. Consumed by `crate::log` (fatal/fatalx),
//! `crate::server` (the panic hook + signal-handler install), and log rotation.
use core::ffi::c_int;
use std::io::Write as _;
use std::path::PathBuf;
use std::ptr::null_mut;

/// The ztmux home directory — `~/.ztmux` — resolved without creating
/// anything, so a caller can ask about a path it does not want to bring into
/// existence (`ztmux shadow`'s install root, and `ztmux doctor` reporting on a
/// shadow that was never installed). `None` when `$HOME` is unset or empty.
pub(crate) fn path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".ztmux"))
}

/// Directory that holds every ztmux log and crash file: `~/.ztmux` (created if
/// missing, tightened to mode 0700 since logs can contain pane contents).
///
/// Falls back to the current directory only if `$HOME` is unset or the
/// directory can't be created, so diagnostics never fail hard. Used by
/// `log_open`, `dump_backtrace`, and the server panic hook so all output lands
/// in one predictable place regardless of where the server was launched.
pub(crate) fn dir() -> PathBuf {
    let Some(base) = path() else {
        return PathBuf::from(".");
    };

    if let Err(err) = std::fs::create_dir_all(&base) {
        eprintln!("ztmux: failed to create {}: {err}", base.display());
        return PathBuf::from(".");
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700));
    }

    base
}

/// Best-effort crash dump used by every abnormal server exit path
/// (fatal/fatalx and the fatal-signal handler installed in `server_start`).
///
/// Captures a backtrace and records it in three places so the reason a
/// long-running server died is never lost:
///  * the debug log (only when logging is enabled),
///  * a standalone `~/.ztmux/server-crash-<pid>.txt` file (always, regardless
///    of log level),
///  * stderr.
///
/// `Backtrace::force_capture()` allocates, so this is not strictly
/// async-signal-safe when called from a signal handler; in practice it is a
/// reliable best-effort for diagnosing where the server went down.
pub(crate) fn dump_backtrace(reason: &str) {
    let backtrace = std::backtrace::Backtrace::force_capture();
    let pid = std::process::id();
    let body = format!("ztmux server exit: {reason}\npid: {pid}\n\n{backtrace:#?}\n");

    // Mirror it into the debug log (no-op unless logging is on).
    crate::log::log_debug_rs(format_args!("crash: {reason}\n{backtrace:#?}"));

    // Always drop a standalone file so we get a trace even with logging off.
    let path = dir().join(format!("server-crash-{pid}.txt"));
    if let Err(err) = std::fs::write(&path, &body) {
        eprintln!(
            "ztmux: failed to write crash dump to {}: {err}",
            path.display()
        );
    }

    let _ = std::io::stderr().write_all(body.as_bytes());
}

/// Fatal-signal handler installed in the server process. A hardware fault or
/// abort (SIGSEGV/SIGBUS/SIGABRT/SIGILL/SIGFPE) kills the process outright and
/// never runs the Rust panic hook, so those crashes previously left no trace.
/// Dump a backtrace, then restore the default disposition and re-raise so the
/// process still terminates with the original signal (and can core dump).
unsafe extern "C" fn crash_signal(sig: c_int) {
    unsafe {
        let name = crate::_s(crate::libc::strsignal(sig).cast::<u8>());
        dump_backtrace(&format!("fatal signal {sig} ({name})"));

        // Restore the default handler for this signal and re-raise it.
        let mut sa: crate::libc::sigaction = std::mem::zeroed();
        crate::libc::sigemptyset(&raw mut sa.sa_mask);
        sa.sa_flags = 0;
        sa.sa_sigaction = crate::libc::SIG_DFL;
        crate::libc::sigaction(sig, &raw const sa, null_mut());
        crate::libc::raise(sig);
    }
}

/// Install `crash_signal` for the fatal signals so an unexpected server death
/// always leaves a `server-crash-<pid>.txt` behind.
pub(crate) unsafe fn install_crash_handlers() {
    unsafe {
        let mut sa: crate::libc::sigaction = std::mem::zeroed();
        crate::libc::sigemptyset(&raw mut sa.sa_mask);
        // No SA_RESTART: we are re-raising to die, not resuming.
        sa.sa_flags = 0;
        sa.sa_sigaction = crash_signal as *const () as usize;

        for sig in [
            crate::libc::SIGSEGV,
            crate::libc::SIGBUS,
            crate::libc::SIGABRT,
            crate::libc::SIGILL,
            crate::libc::SIGFPE,
        ] {
            crate::libc::sigaction(sig, &raw const sa, null_mut());
        }
    }
}

/// Cap for an append-only audit log (see [`record_event`]); at ~2 KB an entry
/// that is thousands of events, far more than any real occurrence needs.
const AUDIT_LOG_MAX: u64 = 8 * 1024 * 1024;

/// Local `%Y-%m-%d %H:%M:%S` stamp for an audit line.
fn timestamp() -> String {
    unsafe {
        let now: crate::libc::time_t = crate::libc::time(null_mut());
        let mut tm: crate::libc::tm = std::mem::zeroed();
        if crate::libc::localtime_r(&raw const now, &raw mut tm).is_null() {
            return now.to_string();
        }
        let mut buf = [0u8; 32];
        let n = crate::libc::strftime(
            buf.as_mut_ptr(),
            buf.len(),
            crate::c!("%Y-%m-%d %H:%M:%S"),
            &raw mut tm,
        );
        String::from_utf8_lossy(&buf[..n]).into_owned()
    }
}

/// Append a timestamped event and the caller's backtrace to `~/.ztmux/<file>`.
///
/// For state changes that are rare, unreproducible on demand, and invisible
/// once they have happened — where the only way to learn the cause is to have
/// recorded the caller at the time. Unlike `log_debug!`, this does not need
/// `-v` to have been passed to the server that is about to hit the bug, which
/// is the whole point: the server that loses the state has already been running
/// for days. Never writes to the terminal; the file grows to at most
/// [`AUDIT_LOG_MAX`], after which further events are dropped.
pub(crate) fn record_event(file: &str, what: &str) {
    // Unit tests drive the same code paths in-process; they must not write into
    // the developer's real `~/.ztmux`.
    if cfg!(test) {
        return;
    }
    let backtrace = std::backtrace::Backtrace::force_capture();
    crate::log::log_debug_rs(format_args!("{what}"));

    let path = dir().join(file);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() >= AUDIT_LOG_MAX) {
        return;
    }
    let entry = format!(
        "{} pid {} {what}\n{backtrace:#?}\n\n",
        timestamp(),
        std::process::id(),
    );
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = f.write_all(entry.as_bytes());
    }
}

/// ztmux: record one key-table destruction, with the caller's backtrace, to
/// `~/.ztmux/key-tables.log`.
///
/// A table leaving `KEY_TABLES` is the state change behind "all my key
/// bindings disappeared": afterwards the server looks like one that was never
/// configured (`server_client_set_key_table` silently recreates `root`/`prefix`
/// empty on the next key), so nothing about the running process says which code
/// path removed them. Destruction is rare — the tree holds a reference for the
/// table's whole life — so recording every one costs nothing and is the only
/// way to name the path after the fact.
pub(crate) unsafe fn record_table_drop(what: &str, table: *mut crate::key_table) {
    unsafe {
        let keys = crate::rb_foreach(&raw mut (*table).key_bindings).count();
        let defaults = crate::rb_foreach(&raw mut (*table).default_key_bindings).count();
        record_event(
            "key-tables.log",
            &format!(
                "{what}: table {} ({keys} bindings, {defaults} defaults, {} references)",
                crate::_s((*table).name_ptr()),
                (*table).references,
            ),
        );
    }
}
