//! `bufferevent` — a descriptor with an input and an output queue attached.
//!
//! This is libevent's classic bufferevent, the one tmux uses for pane ptys,
//! jobs, control-mode clients and file transfers: read into `input` and call
//! `readcb`, drain `output` when the descriptor accepts more and call `writecb`
//! once it has fallen to the low watermark, and report end of file or a failed
//! read/write through `errorcb`.
use std::ffi::{c_int, c_short, c_void};

use crate::event_::{
    EV_PERSIST, EV_READ, EV_WRITE, evbuffer, evbuffer_add, evbuffer_drain, evbuffer_free,
    evbuffer_get_length, evbuffer_new, evbuffer_pullup, evbuffer_read, evbuffer_write, event,
    event_add, event_del, event_set,
};

/// What `errorcb` is told happened, using libevent's legacy `EVBUFFER_*` values.
pub const EVBUFFER_READ: c_short = 0x01;
pub const EVBUFFER_WRITE: c_short = 0x02;
pub const EVBUFFER_EOF: c_short = 0x10;
pub const EVBUFFER_ERROR: c_short = 0x20;

pub type bufferevent_data_cb =
    Option<unsafe extern "C-unwind" fn(bev: *mut bufferevent, ctx: *mut c_void)>;
pub type bufferevent_event_cb =
    Option<unsafe extern "C-unwind" fn(bev: *mut bufferevent, what: c_short, ctx: *mut c_void)>;

/// Low and high watermark for one direction.
#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct event_watermark {
    pub low: usize,
    pub high: usize,
}

/// A buffered descriptor. Fields mirror the names tmux reads directly, most of
/// all `input` and `output`.
#[repr(C)]
pub struct bufferevent {
    pub ev_read: event,
    pub ev_write: event,
    pub input: *mut evbuffer,
    pub output: *mut evbuffer,
    pub wm_read: event_watermark,
    pub wm_write: event_watermark,
    pub readcb: bufferevent_data_cb,
    pub writecb: bufferevent_data_cb,
    pub errorcb: bufferevent_event_cb,
    pub cbarg: *mut c_void,
    /// `EV_READ` and/or `EV_WRITE`, whichever the owner has enabled.
    pub enabled: c_short,
    fd: c_int,
}

/// Create a buffered descriptor. Nothing is enabled yet: the caller picks the
/// directions with [`bufferevent_enable`].
///
/// # Safety
/// `fd` must stay open, and `cbarg` valid, until the bufferevent is freed.
pub unsafe fn bufferevent_new(
    fd: c_int,
    readcb: bufferevent_data_cb,
    writecb: bufferevent_data_cb,
    errorcb: bufferevent_event_cb,
    cbarg: *mut c_void,
) -> *mut bufferevent {
    unsafe {
        let bev = Box::into_raw(Box::new(bufferevent {
            ev_read: std::mem::zeroed(),
            ev_write: std::mem::zeroed(),
            input: evbuffer_new(),
            output: evbuffer_new(),
            wm_read: event_watermark::default(),
            wm_write: event_watermark::default(),
            readcb,
            writecb,
            errorcb,
            cbarg,
            enabled: 0,
            fd,
        }));

        event_set(
            &raw mut (*bev).ev_read,
            fd,
            EV_READ | EV_PERSIST,
            Some(read_ready),
            bev.cast(),
        );
        event_set(
            &raw mut (*bev).ev_write,
            fd,
            EV_WRITE | EV_PERSIST,
            Some(write_ready),
            bev.cast(),
        );
        bev
    }
}

/// Tear down a buffered descriptor. The descriptor itself belongs to the
/// caller and is left open, as in libevent.
///
/// # Safety
/// `bev` must come from [`bufferevent_new`] and must not be used again.
pub unsafe fn bufferevent_free(bev: *mut bufferevent) {
    if bev.is_null() {
        return;
    }
    unsafe {
        event_del(&raw mut (*bev).ev_read);
        event_del(&raw mut (*bev).ev_write);
        evbuffer_free((*bev).input);
        evbuffer_free((*bev).output);
        drop(Box::from_raw(bev));
    }
}

