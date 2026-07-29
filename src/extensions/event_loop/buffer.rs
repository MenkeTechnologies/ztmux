//! `evbuffer` — the byte queue the event loop hands to buffered events.
//!
//! Same API as libevent's evbuffer, minus everything ztmux never calls
//! (reference counting, chained buffers, sendfile, callbacks, iovec access).
//! A single contiguous `Vec<u8>` plus a read offset covers every use in the
//! port: append at the back, drain at the front, hand out a contiguous
//! pointer for the parser.
use std::ffi::{c_int, c_void};

/// How much `evbuffer_read` takes per call when the caller passes a negative
/// `howmuch`. libevent's `EVBUFFER_MAX_READ`.
const MAX_READ: usize = 4096;

/// Bytes of consumed prefix tolerated before the buffer is compacted. Draining
/// only bumps `off`, so this bounds the wasted head at the cost of one memmove
/// per 4k drained.
const COMPACT_THRESHOLD: usize = 4096;

/// Line ending to split on, for [`evbuffer_readln`].
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq)]
// The port passes LF and ANY today; the other styles are part of the same
// libevent API and are covered by the unit tests, so a call site can move
// between them.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "complete line-ending API, exercised by the tests")
)]
pub enum evbuffer_eol_style {
    /// Any run of CR and LF characters.
    EVBUFFER_EOL_ANY = 0,
    /// A CR followed by an LF; a lone CR at the end of the data waits for more.
    EVBUFFER_EOL_CRLF = 1,
    /// A CR followed by an LF, and nothing else.
    EVBUFFER_EOL_CRLF_STRICT = 2,
    /// A single LF.
    EVBUFFER_EOL_LF = 3,
    /// A single NUL byte.
    EVBUFFER_EOL_NUL = 4,
}

/// A growable byte queue. Always heap-allocated and handed around as a raw
/// pointer, because the structs that hold one mirror tmux's C layout.
///
/// Two properties of libevent's buffers that tmux leans on are kept here, both
/// upheld by [`evbuffer::seal`]:
///
/// * the bytes are followed by a NUL, so a pointer from [`evbuffer_pullup`] can
///   be read as a C string — `format.rs` and `control.rs` both do;
/// * appending never moves the bytes already in the buffer, so a pointer taken
///   before an append stays good. `server_client_print` takes the pointer, then
///   appends the terminating NUL, then uses the pointer. libevent gets this
///   from its chained storage; here it comes from always holding spare capacity
///   for that one byte.
pub struct evbuffer {
    data: Vec<u8>,
    /// Bytes at the front of `data` that have been drained already.
    off: usize,
}

impl evbuffer {
    /// The bytes not yet drained.
    fn as_slice(&self) -> &[u8] {
        &self.data[self.off..]
    }

    /// Establish both invariants documented on [`evbuffer`]: room for one more
    /// byte, and a NUL sitting in it.
    ///
    /// This runs when a pointer is about to be handed out, never after the
    /// contents change. Reserving afterwards would be exactly backwards: the
    /// caller's one-byte append would fit, and the reserve that followed it
    /// would move the bytes out from under the pointer the caller still holds.
    fn seal(&mut self) {
        self.data.reserve(1);
        self.data.spare_capacity_mut()[0].write(0);
    }

    /// Drop `n` bytes from the front, compacting once the consumed prefix grows
    /// past [`COMPACT_THRESHOLD`].
    fn drain(&mut self, n: usize) {
        self.off = (self.off + n).min(self.data.len());
        if self.off == self.data.len() {
            self.data.clear();
            self.off = 0;
        } else if self.off >= COMPACT_THRESHOLD {
            self.data.drain(..self.off);
            self.off = 0;
        }
    }
}

pub fn evbuffer_new() -> *mut evbuffer {
    Box::into_raw(Box::new(evbuffer {
        data: Vec::new(),
        off: 0,
    }))
}

/// # Safety
/// `buf` must come from [`evbuffer_new`] and must not be used afterwards.
pub unsafe fn evbuffer_free(buf: *mut evbuffer) {
    if buf.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(buf)) }
}

/// # Safety
/// `buf` must be a live buffer.
pub unsafe fn evbuffer_get_length(buf: *const evbuffer) -> usize {
    unsafe { (*buf).as_slice().len() }
}

