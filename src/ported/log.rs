// Copyright (c) 2007 Nicholas Marriott <nicholas.marriott@gmail.com>
//
// Permission to use, copy, modify, and distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF MIND, USE, DATA OR PROFITS, WHETHER
// IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING
// OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
use std::io::BufWriter;
use std::path::PathBuf;
use std::{
    fs::File,
    io::{LineWriter, Write},
    sync::atomic::{AtomicI32, AtomicU64, Ordering},
};

use crate::compat::{stravis, vis_flags};
use crate::event_::event_set_log_callback;
use crate::*;

macro_rules! log_debug {
    ($($arg:tt)*) => {$crate::log::log_debug_rs(format_args!($($arg)*))};
}
pub(crate) use log_debug;

// can't use File because it's open before fork which causes issues with how file works
static LOG_FILE: Mutex<Option<LineWriter<File>>> = Mutex::new(None);
static LOG_LEVEL: AtomicI32 = AtomicI32::new(0);

// Path of the currently open log file, kept alongside LOG_FILE so the rotation
// logic can rename it. Guarded by its own mutex; never locked while LOG_FILE is
// held on the write path (see log_rotate) to avoid a lock-order deadlock.
static LOG_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);
// Approximate bytes written to the current log file since it was opened. Drives
// size-based rotation without a stat() per line.
static LOG_BYTES: AtomicU64 = AtomicU64::new(0);

/// Rotate the log once it grows past this many bytes (16 MiB). A long-lived
/// server that runs with logging enabled would otherwise grow one unbounded
/// file.
const LOG_MAX_BYTES: u64 = 16 * 1024 * 1024;
/// How many rotated files to keep: `<log>.1` .. `<log>.LOG_BACKUPS`.
const LOG_BACKUPS: usize = 5;

const DEFAULT_ORDERING: Ordering = Ordering::SeqCst;

/// C `vendor/tmux/log.c:34`: `static void log_event_cb(__unused int severity, const char *msg)`
unsafe extern "C-unwind" fn log_event_cb(_severity: c_int, msg: *const u8) {
    unsafe { log_debug!("{}", _s(msg)) }
}

/// C `vendor/tmux/log.c:41`: `void log_add_level(void)`
pub fn log_add_level() {
    LOG_LEVEL.fetch_add(1, DEFAULT_ORDERING);
}

/// C `vendor/tmux/log.c:48`: `int log_get_level(void)`
pub fn log_get_level() -> i32 {
    LOG_LEVEL.load(DEFAULT_ORDERING)
}

/// C `vendor/tmux/log.c:55`: `void log_open(const char *name)`
pub fn log_open(name: &CStr) {
    if LOG_LEVEL.load(DEFAULT_ORDERING) == 0 {
        return;
    }

    log_close();
    let pid = std::process::id();
    let path = crate::extensions::diagnostics::dir().join(format!(
        "tmux-{}-{}.log",
        name.to_str().unwrap(),
        pid
    ));
    let Ok(file) = std::fs::File::options()
        .read(false)
        .append(true)
        .create(true)
        .open(&path)
    else {
        return;
    };

    // Seed the byte counter from any pre-existing content (append mode) so
    // rotation accounts for a reopened file too.
    let existing = file.metadata().map_or(0, |m| m.len());
    LOG_BYTES.store(existing, DEFAULT_ORDERING);
    *LOG_PATH.lock().unwrap() = Some(path);
    *LOG_FILE.lock().unwrap() = Some(LineWriter::new(file));
    unsafe { event_set_log_callback(Some(log_event_cb)) };
}

/// C `vendor/tmux/log.c:75`: `void log_toggle(const char *name)`
pub fn log_toggle(name: &CStr) {
    if LOG_LEVEL.fetch_xor(1, DEFAULT_ORDERING) == 0 {
        log_open(name);
        log_debug!("log opened");
    } else {
        log_debug!("log closed");
        log_close();
    }
}

