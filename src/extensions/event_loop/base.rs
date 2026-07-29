//! The event base: descriptor, timer and signal registration plus the dispatch
//! loop.
//!
//! Same shape as libevent's classic (pre-`event_base_*`) API, because that is
//! the API tmux is written against: one implicit global base created by
//! `event_init`, events owned by the caller and embedded by value in tmux's
//! structs, and callbacks of the form `fn(fd, events, arg)`.
//!
//! A registered event is tracked by the address of the caller's `struct event`.
//! Nothing is stored behind that pointer that the loop needs to read while the
//! event is not registered, so freeing a struct that holds a deleted event is
//! safe — and `event_del` scrubs the pending and active queues, so a callback
//! that frees another event's owner cannot leave a dangling entry behind.
use std::collections::{HashMap, VecDeque};
use std::ffi::{c_int, c_short, c_void};
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use libc::timeval;

use crate::event_::backend::{self, Watch};

/// Callback signature shared by every event.
pub type event_callback_fn = Option<unsafe extern "C-unwind" fn(c_int, c_short, *mut c_void)>;

/// Where libevent's own diagnostics go (`event_set_log_callback`).
pub type event_log_cb = Option<unsafe extern "C-unwind" fn(severity: c_int, msg: *const u8)>;

/// Log severity handed to [`event_log_cb`], matching libevent's `_EVENT_LOG_*`.
const EVENT_LOG_ERR: c_int = 3;

// Event bits, same values as libevent so tmux's constants keep their meaning.
pub const EV_TIMEOUT: i16 = 0x01;
pub const EV_READ: i16 = 0x02;
pub const EV_WRITE: i16 = 0x04;
pub const EV_SIGNAL: i16 = 0x08;
pub const EV_PERSIST: i16 = 0x10;

// Loop flags. libevent's remaining flags (`EVLOOP_NO_EXIT_ON_EMPTY`) and event
// bits (`EV_ET`, `EV_FINALIZE`, `EV_CLOSED`) have no user in the port and no
// implementation here.
pub const EVLOOP_ONCE: i32 = 0x01;
pub const EVLOOP_NONBLOCK: i32 = 0x02;

/// Set in `ev_flags` once `event_set` has run, so `event_initialized` can tell
/// a configured event from the zeroed memory tmux embeds in its structs.
const EVLIST_INIT: c_short = 0x80;
/// Set while the event is registered with the base.
const EVLIST_PENDING: c_short = 0x01;

/// A single event registration, owned by the caller.
///
/// tmux embeds these by value in zero-initialized structs, so every field must
/// read correctly as "not initialized" when the memory is all zero.
#[repr(C)]
pub struct event {
    /// Descriptor for `EV_READ`/`EV_WRITE`, signal number for `EV_SIGNAL`,
    /// and -1 for a pure timer.
    pub ev_fd: c_int,
    /// What the caller asked for.
    pub ev_events: c_short,
    /// What fired, as passed to the callback.
    pub ev_res: c_short,
    /// `EVLIST_INIT` / `EVLIST_PENDING`.
    pub ev_flags: c_short,
    pub ev_callback: event_callback_fn,
    pub ev_arg: *mut c_void,
    pub ev_base: *mut event_base,
    /// Registration counter, so a queue entry left over from an earlier
    /// registration can be recognized and dropped.
    pub ev_gen: u64,
}

/// The loop's view of a registered event.
struct Pending {
    generation: u64,
    fd: c_int,
    events: c_short,
    /// When the timeout expires, for a registration that carries one.
    deadline: Option<Instant>,
    /// The timeout as given, to re-arm a persistent timer.
    interval: Option<Duration>,
}

/// An event whose callback is due to run.
#[derive(Clone, Copy)]
struct Active {
    ev: usize,
    generation: u64,
    res: c_short,
}