/// Append `datlen` bytes. Always succeeds (a failed allocation aborts, as
/// everywhere else in ztmux), so the return is always 0.
///
/// # Safety
/// `buf` must be a live buffer and `data` must point to `datlen` readable bytes.
pub unsafe fn evbuffer_add(buf: *mut evbuffer, data: *const c_void, datlen: usize) -> c_int {
    unsafe {
        if datlen != 0 {
            (*buf)
                .data
                .extend_from_slice(std::slice::from_raw_parts(data.cast::<u8>(), datlen));
        }
        0
    }
}

/// Write as much of the buffer as the kernel takes, dropping what went out.
/// Returns the byte count, or -1 with `errno` set.
///
/// # Safety
/// `buf` must be a live buffer.
pub unsafe fn evbuffer_write(buf: *mut evbuffer, fd: i32) -> i32 {
    unsafe {
        let len = (*buf).as_slice().len();
        if len == 0 {
            return 0;
        }
        let n = libc::write(fd, (*buf).as_slice().as_ptr().cast::<c_void>(), len);
        if n > 0 {
            (*buf).drain(n as usize);
        }
        n as i32
    }
}

/// Read up to `howmuch` bytes (4096 when negative) onto the back of the buffer.
/// Returns the byte count, 0 at end of file, or -1 with `errno` set.
///
/// # Safety
/// `buf` must be a live buffer.
pub unsafe fn evbuffer_read(buf: *mut evbuffer, fd: i32, howmuch: i32) -> i32 {
    unsafe {
        let want = if howmuch < 0 {
            MAX_READ
        } else {
            howmuch as usize
        };
        if want == 0 {
            return 0;
        }

        let b = &mut *buf;
        let start = b.data.len();
        b.data.resize(start + want, 0);
        let n = libc::read(fd, b.data.as_mut_ptr().add(start).cast::<c_void>(), want);
        b.data.truncate(start + if n > 0 { n as usize } else { 0 });
        n as i32
    }
}

/// Drop `len` bytes from the front.
///
/// # Safety
/// `buf` must be a live buffer.
pub unsafe fn evbuffer_drain(buf: *mut evbuffer, len: usize) -> c_int {
    unsafe {
        (*buf).drain(len);
        0
    }
}

/// A pointer to the first `size` undrained bytes (all of them when negative).
/// The data is already contiguous here, so this only bounds-checks. Returns
/// null when the buffer holds fewer bytes than asked for, as libevent does.
///
/// The bytes are followed by a NUL, so the result also reads as a C string.
///
/// # Safety
/// `buf` must be a live buffer. Draining, or an append larger than the one
/// byte of slack the buffer keeps, may move the data and leave this pointer
/// stale.
pub unsafe fn evbuffer_pullup(buf: *mut evbuffer, size: isize) -> *mut u8 {
    unsafe {
        let b = &mut *buf;
        let len = b.as_slice().len();
        if size > 0 && size as usize > len {
            return std::ptr::null_mut();
        }
        b.seal();
        b.data.as_mut_ptr().add(b.off)
    }
}

/// Pull one line off the front, or null when the buffer holds no complete line
/// yet. The line is NUL-terminated, has its line ending stripped, and is
/// `malloc`ed for the caller to `free`. `n_read_out`, when given, receives the
/// line length without the NUL.
///
/// # Safety
/// `buf` must be a live buffer; `n_read_out` must be null or writable.
pub unsafe fn evbuffer_readln(
    buf: *mut evbuffer,
    n_read_out: *mut usize,
    eol_style: evbuffer_eol_style,
) -> *mut u8 {
    unsafe {
        let Some((line_len, eol_len)) = find_eol((*buf).as_slice(), eol_style) else {
            return std::ptr::null_mut();
        };

        let out = libc::malloc(line_len + 1).cast::<u8>();
        assert!(!out.is_null(), "out of memory");
        std::ptr::copy_nonoverlapping((*buf).as_slice().as_ptr(), out, line_len);
        *out.add(line_len) = b'\0';

        (*buf).drain(line_len + eol_len);
        if !n_read_out.is_null() {
            *n_read_out = line_len;
        }
        out
    }
}

/// [`evbuffer_readln`] with libevent's legacy "any run of CR/LF ends the line"
/// rule.
///
/// # Safety
/// `buf` must be a live buffer.
pub unsafe fn evbuffer_readline(buf: *mut evbuffer) -> *mut u8 {
    unsafe {
        evbuffer_readln(
            buf,
            std::ptr::null_mut(),
            evbuffer_eol_style::EVBUFFER_EOL_ANY,
        )
    }
}

