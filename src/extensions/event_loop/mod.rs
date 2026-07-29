//! ztmux's event loop.
//!
//! tmux is written against libevent, so ztmux used to link the C library and
//! call it through FFI. This module is that library's job done in Rust: the
//! same API surface tmux uses — an implicit global base, caller-owned `struct
//! event` registrations, `evbuffer` byte queues and classic `bufferevent`s —
//! with none of the C dependency, so `cargo build` needs no `libevent-dev`, no
//! `pkg-config` probe and no Homebrew prefix, and the binary is
//! self-contained.
//!
//! It lives under `src/extensions` because it is ztmux's own code, not a port
//! of a tmux C file: there is no `vendor/tmux/event.c` to port, only calls
//! into a library that is now ours. The rest of `src/ported` mirrors tmux
//! file for file and is held to that by the anti-drift gate; this subtree
//! answers to its own unit tests instead.
//!
//! Only what the port actually calls is implemented. Everything libevent grew
//! for other users (rate limiting, OpenSSL bufferevents, DNS, HTTP, threading,
//! `event_base_*` multiplicity) is deliberately absent.
//!
//! * [`base`] — registration and dispatch, plus signals and timers.
//! * [`backend`] — the readiness syscall: `select` on macOS, `poll` elsewhere.
//! * [`buffer`] — `evbuffer`.
//! * [`bufev`] — buffered descriptors (`bufferevent`).
use std::ffi::{c_int, c_short, c_void};
use std::ptr::NonNull;

use libc::timeval;

mod backend;
pub(crate) mod base;
mod bufev;
mod buffer;

pub use base::{
    EV_PERSIST, EV_READ, EV_SIGNAL, EV_TIMEOUT, EV_WRITE, EVLOOP_ONCE, event, event_active,
    event_add, event_base, event_del, event_get_method, event_get_version, event_init,
    event_initialized, event_loop, event_once, event_pending, event_reinit, event_set,
    event_set_log_callback,
};
pub use bufev::{
    bufferevent, bufferevent_disable, bufferevent_enable, bufferevent_free, bufferevent_get_output,
    bufferevent_new, bufferevent_setwatermark, bufferevent_write, bufferevent_write_buffer,
};
pub use buffer::{
    evbuffer, evbuffer_add, evbuffer_drain, evbuffer_eol_style, evbuffer_free, evbuffer_get_length,
    evbuffer_new, evbuffer_pullup, evbuffer_read, evbuffer_readline, evbuffer_readln,
    evbuffer_write,
};

/// Serializes the tests that drive the loop.
///
/// There is one base per process — the API tmux uses has no room for a second
/// one — and ztmux is single threaded, so the loop is not built to be shared.
/// The test harness runs tests on several threads, so every test that touches
/// the base takes this first.
#[cfg(test)]
pub(crate) static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Take [`TEST_LOCK`], ignoring poisoning from an unrelated failing test.
#[cfg(test)]
pub(crate) fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Append a formatted string to a buffer, as libevent's `evbuffer_add_printf`.
macro_rules! evbuffer_add_printf {
   ($buf:expr, $fmt:literal $(, $args:expr)* $(,)?) => {
        crate::event_::evbuffer_add_vprintf($buf, format_args!($fmt $(, $args)*))
    };
}
pub(crate) use evbuffer_add_printf;

/// Backing function for [`evbuffer_add_printf`].
///
/// # Safety
/// `buf` must be a live buffer.
#[expect(clippy::disallowed_methods)]
pub unsafe fn evbuffer_add_vprintf(buf: *mut evbuffer, args: std::fmt::Arguments) -> i32 {
    let s = args.to_string(); // TODO this is doing unecessary allocating and freeing
    unsafe { evbuffer_add(buf, s.as_ptr().cast(), s.len()) }
}

// The `evtimer_*` and `signal_*` families are libevent macros over the same
// `event_*` calls; they are kept as functions here so the port reads like the
// C it came from.