/// The event loop state. One per process; `event_init` installs it as the
/// current base and everything else reaches it through [`with_base`].
pub struct event_base {
    pending: HashMap<usize, Pending>,
    active: VecDeque<Active>,
    /// Read and write ends of the pipe the signal handler pokes to wake the
    /// loop out of its wait.
    signal_pipe: [c_int; 2],
    /// Registration counter handed out to events.
    next_gen: u64,
}

/// The process-wide base. tmux is single threaded: one base, created once by
/// `event_init`, used by every `event_*` call.
static mut CURRENT_BASE: *mut event_base = std::ptr::null_mut();

/// Signals raised since the last loop turn, one bit per signal number. Written
/// from the signal handler, so it must stay a plain atomic.
static SIGNALS_RAISED: AtomicU64 = AtomicU64::new(0);

/// Write end of the signal pipe, for the handler, or -1 when there is none.
/// Kept separately from the base because the handler must not touch anything
/// that needs locking or borrowing.
static SIGNAL_PIPE_WRITE: AtomicI32 = AtomicI32::new(-1);

/// Where to send loop diagnostics, if anyone asked for them.
static mut LOG_CALLBACK: event_log_cb = None;

/// Run `f` against the current base, creating it if `event_init` has not run —
/// teardown paths reach `event_del` without one.
///
/// The borrow must not span a callback: a callback re-enters the `event_*` API
/// and would take a second borrow. Every caller here keeps the closure to
/// bookkeeping and dispatches outside it.
fn with_base<R>(f: impl FnOnce(&mut event_base) -> R) -> R {
    // SAFETY: single-threaded; the pointer is set by `event_init` before any
    // other event call and stays valid for the life of the process.
    let mut base = unsafe { CURRENT_BASE };
    if base.is_null() {
        base = event_init();
    }
    f(unsafe { &mut *base })
}

/// Report a loop failure through the configured log callback.
fn log_err(msg: &str) {
    // SAFETY: single-threaded read of the callback slot; the message is
    // NUL-terminated below.
    unsafe {
        let cb = LOG_CALLBACK;
        if let Some(cb) = cb {
            let c = std::ffi::CString::new(msg).unwrap_or_default();
            cb(EVENT_LOG_ERR, c.as_ptr().cast());
        }
    }
}

/// Create the event base and make it current.
pub fn event_init() -> *mut event_base {
    let base = Box::into_raw(Box::new(event_base {
        pending: HashMap::new(),
        active: VecDeque::new(),
        signal_pipe: [-1, -1],
        next_gen: 1,
    }));
    // SAFETY: single-threaded, and this runs before any other event call.
    unsafe {
        CURRENT_BASE = base;
        open_signal_pipe(&mut *base);
    }
    base
}

/// Rebuild the parts of the base that do not survive `fork`: the wakeup pipe
/// and the installed signal handlers. Registrations are kept, exactly as
/// libevent's `event_reinit` keeps them.
///
/// # Safety
/// `base` must be the current base.
pub unsafe fn event_reinit(base: *mut event_base) -> c_int {
    unsafe {
        let b = &mut *base;
        close_signal_pipe(b);
        open_signal_pipe(b);

        // Re-arm a handler for every signal still registered. The fork
        // inherited the dispositions, but the pipe they wrote to is gone.
        let signals: Vec<c_int> = b
            .pending
            .values()
            .filter(|p| p.events & EV_SIGNAL != 0)
            .map(|p| p.fd)
            .collect();
        for sig in signals {
            install_signal_handler(sig);
        }
        0
    }
}

/// Point the loop's diagnostics at `cb`.
///
/// # Safety
/// `cb` must stay callable for as long as it is installed.
pub unsafe fn event_set_log_callback(cb: event_log_cb) {
    // SAFETY: single-threaded.
    unsafe { LOG_CALLBACK = cb }
}

/// Identifies this loop where tmux would print libevent's version.
pub fn event_get_version() -> *const u8 {
    c"ztmux-event 1.0".as_ptr().cast()
}

