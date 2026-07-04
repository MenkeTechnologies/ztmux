#![allow(dead_code)]
// Port of the current OpenBSD imsg API as vendored in vendor/tmux/compat/imsg.c
// and imsg.h ($OpenBSD: imsg.c,v 1.42 / imsg.h,v 1.24). This replaces an older
// imsg generation: the write/read state now lives in a heap `struct msgbuf`
// reached through `imsgbuf->w`, message framing is driven by a header-parse
// callback (`imsg_parse_hdr`) instead of an embedded read buffer, and stack
// buffers are marked by `fd == IBUF_FD_MARK_ON_STACK` rather than `max == 0`.
use core::ffi::{c_int, c_uchar, c_void};
use std::ptr::null_mut;

use libc::{EBADMSG, EINVAL, ERANGE, iovec, pid_t};

use super::imsg_buffer::{
    ibuf_add, ibuf_add_buf, ibuf_close, ibuf_data, ibuf_dynamic, ibuf_fd_avail, ibuf_fd_get,
    ibuf_fd_set, ibuf_free, ibuf_get, ibuf_get_ibuf, ibuf_open, ibuf_read, ibuf_rewind,
    ibuf_set_h32, ibuf_set_maxsize, ibuf_size, ibuf_skip, msgbuf_free, msgbuf_get, msgbuf_new_reader,
    msgbuf_queuelen, msgbuf_read, msgbuf_write,
};
use super::queue::{Entry, tailq_entry, tailq_head};
use crate::errno;

// begin imsg.h

pub const IBUF_READ_SIZE: usize = 65535;
pub const IMSG_HEADER_SIZE: usize = size_of::<imsg_hdr>();
pub const MAX_IMSGSIZE: usize = 16384;

/// imsgbuf flag: allow `SCM_RIGHTS` fd passing on this channel.
const IMSG_ALLOW_FDPASS: c_int = 0x01;
/// Set in the on-wire header length when the message carries an fd.
const IMSG_FD_MARK: u32 = 0x8000_0000;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ibuf {
    pub entry: tailq_entry<ibuf>,
    pub buf: *mut c_uchar,
    pub size: usize,
    pub max: usize,
    pub wpos: usize,
    pub rpos: usize,
    pub fd: c_int,
}
impl Entry<ibuf> for ibuf {
    unsafe fn entry(this: *mut Self) -> *mut tailq_entry<ibuf> {
        unsafe { &raw mut (*this).entry }
    }
}

/// C `struct ibufqueue` (imsg-buffer.c:48): a queue of ibufs plus its length.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ibufqueue {
    pub bufs: tailq_head<ibuf>,
    pub queued: u32,
}

/// C `struct msgbuf` (imsg-buffer.c:53): owns the write queue (`bufs`), the
/// parsed-read queue (`rbufs`), and the partial-read state (`rbuf`, `rpmsg`,
/// `roff`) plus the framing callback (`readhdr`/`hdrsize`).
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct msgbuf {
    pub bufs: ibufqueue,
    pub rbufs: ibufqueue,
    pub rbuf: *mut c_uchar,
    pub rpmsg: *mut ibuf,
    pub readhdr: readhdr_fn,
    pub rarg: *mut c_void,
    pub roff: usize,
    pub hdrsize: usize,
}