/// Locate the first line ending in `data` for `style`, as `(line length, line
/// ending length)`. `None` means no complete line is present yet.
fn find_eol(data: &[u8], style: evbuffer_eol_style) -> Option<(usize, usize)> {
    use evbuffer_eol_style::{
        EVBUFFER_EOL_ANY, EVBUFFER_EOL_CRLF, EVBUFFER_EOL_CRLF_STRICT, EVBUFFER_EOL_LF,
        EVBUFFER_EOL_NUL,
    };

    match style {
        EVBUFFER_EOL_ANY => {
            // The line ends at the first CR or LF and swallows every CR/LF that
            // follows it, so "a\r\n\r\nb" yields "a" then "b".
            let at = data.iter().position(|&b| b == b'\r' || b == b'\n')?;
            let run = data[at..]
                .iter()
                .take_while(|&&b| b == b'\r' || b == b'\n')
                .count();
            Some((at, run))
        }
        EVBUFFER_EOL_CRLF => {
            // A bare LF also ends the line; a CR only does when the LF after it
            // has already arrived.
            let at = data.iter().position(|&b| b == b'\n')?;
            if at > 0 && data[at - 1] == b'\r' {
                Some((at - 1, 2))
            } else {
                Some((at, 1))
            }
        }
        EVBUFFER_EOL_CRLF_STRICT => memchr::memmem::find(data, b"\r\n").map(|at| (at, 2)),
        EVBUFFER_EOL_LF => memchr::memchr(b'\n', data).map(|at| (at, 1)),
        EVBUFFER_EOL_NUL => memchr::memchr(b'\0', data).map(|at| (at, 1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bytes still queued in `buf`.
    unsafe fn contents(buf: *mut evbuffer) -> Vec<u8> {
        unsafe { (*buf).as_slice().to_vec() }
    }

    unsafe fn add(buf: *mut evbuffer, s: &[u8]) {
        unsafe { evbuffer_add(buf, s.as_ptr().cast(), s.len()) };
    }

    /// A line read back as an owned `Vec`, with the C string freed.
    unsafe fn take_line(buf: *mut evbuffer, style: evbuffer_eol_style) -> Option<Vec<u8>> {
        unsafe {
            let mut n = 0usize;
            let p = evbuffer_readln(buf, &raw mut n, style);
            if p.is_null() {
                return None;
            }
            let out = std::slice::from_raw_parts(p, n).to_vec();
            libc::free(p.cast());
            Some(out)
        }
    }

    #[test]
    fn drain_then_add_keeps_data_contiguous() {
        unsafe {
            let buf = evbuffer_new();
            add(buf, b"hello world");
            evbuffer_drain(buf, 6);
            add(buf, b"!!");
            assert_eq!(contents(buf), b"world!!");
            // pullup must see the same bytes the length reports.
            let p = evbuffer_pullup(buf, -1);
            let len = evbuffer_get_length(buf);
            assert_eq!(std::slice::from_raw_parts(p, len), b"world!!");
            evbuffer_free(buf);
        }
    }

    #[test]
    fn compaction_does_not_lose_bytes() {
        unsafe {
            let buf = evbuffer_new();
            // Drain past the compaction threshold in small steps, refilling as
            // we go: the tail must survive every memmove.
            for i in 0..40 {
                add(buf, &vec![b'a' + (i % 26) as u8; 300]);
            }
            let mut drained = 0;
            while evbuffer_get_length(buf) > 500 {
                evbuffer_drain(buf, 137);
                drained += 137;
            }
            assert_eq!(evbuffer_get_length(buf), 40 * 300 - drained);
            let expected: Vec<u8> = (0..40)
                .flat_map(|i| vec![b'a' + (i % 26) as u8; 300])
                .skip(drained)
                .collect();
            assert_eq!(contents(buf), expected);
            evbuffer_free(buf);
        }
    }

    #[test]
    fn readln_lf_returns_none_until_the_line_is_complete() {
        unsafe {
            let buf = evbuffer_new();
            add(buf, b"partial");
            assert!(take_line(buf, evbuffer_eol_style::EVBUFFER_EOL_LF).is_none());
            add(buf, b" line\nnext");
            assert_eq!(
                take_line(buf, evbuffer_eol_style::EVBUFFER_EOL_LF).unwrap(),
                b"partial line"
            );
            assert!(take_line(buf, evbuffer_eol_style::EVBUFFER_EOL_LF).is_none());
            assert_eq!(contents(buf), b"next");
            evbuffer_free(buf);
        }
    }

    #[test]
    fn readline_swallows_a_whole_run_of_cr_and_lf() {
        unsafe {
            let buf = evbuffer_new();
            add(buf, b"a\r\n\r\nb\rc\n");
            assert_eq!(
                take_line(buf, evbuffer_eol_style::EVBUFFER_EOL_ANY).unwrap(),
                b"a"
            );
            assert_eq!(
                take_line(buf, evbuffer_eol_style::EVBUFFER_EOL_ANY).unwrap(),
                b"b"
            );
            assert_eq!(
                take_line(buf, evbuffer_eol_style::EVBUFFER_EOL_ANY).unwrap(),
                b"c"
            );
            assert_eq!(evbuffer_get_length(buf), 0);
            evbuffer_free(buf);
        }
    }

    #[test]
    fn readln_crlf_variants_differ_on_a_bare_lf() {
        unsafe {
            let buf = evbuffer_new();
            add(buf, b"one\ntwo\r\n");
            // CRLF accepts the bare LF, CRLF_STRICT skips past it.
            assert_eq!(
                take_line(buf, evbuffer_eol_style::EVBUFFER_EOL_CRLF).unwrap(),
                b"one"
            );
            assert_eq!(
                take_line(buf, evbuffer_eol_style::EVBUFFER_EOL_CRLF_STRICT).unwrap(),
                b"two"
            );

            add(buf, b"three\nfour\r\n");
            assert_eq!(
                take_line(buf, evbuffer_eol_style::EVBUFFER_EOL_CRLF_STRICT).unwrap(),
                b"three\nfour"
            );
            evbuffer_free(buf);
        }
    }

    #[test]
    fn readln_nul_splits_on_zero_bytes() {
        unsafe {
            let buf = evbuffer_new();
            add(buf, b"a\0b\0");
            assert_eq!(
                take_line(buf, evbuffer_eol_style::EVBUFFER_EOL_NUL).unwrap(),
                b"a"
            );
            assert_eq!(
                take_line(buf, evbuffer_eol_style::EVBUFFER_EOL_NUL).unwrap(),
                b"b"
            );
            evbuffer_free(buf);
        }
    }

    #[test]
    fn a_pointer_survives_the_terminating_nul_append() {
        // server_client_print takes EVBUFFER_DATA, appends a NUL because the
        // message does not end in one, and then reads the pointer it already
        // has. Whether the append lands on a capacity boundary must not matter,
        // so try every length across several growth steps: a stale pointer here
        // silently drops command output (it dropped every 15-byte message).
        unsafe {
            for len in 0..300usize {
                let buf = evbuffer_new();
                let payload = vec![b'x'; len];
                add(buf, &payload);

                let msg = evbuffer_pullup(buf, -1);
                let size = evbuffer_get_length(buf);
                assert_eq!(size, len);
                if size == 0 || *msg.add(size - 1) != b'\0' {
                    add(buf, b"\0");
                }

                let seen = std::ffi::CStr::from_ptr(msg.cast()).to_bytes();
                assert_eq!(seen, &payload[..], "message of {len} bytes came back wrong");
                evbuffer_free(buf);
            }
        }
    }

    #[test]
    fn pullup_result_is_nul_terminated() {
        unsafe {
            let buf = evbuffer_new();
            // Even an empty buffer must give back a readable C string.
            assert_eq!(
                std::ffi::CStr::from_ptr(evbuffer_pullup(buf, -1).cast()).to_bytes(),
                b""
            );
            add(buf, b"terminated");
            assert_eq!(
                std::ffi::CStr::from_ptr(evbuffer_pullup(buf, -1).cast()).to_bytes(),
                b"terminated"
            );
            // Asking for more than is queued fails, as in libevent.
            assert!(evbuffer_pullup(buf, 11).is_null());
            evbuffer_free(buf);
        }
    }

    #[test]
    fn read_and_write_move_bytes_through_a_pipe() {
        unsafe {
            let mut fds = [0i32; 2];
            assert_eq!(libc::pipe(fds.as_mut_ptr()), 0);

            let out = evbuffer_new();
            add(out, b"through the pipe");
            let written = evbuffer_write(out, fds[1]);
            assert_eq!(written, 16);
            assert_eq!(evbuffer_get_length(out), 0);

            let inp = evbuffer_new();
            assert_eq!(evbuffer_read(inp, fds[0], -1), 16);
            assert_eq!(contents(inp), b"through the pipe");

            // EOF reads back as 0 and leaves the buffer untouched.
            libc::close(fds[1]);
            assert_eq!(evbuffer_read(inp, fds[0], -1), 0);
            assert_eq!(evbuffer_get_length(inp), 16);

            libc::close(fds[0]);
            evbuffer_free(out);
            evbuffer_free(inp);
        }
    }
}