/// `evtimer_set`: a timer that calls `cb` with `arg`.
///
/// # Safety
/// `ev` must outlive the registration, and `arg` must stay valid until it fires.
pub unsafe fn evtimer_set<T>(
    ev: *mut event,
    cb: unsafe extern "C-unwind" fn(_: c_int, _: c_short, _: NonNull<T>),
    arg: NonNull<T>,
) {
    unsafe {
        event_set(
            ev,
            -1,
            0,
            std::mem::transmute::<
                Option<unsafe extern "C-unwind" fn(_: c_int, _: c_short, _: NonNull<T>)>,
                Option<unsafe extern "C-unwind" fn(_: c_int, _: c_short, _: *mut c_void)>,
            >(Some(cb)),
            arg.as_ptr().cast(),
        );
    }
}

/// `evtimer_set` for a callback that takes no argument.
///
/// # Safety
/// `ev` must outlive the registration.
pub unsafe fn evtimer_set_no_args(
    ev: *mut event,
    cb: unsafe extern "C-unwind" fn(_: c_int, _: c_short, _: *mut c_void),
) {
    unsafe { event_set(ev, -1, 0, Some(cb), std::ptr::null_mut()) }
}

/// `evtimer_add`: arm a timer for `tv` from now.
///
/// # Safety
/// `ev` must be a configured timer; `tv` must be null or readable.
pub unsafe fn evtimer_add(ev: *mut event, tv: *const timeval) -> c_int {
    unsafe { event_add(ev, tv) }
}

/// `evtimer_initialized`.
///
/// # Safety
/// `ev` must point to readable memory.
pub unsafe fn evtimer_initialized(ev: *mut event) -> bool {
    unsafe { event_initialized(ev) != 0 }
}

/// `evtimer_del`: disarm a timer.
///
/// # Safety
/// `ev` must point to readable, writable memory.
pub unsafe fn evtimer_del(ev: *mut event) -> c_int {
    unsafe { event_del(ev) }
}

/// `evtimer_pending`: whether the timer is armed, and how much time is left.
///
/// # Safety
/// `ev` must point to readable memory; `tv` must be null or writable.
pub unsafe fn evtimer_pending(ev: *const event, tv: *mut timeval) -> c_int {
    unsafe { event_pending(ev, EV_TIMEOUT, tv) }
}

/// `signal_add`.
///
/// # Safety
/// `ev` must be a configured signal event that outlives the registration.
#[inline]
pub unsafe fn signal_add(ev: *mut event, tv: *const timeval) -> i32 {
    unsafe { event_add(ev, tv) }
}

/// `signal_set`: deliver signal `x` to `cb`. Signal events are persistent, so
/// one registration covers every delivery.
///
/// # Safety
/// `ev` must outlive the registration and `arg` must stay valid.
#[inline]
pub unsafe fn signal_set(
    ev: *mut event,
    x: i32,
    cb: Option<unsafe extern "C-unwind" fn(c_int, c_short, *mut c_void)>,
    arg: *mut c_void,
) {
    unsafe { event_set(ev, x, EV_SIGNAL | EV_PERSIST, cb, arg) }
}

/// `EVBUFFER_LENGTH`: bytes queued in a buffer.
///
/// # Safety
/// `x` must be a live buffer.
#[expect(non_snake_case)]
#[inline]
pub unsafe fn EVBUFFER_LENGTH(x: *mut evbuffer) -> usize {
    unsafe { evbuffer_get_length(x) }
}

/// `EVBUFFER_DATA`: a pointer to those bytes.
///
/// # Safety
/// `x` must be a live buffer; the pointer dies with the next add or drain.
#[expect(non_snake_case)]
#[inline]
pub unsafe fn EVBUFFER_DATA(x: *mut evbuffer) -> *mut u8 {
    unsafe { evbuffer_pullup(x, -1) }
}

/// `EVBUFFER_OUTPUT`: the write queue of a buffered descriptor.
///
/// # Safety
/// `x` must be a live bufferevent.
#[expect(non_snake_case)]
#[inline]
pub unsafe fn EVBUFFER_OUTPUT(x: *mut bufferevent) -> *mut evbuffer {
    unsafe { bufferevent_get_output(x) }
}