/// Start watching for `events` (`EV_READ`, `EV_WRITE` or both).
///
/// # Safety
/// `bev` must be live.
pub unsafe fn bufferevent_enable(bev: *mut bufferevent, events: i16) -> c_int {
    unsafe {
        (*bev).enabled |= events;
        if events & EV_READ != 0 {
            event_add(&raw mut (*bev).ev_read, std::ptr::null());
        }
        // Writing is only watched while there is something to write; otherwise
        // the descriptor would report ready on every turn and spin the loop.
        if events & EV_WRITE != 0 && evbuffer_get_length((*bev).output) > 0 {
            event_add(&raw mut (*bev).ev_write, std::ptr::null());
        }
        0
    }
}

/// Stop watching for `events`.
///
/// # Safety
/// `bev` must be live.
pub unsafe fn bufferevent_disable(bev: *mut bufferevent, events: i16) -> c_int {
    unsafe {
        (*bev).enabled &= !events;
        if events & EV_READ != 0 {
            event_del(&raw mut (*bev).ev_read);
        }
        if events & EV_WRITE != 0 {
            event_del(&raw mut (*bev).ev_write);
        }
        0
    }
}

/// Set the watermarks for a direction. A read low watermark holds `readcb`
/// back until that many bytes have arrived; a write low watermark fires
/// `writecb` once the queue has drained to it. A zero high watermark means no
/// limit, as in libevent.
///
/// # Safety
/// `bev` must be live.
pub unsafe fn bufferevent_setwatermark(
    bev: *mut bufferevent,
    events: i16,
    lowmark: usize,
    highmark: usize,
) {
    unsafe {
        if events & EV_READ != 0 {
            (*bev).wm_read = event_watermark {
                low: lowmark,
                high: highmark,
            };
        }
        if events & EV_WRITE != 0 {
            (*bev).wm_write = event_watermark {
                low: lowmark,
                high: highmark,
            };
        }
    }
}

/// The output queue, for callers that write to it directly.
///
/// # Safety
/// `bev` must be live.
pub unsafe fn bufferevent_get_output(bev: *mut bufferevent) -> *mut evbuffer {
    unsafe { (*bev).output }
}

/// Queue `size` bytes for writing.
///
/// # Safety
/// `bev` must be live and `data` must point to `size` readable bytes.
pub unsafe fn bufferevent_write(bev: *mut bufferevent, data: *const c_void, size: usize) -> c_int {
    unsafe {
        evbuffer_add((*bev).output, data, size);
        arm_write(bev);
        0
    }
}

/// Move everything in `buf` onto the output queue.
///
/// # Safety
/// `bev` and `buf` must be live.
pub unsafe fn bufferevent_write_buffer(bev: *mut bufferevent, buf: *mut evbuffer) -> c_int {
    unsafe {
        let len = evbuffer_get_length(buf);
        if len != 0 {
            evbuffer_add(
                (*bev).output,
                evbuffer_pullup(buf, -1).cast::<c_void>(),
                len,
            );
            evbuffer_drain(buf, len);
        }
        arm_write(bev);
        0
    }
}

/// Watch for writability if writing is enabled and something is queued.
///
/// # Safety
/// `bev` must be live.
unsafe fn arm_write(bev: *mut bufferevent) {
    unsafe {
        if (*bev).enabled & EV_WRITE != 0 && evbuffer_get_length((*bev).output) > 0 {
            event_add(&raw mut (*bev).ev_write, std::ptr::null());
        }
    }
}

