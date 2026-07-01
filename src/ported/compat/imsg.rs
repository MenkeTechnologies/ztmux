#![allow(dead_code)]
use core::ffi::{c_int, c_uchar, c_void};
use std::ptr::NonNull;
use std::{mem::MaybeUninit, ptr::null_mut};

use libc::{
    CMSG_DATA, CMSG_FIRSTHDR, CMSG_NXTHDR, CMSG_SPACE, EAGAIN, EBADMSG, EINTR, EINVAL, ERANGE,
    SCM_RIGHTS, SOL_SOCKET, calloc, close, cmsghdr, free, getdtablesize, iovec, memcpy, memmove,
    memset, msghdr, pid_t,
};

use super::getdtablecount::getdtablecount;
use super::imsg_buffer::{
    ibuf_add, ibuf_add_buf, ibuf_close, ibuf_data, ibuf_dynamic, ibuf_fd_avail, ibuf_fd_set,
    ibuf_free, ibuf_get, ibuf_get_ibuf, ibuf_open, ibuf_rewind, ibuf_size, msgbuf_clear,
    msgbuf_init, msgbuf_write,
};
use super::queue::{
    Entry, tailq_entry, tailq_first, tailq_head, tailq_init, tailq_insert_tail, tailq_remove,
};
use crate::errno;
// begin imsg.h

pub const IBUF_READ_SIZE: usize = 65535;
pub const IMSG_HEADER_SIZE: usize = size_of::<imsg_hdr>();
pub const MAX_IMSGSIZE: usize = 16384;