/// The header-parse callback: given the first `hdrsize` bytes as a stack ibuf,
/// return a freshly-opened ibuf sized for the whole message (optionally taking
/// the passed fd via `*fd`), or NULL on a framing error.
pub type readhdr_fn =
    Option<unsafe fn(buf: *mut ibuf, arg: *mut c_void, fd: *mut c_int) -> *mut ibuf>;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct imsgbuf {
    pub w: *mut msgbuf,
    pub pid: pid_t,
    pub maxsize: u32,
    pub fd: c_int,
    pub flags: c_int,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct imsg_hdr {
    pub type_: u32,
    pub len: u32,
    pub peerid: u32,
    pub pid: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct imsg {
    pub hdr: imsg_hdr,
    pub data: *mut c_void,
    pub buf: *mut ibuf,
}
// end imsg.h
// begin imsg.c

/// C `vendor/tmux/compat/imsg.c:38`: `int imsgbuf_init(struct imsgbuf *imsgbuf, int fd)`
pub unsafe fn imsgbuf_init(imsgbuf: *mut imsgbuf, fd: c_int) -> c_int {
    unsafe {
        (*imsgbuf).w = msgbuf_new_reader(IMSG_HEADER_SIZE, Some(imsg_parse_hdr), imsgbuf.cast());
        if (*imsgbuf).w.is_null() {
            return -1;
        }
        (*imsgbuf).pid = std::process::id() as pid_t;
        (*imsgbuf).maxsize = MAX_IMSGSIZE as u32;
        (*imsgbuf).fd = fd;
        (*imsgbuf).flags = 0;
        0
    }
}

/// C `vendor/tmux/compat/imsg.c:52`: `void imsgbuf_allow_fdpass(struct imsgbuf *imsgbuf)`
pub unsafe fn imsgbuf_allow_fdpass(imsgbuf: *mut imsgbuf) {
    unsafe {
        (*imsgbuf).flags |= IMSG_ALLOW_FDPASS;
    }
}

/// C `vendor/tmux/compat/imsg.c:58`: `int imsgbuf_set_maxsize(struct imsgbuf *imsgbuf, uint32_t max)`
pub unsafe fn imsgbuf_set_maxsize(imsgbuf: *mut imsgbuf, mut max: u32) -> c_int {
    unsafe {
        if max > u32::MAX - IMSG_HEADER_SIZE as u32 {
            errno!() = ERANGE;
            return -1;
        }
        max += IMSG_HEADER_SIZE as u32;
        if max & IMSG_FD_MARK != 0 {
            errno!() = EINVAL;
            return -1;
        }
        (*imsgbuf).maxsize = max;
        0
    }
}

/// C `vendor/tmux/compat/imsg.c:74`: `int imsgbuf_read(struct imsgbuf *imsgbuf)`
pub unsafe fn imsgbuf_read(imsgbuf: *mut imsgbuf) -> c_int {
    unsafe {
        if (*imsgbuf).flags & IMSG_ALLOW_FDPASS != 0 {
            msgbuf_read((*imsgbuf).fd, (*imsgbuf).w)
        } else {
            ibuf_read((*imsgbuf).fd, (*imsgbuf).w)
        }
    }
}

/// C `vendor/tmux/compat/imsg.c:83`: `int imsgbuf_write(struct imsgbuf *imsgbuf)`
pub unsafe fn imsgbuf_write(imsgbuf: *mut imsgbuf) -> c_int {
    unsafe {
        if (*imsgbuf).flags & IMSG_ALLOW_FDPASS != 0 {
            msgbuf_write((*imsgbuf).fd, (*imsgbuf).w)
        } else {
            super::imsg_buffer::ibuf_write((*imsgbuf).fd, (*imsgbuf).w)
        }
    }
}

/// C `vendor/tmux/compat/imsg.c:92`: `int imsgbuf_flush(struct imsgbuf *imsgbuf)`
pub unsafe fn imsgbuf_flush(imsgbuf: *mut imsgbuf) -> c_int {
    unsafe {
        while imsgbuf_queuelen(imsgbuf) > 0 {
            if imsgbuf_write(imsgbuf) == -1 {
                return -1;
            }
        }
        0
    }
}

/// C `vendor/tmux/compat/imsg.c:102`: `void imsgbuf_clear(struct imsgbuf *imsgbuf)`
pub unsafe fn imsgbuf_clear(imsgbuf: *mut imsgbuf) {
    unsafe {
        msgbuf_free((*imsgbuf).w);
        (*imsgbuf).w = null_mut();
    }
}

/// C `vendor/tmux/compat/imsg.c:109`: `uint32_t imsgbuf_queuelen(struct imsgbuf *imsgbuf)`
pub unsafe fn imsgbuf_queuelen(imsgbuf: *mut imsgbuf) -> u32 {
    unsafe { msgbuf_queuelen((*imsgbuf).w) }
}

/// C `vendor/tmux/compat/imsg.c:115`: `int imsgbuf_get(struct imsgbuf *imsgbuf, struct imsg *imsg)`
pub unsafe fn imsgbuf_get(imsgbuf: *mut imsgbuf, imsg: *mut imsg) -> c_int {
    unsafe {
        let buf = msgbuf_get((*imsgbuf).w);
        if buf.is_null() {
            return 0;
        }

        let mut m: imsg = std::mem::zeroed();
        if ibuf_get(buf, (&raw mut m.hdr).cast(), size_of::<imsg_hdr>()) == -1 {
            return -1;
        }

        if ibuf_size(buf) != 0 {
            m.data = ibuf_data(buf);
        } else {
            m.data = null_mut();
        }
        m.buf = buf;
        m.hdr.len &= !IMSG_FD_MARK;

        *imsg = m;
        1
    }
}

/// C `vendor/tmux/compat/imsg.c:138`: `ssize_t imsg_get(struct imsgbuf *imsgbuf, struct imsg *imsg)`
pub unsafe fn imsg_get(imsgbuf: *mut imsgbuf, imsg: *mut imsg) -> isize {
    unsafe {
        let rv = imsgbuf_get(imsgbuf, imsg);
        if rv != 1 {
            return rv as isize;
        }
        (imsg_get_len(imsg) + IMSG_HEADER_SIZE) as isize
    }
}

/// C `vendor/tmux/compat/imsg.c:179`: `int imsg_get_ibuf(struct imsg *imsg, struct ibuf *ibuf)`
pub unsafe fn imsg_get_ibuf(imsg: *mut imsg, ibuf: *mut ibuf) -> c_int {
    unsafe {
        if ibuf_size((*imsg).buf) == 0 {
            errno!() = EBADMSG;
            return -1;
        }
        ibuf_get_ibuf((*imsg).buf, ibuf_size((*imsg).buf), ibuf)
    }
}

/// C `vendor/tmux/compat/imsg.c:189`: `int imsg_get_data(struct imsg *imsg, void *data, size_t len)`
pub unsafe fn imsg_get_data(imsg: *mut imsg, data: *mut c_void, len: usize) -> c_int {
    unsafe {
        if len == 0 {
            errno!() = EINVAL;
            return -1;
        }
        if ibuf_size((*imsg).buf) != len {
            errno!() = EBADMSG;
            return -1;
        }
        ibuf_get((*imsg).buf, data, len)
    }
}

/// C `vendor/tmux/compat/imsg.c:203`: `int imsg_get_buf(struct imsg *imsg, void *data, size_t len)`
pub unsafe fn imsg_get_buf(imsg: *mut imsg, data: *mut c_void, len: usize) -> c_int {
    unsafe { ibuf_get((*imsg).buf, data, len) }
}

/// C `vendor/tmux/compat/imsg.c:215`: `int imsg_get_fd(struct imsg *imsg)`
pub unsafe fn imsg_get_fd(imsg: *mut imsg) -> c_int {
    unsafe { ibuf_fd_get((*imsg).buf) }
}

/// C `vendor/tmux/compat/imsg.c:221`: `uint32_t imsg_get_id(struct imsg *imsg)`
pub unsafe fn imsg_get_id(imsg: *const imsg) -> u32 {
    unsafe { (*imsg).hdr.peerid }
}

/// C `vendor/tmux/compat/imsg.c:227`: `size_t imsg_get_len(struct imsg *imsg)`
pub unsafe fn imsg_get_len(imsg: *const imsg) -> usize {
    unsafe {
        if (*imsg).buf.is_null() {
            return 0;
        }
        ibuf_size((*imsg).buf)
    }
}

/// C `vendor/tmux/compat/imsg.c:233`: `pid_t imsg_get_pid(struct imsg *imsg)`
pub unsafe fn imsg_get_pid(imsg: *const imsg) -> pid_t {
    unsafe { (*imsg).hdr.pid as pid_t }
}

/// C `vendor/tmux/compat/imsg.c:239`: `uint32_t imsg_get_type(struct imsg *imsg)`
pub unsafe fn imsg_get_type(imsg: *const imsg) -> u32 {
    unsafe { (*imsg).hdr.type_ }
}

/// C `vendor/tmux/compat/imsg.c:246`: `int imsg_compose(...)`
pub unsafe fn imsg_compose(
    imsgbuf: *mut imsgbuf,
    type_: u32,
    id: u32,
    pid: pid_t,
    fd: c_int,
    data: *const c_void,
    datalen: usize,
) -> c_int {
    unsafe {
        // ibuf_add (not imsg_add) leaves ownership of wbuf here; the single
        // `ibuf_free` below is the fail path (tolerates a null wbuf from a
        // failed imsg_create), matching vendor/tmux/compat/imsg.c.
        let wbuf = imsg_create(imsgbuf, type_, id, pid, datalen);
        if !wbuf.is_null() && ibuf_add(wbuf, data, datalen) != -1 {
            ibuf_fd_set(wbuf, fd);
            imsg_close(imsgbuf, wbuf);
            return 1;
        }

        ibuf_free(wbuf);
        -1
    }
}

/// C `vendor/tmux/compat/imsg.c:267`: `int imsg_composev(...)`
pub unsafe fn imsg_composev(
    imsgbuf: *mut imsgbuf,
    type_: u32,
    id: u32,
    pid: pid_t,
    fd: c_int,
    iov: *const iovec,
    iovcnt: c_int,
) -> c_int {
    unsafe {
        let mut datalen: usize = 0;
        for i in 0..iovcnt {
            datalen += (*iov.add(i as usize)).iov_len;
        }

        let wbuf = imsg_create(imsgbuf, type_, id, pid, datalen);
        if wbuf.is_null() {
            return -1;
        }

        for i in 0..iovcnt {
            if ibuf_add(
                wbuf,
                (*iov.add(i as usize)).iov_base,
                (*iov.add(i as usize)).iov_len,
            ) == -1
            {
                ibuf_free(wbuf);
                return -1;
            }
        }

        ibuf_fd_set(wbuf, fd);
        imsg_close(imsgbuf, wbuf);
        1
    }
}

/// C `vendor/tmux/compat/imsg.c:299`: `int imsg_compose_ibuf(...)`
///
/// Enqueue an imsg whose payload is the caller's ibuf `buf`. fd passing is not
/// possible with this function.
pub unsafe fn imsg_compose_ibuf(
    imsgbuf: *mut imsgbuf,
    type_: u32,
    id: u32,
    pid: pid_t,
    buf: *mut ibuf,
) -> c_int {
    unsafe {
        let mut hdrbuf: *mut ibuf = null_mut();

        'fail: {
            if ibuf_size(buf) + IMSG_HEADER_SIZE > (*imsgbuf).maxsize as usize {
                errno!() = ERANGE;
                break 'fail;
            }

            let hdr = imsg_hdr {
                type_,
                len: (ibuf_size(buf) + IMSG_HEADER_SIZE) as u32,
                peerid: id,
                pid: if pid != 0 {
                    pid as u32
                } else {
                    (*imsgbuf).pid as u32
                },
            };

            hdrbuf = ibuf_open(IMSG_HEADER_SIZE);
            if hdrbuf.is_null() {
                break 'fail;
            }
            // ibuf_add (NOT imsg_add): imsg_add would free hdrbuf on failure and
            // the fail path below frees it again. ibuf_add leaves ownership here.
            if ibuf_add(hdrbuf, (&raw const hdr).cast(), size_of::<imsg_hdr>()) == -1 {
                break 'fail;
            }

            ibuf_close((*imsgbuf).w, hdrbuf);
            ibuf_close((*imsgbuf).w, buf);
            return 1;
        }

        ibuf_free(buf);
        ibuf_free(hdrbuf);
        -1
    }
}

/// C `vendor/tmux/compat/imsg.c:335`: `int imsg_forward(struct imsgbuf *imsgbuf, struct imsg *msg)`
///
/// Forward imsg to another channel. Any attached fd is closed.
pub unsafe fn imsg_forward(imsgbuf: *mut imsgbuf, msg: *mut imsg) -> c_int {
    unsafe {
        ibuf_rewind((*msg).buf);
        ibuf_skip((*msg).buf, size_of::<imsg_hdr>());
        let len = ibuf_size((*msg).buf);

        let wbuf = imsg_create(
            imsgbuf,
            (*msg).hdr.type_,
            (*msg).hdr.peerid,
            (*msg).hdr.pid as pid_t,
            len,
        );
        if wbuf.is_null() {
            return -1;
        }

        if len != 0 && ibuf_add_buf(wbuf, (*msg).buf) == -1 {
            ibuf_free(wbuf);
            return -1;
        }

        imsg_close(imsgbuf, wbuf);
        1
    }
}

/// C `vendor/tmux/compat/imsg.c:360`: `struct ibuf *imsg_create(...)`
pub unsafe fn imsg_create(
    imsgbuf: *mut imsgbuf,
    type_: u32,
    id: u32,
    pid: pid_t,
    mut datalen: usize,
) -> *mut ibuf {
    unsafe {
        datalen += IMSG_HEADER_SIZE;
        if datalen > (*imsgbuf).maxsize as usize {
            errno!() = ERANGE;
            return null_mut();
        }

        let hdr = imsg_hdr {
            len: 0,
            type_,
            peerid: id,
            pid: if pid != 0 {
                pid as u32
            } else {
                (*imsgbuf).pid as u32
            },
        };

        let wbuf = ibuf_dynamic(datalen, (*imsgbuf).maxsize as usize);
        if wbuf.is_null() {
            return null_mut();
        }
        if ibuf_add(wbuf, (&raw const hdr).cast(), size_of::<imsg_hdr>()) == -1 {
            ibuf_free(wbuf);
            return null_mut();
        }

        wbuf
    }
}

/// C `vendor/tmux/compat/imsg.c:390`: `int imsg_add(struct ibuf *msg, const void *data, size_t datalen)`
pub unsafe fn imsg_add(msg: *mut ibuf, data: *const c_void, datalen: usize) -> c_int {
    unsafe {
        if datalen != 0 && ibuf_add(msg, data, datalen) == -1 {
            ibuf_free(msg);
            return -1;
        }
        datalen as c_int
    }
}

/// C `vendor/tmux/compat/imsg.c:401`: `void imsg_close(struct imsgbuf *imsgbuf, struct ibuf *msg)`
pub unsafe fn imsg_close(imsgbuf: *mut imsgbuf, msg: *mut ibuf) {
    unsafe {
        let mut len = ibuf_size(msg) as u32;
        if ibuf_fd_avail(msg) != 0 {
            len |= IMSG_FD_MARK;
        }
        // Header `len` field lives at offset 4 (after `type`).
        let _ = ibuf_set_h32(msg, size_of::<u32>(), len as u64);
        ibuf_close((*imsgbuf).w, msg);
    }
}

/// C `vendor/tmux/compat/imsg.c:413`: `void imsg_free(struct imsg *imsg)`
pub unsafe fn imsg_free(imsg: *mut imsg) {
    unsafe { ibuf_free((*imsg).buf) }
}

/// C `vendor/tmux/compat/imsg.c:419`: `int imsg_set_maxsize(struct ibuf *msg, size_t max)`
pub unsafe fn imsg_set_maxsize(msg: *mut ibuf, max: usize) -> c_int {
    unsafe {
        if max > u32::MAX as usize - IMSG_HEADER_SIZE {
            errno!() = ERANGE;
            return -1;
        }
        ibuf_set_maxsize(msg, max + IMSG_HEADER_SIZE)
    }
}

/// C `vendor/tmux/compat/imsg.c:429`: `static struct ibuf *imsg_parse_hdr(struct ibuf *buf, void *arg, int *fd)`
///
/// The framing callback handed to `msgbuf_new_reader`: parse the fixed header
/// out of the first `IMSG_HEADER_SIZE` bytes and return an ibuf sized for the
/// whole message (taking the passed fd if the header marks one).
unsafe fn imsg_parse_hdr(buf: *mut ibuf, arg: *mut c_void, fd: *mut c_int) -> *mut ibuf {
    unsafe {
        let imsgbuf = arg as *mut imsgbuf;
        let mut hdr: imsg_hdr = std::mem::zeroed();

        if ibuf_get(buf, (&raw mut hdr).cast(), size_of::<imsg_hdr>()) == -1 {
            return null_mut();
        }

        let len = hdr.len & !IMSG_FD_MARK;

        if (len as usize) < IMSG_HEADER_SIZE || len > (*imsgbuf).maxsize {
            errno!() = ERANGE;
            return null_mut();
        }
        let b = ibuf_open(len as usize);
        if b.is_null() {
            return null_mut();
        }
        if hdr.len & IMSG_FD_MARK != 0 {
            ibuf_fd_set(b, *fd);
            *fd = -1;
        }

        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::imsg_buffer::ibuf_data;
    use core::ffi::c_void;
    use libc::{AF_UNIX, SOCK_STREAM, socketpair};

    // A connected imsgbuf pair over an AF_UNIX socketpair, both with fd passing
    // enabled, mirroring how proc.c wires a peer. Messages composed on `a` are
    // read back on `b` through the real write -> read -> parse path.
    struct Pair {
        a: Box<imsgbuf>,
        b: Box<imsgbuf>,
        fds: [c_int; 2],
    }

    unsafe fn new_pair() -> Pair {
        unsafe {
            let mut fds = [0 as c_int; 2];
            assert_eq!(socketpair(AF_UNIX, SOCK_STREAM, 0, fds.as_mut_ptr()), 0);
            let mut a: Box<imsgbuf> = Box::new(std::mem::zeroed());
            let mut b: Box<imsgbuf> = Box::new(std::mem::zeroed());
            assert_eq!(imsgbuf_init(&raw mut *a, fds[0]), 0);
            assert_eq!(imsgbuf_init(&raw mut *b, fds[1]), 0);
            imsgbuf_allow_fdpass(&raw mut *a);
            imsgbuf_allow_fdpass(&raw mut *b);
            Pair { a, b, fds }
        }
    }

    impl Drop for Pair {
        fn drop(&mut self) {
            unsafe {
                imsgbuf_clear(&raw mut *self.a);
                imsgbuf_clear(&raw mut *self.b);
                libc::close(self.fds[0]);
                libc::close(self.fds[1]);
            }
        }
    }

    // imsg_hdr layout is u32,u32,u32,u32 => 16 bytes (imsg.h).
    #[test]
    fn test_header_size_is_16() {
        assert_eq!(IMSG_HEADER_SIZE, 16);
        assert_eq!(size_of::<imsg_hdr>(), 16);
    }

    // Full compose -> flush -> read -> get round-trip with a payload, exercising
    // the real socket write/read + framing path.
    #[test]
    fn test_compose_get_round_trip_payload() {
        unsafe {
            let mut pair = new_pair();
            let payload: [u8; 7] = [0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03];

            let rc = imsg_compose(
                &raw mut *pair.a,
                0x4142_4344,
                0x5566_7788,
                4242,
                -1,
                payload.as_ptr() as *const c_void,
                payload.len(),
            );
            assert_eq!(rc, 1);
            assert_eq!(imsgbuf_flush(&raw mut *pair.a), 0);

            assert_eq!(imsgbuf_read(&raw mut *pair.b), 1);
            let mut msg: imsg = std::mem::zeroed();
            let rv = imsg_get(&raw mut *pair.b, &raw mut msg);
            assert_eq!(rv as usize, IMSG_HEADER_SIZE + payload.len());

            assert_eq!(imsg_get_type(&msg), 0x4142_4344);
            assert_eq!(imsg_get_id(&msg), 0x5566_7788);
            assert_eq!(imsg_get_pid(&msg), 4242);
            assert_eq!(imsg_get_len(&msg), payload.len());
            let got = std::slice::from_raw_parts(msg.data as *const u8, payload.len());
            assert_eq!(got, payload);
            // No fd was passed.
            assert_eq!(imsg_get_fd(&raw mut msg), -1);

            imsg_free(&raw mut msg);
        }
    }

    // Empty payload: datalen == 0, so imsg_get leaves data NULL and len 0.
    #[test]
    fn test_compose_get_round_trip_empty() {
        unsafe {
            let mut pair = new_pair();

            let rc = imsg_compose(&raw mut *pair.a, 7, 9, 0, -1, null_mut(), 0);
            assert_eq!(rc, 1);
            assert_eq!(imsgbuf_flush(&raw mut *pair.a), 0);

            assert_eq!(imsgbuf_read(&raw mut *pair.b), 1);
            let mut msg: imsg = std::mem::zeroed();
            let rv = imsg_get(&raw mut *pair.b, &raw mut msg);
            assert_eq!(rv as usize, IMSG_HEADER_SIZE);
            assert_eq!(imsg_get_type(&msg), 7);
            assert_eq!(imsg_get_id(&msg), 9);
            assert_eq!(imsg_get_len(&msg), 0);
            assert!(msg.data.is_null());

            imsg_free(&raw mut msg);
        }
    }

    // Two messages coalesced into one read must be framed into two imsgs.
    #[test]
    fn test_two_messages_one_read() {
        unsafe {
            let mut pair = new_pair();
            let p1: [u8; 3] = [1, 2, 3];
            let p2: [u8; 2] = [9, 8];

            assert_eq!(
                imsg_compose(&raw mut *pair.a, 100, 0, 0, -1, p1.as_ptr().cast(), p1.len()),
                1
            );
            assert_eq!(
                imsg_compose(&raw mut *pair.a, 200, 0, 0, -1, p2.as_ptr().cast(), p2.len()),
                1
            );
            assert_eq!(imsgbuf_flush(&raw mut *pair.a), 0);

            assert_eq!(imsgbuf_read(&raw mut *pair.b), 1);

            let mut m1: imsg = std::mem::zeroed();
            assert_eq!(imsg_get(&raw mut *pair.b, &raw mut m1) as usize, IMSG_HEADER_SIZE + 3);
            assert_eq!(imsg_get_type(&m1), 100);
            assert_eq!(
                std::slice::from_raw_parts(ibuf_data(m1.buf) as *const u8, imsg_get_len(&m1)),
                &p1
            );
            imsg_free(&raw mut m1);

            let mut m2: imsg = std::mem::zeroed();
            assert_eq!(imsg_get(&raw mut *pair.b, &raw mut m2) as usize, IMSG_HEADER_SIZE + 2);
            assert_eq!(imsg_get_type(&m2), 200);
            imsg_free(&raw mut m2);

            // Nothing left.
            let mut m3: imsg = std::mem::zeroed();
            assert_eq!(imsg_get(&raw mut *pair.b, &raw mut m3), 0);
        }
    }
}