/// The descriptor has data: read it and hand it to `readcb`.
unsafe extern "C-unwind" fn read_ready(_fd: c_int, _events: c_short, arg: *mut c_void) {
    unsafe {
        let bev = arg.cast::<bufferevent>();

        // Honour a read high watermark by reading no further than it.
        let high = (*bev).wm_read.high;
        let howmuch = if high == 0 {
            -1
        } else {
            let have = evbuffer_get_length((*bev).input);
            if have >= high {
                // Already at the limit: stop watching until the owner drains
                // the queue and re-enables reading.
                event_del(&raw mut (*bev).ev_read);
                return;
            }
            c_int::try_from(high - have).unwrap_or(c_int::MAX)
        };

        let n = evbuffer_read((*bev).input, (*bev).fd, howmuch);
        if n <= 0 {
            if n == 0 {
                // End of file.
                report_error(bev, EVBUFFER_READ | EVBUFFER_EOF);
            } else if !would_block() {
                report_error(bev, EVBUFFER_READ | EVBUFFER_ERROR);
            }
            return;
        }

        if evbuffer_get_length((*bev).input) >= (*bev).wm_read.low
            && let Some(cb) = (*bev).readcb
        {
            cb(bev, (*bev).cbarg);
        }
    }
}

/// The descriptor accepts more: write what is queued and tell `writecb` once
/// the queue has fallen to the low watermark.
unsafe extern "C-unwind" fn write_ready(_fd: c_int, _events: c_short, arg: *mut c_void) {
    unsafe {
        let bev = arg.cast::<bufferevent>();

        if evbuffer_get_length((*bev).output) != 0 {
            let n = evbuffer_write((*bev).output, (*bev).fd);
            if n < 0 && !would_block() {
                report_error(bev, EVBUFFER_WRITE | EVBUFFER_ERROR);
                return;
            }
        }

        // Nothing left to send: stop watching for writability, or the loop
        // would wake on every turn.
        if evbuffer_get_length((*bev).output) == 0 {
            event_del(&raw mut (*bev).ev_write);
        }

        if evbuffer_get_length((*bev).output) <= (*bev).wm_write.low
            && let Some(cb) = (*bev).writecb
        {
            cb(bev, (*bev).cbarg);
        }
    }
}

/// Whether the last syscall failed only because there was nothing to do yet.
fn would_block() -> bool {
    let err = std::io::Error::last_os_error();
    matches!(
        err.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
    )
}