/// The readiness syscall in use, the counterpart of libevent's backend name.
pub fn event_get_method() -> *const u8 {
    if cfg!(target_os = "macos") {
        c"select".as_ptr().cast()
    } else {
        c"poll".as_ptr().cast()
    }
}

/// Configure an event. Does not register it; that is [`event_add`].
///
/// # Safety
/// `ev` must point to writable memory that outlives every registration of it.
pub unsafe fn event_set(
    ev: *mut event,
    fd: c_int,
    events: c_short,
    callback: event_callback_fn,
    arg: *mut c_void,
) {
    unsafe {
        // Reconfiguring a registered event drops the old registration first,
        // the way libevent's event_assign does.
        if is_pending(ev) {
            event_del(ev);
        }
        (*ev).ev_fd = fd;
        (*ev).ev_events = events;
        (*ev).ev_res = 0;
        (*ev).ev_callback = callback;
        (*ev).ev_arg = arg;
        (*ev).ev_flags = EVLIST_INIT;
        (*ev).ev_base = with_base(std::ptr::from_mut);
    }
}

/// Whether this event has been configured by [`event_set`].
///
/// # Safety
/// `ev` must point to readable memory (all-zero counts as not initialized).
pub unsafe fn event_initialized(ev: *const event) -> c_int {
    unsafe { c_int::from((*ev).ev_flags & EVLIST_INIT != 0) }
}

/// Whether `ev` is currently registered.
///
/// # Safety
/// `ev` must point to readable memory.
unsafe fn is_pending(ev: *const event) -> bool {
    unsafe { (*ev).ev_flags & EVLIST_PENDING != 0 }
}

/// Register `ev`, with `timeout` as a relative deadline when non-null. A
/// registration replaces any earlier one for the same event.
///
/// # Safety
/// `ev` must have been configured by [`event_set`] and must stay alive and
/// unmoved until it fires (non-persistent) or is deleted.
pub unsafe fn event_add(ev: *mut event, timeout: *const timeval) -> c_int {
    unsafe {
        let interval = if timeout.is_null() {
            None
        } else {
            Some(Duration::new(
                (*timeout).tv_sec.max(0) as u64,
                ((*timeout).tv_usec.max(0) as u32).min(999_999) * 1_000,
            ))
        };

        if (*ev).ev_events & EV_SIGNAL != 0 {
            install_signal_handler((*ev).ev_fd);
        }

        let generation = with_base(|base| {
            let generation = base.next_gen;
            base.next_gen += 1;
            // Drop any earlier registration, including queued activations.
            base.active.retain(|a| a.ev != ev as usize);
            base.pending.insert(
                ev as usize,
                Pending {
                    generation,
                    fd: (*ev).ev_fd,
                    events: (*ev).ev_events,
                    deadline: interval.map(|d| Instant::now() + d),
                    interval,
                },
            );
            generation
        });

        (*ev).ev_gen = generation;
        (*ev).ev_flags |= EVLIST_PENDING;
        0
    }
}

/// Unregister `ev`, dropping any activation queued for it. Safe to call on an
/// event that is not registered, and on one that was never added.
///
/// # Safety
/// `ev` must point to readable, writable memory.
pub unsafe fn event_del(ev: *mut event) -> c_int {
    unsafe {
        let signal = if (*ev).ev_flags & EVLIST_INIT != 0 && (*ev).ev_events & EV_SIGNAL != 0 {
            Some((*ev).ev_fd)
        } else {
            None
        };

        let last_for_signal = with_base(|base| {
            base.pending.remove(&(ev as usize));
            base.active.retain(|a| a.ev != ev as usize);
            signal.is_some_and(|sig| {
                !base
                    .pending
                    .values()
                    .any(|p| p.events & EV_SIGNAL != 0 && p.fd == sig)
            })
        });
        if last_for_signal && let Some(sig) = signal {
            restore_signal_handler(sig);
        }

        (*ev).ev_flags &= !EVLIST_PENDING;
        0
    }
}