const IMSGF_HASFD: u16 = 1; // this needs to be u16, i think, but it's u32 in auto generated header

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

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct msgbuf {
    pub bufs: tailq_head<ibuf>,
    pub queued: u32,
    pub fd: c_int,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ibuf_read {
    pub buf: [u8; IBUF_READ_SIZE],
    pub rptr: *mut u8,
    pub wpos: usize,
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct imsgbuf {
    pub fds: tailq_head<imsg_fd>,
    pub r: ibuf_read,
    pub w: msgbuf,
    pub fd: c_int,
    pub pid: pid_t,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct imsg_hdr {
    pub type_: u32,
    pub len: u16,
    pub flags: u16,
    pub peerid: u32,
    pub pid: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct imsg {
    pub hdr: imsg_hdr,
    pub fd: c_int,
    pub data: *mut c_void,
    pub buf: *mut ibuf,
}
// end imsg.h
// begin imsg.c

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct imsg_fd {
    entry: tailq_entry<imsg_fd>,
    fd: i32,
}
impl super::queue::Entry<imsg_fd> for imsg_fd {
    unsafe fn entry(this: *mut Self) -> *mut tailq_entry<imsg_fd> {
        unsafe { &raw mut (*this).entry }
    }
}

static mut IMSG_FD_OVERHEAD: i32 = 0;

pub unsafe fn imsg_init(imsgbuf: *mut imsgbuf, fd: c_int) {
    unsafe {
        msgbuf_init(&raw mut (*imsgbuf).w);
        memset((&raw mut (*imsgbuf).r).cast(), 0, size_of::<ibuf_read>());
        (*imsgbuf).fd = fd;
        (*imsgbuf).w.fd = fd;
        (*imsgbuf).pid = std::process::id() as i32;
        tailq_init(&raw mut (*imsgbuf).fds);
    }
}

pub unsafe fn imsg_read(imsgbuf: *mut imsgbuf) -> isize {
    const BUFSIZE: usize = unsafe { CMSG_SPACE(size_of::<c_int>() as u32) } as usize;
    union cmsgbuf {
        _hdr: cmsghdr,
        buf: [u8; BUFSIZE],
    }

    unsafe {
        let mut msg: msghdr = core::mem::zeroed();
        let mut cmsgbuf: cmsgbuf = core::mem::zeroed();

        let mut iov: iovec = iovec {
            iov_base: (*imsgbuf).r.buf.as_mut_ptr().add((*imsgbuf).r.wpos) as *mut c_void,
            iov_len: IBUF_READ_SIZE - (*imsgbuf).r.wpos, // size_of(imsgbuf->.r.buf)
        };
        msg.msg_iov = &raw mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = &raw mut cmsgbuf.buf as *mut c_void;
        msg.msg_controllen = BUFSIZE as _;

        let mut ifd: *mut imsg_fd = calloc(1, size_of::<imsg_fd>()) as *mut imsg_fd;
        if ifd.is_null() {
            return -1;
        }

        let mut n: isize;
        // this extra labeled block isn't necessary, but makes the breaks more semantic
        // goto fail => break 'fail
        // goto again => continue 'again
        'fail: {
            'again: loop {
                if getdtablecount()
                    + IMSG_FD_OVERHEAD
                    + ((CMSG_SPACE(size_of::<libc::c_int>() as u32) - CMSG_SPACE(0)) as i32
                        / size_of::<c_int>() as i32)
                    >= getdtablesize()
                {
                    errno!() = EAGAIN;
                    free(ifd as *mut c_void);
                    return -1;
                }

                n = libc::recvmsg((*imsgbuf).fd, &raw mut msg, 0);
                if n == -1 {
                    if errno!() == EINTR {
                        continue 'again;
                    }
                    break 'fail;
                }

                (*imsgbuf).r.wpos += n as usize;

                // really?
                let mut cmsg: *mut cmsghdr = CMSG_FIRSTHDR(&raw const msg);
                while !cmsg.is_null() {
                    if (*cmsg).cmsg_level == SOL_SOCKET && (*cmsg).cmsg_type == SCM_RIGHTS {
                        let j: i32 = (((cmsg as *mut u8).add((*cmsg).cmsg_len as usize).addr()
                            - CMSG_DATA(cmsg).addr())
                            / size_of::<c_int>()) as i32;
                        for i in 0..j {
                            let fd = *(CMSG_DATA(cmsg) as *mut c_int).add(i as usize);
                            if !ifd.is_null() {
                                (*ifd).fd = fd;
                                tailq_insert_tail(&raw mut (*imsgbuf).fds, ifd);
                                ifd = null_mut();
                            } else {
                                close(fd);
                            }
                        }
                    }

                    cmsg = CMSG_NXTHDR(&raw const msg, cmsg);
                }

                break; // no looping on success
            }
        }

        // fail:
        free(ifd as *mut c_void);
        n
    }
}

/// C `vendor/tmux/compat/imsg.c:139`: `ssize_t imsg_get(struct imsgbuf *imsgbuf, struct imsg *imsg)`
pub unsafe fn imsg_get(imsgbuf: *mut imsgbuf, imsg: *mut imsg) -> isize {
    unsafe {
        let mut m = MaybeUninit::<imsg>::uninit();
        #[expect(clippy::shadow_reuse)]
        let m = m.as_mut_ptr();
        let av: usize = (*imsgbuf).r.wpos;

        if IMSG_HEADER_SIZE > av {
            return 0;
        }

        memcpy(
            &raw mut (*m).hdr as *mut c_void,
            (*imsgbuf).r.buf.as_ptr() as *const c_void,
            size_of::<imsg_hdr>(),
        );
        if ((*m).hdr.len as usize) < IMSG_HEADER_SIZE || (*m).hdr.len > MAX_IMSGSIZE as u16 {
            errno!() = ERANGE;
            return -1;
        }
        if ((*m).hdr.len as usize) > av {
            return 0;
        }

        (*m).fd = -1;
        (*m).buf = null_mut();
        (*m).data = null_mut();

        let datalen = (*m).hdr.len as usize - IMSG_HEADER_SIZE;
        (*imsgbuf).r.rptr = (*imsgbuf).r.buf.as_mut_ptr().add(IMSG_HEADER_SIZE);
        if datalen != 0 {
            (*m).buf = ibuf_open(datalen);
            if (*m).buf.is_null() {
                return -1;
            }
            if ibuf_add((*m).buf, (*imsgbuf).r.rptr as *mut c_void, datalen) == -1 {
                // this should never fail
                ibuf_free((*m).buf);
                return -1;
            }
            (*m).data = ibuf_data((*m).buf);
        }

        if (*m).hdr.flags & IMSGF_HASFD != 0 {
            (*m).fd = imsg_dequeue_fd(imsgbuf);
        }

        if ((*m).hdr.len as usize) < av {
            let left = av - (*m).hdr.len as usize;
            memmove(
                &raw mut (*imsgbuf).r.buf as *mut c_void,
                (*imsgbuf).r.buf.as_ptr().add((*m).hdr.len as usize) as *const c_void,
                left,
            );
            (*imsgbuf).r.wpos = left;
        } else {
            (*imsgbuf).r.wpos = 0;
        }

        core::ptr::copy_nonoverlapping(m, imsg, 1);

        (datalen + IMSG_HEADER_SIZE) as isize
    }
}

/// C `vendor/tmux/compat/imsg.c:180`: `int imsg_get_ibuf(struct imsg *imsg, struct ibuf *ibuf)`
pub unsafe fn imsg_get_ibuf(imsg: *mut imsg, ibuf: *mut ibuf) -> c_int {
    unsafe {
        if (*imsg).buf.is_null() {
            errno!() = EBADMSG;
            return -1;
        }
        ibuf_get_ibuf((*imsg).buf, ibuf_size((*imsg).buf), ibuf)
    }
}

/// C `vendor/tmux/compat/imsg.c:190`: `int imsg_get_data(struct imsg *imsg, void *data, size_t len)`
pub unsafe fn imsg_get_data(imsg: *mut imsg, data: *mut c_void, len: usize) -> i32 {
    unsafe {
        if len == 0 {
            errno!() = EINVAL;
            return -1;
        }
        if (*imsg).buf.is_null() || ibuf_size((*imsg).buf) != len {
            errno!() = EBADMSG;
            return -1;
        }
        ibuf_get((*imsg).buf, data, len)
    }
}

/// C `vendor/tmux/compat/imsg.c:216`: `int imsg_get_fd(struct imsg *imsg)`
pub unsafe fn imsg_get_fd(imsg: *mut imsg) -> i32 {
    unsafe { std::ptr::replace(&raw mut (*imsg).fd, -1) }
}

/// C `vendor/tmux/compat/imsg.c:222`: `uint32_t imsg_get_id(struct imsg *imsg)`
pub unsafe fn imsg_get_id(imsg: *const imsg) -> u32 {
    unsafe { (*imsg).hdr.peerid }
}

/// C `vendor/tmux/compat/imsg.c:228`: `size_t imsg_get_len(struct imsg *imsg)`
pub unsafe fn imsg_get_len(imsg: *const imsg) -> usize {
    unsafe {
        if (*imsg).buf.is_null() {
            return 0;
        }
        ibuf_size((*imsg).buf)
    }
}

/// C `vendor/tmux/compat/imsg.c:234`: `pid_t imsg_get_pid(struct imsg *imsg)`
pub unsafe fn imsg_get_pid(imsg: *const imsg) -> pid_t {
    unsafe { (*imsg).hdr.pid as pid_t }
}

/// C `vendor/tmux/compat/imsg.c:240`: `uint32_t imsg_get_type(struct imsg *imsg)`
pub unsafe fn imsg_get_type(imsg: *const imsg) -> u32 {
    unsafe { (*imsg).hdr.type_ }
}

/// C `vendor/tmux/compat/imsg.c:246`: `int imsg_compose(struct imsgbuf *imsgbuf, uint32_t type, uint32_t id, pid_t pid, int fd, const void *data, size_t datalen)`
pub unsafe fn imsg_compose(
    imsgbuf: *mut imsgbuf,
    type_: u32,
    id: u32,
    pid: pid_t,
    fd: c_int,
    data: *const c_void,
    datalen: usize,
) -> i32 {
    unsafe {
        let wbuf = imsg_create(imsgbuf, type_, id, pid, datalen);
        if wbuf.is_null() {
            return -1;
        }

        if imsg_add(wbuf, data, datalen) == -1 {
            return -1;
        }

        ibuf_fd_set(wbuf, fd);
        imsg_close(imsgbuf, wbuf);

        1
    }
}

/// C `vendor/tmux/compat/imsg.c:268`: `int imsg_composev(struct imsgbuf *imsgbuf, uint32_t type, uint32_t id, pid_t pid, int fd, const struct iovec *iov, int iovcnt)`
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
            if imsg_add(
                wbuf,
                (*iov.add(i as usize)).iov_base,
                (*iov.add(i as usize)).iov_len,
            ) == -1
            {
                return -1;
            }
        }

        ibuf_fd_set(wbuf, fd);
        imsg_close(imsgbuf, wbuf);

        1
    }
}