/// Stop watching the descriptor and report `what` to the owner.
///
/// # Safety
/// `bev` must be live.
unsafe fn report_error(bev: *mut bufferevent, what: c_short) {
    unsafe {
        event_del(&raw mut (*bev).ev_read);
        event_del(&raw mut (*bev).ev_write);
        if let Some(cb) = (*bev).errorcb {
            cb(bev, what, (*bev).cbarg);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::event_::base::EVLOOP_NONBLOCK;
    use crate::event_::{EVLOOP_ONCE, event_loop};

    thread_local! {
        /// Bytes handed to the read callback, and which callbacks ran.
        static SEEN: RefCell<(Vec<u8>, Vec<&'static str>)> =
            const { RefCell::new((Vec::new(), Vec::new())) };
    }

    fn note(tag: &'static str) {
        SEEN.with(|s| s.borrow_mut().1.push(tag));
    }

    fn tags() -> Vec<&'static str> {
        SEEN.with(|s| s.borrow().1.clone())
    }

    fn data() -> Vec<u8> {
        SEEN.with(|s| s.borrow().0.clone())
    }

    /// Claim the loop for this test; see `base::tests::base`.
    fn claim_loop() -> std::sync::MutexGuard<'static, ()> {
        crate::event_::test_guard()
    }

    fn reset() {
        SEEN.with(|s| {
            let mut s = s.borrow_mut();
            s.0.clear();
            s.1.clear();
        });
    }

    unsafe extern "C-unwind" fn on_read(bev: *mut bufferevent, _ctx: *mut c_void) {
        unsafe {
            note("read");
            let len = evbuffer_get_length((*bev).input);
            let p = evbuffer_pullup((*bev).input, -1);
            let bytes = std::slice::from_raw_parts(p, len).to_vec();
            evbuffer_drain((*bev).input, len);
            SEEN.with(|s| s.borrow_mut().0.extend_from_slice(&bytes));
        }
    }

    unsafe extern "C-unwind" fn on_write(_bev: *mut bufferevent, _ctx: *mut c_void) {
        note("write");
    }

    unsafe extern "C-unwind" fn on_error(_bev: *mut bufferevent, what: c_short, _ctx: *mut c_void) {
        if what & EVBUFFER_EOF != 0 {
            note("eof");
        } else {
            note("error");
        }
    }

    /// A pipe with both ends non-blocking, as tmux sets up its job and pane fds.
    fn pipe() -> (c_int, c_int) {
        let mut fds = [0 as c_int; 2];
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        for fd in fds {
            unsafe {
                let f = libc::fcntl(fd, libc::F_GETFL, 0);
                libc::fcntl(fd, libc::F_SETFL, f | libc::O_NONBLOCK);
            }
        }
        (fds[0], fds[1])
    }

    #[test]
    fn reads_are_delivered_to_the_read_callback() {
        let _loop = claim_loop();
        reset();
        unsafe {
            let (r, w) = pipe();
            let bev = bufferevent_new(
                r,
                Some(on_read),
                Some(on_write),
                Some(on_error),
                std::ptr::null_mut(),
            );
            bufferevent_enable(bev, EV_READ);

            libc::write(w, c"payload".as_ptr().cast(), 7);
            event_loop(EVLOOP_ONCE);
            assert_eq!(tags(), ["read"]);
            assert_eq!(data(), b"payload");

            bufferevent_free(bev);
            libc::close(r);
            libc::close(w);
        }
    }

    #[test]
    fn writes_drain_and_then_stop_waking_the_loop() {
        let _loop = claim_loop();
        reset();
        unsafe {
            let (r, w) = pipe();
            let bev = bufferevent_new(
                w,
                Some(on_read),
                Some(on_write),
                Some(on_error),
                std::ptr::null_mut(),
            );
            bufferevent_enable(bev, EV_WRITE);
            bufferevent_write(bev, c"out".as_ptr().cast(), 3);

            event_loop(EVLOOP_ONCE);
            assert_eq!(tags(), ["write"]);
            assert_eq!(evbuffer_get_length((*bev).output), 0);

            // The other end really got it.
            let mut buf = [0u8; 8];
            assert_eq!(libc::read(r, buf.as_mut_ptr().cast(), buf.len()), 3);
            assert_eq!(&buf[..3], b"out");

            // With an empty output queue the write event must be unregistered,
            // so another turn does nothing at all.
            event_loop(EVLOOP_NONBLOCK);
            assert_eq!(tags(), ["write"]);

            bufferevent_free(bev);
            libc::close(r);
            libc::close(w);
        }
    }

    #[test]
    fn end_of_file_reaches_the_error_callback() {
        let _loop = claim_loop();
        reset();
        unsafe {
            let (r, w) = pipe();
            let bev = bufferevent_new(
                r,
                Some(on_read),
                Some(on_write),
                Some(on_error),
                std::ptr::null_mut(),
            );
            bufferevent_enable(bev, EV_READ);

            libc::close(w);
            event_loop(EVLOOP_ONCE);
            assert_eq!(tags(), ["eof"]);

            bufferevent_free(bev);
            libc::close(r);
        }
    }

    #[test]
    fn write_low_watermark_holds_the_callback_back() {
        let _loop = claim_loop();
        reset();
        unsafe {
            let (r, w) = pipe();
            let bev = bufferevent_new(
                w,
                Some(on_read),
                Some(on_write),
                Some(on_error),
                std::ptr::null_mut(),
            );
            // Only tell us once the queue is fully drained past 4 bytes left.
            bufferevent_setwatermark(bev, EV_WRITE, 4, 0);
            bufferevent_enable(bev, EV_WRITE);
            bufferevent_write(bev, c"0123456789".as_ptr().cast(), 10);

            event_loop(EVLOOP_ONCE);
            // The pipe took all ten bytes, so the queue is under the mark.
            assert_eq!(tags(), ["write"]);

            let mut buf = [0u8; 16];
            assert_eq!(libc::read(r, buf.as_mut_ptr().cast(), buf.len()), 10);

            bufferevent_free(bev);
            libc::close(r);
            libc::close(w);
        }
    }
}