/// Queue `ev`'s callback to run on the next loop turn with `res` as the reported
/// events. `ncalls` exists for libevent signature compatibility and is ignored;
/// tmux only ever passes 1.
///
/// # Safety
/// `ev` must be a configured event that stays alive until the loop runs it.
pub unsafe fn event_active(ev: *mut event, res: c_int, _ncalls: c_short) {
    unsafe {
        let generation = (*ev).ev_gen;
        with_base(|base| {
            base.active.push_back(Active {
                ev: ev as usize,
                generation,
                res: res as c_short,
            });
        });
    }
}

/// Whether `ev` is registered for any of `events`; when `tv` is non-null and a
/// timeout is pending, it receives the remaining time.
///
/// # Safety
/// `ev` must point to readable memory; `tv` must be null or writable.
pub unsafe fn event_pending(ev: *const event, events: c_short, tv: *mut timeval) -> c_int {
    unsafe {
        if !is_pending(ev) {
            return 0;
        }
        let (mut found, remaining) = with_base(|base| {
            let Some(p) = base.pending.get(&(ev as usize)) else {
                return (0, None);
            };
            let mut found = p.events & events & (EV_READ | EV_WRITE | EV_SIGNAL);
            if p.deadline.is_some() && events & EV_TIMEOUT != 0 {
                found |= EV_TIMEOUT;
            }
            (
                found,
                p.deadline
                    .map(|d| d.saturating_duration_since(Instant::now())),
            )
        });
        if found == 0 {
            return 0;
        }
        if !tv.is_null()
            && let Some(left) = remaining
        {
            (*tv).tv_sec = left.as_secs() as libc::time_t;
            (*tv).tv_usec = left.subsec_micros() as libc::suseconds_t;
            found |= EV_TIMEOUT;
        }
        c_int::from(found != 0)
    }
}

/// A one-shot event the loop owns, created by [`event_once`] and freed once its
/// callback has run.
struct OnceEvent {
    ev: event,
    callback: event_callback_fn,
    arg: *mut c_void,
}

/// Run `callback` once, when `fd` is ready for `events` or after `tv`. A null or
/// zero `tv` on a pure timeout fires on the next loop turn, which is how tmux
/// defers work out of the current callback.
///
/// # Safety
/// `arg` must stay valid until the callback runs.
pub unsafe fn event_once(
    fd: c_int,
    events: c_short,
    callback: event_callback_fn,
    arg: *mut c_void,
    tv: *const timeval,
) -> c_int {
    unsafe {
        let once = Box::into_raw(Box::new(OnceEvent {
            ev: zeroed_event(),
            callback,
            arg,
        }));
        event_set(
            &raw mut (*once).ev,
            fd,
            events & !EV_PERSIST,
            Some(event_once_trampoline),
            once.cast(),
        );

        let immediate =
            events & (EV_READ | EV_WRITE | EV_SIGNAL) == 0 && (tv.is_null() || is_zero(tv));
        if immediate {
            // No descriptor and no delay: register so event_del can still find
            // it, then queue it for the next turn.
            event_add(&raw mut (*once).ev, std::ptr::null());
            event_active(&raw mut (*once).ev, EV_TIMEOUT as c_int, 1);
        } else {
            event_add(&raw mut (*once).ev, tv);
        }
        0
    }
}

/// Whether a timeout is zero, i.e. "as soon as possible".
///
/// # Safety
/// `tv` must be non-null and readable.
unsafe fn is_zero(tv: *const timeval) -> bool {
    unsafe { (*tv).tv_sec == 0 && (*tv).tv_usec == 0 }
}

/// Deliver a [`event_once`] callback and free the event that carried it.
unsafe extern "C-unwind" fn event_once_trampoline(fd: c_int, events: c_short, arg: *mut c_void) {
    unsafe {
        let once = Box::from_raw(arg.cast::<OnceEvent>());
        event_del(&raw const once.ev as *mut event);
        if let Some(cb) = once.callback {
            cb(fd, events, once.arg);
        }
    }
}