/// C `vendor/tmux/compat/imsg.c:300`: `int imsg_compose_ibuf(struct imsgbuf *imsgbuf, uint32_t type, uint32_t id, pid_t pid, struct ibuf *buf)`
pub unsafe fn imsg_compose_ibuf(
    imsgbuf: *mut imsgbuf,
    type_: u32,
    id: u32,
    pid: pid_t,
    buf: *mut ibuf,
) -> i32 {
    unsafe {
        let mut hdrbuf: *mut ibuf = null_mut();

        'fail: {
            if ibuf_size(buf) + IMSG_HEADER_SIZE > MAX_IMSGSIZE {
                errno!() = ERANGE;
                break 'fail;
            }

            let mut hdr: imsg_hdr = imsg_hdr {
                type_,
                len: (ibuf_size(buf) + IMSG_HEADER_SIZE) as u16,
                flags: 0,
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
            if imsg_add(hdrbuf, &raw mut hdr as *mut c_void, size_of::<imsg_hdr>()) == -1 {
                break 'fail;
            }

            ibuf_close(&raw mut (*imsgbuf).w, hdrbuf);
            ibuf_close(&raw mut (*imsgbuf).w, buf);
            return 1;
        }

        let save_errno = errno!();
        ibuf_free(buf);
        ibuf_free(hdrbuf);
        errno!() = save_errno;
        -1
    }
}

/// C `vendor/tmux/compat/imsg.c:336`: `int imsg_forward(struct imsgbuf *imsgbuf, struct imsg *msg)`
pub unsafe fn imsg_forward(imsgbuf: *mut imsgbuf, msg: *mut imsg) -> c_int {
    unsafe {
        let mut len = 0;

        if (*msg).fd != -1 {
            close((*msg).fd);
            (*msg).fd = -1;
        }

        if !(*msg).buf.is_null() {
            ibuf_rewind((*msg).buf);
            len = ibuf_size((*msg).buf);
        }

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

        if !(*msg).buf.is_null() && ibuf_add_buf(wbuf, (*msg).buf) == -1 {
            ibuf_free(wbuf);
            return -1;
        }

        imsg_close(imsgbuf, wbuf);
        1
    }
}

/// C `vendor/tmux/compat/imsg.c:361`: `struct ibuf *imsg_create(struct imsgbuf *imsgbuf, uint32_t type, uint32_t id, pid_t pid, size_t datalen)`
pub unsafe fn imsg_create(
    imsgbuf: *mut imsgbuf,
    type_: u32,
    id: u32,
    pid: pid_t,
    mut datalen: usize,
) -> *mut ibuf {
    unsafe {
        datalen += IMSG_HEADER_SIZE;
        if datalen > MAX_IMSGSIZE {
            errno!() = ERANGE;
            return null_mut();
        }

        let hdr: imsg_hdr = imsg_hdr {
            type_,
            flags: 0,
            peerid: id,
            pid: if pid != 0 {
                pid as u32
            } else {
                (*imsgbuf).pid as u32
            },
            len: 0, // TODO can be uninit
        };

        let wbuf = ibuf_dynamic(datalen, MAX_IMSGSIZE);
        if wbuf.is_null() {
            return null_mut();
        }
        if imsg_add(wbuf, &raw const hdr as *const c_void, size_of::<imsg_hdr>()) == -1 {
            return null_mut();
        }

        wbuf
    }
}

/// C `vendor/tmux/compat/imsg.c:391`: `int imsg_add(struct ibuf *msg, const void *data, size_t datalen)`
pub unsafe fn imsg_add(msg: *mut ibuf, data: *const c_void, datalen: usize) -> i32 {
    unsafe {
        if datalen != 0 && ibuf_add(msg, data, datalen) == -1 {
            ibuf_free(msg);
            return -1;
        }
        datalen as i32
    }
}

/// C `vendor/tmux/compat/imsg.c:402`: `void imsg_close(struct imsgbuf *imsgbuf, struct ibuf *msg)`
pub unsafe fn imsg_close(imsgbuf: *mut imsgbuf, msg: *mut ibuf) {
    unsafe {
        let hdr: *mut imsg_hdr = (*msg).buf as *mut imsg_hdr;

        (*hdr).flags &= !IMSGF_HASFD;
        if ibuf_fd_avail(msg) != 0 {
            (*hdr).flags |= IMSGF_HASFD;
        }
        (*hdr).len = ibuf_size(msg) as u16;

        ibuf_close(&raw mut (*imsgbuf).w, msg);
    }
}

