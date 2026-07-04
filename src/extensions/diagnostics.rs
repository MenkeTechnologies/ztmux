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

/// Directory that holds every ztmux log and crash file: `~/.ztmux` (created if
/// missing, tightened to mode 0700 since logs can contain pane contents).
///
/// Falls back to the current directory only if `$HOME` is unset or the
/// directory can't be created, so diagnostics never fail hard. Used by
/// `log_open`, `dump_backtrace`, and the server panic hook so all output lands
/// in one predictable place regardless of where the server was launched.
pub(crate) fn dir() -> PathBuf {
    let base = match std::env::var_os("HOME") {
        Some(home) if !home.is_empty() => PathBuf::from(home).join(".ztmux"),
        _ => return PathBuf::from("."),
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