/// A fresh, unconfigured event, for the loop's own allocations.
fn zeroed_event() -> event {
    event {
        ev_fd: -1,
        ev_events: 0,
        ev_res: 0,
        ev_flags: 0,
        ev_callback: None,
        ev_arg: std::ptr::null_mut(),
        ev_base: std::ptr::null_mut(),
        ev_gen: 0,
    }
}

/// Run the loop.
///
/// `EVLOOP_ONCE` (what tmux uses) waits for something to happen, runs
/// everything that became ready, and returns. `EVLOOP_NONBLOCK` never waits.
/// Without either flag this keeps going until no registrations are left.
pub fn event_loop(flags: c_int) -> c_int {
    loop {
        let ran = run_one_turn(flags & EVLOOP_NONBLOCK != 0);

        if flags & (EVLOOP_ONCE | EVLOOP_NONBLOCK) != 0 {
            return 0;
        }
        if !ran && with_base(|base| base.pending.is_empty() && base.active.is_empty()) {
            return 0;
        }
    }
}

/// One turn: wait for readiness (unless something is already due), then run
/// every callback that came up. Returns whether any callback ran.
fn run_one_turn(nonblock: bool) -> bool {
    // Anything already queued (event_active, or a leftover from the previous
    // turn) runs without waiting.
    let due = with_base(|base| !base.active.is_empty());
    if !due {
        let (mut watches, owners, timeout) = with_base(collect_watches);
        let timeout = if nonblock {
            Some(Duration::ZERO)
        } else {
            timeout
        };
        if let Err(err) = backend::wait(&mut watches, timeout) {
            log_err(&format!("event loop wait failed: {err}"));
            return false;
        }
        with_base(|base| activate_ready(base, &watches, &owners));
    }

    dispatch()
}

/// Build the descriptor list for this turn and work out how long we may wait.
/// The first watch is always the signal pipe; the rest are paired with the
/// event that asked for them, so nothing depends on map iteration order.
fn collect_watches(base: &mut event_base) -> (Vec<Watch>, Vec<usize>, Option<Duration>) {
    let mut watches = vec![Watch::new(base.signal_pipe[0], EV_READ)];
    let mut owners = vec![0usize];
    let mut earliest: Option<Instant> = None;

    for (ev, p) in &base.pending {
        if p.events & (EV_READ | EV_WRITE) != 0 && p.fd >= 0 {
            watches.push(Watch::new(p.fd, p.events & (EV_READ | EV_WRITE)));
            owners.push(*ev);
        }
        if let Some(deadline) = p.deadline {
            earliest = Some(earliest.map_or(deadline, |e: Instant| e.min(deadline)));
        }
    }

    let timeout = earliest.map(|d| d.saturating_duration_since(Instant::now()));
    (watches, owners, timeout)
}