/// C `vendor/tmux/compat/imsg.c:414`: `void imsg_free(struct imsg *imsg)`
pub unsafe fn imsg_free(imsg: *mut imsg) {
    unsafe { ibuf_free((*imsg).buf) }
}

unsafe fn imsg_dequeue_fd(imsgbuf: *mut imsgbuf) -> i32 {
    unsafe {
        let Some(ifd) = NonNull::new(tailq_first(&raw mut (*imsgbuf).fds)) else {
            return -1;
        };
        #[expect(clippy::shadow_reuse)]
        let ifd = ifd.as_ptr();

        let fd = (*ifd).fd;
        tailq_remove(&raw mut (*imsgbuf).fds, ifd);
        free(ifd as *mut c_void);

        fd
    }
}

pub unsafe fn imsg_flush(imsgbuf: *mut imsgbuf) -> c_int {
    unsafe {
        while (*imsgbuf).w.queued != 0 {
            if msgbuf_write(&raw mut (*imsgbuf).w) <= 0 {
                return -1;
            }
        }
        0
    }
}

pub unsafe fn imsg_clear(imsgbuf: *mut imsgbuf) {
    unsafe {
        msgbuf_clear(&raw mut (*imsgbuf).w);

        let mut fd;
        while {
            fd = imsg_dequeue_fd(imsgbuf);
            fd != -1
        } {
            close(fd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::imsg_buffer::{
        ibuf_data, ibuf_dynamic, ibuf_fd_set, ibuf_free, ibuf_size, msgbuf_clear,
    };
    use crate::compat::queue::tailq_first;
    use core::ffi::c_void;
    use libc::{EBADMSG, EINVAL, ERANGE};

    // Fresh, zeroed imsgbuf on the heap (it embeds a 64 KiB read buffer),
    // initialised via imsg_init (imsg.c:107) with fd == -1.
    unsafe fn new_imsgbuf() -> Box<imsgbuf> {
        unsafe {
            let mut b: Box<imsgbuf> = Box::new(std::mem::zeroed());
            imsg_init(&raw mut *b, -1);
            b
        }
    }

    // Compose one message into imsgbuf->w, then copy the serialized bytes
    // (header + payload) out of the single queued ibuf and clear the write
    // queue. This is the on-wire form imsg_get() would later parse.
    unsafe fn compose_to_bytes(
        p: *mut imsgbuf,
        type_: u32,
        id: u32,
        pid: pid_t,
        data: &[u8],
    ) -> Vec<u8> {
        unsafe {
            let rc = imsg_compose(
                p,
                type_,
                id,
                pid,
                -1,
                data.as_ptr() as *const c_void,
                data.len(),
            );
            assert_eq!(rc, 1);
            let wbuf = tailq_first(&raw mut (*p).w.bufs);
            assert!(!wbuf.is_null());
            let sz = ibuf_size(wbuf);
            let bytes = std::slice::from_raw_parts(ibuf_data(wbuf) as *const u8, sz).to_vec();
            msgbuf_clear(&raw mut (*p).w);
            assert_eq!((*p).w.queued, 0);
            bytes
        }
    }

    // Place raw bytes into the read buffer and set the write position, as if
    // imsg_read() had filled it from the socket.
    unsafe fn load(p: *mut imsgbuf, bytes: &[u8]) {
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                (*p).r.buf.as_mut_ptr(),
                bytes.len(),
            );
            (*p).r.wpos = bytes.len();
        }
    }

    // Read the imsg_hdr sitting at the front of an ibuf's readable region.
    unsafe fn hdr_of(buf: *const ibuf) -> imsg_hdr {
        unsafe { *(ibuf_data(buf) as *const imsg_hdr) }
    }

    // imsg_hdr layout is u32,u16,u16,u32,u32 => 16 bytes (imsg.c:74).
    #[test]
    fn test_header_size_is_16() {
        assert_eq!(IMSG_HEADER_SIZE, 16);
        assert_eq!(size_of::<imsg_hdr>(), 16);
    }

    // Full compose -> get round-trip with a payload. Verifies type/peerid/pid,
    // header length, the ssize_t return value, and payload integrity.
    // imsg_compose (imsg.c:324) -> imsg_get (imsg.c:202).
    #[test]
    fn test_compose_get_round_trip_payload() {
        unsafe {
            let mut b = new_imsgbuf();
            let p = &raw mut *b;
            let payload: [u8; 7] = [0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03];

            let bytes = compose_to_bytes(p, 0x4142_4344, 0x5566_7788, 4242, &payload);
            // Serialized length == header + payload.
            assert_eq!(bytes.len(), IMSG_HEADER_SIZE + payload.len());
            load(p, &bytes);

            let mut msg: imsg = std::mem::zeroed();
            let rv = imsg_get(p, &raw mut msg);
            assert_eq!(rv as usize, IMSG_HEADER_SIZE + payload.len());

            assert_eq!(imsg_get_type(&msg), 0x4142_4344);
            assert_eq!(imsg_get_id(&msg), 0x5566_7788);
            assert_eq!(imsg_get_pid(&msg), 4242);
            assert_eq!(msg.hdr.len as usize, IMSG_HEADER_SIZE + payload.len());
            // No fd was passed.
            assert_eq!(msg.fd, -1);

            // Payload integrity via imsg_get_len + imsg.data.
            assert_eq!(imsg_get_len(&msg), payload.len());
            let got = std::slice::from_raw_parts(msg.data as *const u8, payload.len());
            assert_eq!(got, payload);

            // Whole read buffer consumed.
            assert_eq!((*p).r.wpos, 0);

            imsg_free(&raw mut msg);
        }
    }

    // Empty payload: datalen == 0, so imsg_get leaves buf/data NULL and
    // imsg_get_len returns 0 (imsg.c:305 handles the NULL buf).
    #[test]
    fn test_compose_get_round_trip_empty() {
        unsafe {
            let mut b = new_imsgbuf();
            let p = &raw mut *b;

            let bytes = compose_to_bytes(p, 7, 9, 100, &[]);
            assert_eq!(bytes.len(), IMSG_HEADER_SIZE);
            load(p, &bytes);

            let mut msg: imsg = std::mem::zeroed();
            let rv = imsg_get(p, &raw mut msg);
            assert_eq!(rv as usize, IMSG_HEADER_SIZE);
            assert_eq!(imsg_get_type(&msg), 7);
            assert_eq!(imsg_get_id(&msg), 9);
            assert_eq!(imsg_get_pid(&msg), 100);
            assert_eq!(msg.hdr.len as usize, IMSG_HEADER_SIZE);
            assert!(msg.buf.is_null());
            assert!(msg.data.is_null());
            assert_eq!(imsg_get_len(&msg), 0);
            assert_eq!((*p).r.wpos, 0);

            // imsg_free on a NULL buf is a safe no-op (imsg.c:543).
            imsg_free(&raw mut msg);
        }
    }

    // pid == 0 in imsg_compose/imsg_create means "use imsgbuf->pid", which
    // imsg_init sets to getpid() (imsg.c:113, 496-500).
    #[test]
    fn test_compose_pid_zero_uses_imsgbuf_pid() {
        unsafe {
            let mut b = new_imsgbuf();
            let p = &raw mut *b;
            assert_eq!((*p).pid, std::process::id() as i32);

            let bytes = compose_to_bytes(p, 1, 2, 0, &[0xAA]);
            load(p, &bytes);

            let mut msg: imsg = std::mem::zeroed();
            assert!(imsg_get(p, &raw mut msg) > 0);
            assert_eq!(imsg_get_pid(&msg), std::process::id() as i32);
            imsg_free(&raw mut msg);
        }
    }

    // Fewer than IMSG_HEADER_SIZE bytes available => imsg_get returns 0 and
    // leaves the read buffer untouched (imsg.c:210-212).
    #[test]
    fn test_get_needs_full_header() {
        unsafe {
            let mut b = new_imsgbuf();
            let p = &raw mut *b;
            load(p, &[0u8; 5]);
            let mut msg: imsg = std::mem::zeroed();
            assert_eq!(imsg_get(p, &raw mut msg), 0);
            assert_eq!((*p).r.wpos, 5);
        }
    }

    // Header present but full message not yet buffered (hdr.len > available)
    // => imsg_get returns 0, wpos untouched (imsg.c:223-225).
    #[test]
    fn test_get_needs_full_payload() {
        unsafe {
            let mut b = new_imsgbuf();
            let p = &raw mut *b;
            let hdr = imsg_hdr {
                type_: 3,
                len: (IMSG_HEADER_SIZE + 10) as u16,
                flags: 0,
                peerid: 0,
                pid: 0,
            };
            let mut bytes = Vec::new();
            bytes.extend_from_slice(std::slice::from_raw_parts(
                &raw const hdr as *const u8,
                IMSG_HEADER_SIZE,
            ));
            bytes.extend_from_slice(&[0u8; 2]); // only 2 of 10 payload bytes
            load(p, &bytes);

            let mut msg: imsg = std::mem::zeroed();
            assert_eq!(imsg_get(p, &raw mut msg), 0);
            assert_eq!((*p).r.wpos, IMSG_HEADER_SIZE + 2);
        }
    }

    // hdr.len < IMSG_HEADER_SIZE => ERANGE / -1 (imsg.c:219-222).
    #[test]
    fn test_get_len_too_small_erange() {
        unsafe {
            let mut b = new_imsgbuf();
            let p = &raw mut *b;
            let hdr = imsg_hdr {
                type_: 0,
                len: 4,
                flags: 0,
                peerid: 0,
                pid: 0,
            };
            load(
                p,
                std::slice::from_raw_parts(&raw const hdr as *const u8, IMSG_HEADER_SIZE),
            );
            let mut msg: imsg = std::mem::zeroed();
            crate::errno!() = 0;
            assert_eq!(imsg_get(p, &raw mut msg), -1);
            assert_eq!(crate::errno!(), ERANGE);
        }
    }

    // hdr.len > MAX_IMSGSIZE => ERANGE / -1 (imsg.c:219-222).
    #[test]
    fn test_get_len_too_big_erange() {
        unsafe {
            let mut b = new_imsgbuf();
            let p = &raw mut *b;
            let hdr = imsg_hdr {
                type_: 0,
                len: (MAX_IMSGSIZE + 1) as u16,
                flags: 0,
                peerid: 0,
                pid: 0,
            };
            load(
                p,
                std::slice::from_raw_parts(&raw const hdr as *const u8, IMSG_HEADER_SIZE),
            );
            let mut msg: imsg = std::mem::zeroed();
            crate::errno!() = 0;
            assert_eq!(imsg_get(p, &raw mut msg), -1);
            assert_eq!(crate::errno!(), ERANGE);
        }
    }

    // Two messages back-to-back in the read buffer: imsg_get returns the first
    // and memmoves the remainder to the front (imsg.c:250-260), then the second
    // read drains the buffer to empty.
    #[test]
    fn test_get_leftover_memmove_two_messages() {
        unsafe {
            let mut b = new_imsgbuf();
            let p = &raw mut *b;
            let pa: [u8; 3] = [0xA0, 0xA1, 0xA2];
            let pb: [u8; 5] = [0xB0, 0xB1, 0xB2, 0xB3, 0xB4];

            let mut bytes = compose_to_bytes(p, 0x11, 0x1111, 11, &pa);
            let lena = bytes.len();
            let bb = compose_to_bytes(p, 0x22, 0x2222, 22, &pb);
            let lenb = bb.len();
            bytes.extend_from_slice(&bb);
            load(p, &bytes);

            // First message.
            let mut m1: imsg = std::mem::zeroed();
            assert_eq!(imsg_get(p, &raw mut m1) as usize, lena);
            assert_eq!(imsg_get_type(&m1), 0x11);
            assert_eq!(imsg_get_id(&m1), 0x1111);
            assert_eq!(imsg_get_len(&m1), pa.len());
            assert_eq!(
                std::slice::from_raw_parts(m1.data as *const u8, pa.len()),
                pa
            );
            // Remainder shifted to the front.
            assert_eq!((*p).r.wpos, lenb);
            imsg_free(&raw mut m1);

            // Second message.
            let mut m2: imsg = std::mem::zeroed();
            assert_eq!(imsg_get(p, &raw mut m2) as usize, lenb);
            assert_eq!(imsg_get_type(&m2), 0x22);
            assert_eq!(imsg_get_id(&m2), 0x2222);
            assert_eq!(
                std::slice::from_raw_parts(m2.data as *const u8, pb.len()),
                pb
            );
            assert_eq!((*p).r.wpos, 0);
            imsg_free(&raw mut m2);
        }
    }

    // imsg_create (imsg.c:477) allocates an ibuf pre-loaded with the header
    // (len placeholder 0, flags 0, correct type/peerid/pid).
    #[test]
    fn test_imsg_create_header_only() {
        unsafe {
            let mut b = new_imsgbuf();
            let p = &raw mut *b;
            let wbuf = imsg_create(p, 0xCAFE, 0xBEEF, 777, 8);
            assert!(!wbuf.is_null());
            // Only the header is present so far.
            assert_eq!(ibuf_size(wbuf), IMSG_HEADER_SIZE);
            let h = hdr_of(wbuf);
            assert_eq!(h.type_, 0xCAFE);
            assert_eq!(h.peerid, 0xBEEF);
            assert_eq!(h.pid, 777);
            assert_eq!(h.flags, 0);
            assert_eq!(h.len, 0); // filled in later by imsg_close
            ibuf_free(wbuf);
        }
    }

    // imsg_create rejects datalen that would push past MAX_IMSGSIZE (imsg.c:486-490).
    #[test]
    fn test_imsg_create_too_big_erange() {
        unsafe {
            let mut b = new_imsgbuf();
            let p = &raw mut *b;
            crate::errno!() = 0;
            // datalen + IMSG_HEADER_SIZE > MAX_IMSGSIZE.
            let wbuf = imsg_create(p, 1, 2, 3, MAX_IMSGSIZE);
            assert!(wbuf.is_null());
            assert_eq!(crate::errno!(), ERANGE);
        }
    }

    // imsg_add (imsg.c:517) appends payload and returns datalen; a 0-length add
    // is a no-op returning 0.
    #[test]
    fn test_imsg_add_appends_and_zero() {
        unsafe {
            let buf = ibuf_dynamic(64, MAX_IMSGSIZE);
            assert!(!buf.is_null());
            let data: [u8; 5] = [1, 2, 3, 4, 5];
            assert_eq!(imsg_add(buf, data.as_ptr() as *const c_void, 5), 5);
            assert_eq!(ibuf_size(buf), 5);
            assert_eq!(
                std::slice::from_raw_parts(ibuf_data(buf) as *const u8, 5),
                data
            );
            // Zero-length add: no-op, returns 0, size unchanged.
            assert_eq!(imsg_add(buf, std::ptr::null(), 0), 0);
            assert_eq!(ibuf_size(buf), 5);
            ibuf_free(buf);
        }
    }

    // imsg_add failure path (imsg.c:519-522): a failing ibuf_add frees the msg
    // and returns -1. buf must NOT be freed again by the caller here.
    #[test]
    fn test_imsg_add_overflow_frees_and_errors() {
        unsafe {
            // Fixed capacity 4; adding 8 overflows.
            let buf = ibuf_dynamic(4, 4);
            assert!(!buf.is_null());
            let data = [0u8; 8];
            assert_eq!(imsg_add(buf, data.as_ptr() as *const c_void, 8), -1);
            // buf was freed by imsg_add; do not touch it further.
        }
    }

    // imsg_close (imsg.c:527) backfills hdr.len with the ibuf size and enqueues
    // the buffer; with no fd, IMSGF_HASFD stays clear.
    #[test]
    fn test_imsg_close_sets_len_no_fd() {
        unsafe {
            let mut b = new_imsgbuf();
            let p = &raw mut *b;
            let wbuf = imsg_create(p, 5, 6, 7, 4);
            assert!(!wbuf.is_null());
            let data: [u8; 4] = [9, 8, 7, 6];
            assert_eq!(imsg_add(wbuf, data.as_ptr() as *const c_void, 4), 4);
            imsg_close(p, wbuf);

            assert_eq!((*p).w.queued, 1);
            let q = tailq_first(&raw mut (*p).w.bufs);
            assert!(!q.is_null());
            let h = hdr_of(q);
            assert_eq!(h.len as usize, IMSG_HEADER_SIZE + 4);
            assert_eq!(h.flags & IMSGF_HASFD, 0);
            msgbuf_clear(&raw mut (*p).w);
        }
    }

    // imsg_close marks IMSGF_HASFD when the ibuf carries an fd (imsg.c:532-535).
    #[test]
    fn test_imsg_close_sets_hasfd_flag() {
        unsafe {
            let mut b = new_imsgbuf();
            let p = &raw mut *b;
            let wbuf = imsg_create(p, 1, 1, 1, 0);
            assert!(!wbuf.is_null());
            // A real, dup'd fd; ibuf_free (via msgbuf_clear) will close it.
            let fd = libc::dup(2);
            assert!(fd >= 0);
            ibuf_fd_set(wbuf, fd);
            imsg_close(p, wbuf);

            let q = tailq_first(&raw mut (*p).w.bufs);
            let h = hdr_of(q);
            assert_ne!(h.flags & IMSGF_HASFD, 0);
            assert_eq!(h.len as usize, IMSG_HEADER_SIZE);
            msgbuf_clear(&raw mut (*p).w);
        }
    }

    // imsg_get_data (imsg.c:280) returns the payload only when the length
    // matches exactly; otherwise EBADMSG, and EINVAL for len == 0.
    #[test]
    fn test_imsg_get_data_and_errors() {
        unsafe {
            let mut b = new_imsgbuf();
            let p = &raw mut *b;
            let payload: [u8; 4] = [0x11, 0x22, 0x33, 0x44];
            let bytes = compose_to_bytes(p, 1, 2, 3, &payload);
            load(p, &bytes);
            let mut msg: imsg = std::mem::zeroed();
            assert!(imsg_get(p, &raw mut msg) > 0);

            // len == 0 => EINVAL.
            crate::errno!() = 0;
            let mut scratch = [0u8; 4];
            assert_eq!(
                imsg_get_data(&raw mut msg, scratch.as_mut_ptr() as *mut c_void, 0),
                -1
            );
            assert_eq!(crate::errno!(), EINVAL);

            // Wrong length => EBADMSG.
            crate::errno!() = 0;
            assert_eq!(
                imsg_get_data(&raw mut msg, scratch.as_mut_ptr() as *mut c_void, 3),
                -1
            );
            assert_eq!(crate::errno!(), EBADMSG);

            // Exact length => success, bytes copied out.
            assert_eq!(
                imsg_get_data(&raw mut msg, scratch.as_mut_ptr() as *mut c_void, 4),
                0
            );
            assert_eq!(scratch, payload);

            imsg_free(&raw mut msg);
        }
    }

    // imsg_get_fd (imsg.c:295) returns the stored fd once, then -1.
    #[test]
    fn test_imsg_get_fd_takes_once() {
        unsafe {
            let mut msg: imsg = std::mem::zeroed();
            msg.fd = 42;
            assert_eq!(imsg_get_fd(&raw mut msg), 42);
            assert_eq!(msg.fd, -1);
            assert_eq!(imsg_get_fd(&raw mut msg), -1);
        }
    }

    // Snapshot the single queued write buffer as raw bytes, then clear the
    // write queue (companion to compose_to_bytes for the *v/ibuf composers).
    unsafe fn drain_first_wbuf(p: *mut imsgbuf) -> Vec<u8> {
        unsafe {
            let wbuf = tailq_first(&raw mut (*p).w.bufs);
            assert!(!wbuf.is_null());
            let sz = ibuf_size(wbuf);
            let bytes = std::slice::from_raw_parts(ibuf_data(wbuf) as *const u8, sz).to_vec();
            msgbuf_clear(&raw mut (*p).w);
            bytes
        }
    }

    // imsg_composev (imsg.c:351) concatenates the iovec segments into one
    // payload; the round-trip through imsg_get must recover the joined bytes.
    #[test]
    fn test_composev_round_trip() {
        unsafe {
            let mut b = new_imsgbuf();
            let p = &raw mut *b;
            let s1: [u8; 2] = [0x01, 0x02];
            let s2: [u8; 3] = [0x03, 0x04, 0x05];
            let iov = [
                libc::iovec {
                    iov_base: s1.as_ptr() as *mut c_void,
                    iov_len: s1.len(),
                },
                libc::iovec {
                    iov_base: s2.as_ptr() as *mut c_void,
                    iov_len: s2.len(),
                },
            ];
            let rc = imsg_composev(p, 0xAA, 0xBB, 5, -1, iov.as_ptr(), 2);
            assert_eq!(rc, 1);
            let bytes = drain_first_wbuf(p);
            assert_eq!(bytes.len(), IMSG_HEADER_SIZE + 5);
            load(p, &bytes);

            let mut msg: imsg = std::mem::zeroed();
            assert!(imsg_get(p, &raw mut msg) > 0);
            assert_eq!(imsg_get_type(&msg), 0xAA);
            assert_eq!(imsg_get_id(&msg), 0xBB);
            assert_eq!(imsg_get_len(&msg), 5);
            let got = std::slice::from_raw_parts(msg.data as *const u8, 5);
            assert_eq!(got, [0x01, 0x02, 0x03, 0x04, 0x05]);
            imsg_free(&raw mut msg);
        }
    }

    // imsg_composev with iovcnt 0: datalen 0, so the message is header-only.
    #[test]
    fn test_composev_empty() {
        unsafe {
            let mut b = new_imsgbuf();
            let p = &raw mut *b;
            let rc = imsg_composev(p, 1, 2, 3, -1, std::ptr::null(), 0);
            assert_eq!(rc, 1);
            let bytes = drain_first_wbuf(p);
            assert_eq!(bytes.len(), IMSG_HEADER_SIZE);
            load(p, &bytes);
            let mut msg: imsg = std::mem::zeroed();
            assert_eq!(imsg_get(p, &raw mut msg) as usize, IMSG_HEADER_SIZE);
            assert_eq!(imsg_get_len(&msg), 0);
            assert!(msg.buf.is_null());
            imsg_free(&raw mut msg);
        }
    }

    // imsg_forward (imsg.c:442) re-emits a received message with the same
    // type/peerid/pid and payload into the write queue.
    #[test]
    fn test_imsg_forward_reemits() {
        unsafe {
            let mut b = new_imsgbuf();
            let p = &raw mut *b;
            let payload: [u8; 4] = [0x10, 0x20, 0x30, 0x40];
            let bytes = compose_to_bytes(p, 0x77, 0x88, 55, &payload);
            load(p, &bytes);
            let mut msg: imsg = std::mem::zeroed();
            assert!(imsg_get(p, &raw mut msg) > 0);

            assert_eq!(imsg_forward(p, &raw mut msg), 1);
            let q = tailq_first(&raw mut (*p).w.bufs);
            assert!(!q.is_null());
            let h = hdr_of(q);
            assert_eq!(h.type_, 0x77);
            assert_eq!(h.peerid, 0x88);
            assert_eq!(h.pid, 55);
            assert_eq!(h.len as usize, IMSG_HEADER_SIZE + payload.len());
            let sz = ibuf_size(q);
            let all = std::slice::from_raw_parts(ibuf_data(q) as *const u8, sz);
            assert_eq!(&all[IMSG_HEADER_SIZE..], payload);
            msgbuf_clear(&raw mut (*p).w);
            imsg_free(&raw mut msg);
        }
    }

    // imsg_compose_ibuf (imsg.c:392) queues two buffers: a header buffer whose
    // len already reflects header + payload, followed by the payload ibuf.
    #[test]
    fn test_imsg_compose_ibuf_two_buffers() {
        unsafe {
            let mut b = new_imsgbuf();
            let p = &raw mut *b;
            let payload: [u8; 6] = [1, 2, 3, 4, 5, 6];
            let buf = ibuf_dynamic(payload.len(), MAX_IMSGSIZE);
            assert!(!buf.is_null());
            assert_eq!(ibuf_add(buf, payload.as_ptr() as *const c_void, 6), 0);

            assert_eq!(imsg_compose_ibuf(p, 0xABCD, 0x1234, 9, buf), 1);
            assert_eq!((*p).w.queued, 2);

            let first = tailq_first(&raw mut (*p).w.bufs);
            assert!(!first.is_null());
            assert_eq!(ibuf_size(first), IMSG_HEADER_SIZE);
            let h = hdr_of(first);
            assert_eq!(h.type_, 0xABCD);
            assert_eq!(h.peerid, 0x1234);
            assert_eq!(h.len as usize, IMSG_HEADER_SIZE + payload.len());

            let second = crate::compat::queue::tailq_next::<ibuf, ibuf, ()>(first);
            assert!(!second.is_null());
            assert_eq!(ibuf_size(second), payload.len());
            let body = std::slice::from_raw_parts(ibuf_data(second) as *const u8, payload.len());
            assert_eq!(body, payload);
            msgbuf_clear(&raw mut (*p).w);
        }
    }

    // imsg_get_ibuf (imsg.c:269) exposes the message payload as a sub-ibuf
    // whose readable window matches the original data.
    #[test]
    fn test_imsg_get_ibuf_success() {
        unsafe {
            let mut b = new_imsgbuf();
            let p = &raw mut *b;
            let payload: [u8; 5] = [0x9A, 0x9B, 0x9C, 0x9D, 0x9E];
            let bytes = compose_to_bytes(p, 1, 2, 3, &payload);
            load(p, &bytes);
            let mut msg: imsg = std::mem::zeroed();
            assert!(imsg_get(p, &raw mut msg) > 0);

            let mut sub: ibuf = std::mem::zeroed();
            assert_eq!(imsg_get_ibuf(&raw mut msg, &raw mut sub), 0);
            assert_eq!(ibuf_size(&raw const sub), payload.len());
            let d = std::slice::from_raw_parts(ibuf_data(&raw const sub) as *const u8, payload.len());
            assert_eq!(d, payload);
            // sub is a stack ibuf (max == 0); never free it. Read before dropping msg.
            imsg_free(&raw mut msg);
        }
    }

    // imsg_get_ibuf on a message with no payload buffer errors with EBADMSG
    // (imsg.c:271-273).
    #[test]
    fn test_imsg_get_ibuf_null_buf_ebadmsg() {
        unsafe {
            let mut msg: imsg = std::mem::zeroed();
            msg.buf = std::ptr::null_mut();
            let mut sub: ibuf = std::mem::zeroed();
            crate::errno!() = 0;
            assert_eq!(imsg_get_ibuf(&raw mut msg, &raw mut sub), -1);
            assert_eq!(crate::errno!(), EBADMSG);
        }
    }

    // Composing several messages without draining leaves them all queued in
    // imsgbuf->w (imsg_close increments queued, imsg-buffer.c ibuf_enqueue).
    #[test]
    fn test_multiple_messages_queued_count() {
        unsafe {
            let mut b = new_imsgbuf();
            let p = &raw mut *b;
            for i in 0..3u32 {
                assert_eq!(
                    imsg_compose(p, i, i, 1, -1, std::ptr::null(), 0),
                    1
                );
            }
            assert_eq!((*p).w.queued, 3);
            msgbuf_clear(&raw mut (*p).w);
            assert_eq!((*p).w.queued, 0);
        }
    }

    // The header field getters read straight out of imsg.hdr (imsg.c:300-321);
    // imsg_get_len returns 0 for a NULL payload buffer (imsg.c:305).
    #[test]
    fn test_header_getters_direct() {
        unsafe {
            let mut msg: imsg = std::mem::zeroed();
            msg.hdr.type_ = 0x1234;
            msg.hdr.peerid = 0x5678;
            msg.hdr.pid = 999;
            msg.buf = std::ptr::null_mut();
            assert_eq!(imsg_get_type(&msg), 0x1234);
            assert_eq!(imsg_get_id(&msg), 0x5678);
            assert_eq!(imsg_get_pid(&msg), 999);
            assert_eq!(imsg_get_len(&msg), 0);
        }
    }
}