/// C `vendor/tmux/log.c:90`: `void log_close(void)`
pub fn log_close() {
    // If we drop the file when it's already closed it will panic in debug mode.
    // Because of this and our use of fork, extra care has to be made when closing the file.
    // see std::sys::pal::unix::fs::debug_assert_fd_is_open;
    use std::os::fd::AsRawFd;
    if let Some(mut old_handle) = LOG_FILE.lock().unwrap().take() {
        let _flush_err = old_handle.flush(); // TODO
        match old_handle.into_inner() {
            Ok(file) => unsafe {
                libc::close(file.as_raw_fd());
                std::mem::forget(file);
            },
            Err(err) => {
                let lw = err.into_inner();
                // TODO this is invalid, and compiler version dependent, but prevents a memory leak
                // need a way to properly get out the file and drop the buffer
                unsafe {
                    let bw = std::mem::transmute::<
                        std::io::LineWriter<std::fs::File>,
                        BufWriter<File>,
                    >(lw);
                    let (file, _) = bw.into_parts();
                    std::mem::forget(file);
                }
            }
        }

        unsafe {
            event_set_log_callback(None);
        }
        *LOG_PATH.lock().unwrap() = None;
        LOG_BYTES.store(0, DEFAULT_ORDERING);
    }
}

#[track_caller]
pub fn log_debug_rs(args: std::fmt::Arguments) {
    // Fast path when logging is disabled: a single atomic load, matching C
    // tmux's `if (log_level == 0) return;`. Must not take the LOG_FILE mutex
    // here - log_debug! is called thousands of times per frame on the hot
    // parse/redraw path, and locking a mutex per call saturates CPU under
    // heavy TUI output (the pane-freeze bug).
    if LOG_LEVEL.load(DEFAULT_ORDERING) == 0 {
        return;
    }
    log_vwrite_rs(args, "");
}

#[track_caller]
fn log_vwrite_rs(args: std::fmt::Arguments, prefix: &str) {
    unsafe {
        if LOG_FILE.lock().unwrap().is_none() {
            return;
        }

        // Mirror C string semantics. The C original (`log_vwrite`) formats via
        // vasprintf then passes the result to stravis, where an embedded NUL —
        // e.g. a raw C0 control byte logged through `%c` in `input_c0_dispatch`
        // — simply terminates the string. `CString::new(..).unwrap()` instead
        // aborts the whole server on that NUL, so truncate at the first NUL to
        // match C and guarantee the conversion can never fail.
        let raw = format!("{args}");
        let end = raw
            .as_bytes()
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(raw.len());
        let msg = CString::new(&raw.as_bytes()[..end])
            .expect("slice ends at first NUL, so it is NUL-free");
        let mut out = null_mut();
        if stravis(
            &mut out,
            msg.as_ptr().cast(),
            vis_flags::VIS_OCTAL | vis_flags::VIS_CSTYLE | vis_flags::VIS_TAB | vis_flags::VIS_NL,
        ) == -1
        {
            return;
        }
        let duration = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = duration.as_secs();
        let micros = duration.subsec_micros();

        let str_out = CStr::from_ptr(out.cast()).to_string_lossy();
        let location = std::panic::Location::caller();
        let file = location.file();
        let line = location.line();
        let out_line = format!("{secs}.{micros:06} {file}:{line} {prefix}{str_out}\n");
        if let Some(f) = LOG_FILE.lock().unwrap().as_mut() {
            let _ = f.write_all(out_line.as_bytes());
        }

        crate::free_(out);

        // Track size and rotate once the file grows too large. Done after the
        // LOG_FILE lock is released above, since log_rotate reacquires it.
        let written =
            LOG_BYTES.fetch_add(out_line.len() as u64, DEFAULT_ORDERING) + out_line.len() as u64;
        if written >= LOG_MAX_BYTES {
            // Size-based rotation, kept as a local helper of its only caller so
            // it stays bound to the log-file statics it manages. When the active
            // log passes LOG_MAX_BYTES: close it, shift `<log>.(N-1)` ->
            // `<log>.N` (dropping the oldest), move the live file to `<log>.1`,
            // and reopen a fresh one. Best-effort - any fs error just leaves the
            // current file in place. Must not run while the LOG_FILE lock is
            // held (it reacquires it via log_close/reopen).
            fn log_rotate() {
                let base = { LOG_PATH.lock().unwrap().clone() };
                let Some(base) = base else { return };

                log_close();

                let suffixed = |n: usize| -> PathBuf {
                    let mut p = base.clone().into_os_string();
                    p.push(format!(".{n}"));
                    PathBuf::from(p)
                };

                let _ = std::fs::remove_file(suffixed(LOG_BACKUPS));
                for n in (1..LOG_BACKUPS).rev() {
                    let _ = std::fs::rename(suffixed(n), suffixed(n + 1));
                }
                let _ = std::fs::rename(&base, suffixed(1));

                if let Ok(file) = std::fs::File::options()
                    .read(false)
                    .append(true)
                    .create(true)
                    .open(&base)
                {
                    LOG_BYTES.store(0, DEFAULT_ORDERING);
                    *LOG_PATH.lock().unwrap() = Some(base);
                    *LOG_FILE.lock().unwrap() = Some(LineWriter::new(file));
                    unsafe { event_set_log_callback(Some(log_event_cb)) };
                }
            }
            log_rotate();
        }
    }
}