/// Queue every event that the wait, the clock or a signal made ready.
fn activate_ready(base: &mut event_base, watches: &[Watch], owners: &[usize]) {
    // The signal pipe is watch 0; drain it and pick up the raised signals.
    let raised = if watches[0].got & EV_READ != 0 {
        drain_signal_pipe(base.signal_pipe[0]);
        SIGNALS_RAISED.swap(0, Ordering::Relaxed)
    } else {
        0
    };

    let now = Instant::now();
    let mut ready: Vec<Active> = Vec::new();

    // What the descriptors reported, per event.
    let mut io_res: HashMap<usize, c_short> = HashMap::new();
    for (w, ev) in watches.iter().zip(owners).skip(1) {
        if w.got != 0 {
            *io_res.entry(*ev).or_default() |= w.got & (EV_READ | EV_WRITE);
        }
    }

    for (ev, p) in &base.pending {
        let mut res: c_short = io_res.get(ev).copied().unwrap_or(0);

        if p.events & EV_SIGNAL != 0 && p.fd >= 0 && p.fd < 64 && raised & (1u64 << p.fd) != 0 {
            res |= EV_SIGNAL;
        }
        if p.deadline.is_some_and(|d| d <= now) {
            res |= EV_TIMEOUT;
        }

        if res != 0 {
            ready.push(Active {
                ev: *ev,
                generation: p.generation,
                res,
            });
        }
    }

    // Dispatch in registration order rather than whatever order the map
    // happened to hand back, so a given set of ready events always runs its
    // callbacks in the same sequence.
    ready.sort_by_key(|a| a.generation);

    for a in ready {
        // A non-persistent event is unregistered before its callback runs, so
        // the callback is free to re-add it — which is what tmux's peer and
        // timer callbacks do.
        let persist = base
            .pending
            .get(&a.ev)
            .is_some_and(|p| p.events & EV_PERSIST != 0);
        if persist {
            // A persistent event with a timeout re-arms from now.
            if let Some(p) = base.pending.get_mut(&a.ev)
                && let Some(interval) = p.interval
            {
                p.deadline = Some(Instant::now() + interval);
            }
        } else {
            base.pending.remove(&a.ev);
            // SAFETY: the event is registered, so its memory is live.
            unsafe { (*(a.ev as *mut event)).ev_flags &= !EVLIST_PENDING };
        }
        base.active.push_back(a);
    }
}

/// Run queued callbacks until the queue is empty. Returns whether any ran.
///
/// Entries are taken one at a time because a callback can delete other events —
/// `event_del` scrubs the queue, so anything it removed is gone before we reach
/// it.
fn dispatch() -> bool {
    let mut ran = false;
    loop {
        let Some(a) = with_base(|base| base.active.pop_front()) else {
            return ran;
        };

        // SAFETY: the event was on the active queue, and event_del removes an
        // event from that queue before its memory can be freed, so it is live.
        unsafe {
            let ev = a.ev as *mut event;
            if (*ev).ev_gen != a.generation {
                // Left over from a registration that has since been replaced.
                continue;
            }
            (*ev).ev_res = a.res;
            if let Some(cb) = (*ev).ev_callback {
                ran = true;
                cb((*ev).ev_fd, a.res, (*ev).ev_arg);
            }
        }
    }
}

/// Create the pipe the signal handler writes to, and publish its write end.
fn open_signal_pipe(base: &mut event_base) {
    let mut fds = [-1 as c_int; 2];
    // SAFETY: fds is a two-element array, as pipe requires.
    let rc = unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert!(rc == 0, "event loop: pipe failed");
    for fd in fds {
        // SAFETY: both descriptors are open.
        unsafe {
            libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
            let flags = libc::fcntl(fd, libc::F_GETFL, 0);
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
    }
    base.signal_pipe = fds;
    SIGNAL_PIPE_WRITE.store(fds[1], Ordering::Relaxed);
}

/// Close the wakeup pipe, if one is open.
fn close_signal_pipe(base: &mut event_base) {
    SIGNAL_PIPE_WRITE.store(-1, Ordering::Relaxed);
    for fd in base.signal_pipe {
        if fd >= 0 {
            // SAFETY: the descriptor came from open_signal_pipe.
            unsafe { libc::close(fd) };
        }
    }
    base.signal_pipe = [-1, -1];
}

/// Read the wakeup bytes so the pipe does not stay readable.
fn drain_signal_pipe(fd: c_int) {
    let mut buf = [0u8; 128];
    loop {
        // SAFETY: reading into a local buffer from a non-blocking descriptor.
        let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast::<c_void>(), buf.len()) };
        if n <= 0 {
            return;
        }
    }
}