/// C `vendor/tmux/log.c:140`: `__dead void fatal(const char *msg, ...)`
pub fn fatal(msg: &str) -> ! {
    let os_error = std::io::Error::last_os_error();
    let error_msg = os_error.to_string();

    let prefix = format!("fatal: {error_msg}: ");

    log_vwrite_rs(format_args!("{msg}"), &prefix);
    crate::extensions::diagnostics::dump_backtrace(&format!("fatal: {msg}: {error_msg}"));

    std::process::exit(1)
}

macro_rules! fatalx_ {
   ($fmt:literal $(, $args:expr)* $(,)?) => {
        crate::log::fatalx_c(format_args!($fmt $(, $args)*))
    };
}
pub(crate) use fatalx_;
pub fn fatalx_c(args: std::fmt::Arguments) -> ! {
    log_vwrite_rs(args, "fatal: ");
    crate::extensions::diagnostics::dump_backtrace(&format!("fatalx: {args}"));
    std::process::exit(1)
}

#[track_caller]
/// C `vendor/tmux/log.c:157`: `__dead void fatalx(const char *msg, ...)`
pub fn fatalx(msg: &str) -> ! {
    let location = std::panic::Location::caller();
    let file = location.file();
    let line = location.line();

    log_vwrite_rs(format_args!("{file}:{line} {msg}"), "fatal: ");
    crate::extensions::diagnostics::dump_backtrace(&format!("fatalx: {file}:{line} {msg}"));
    std::process::exit(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Both checks below read/mutate the process-global LOG_LEVEL / LOG_FILE, so
    // they must run as a single (non-parallel) test to avoid racing each other.
    #[test]
    fn test_log_level_and_disabled_fast_path() {
        // Regression (bug 5): the logging-disabled fast path must gate on the
        // LOG_LEVEL atomic ALONE and must not take the LOG_FILE mutex - on the
        // hot parse/redraw path that lock, taken thousands of times per frame,
        // saturated the CPU and froze panes under heavy TUI output. Prove the
        // fast path never touches LOG_FILE by calling log_debug! while holding
        // that mutex: with the fix it returns via the atomic check; a regression
        // to locking LOG_FILE here would deadlock (std Mutex is not reentrant).
        assert_eq!(log_get_level(), 0, "logging must be disabled for this test");
        {
            let _held = LOG_FILE.lock().unwrap();
            log_debug!("hot-path message {} {}", 1, "two");
        }

        // C `vendor/tmux/log.c`: log_add_level increments and log_get_level
        // reads the level counter. Restore it afterwards so the global stays at
        // its disabled default for any later use.
        log_add_level();
        assert_eq!(log_get_level(), 1);
        LOG_LEVEL.fetch_sub(1, DEFAULT_ORDERING);
        assert_eq!(log_get_level(), 0);
    }
}