/// Record `sig` and wake the loop. Runs in signal context, so it touches
/// nothing but an atomic and a non-blocking write.
unsafe extern "C" fn handle_signal(sig: c_int) {
    if (0..64).contains(&sig) {
        SIGNALS_RAISED.fetch_or(1u64 << sig, Ordering::Relaxed);
    }
    let fd = SIGNAL_PIPE_WRITE.load(Ordering::Relaxed);
    if fd >= 0 {
        let byte = sig as u8;
        // SAFETY: write is async-signal-safe; a full pipe just means the loop
        // has not drained its wakeups yet and will still see SIGNALS_RAISED.
        unsafe { libc::write(fd, std::ptr::from_ref(&byte).cast::<c_void>(), 1) };
    }
}

/// Route `sig` to the loop.
fn install_signal_handler(sig: c_int) {
    // SAFETY: a zeroed sigaction with a handler is the documented way to
    // install one; SA_RESTART matches what tmux asks libevent for.
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = handle_signal as *const () as usize;
        libc::sigemptyset(&raw mut sa.sa_mask);
        sa.sa_flags = libc::SA_RESTART;
        libc::sigaction(sig, &raw const sa, std::ptr::null_mut());
    }
}

/// Put `sig` back to its default disposition once nothing watches it.
fn restore_signal_handler(sig: c_int) {
    // SAFETY: as above; SIG_DFL is always a valid disposition.
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = libc::SIG_DFL;
        libc::sigemptyset(&raw mut sa.sa_mask);
        sa.sa_flags = libc::SA_RESTART;
        libc::sigaction(sig, &raw const sa, std::ptr::null_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        /// What the callbacks in these tests recorded, in order.
        static FIRED: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    }

    fn record(tag: &'static str) {
        FIRED.with(|f| f.borrow_mut().push(tag));
    }

    fn fired() -> Vec<&'static str> {
        FIRED.with(|f| f.borrow().clone())
    }

    fn reset() {
        FIRED.with(|f| f.borrow_mut().clear());
    }

    unsafe extern "C-unwind" fn cb_timer(_fd: c_int, _ev: c_short, _arg: *mut c_void) {
        record("timer");
    }

    unsafe extern "C-unwind" fn cb_read(_fd: c_int, _ev: c_short, _arg: *mut c_void) {
        record("read");
    }

    unsafe extern "C-unwind" fn cb_once(_fd: c_int, _ev: c_short, _arg: *mut c_void) {
        record("once");
    }

    /// Delete the event handed in as `arg` from inside a callback, which is the
    /// pattern that would leave a dangling activation if the queues were not
    /// scrubbed.
    unsafe extern "C-unwind" fn cb_deletes_other(_fd: c_int, _ev: c_short, arg: *mut c_void) {
        record("deleter");
        unsafe { event_del(arg.cast::<event>()) };
    }

    /// Claim the loop for this test: one base per process, so the tests that
    /// drive it run one at a time. The guard is returned so it is held for the
    /// body of the test.
    fn base() -> std::sync::MutexGuard<'static, ()> {
        let guard = crate::event_::test_guard();
        // SAFETY: the guard makes this the only thread in the loop.
        unsafe {
            if CURRENT_BASE.is_null() {
                event_init();
            }
        }
        reset();
        guard
    }

    /// A non-blocking pipe, as (read end, write end).
    fn pipe() -> (c_int, c_int) {
        let mut fds = [0 as c_int; 2];
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        (fds[0], fds[1])
    }

    #[test]
    fn zeroed_memory_reads_as_uninitialized() {
        // tmux embeds events in calloc'd structs and asks event_initialized
        // whether they were ever set up; all-zero must answer "no".
        let ev: event = unsafe { std::mem::zeroed() };
        assert_eq!(unsafe { event_initialized(&raw const ev) }, 0);
    }

    #[test]
    fn timer_fires_once_and_does_not_repeat() {
        let _loop = base();
        let mut ev = zeroed_event();
        unsafe {
            event_set(
                &raw mut ev,
                -1,
                EV_TIMEOUT,
                Some(cb_timer),
                std::ptr::null_mut(),
            );
            assert_eq!(event_initialized(&raw const ev), 1);
            let tv = timeval {
                tv_sec: 0,
                tv_usec: 1000,
            };
            event_add(&raw mut ev, &raw const tv);
            event_loop(EVLOOP_ONCE);
            assert_eq!(fired(), ["timer"]);

            // A one-shot timer is unregistered by firing, so a further turn
            // must not run it again.
            event_loop(EVLOOP_NONBLOCK);
            assert_eq!(fired(), ["timer"]);
            assert_eq!(
                event_pending(&raw const ev, EV_TIMEOUT, std::ptr::null_mut()),
                0
            );
        }
    }

    #[test]
    fn persistent_read_event_fires_for_each_wakeup() {
        let _loop = base();
        let (r, w) = pipe();
        let mut ev = zeroed_event();
        unsafe {
            event_set(
                &raw mut ev,
                r,
                EV_READ | EV_PERSIST,
                Some(cb_read),
                std::ptr::null_mut(),
            );
            event_add(&raw mut ev, std::ptr::null());

            libc::write(w, c"a".as_ptr().cast(), 1);
            event_loop(EVLOOP_ONCE);
            assert_eq!(fired(), ["read"]);

            // Still registered: drain and write again, and it fires again.
            let mut buf = [0u8; 8];
            libc::read(r, buf.as_mut_ptr().cast(), buf.len());
            libc::write(w, c"b".as_ptr().cast(), 1);
            event_loop(EVLOOP_ONCE);
            assert_eq!(fired(), ["read", "read"]);

            event_del(&raw mut ev);
            libc::close(r);
            libc::close(w);
        }
    }

    #[test]
    fn deleting_an_event_from_a_callback_cancels_its_pending_activation() {
        let _loop = base();
        let (r1, w1) = pipe();
        let (r2, w2) = pipe();
        let mut victim = zeroed_event();
        let mut deleter = zeroed_event();
        unsafe {
            event_set(
                &raw mut victim,
                r2,
                EV_READ,
                Some(cb_read),
                std::ptr::null_mut(),
            );
            event_set(
                &raw mut deleter,
                r1,
                EV_READ,
                Some(cb_deletes_other),
                (&raw mut victim).cast(),
            );
            // The deleter is registered first, so it runs first: dispatch
            // order follows registration order.
            event_add(&raw mut deleter, std::ptr::null());
            event_add(&raw mut victim, std::ptr::null());

            // Both become ready in the same turn; the first callback deletes
            // the second, whose callback must then not run.
            libc::write(w1, c"x".as_ptr().cast(), 1);
            libc::write(w2, c"y".as_ptr().cast(), 1);
            event_loop(EVLOOP_ONCE);
            assert_eq!(fired(), ["deleter"]);

            libc::close(r1);
            libc::close(w1);
            libc::close(r2);
            libc::close(w2);
        }
    }

    #[test]
    fn event_once_with_no_timeout_runs_on_the_next_turn() {
        let _loop = base();
        unsafe {
            event_once(
                -1,
                EV_TIMEOUT,
                Some(cb_once),
                std::ptr::null_mut(),
                std::ptr::null(),
            );
            event_loop(EVLOOP_ONCE);
            assert_eq!(fired(), ["once"]);

            // The loop owns that event and frees it; nothing may run twice.
            event_loop(EVLOOP_NONBLOCK);
            assert_eq!(fired(), ["once"]);
        }
    }

    #[test]
    fn signal_event_fires_when_the_signal_arrives() {
        let _loop = base();
        let mut ev = zeroed_event();
        unsafe {
            event_set(
                &raw mut ev,
                libc::SIGUSR2,
                EV_SIGNAL | EV_PERSIST,
                Some(cb_timer),
                std::ptr::null_mut(),
            );
            event_add(&raw mut ev, std::ptr::null());

            libc::raise(libc::SIGUSR2);
            event_loop(EVLOOP_ONCE);
            assert_eq!(fired(), ["timer"]);

            event_del(&raw mut ev);
        }
    }
}
