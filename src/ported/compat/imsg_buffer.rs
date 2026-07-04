#![allow(unused)]

use ::core::{
    ffi::{c_int, c_void},
    ptr::{NonNull, null_mut},
};
use ::libc::{
    CMSG_DATA, CMSG_FIRSTHDR, CMSG_LEN, CMSG_NXTHDR, CMSG_SPACE, EAGAIN, EBADMSG, EINTR, EINVAL,
    EMSGSIZE, ENOBUFS, ERANGE, SCM_RIGHTS, SOL_SOCKET, abort, c_uchar, calloc, close, cmsghdr, free,
    iovec, malloc, memcpy, memmove, memset, msghdr, readv, recvmsg, sendmsg, writev,
};

use super::imsg::{IBUF_READ_SIZE, ibuf, ibufqueue, msgbuf, readhdr_fn};
use super::queue::{
    tailq_first, tailq_foreach, tailq_init, tailq_insert_tail, tailq_next, tailq_remove,
};
use super::{freezero, recallocarray::recallocarray};
use crate::errno;

const IOV_MAX: usize = 1024; // TODO find where IOV_MAX is defined

/// C `vendor/tmux/compat/imsg-buffer.c:67`: sentinel `ibuf.fd` value marking a
/// stack-backed (non-owning) ibuf, e.g. from `ibuf_from_buffer`. Such buffers
/// must never be freed, enqueued, or grown.
const IBUF_FD_MARK_ON_STACK: c_int = -2;

/// C `vendor/tmux/compat/imsg-buffer.c:70`: `struct ibuf *ibuf_open(size_t len)`
pub unsafe fn ibuf_open(len: usize) -> *mut ibuf {
    unsafe {
        if len == 0 {
            errno!() = EINVAL;
            return null_mut();
        }
        let buf: *mut ibuf = calloc(1, size_of::<ibuf>()) as *mut ibuf;
        if buf.is_null() {
            return null_mut();
        }
        (*buf).buf = calloc(len, 1) as *mut c_uchar;
        if (*buf).buf.is_null() {
            free(buf as *mut c_void);
            return null_mut();
        }

        (*buf).max = len;
        (*buf).size = len;
        (*buf).fd = -1;

        buf
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:89`: `struct ibuf *ibuf_dynamic(size_t len, size_t max)`
pub unsafe fn ibuf_dynamic(len: usize, max: usize) -> *mut ibuf {
    unsafe {
        // C imsg-buffer.c:106: reject only max==0 or max<len (len==0 is a valid
        // empty dynamic buffer that may grow up to max).
        if max == 0 || max < len {
            errno!() = EINVAL;
            return null_mut();
        }
        let buf: *mut ibuf = calloc(1, size_of::<ibuf>()) as *mut ibuf;
        if buf.is_null() {
            return null_mut();
        }
        if len > 0 {
            (*buf).buf = calloc(len, 1) as *mut c_uchar;
            if (*buf).buf.is_null() {
                free(buf as *mut c_void);
                return null_mut();
            }
        }
        // C imsg-buffer.c:115-116: size is the initial length, max is the growth
        // ceiling (NOT len — that was the port bug that prevented any growth).
        (*buf).size = len;
        (*buf).max = max;
        (*buf).fd = -1;

        buf
    }
}

pub unsafe fn ibuf_realloc(buf: *mut ibuf, len: usize) -> i32 {
    unsafe {
        // on static buffers max is eq size and so the following fails
        if len > usize::MAX - (*buf).wpos || (*buf).wpos + len > (*buf).max {
            errno!() = ERANGE;
            return -1;
        }

        let b = recallocarray((*buf).buf as *mut c_void, (*buf).size, (*buf).wpos + len, 1);
        if b.is_null() {
            return -1;
        }
        (*buf).buf = b as *mut u8;
        (*buf).size = (*buf).wpos + len;

        0
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:114`: `void *ibuf_reserve(struct ibuf *buf, size_t len)`
pub unsafe fn ibuf_reserve(buf: *mut ibuf, len: usize) -> *mut c_void {
    unsafe {
        if len > usize::MAX - (*buf).wpos {
            errno!() = ERANGE;
            return null_mut();
        }
        if (*buf).fd == IBUF_FD_MARK_ON_STACK {
            // can not grow stack buffers
            errno!() = EINVAL;
            return null_mut();
        }

        if (*buf).wpos + len > (*buf).size && ibuf_realloc(buf, len) == -1 {
            return null_mut();
        }

        let b = (*buf).buf.add((*buf).wpos);
        (*buf).wpos += len;
        b as *mut c_void
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:150`: `int ibuf_add(struct ibuf *buf, const void *data, size_t len)`
pub unsafe fn ibuf_add(buf: *mut ibuf, data: *const c_void, len: usize) -> i32 {
    unsafe {
        let b = ibuf_reserve(buf, len);
        if b.is_null() {
            return -1;
        }

        memcpy(b, data, len);
        0
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:165`: `int ibuf_add_ibuf(struct ibuf *buf, const struct ibuf *from)`
pub unsafe fn ibuf_add_ibuf(buf: *mut ibuf, from: *const ibuf) -> c_int {
    unsafe { ibuf_add(buf, ibuf_data(from), ibuf_size(from)) }
}

pub unsafe fn ibuf_add_buf(buf: *mut ibuf, from: *const ibuf) -> c_int {
    unsafe { ibuf_add_ibuf(buf, from) }
}

/// C `vendor/tmux/compat/imsg-buffer.c:171`: `int ibuf_add_n8(struct ibuf *buf, uint64_t value)`
pub unsafe fn ibuf_add_n8(buf: *mut ibuf, value: u64) -> c_int {
    unsafe {
        if value > u8::MAX as u64 {
            errno!() = EINVAL;
            return -1;
        }
        let v = value;
        ibuf_add(buf, &raw const v as _, size_of::<u8>())
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:184`: `int ibuf_add_n16(struct ibuf *buf, uint64_t value)`
pub unsafe fn ibuf_add_n16(buf: *mut ibuf, value: u64) -> c_int {
    unsafe {
        if value > u16::MAX as u64 {
            errno!() = EINVAL;
            return -1;
        }
        let v = (value as u16).to_be();
        ibuf_add(buf, &raw const v as _, size_of::<u16>())
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:197`: `int ibuf_add_n32(struct ibuf *buf, uint64_t value)`
pub unsafe fn ibuf_add_n32(buf: *mut ibuf, value: u64) -> c_int {
    unsafe {
        if value > u32::MAX as u64 {
            errno!() = EINVAL;
            return -1;
        }
        let v = (value as u32).to_be();
        ibuf_add(buf, &raw const v as _, size_of::<u32>())
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:210`: `int ibuf_add_n64(struct ibuf *buf, uint64_t value)`
pub unsafe fn ibuf_add_n64(buf: *mut ibuf, value: u64) -> c_int {
    unsafe {
        let v = value.to_be();
        ibuf_add(buf, &raw const v as _, size_of::<u64>())
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:217`: `int ibuf_add_h16(struct ibuf *buf, uint64_t value)`
pub unsafe fn ibuf_add_h16(buf: *mut ibuf, value: u64) -> c_int {
    unsafe {
        if value > u16::MAX as u64 {
            errno!() = EINVAL;
            return -1;
        }
        let v = value as u16;
        ibuf_add(buf, &raw const v as _, size_of::<u16>())
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:230`: `int ibuf_add_h32(struct ibuf *buf, uint64_t value)`
pub unsafe fn ibuf_add_h32(buf: *mut ibuf, value: u64) -> c_int {
    unsafe {
        if value > u32::MAX as u64 {
            errno!() = EINVAL;
            return -1;
        }
        let v = value as u32;
        ibuf_add(buf, &raw const v as _, size_of::<u32>())
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:243`: `int ibuf_add_h64(struct ibuf *buf, uint64_t value)`
pub unsafe fn ibuf_add_h64(buf: *mut ibuf, value: u64) -> c_int {
    unsafe { ibuf_add(buf, &raw const value as *const c_void, size_of::<u64>()) }
}

/// C `vendor/tmux/compat/imsg-buffer.c:249`: `int ibuf_add_zero(struct ibuf *buf, size_t len)`
pub unsafe fn ibuf_add_zero(buf: *mut ibuf, len: usize) -> c_int {
    unsafe {
        let b: *mut c_void = ibuf_reserve(buf, len);
        if b.is_null() {
            return -1;
        }
        memset(b, 0, len);
        0
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:282`: `void *ibuf_seek(struct ibuf *buf, size_t pos, size_t len)`
pub unsafe fn ibuf_seek(buf: *mut ibuf, pos: usize, len: usize) -> *mut c_void {
    unsafe {
        // only allow seeking between rpos and wpos
        if ibuf_size(buf) < pos || usize::MAX - pos < len || ibuf_size(buf) < pos + len {
            errno!() = ERANGE;
            return null_mut();
        }

        (*buf).buf.add((*buf).rpos + pos) as _
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:295`: `int ibuf_set(struct ibuf *buf, size_t pos, const void *data, size_t len)`
pub unsafe fn ibuf_set(buf: *mut ibuf, pos: usize, data: *const c_void, len: usize) -> c_int {
    unsafe {
        let b = ibuf_seek(buf, pos, len);
        if b.is_null() {
            return -1;
        }

        memcpy(b, data, len);
        0
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:309`: `int ibuf_set_n8(struct ibuf *buf, size_t pos, uint64_t value)`
pub unsafe fn ibuf_set_n8(buf: *mut ibuf, pos: usize, value: u64) -> c_int {
    unsafe {
        if value > u8::MAX as u64 {
            errno!() = EINVAL;
            return -1;
        }
        let v = value as u8;
        ibuf_set(buf, pos, &raw const v as *const c_void, size_of::<u8>())
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:322`: `int ibuf_set_n16(struct ibuf *buf, size_t pos, uint64_t value)`
pub unsafe fn ibuf_set_n16(buf: *mut ibuf, pos: usize, value: u64) -> c_int {
    unsafe {
        if value > u16::MAX as u64 {
            errno!() = EINVAL;
            return -1;
        }
        let v = u16::to_be(value as u16);
        ibuf_set(buf, pos, &raw const v as *const c_void, size_of::<u16>())
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:335`: `int ibuf_set_n32(struct ibuf *buf, size_t pos, uint64_t value)`
pub unsafe fn ibuf_set_n32(buf: *mut ibuf, pos: usize, value: u64) -> c_int {
    unsafe {
        if value > u32::MAX as u64 {
            errno!() = EINVAL;
            return -1;
        }
        let v = u32::to_be(value as u32);
        ibuf_set(buf, pos, &raw const v as *const c_void, size_of::<u32>())
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:348`: `int ibuf_set_n64(struct ibuf *buf, size_t pos, uint64_t value)`
pub unsafe fn ibuf_set_n64(buf: *mut ibuf, pos: usize, value: u64) -> c_int {
    unsafe {
        let v = u64::to_be(value);
        ibuf_set(buf, pos, &raw const v as *const c_void, size_of::<u64>())
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:355`: `int ibuf_set_h16(struct ibuf *buf, size_t pos, uint64_t value)`
pub unsafe fn ibuf_set_h16(buf: *mut ibuf, pos: usize, value: u64) -> c_int {
    unsafe {
        if value > u16::MAX as u64 {
            errno!() = EINVAL;
            return -1;
        }
        let v = value as u16;
        ibuf_set(buf, pos, &raw const v as *const c_void, size_of::<u16>())
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:368`: `int ibuf_set_h32(struct ibuf *buf, size_t pos, uint64_t value)`
pub unsafe fn ibuf_set_h32(buf: *mut ibuf, pos: usize, value: u64) -> c_int {
    unsafe {
        if value > u32::MAX as u64 {
            errno!() = EINVAL;
            return -1;
        }
        let v = value as u32;
        ibuf_set(buf, pos, &raw const v as *const c_void, size_of::<u32>())
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:381`: `int ibuf_set_h64(struct ibuf *buf, size_t pos, uint64_t value)`
pub unsafe fn ibuf_set_h64(buf: *mut ibuf, pos: usize, value: u64) -> c_int {
    unsafe {
        ibuf_set(
            buf,
            pos,
            &raw const value as *const c_void,
            size_of::<u64>(),
        )
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:403`: `void *ibuf_data(const struct ibuf *buf)`
pub unsafe fn ibuf_data(buf: *const ibuf) -> *mut c_void {
    unsafe { (*buf).buf.add((*buf).rpos) as *mut c_void }
}

/// C `vendor/tmux/compat/imsg-buffer.c:409`: `size_t ibuf_size(const struct ibuf *buf)`
pub unsafe fn ibuf_size(buf: *const ibuf) -> usize {
    unsafe { (*buf).wpos - (*buf).rpos }
}

/// C `vendor/tmux/compat/imsg-buffer.c:415`: `size_t ibuf_left(const struct ibuf *buf)`
pub unsafe fn ibuf_left(buf: *const ibuf) -> usize {
    unsafe {
        // on stack buffers have no space left
        if (*buf).fd == IBUF_FD_MARK_ON_STACK {
            return 0;
        }
        (*buf).max - (*buf).wpos
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:424`: `int ibuf_truncate(struct ibuf *buf, size_t len)`
pub unsafe fn ibuf_truncate(buf: *mut ibuf, len: usize) -> c_int {
    unsafe {
        if ibuf_size(buf) >= len {
            (*buf).wpos = (*buf).rpos + len;
            return 0;
        }
        if (*buf).fd == IBUF_FD_MARK_ON_STACK {
            // only allow to truncate down for stack buffers
            errno!() = ERANGE;
            return -1;
        }
        ibuf_add_zero(buf, len - ibuf_size(buf))
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:439`: `void ibuf_rewind(struct ibuf *buf)`
pub unsafe fn ibuf_rewind(buf: *mut ibuf) {
    unsafe {
        (*buf).rpos = 0;
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:445`: `void ibuf_close(struct msgbuf *msgbuf, struct ibuf *buf)`
pub unsafe fn ibuf_close(msgbuf: *mut msgbuf, buf: *mut ibuf) {
    unsafe {
        ibufq_push(&raw mut (*msgbuf).bufs, buf);
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:451`: `void ibuf_from_buffer(struct ibuf *buf, void *data, size_t len)`
pub unsafe fn ibuf_from_buffer(buf: *mut ibuf, data: *mut c_void, len: usize) {
    unsafe {
        memset(buf as _, 0, size_of::<ibuf>());
        (*buf).buf = data as _;
        (*buf).wpos = len;
        (*buf).size = len;
        (*buf).fd = IBUF_FD_MARK_ON_STACK;
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:460`: `void ibuf_from_ibuf(struct ibuf *buf, const struct ibuf *from)`
pub unsafe fn ibuf_from_ibuf(buf: *mut ibuf, from: *const ibuf) {
    unsafe {
        ibuf_from_buffer(buf, ibuf_data(from), ibuf_size(from));
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:466`: `int ibuf_get(struct ibuf *buf, void *data, size_t len)`
pub unsafe fn ibuf_get(buf: *mut ibuf, data: *mut c_void, len: usize) -> c_int {
    unsafe {
        if ibuf_size(buf) < len {
            errno!() = EBADMSG;
            return -1;
        }

        memcpy(data, ibuf_data(buf), len);
        (*buf).rpos += len;
        0
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:479`: `int ibuf_get_ibuf(struct ibuf *buf, size_t len, struct ibuf *new)`
pub unsafe fn ibuf_get_ibuf(buf: *mut ibuf, len: usize, new: *mut ibuf) -> c_int {
    unsafe {
        if ibuf_size(buf) < len {
            errno!() = EBADMSG;
            return -1;
        }

        ibuf_from_buffer(new, ibuf_data(buf), len);
        (*buf).rpos += len;
        0
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:510`: `int ibuf_get_n8(struct ibuf *buf, uint8_t *value)`
pub unsafe fn ibuf_get_n8(buf: *mut ibuf, value: *mut u8) -> c_int {
    unsafe { ibuf_get(buf, value as _, size_of::<u8>()) }
}

/// C `vendor/tmux/compat/imsg-buffer.c:516`: `int ibuf_get_n16(struct ibuf *buf, uint16_t *value)`
pub unsafe fn ibuf_get_n16(buf: *mut ibuf, value: *mut u16) -> c_int {
    unsafe {
        let rv = ibuf_get(buf, value as _, size_of::<u16>());
        *value = u16::from_be(*value);
        rv
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:526`: `int ibuf_get_n32(struct ibuf *buf, uint32_t *value)`
pub unsafe fn ibuf_get_n32(buf: *mut ibuf, value: *mut u32) -> c_int {
    unsafe {
        let rv = ibuf_get(buf, value as _, size_of::<u32>());
        *value = u32::from_be(*value);
        rv
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:536`: `int ibuf_get_n64(struct ibuf *buf, uint64_t *value)`
pub unsafe fn ibuf_get_n64(buf: *mut ibuf, value: *mut u64) -> c_int {
    unsafe {
        let rv = ibuf_get(buf, value as _, size_of::<u64>());
        *value = u64::from_be(*value);
        rv
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:492`: `int ibuf_get_h16(struct ibuf *buf, uint16_t *value)`
pub unsafe fn ibuf_get_h16(buf: *mut ibuf, value: *mut u16) -> c_int {
    unsafe { ibuf_get(buf, value as _, size_of::<u16>()) }
}

/// C `vendor/tmux/compat/imsg-buffer.c:498`: `int ibuf_get_h32(struct ibuf *buf, uint32_t *value)`
pub unsafe fn ibuf_get_h32(buf: *mut ibuf, value: *mut u32) -> c_int {
    unsafe { ibuf_get(buf, value as _, size_of::<u32>()) }
}

/// C `vendor/tmux/compat/imsg-buffer.c:504`: `int ibuf_get_h64(struct ibuf *buf, uint64_t *value)`
pub unsafe fn ibuf_get_h64(buf: *mut ibuf, value: *mut u64) -> c_int {
    unsafe { ibuf_get(buf, value as _, size_of::<u64>()) }
}

/// C `vendor/tmux/compat/imsg-buffer.c:581`: `int ibuf_skip(struct ibuf *buf, size_t len)`
pub unsafe fn ibuf_skip(buf: *mut ibuf, len: usize) -> c_int {
    unsafe {
        if ibuf_size(buf) < len {
            errno!() = EBADMSG;
            return -1;
        }

        (*buf).rpos += len;
        0
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:386`: `int ibuf_set_maxsize(struct ibuf *buf, size_t max)`
pub unsafe fn ibuf_set_maxsize(buf: *mut ibuf, max: usize) -> c_int {
    unsafe {
        if (*buf).fd == IBUF_FD_MARK_ON_STACK {
            // can't fiddle with stack buffers
            errno!() = EINVAL;
            return -1;
        }
        if max > (*buf).max {
            errno!() = ERANGE;
            return -1;
        }
        (*buf).max = max;
        0
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:593`: `void ibuf_free(struct ibuf *buf)`
pub unsafe fn ibuf_free(buf: *mut ibuf) {
    unsafe {
        let save_errno = errno!();
        if buf.is_null() {
            return;
        }
        // if buf lives on the stack abort before causing more harm
        if (*buf).fd == IBUF_FD_MARK_ON_STACK {
            abort();
        }
        if (*buf).fd >= 0 {
            close((*buf).fd);
        }
        freezero((*buf).buf.cast(), (*buf).size);
        free(buf as *mut c_void);
        errno!() = save_errno;
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:610`: `int ibuf_fd_avail(struct ibuf *buf)`
pub unsafe fn ibuf_fd_avail(buf: *mut ibuf) -> c_int {
    unsafe { ((*buf).fd >= 0) as c_int }
}

/// C `vendor/tmux/compat/imsg-buffer.c:616`: `int ibuf_fd_get(struct ibuf *buf)`
pub unsafe fn ibuf_fd_get(buf: *mut ibuf) -> c_int {
    unsafe {
        // negative fds are internal use and equivalent to -1
        if (*buf).fd < 0 {
            return -1;
        }
        let fd = (*buf).fd;
        (*buf).fd = -1;
        fd
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:629`: `void ibuf_fd_set(struct ibuf *buf, int fd)`
pub unsafe fn ibuf_fd_set(buf: *mut ibuf, fd: c_int) {
    unsafe {
        // if buf lives on the stack abort before causing more harm
        if (*buf).fd == IBUF_FD_MARK_ON_STACK {
            abort();
        }
        if (*buf).fd >= 0 {
            close((*buf).fd);
        }
        (*buf).fd = -1;
        if fd >= 0 {
            (*buf).fd = fd;
        }
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:641`: `struct msgbuf *msgbuf_new(void)`
pub unsafe fn msgbuf_new() -> *mut msgbuf {
    unsafe {
        let m = calloc(1, size_of::<msgbuf>()) as *mut msgbuf;
        if m.is_null() {
            return null_mut();
        }
        ibufq_init(&raw mut (*m).bufs);
        ibufq_init(&raw mut (*m).rbufs);
        m
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:654`: `struct msgbuf *msgbuf_new_reader(size_t, ...)`
pub unsafe fn msgbuf_new_reader(
    hdrsz: usize,
    readhdr: readhdr_fn,
    arg: *mut c_void,
) -> *mut msgbuf {
    unsafe {
        if hdrsz == 0 || hdrsz > IBUF_READ_SIZE / 2 {
            errno!() = EINVAL;
            return null_mut();
        }

        let buf = malloc(IBUF_READ_SIZE) as *mut c_uchar;
        if buf.is_null() {
            return null_mut();
        }

        let msgbuf = msgbuf_new();
        if msgbuf.is_null() {
            free(buf as *mut c_void);
            return null_mut();
        }

        (*msgbuf).rbuf = buf;
        (*msgbuf).hdrsize = hdrsz;
        (*msgbuf).readhdr = readhdr;
        (*msgbuf).rarg = arg;

        msgbuf
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:683`: `void msgbuf_free(struct msgbuf *msgbuf)`
pub unsafe fn msgbuf_free(msgbuf: *mut msgbuf) {
    unsafe {
        if msgbuf.is_null() {
            return;
        }
        msgbuf_clear(msgbuf);
        free((*msgbuf).rbuf as *mut c_void);
        free(msgbuf as *mut c_void);
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:693`: `uint32_t msgbuf_queuelen(struct msgbuf *msgbuf)`
pub unsafe fn msgbuf_queuelen(msgbuf: *mut msgbuf) -> u32 {
    unsafe { ibufq_queuelen(&raw mut (*msgbuf).bufs) }
}

/// C `vendor/tmux/compat/imsg-buffer.c:699`: `void msgbuf_clear(struct msgbuf *msgbuf)`
pub unsafe fn msgbuf_clear(msgbuf: *mut msgbuf) {
    unsafe {
        // write side
        ibufq_flush(&raw mut (*msgbuf).bufs);
        // read side
        ibufq_flush(&raw mut (*msgbuf).rbufs);
        (*msgbuf).roff = 0;
        ibuf_free((*msgbuf).rpmsg);
        (*msgbuf).rpmsg = null_mut();
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:712`: `struct ibuf *msgbuf_get(struct msgbuf *msgbuf)`
pub unsafe fn msgbuf_get(msgbuf: *mut msgbuf) -> *mut ibuf {
    unsafe { ibufq_pop(&raw mut (*msgbuf).rbufs) }
}

/// C `vendor/tmux/compat/imsg-buffer.c:718`: `void msgbuf_concat(struct msgbuf *msgbuf, struct ibufqueue *from)`
pub unsafe fn msgbuf_concat(msgbuf: *mut msgbuf, from: *mut ibufqueue) {
    unsafe { ibufq_concat(&raw mut (*msgbuf).bufs, from) }
}

/// C `vendor/tmux/compat/imsg-buffer.c:724`: `int ibuf_write(int fd, struct msgbuf *msgbuf)`
pub unsafe fn ibuf_write(fd: c_int, msgbuf: *mut msgbuf) -> c_int {
    unsafe {
        let mut iov: [iovec; IOV_MAX] = std::mem::zeroed();
        let mut i: u32 = 0;

        for buf in tailq_foreach(&raw mut (*msgbuf).bufs.bufs).map(NonNull::as_ptr) {
            if i as usize >= IOV_MAX {
                break;
            }
            iov[i as usize].iov_base = ibuf_data(buf);
            iov[i as usize].iov_len = ibuf_size(buf);
            i += 1;
        }
        if i == 0 {
            return 0; // nothing queued
        }

        let mut n: isize;
        'again: loop {
            n = writev(fd, iov.as_ptr(), i as i32);
            if n == -1 {
                if errno!() == EINTR {
                    continue 'again;
                }
                if errno!() == EAGAIN || errno!() == ENOBUFS {
                    // lets retry later again
                    return 0;
                }
                return -1;
            }
            break 'again;
        }

        msgbuf_drain(msgbuf, n as usize);
        0
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:757`: `int msgbuf_write(int fd, struct msgbuf *msgbuf)`
pub unsafe fn msgbuf_write(fd: c_int, msgbuf: *mut msgbuf) -> c_int {
    unsafe {
        let mut iov: [iovec; IOV_MAX] = std::mem::zeroed();
        let mut buf0: *mut ibuf = null_mut();
        let mut i: u32 = 0;
        let mut msg: msghdr = std::mem::zeroed();
        let mut cmsgbuf: cmsgbuf = std::mem::zeroed();
        union cmsgbuf {
            _hdr: cmsghdr,
            buf: [u8; unsafe { CMSG_SPACE(size_of::<c_int>() as _) as usize }],
        }

        for buf in tailq_foreach(&raw mut (*msgbuf).bufs.bufs).map(NonNull::as_ptr) {
            if i as usize >= IOV_MAX {
                break;
            }
            if i > 0 && (*buf).fd != -1 {
                break;
            }
            iov[i as usize].iov_base = ibuf_data(buf);
            iov[i as usize].iov_len = ibuf_size(buf);
            i += 1;
            if (*buf).fd != -1 {
                buf0 = buf;
            }
        }

        if i == 0 {
            return 0; // nothing queued
        }

        msg.msg_iov = iov.as_mut_ptr();
        msg.msg_iovlen = i.try_into().unwrap();

        if !buf0.is_null() {
            msg.msg_control = &raw mut cmsgbuf.buf as _;
            msg.msg_controllen = size_of_val(&cmsgbuf.buf) as _;
            let cmsg = CMSG_FIRSTHDR(&raw const msg);
            (*cmsg).cmsg_len = CMSG_LEN(size_of::<c_int>() as u32) as _;
            (*cmsg).cmsg_level = SOL_SOCKET;
            (*cmsg).cmsg_type = SCM_RIGHTS;
            *CMSG_DATA(cmsg).cast() = (*buf0).fd;
        }

        let mut n: isize;
        'again: loop {
            n = sendmsg(fd, &raw const msg, 0);
            if n == -1 {
                if errno!() == EINTR {
                    continue 'again;
                }
                if errno!() == EAGAIN || errno!() == ENOBUFS {
                    // lets retry later again
                    return 0;
                }
                return -1;
            }
            break 'again;
        }

        // assumption: fd got sent if sendmsg sent anything
        if !buf0.is_null() {
            close((*buf0).fd);
            (*buf0).fd = -1;
        }

        msgbuf_drain(msgbuf, n as usize);
        0
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:826`: `static int ibuf_read_process(struct msgbuf *msgbuf, int fd)`
unsafe fn ibuf_read_process(msgbuf: *mut msgbuf, mut fd: c_int) -> c_int {
    unsafe {
        let mut rbuf: ibuf = std::mem::zeroed();
        let mut msg: ibuf = std::mem::zeroed();

        ibuf_from_buffer(&raw mut rbuf, (*msgbuf).rbuf.cast(), (*msgbuf).roff);

        // C is a do/while over the read buffer; `'process` is the goto-fail
        // target and yields false on a framing error, true on success.
        let ok = 'process: {
            loop {
                if (*msgbuf).rpmsg.is_null() {
                    if ibuf_size(&raw const rbuf) < (*msgbuf).hdrsize {
                        break; // not enough for a header yet -> success tail
                    }
                    // get size from header
                    ibuf_from_buffer(&raw mut msg, ibuf_data(&raw const rbuf), (*msgbuf).hdrsize);
                    (*msgbuf).rpmsg =
                        ((*msgbuf).readhdr.unwrap())(&raw mut msg, (*msgbuf).rarg, &raw mut fd);
                    if (*msgbuf).rpmsg.is_null() {
                        break 'process false; // goto fail
                    }
                }

                let sz = if ibuf_left((*msgbuf).rpmsg) <= ibuf_size(&raw const rbuf) {
                    ibuf_left((*msgbuf).rpmsg)
                } else {
                    ibuf_size(&raw const rbuf)
                };

                // neither call below can fail in practice
                if ibuf_get_ibuf(&raw mut rbuf, sz, &raw mut msg) == -1
                    || ibuf_add_ibuf((*msgbuf).rpmsg, &raw const msg) == -1
                {
                    break 'process false; // goto fail
                }

                if ibuf_left((*msgbuf).rpmsg) == 0 {
                    ibufq_push(&raw mut (*msgbuf).rbufs, (*msgbuf).rpmsg);
                    (*msgbuf).rpmsg = null_mut();
                }

                if ibuf_size(&raw const rbuf) == 0 {
                    break; // do/while terminating condition -> success tail
                }
            }
            true
        };

        if ok {
            if ibuf_size(&raw const rbuf) > 0 {
                memmove(
                    (*msgbuf).rbuf.cast(),
                    ibuf_data(&raw const rbuf),
                    ibuf_size(&raw const rbuf),
                );
            }
            (*msgbuf).roff = ibuf_size(&raw const rbuf);
        }

        if fd != -1 {
            close(fd);
        }
        if ok { 1 } else { -1 }
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:877`: `int ibuf_read(int fd, struct msgbuf *msgbuf)`
pub unsafe fn ibuf_read(fd: c_int, msgbuf: *mut msgbuf) -> c_int {
    unsafe {
        if (*msgbuf).rbuf.is_null() {
            errno!() = EINVAL;
            return -1;
        }

        let mut iov: iovec = std::mem::zeroed();
        iov.iov_base = (*msgbuf).rbuf.add((*msgbuf).roff).cast();
        iov.iov_len = IBUF_READ_SIZE - (*msgbuf).roff;

        let mut n: isize;
        'again: loop {
            n = readv(fd, &raw const iov, 1);
            if n == -1 {
                if errno!() == EINTR {
                    continue 'again;
                }
                if errno!() == EAGAIN {
                    return 1; // lets retry later again
                }
                return -1;
            }
            break 'again;
        }
        if n == 0 {
            return 0; // connection closed
        }

        (*msgbuf).roff += n as usize;
        ibuf_read_process(msgbuf, -1)
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:908`: `int msgbuf_read(int fd, struct msgbuf *msgbuf)`
pub unsafe fn msgbuf_read(fd: c_int, msgbuf: *mut msgbuf) -> c_int {
    unsafe {
        let mut msg: msghdr = std::mem::zeroed();
        let mut cmsgbuf: cmsgbuf = std::mem::zeroed();
        union cmsgbuf {
            _hdr: cmsghdr,
            buf: [u8; unsafe { CMSG_SPACE(size_of::<c_int>() as _) as usize }],
        }
        let mut iov: iovec = std::mem::zeroed();
        let mut fdpass: c_int = -1;

        if (*msgbuf).rbuf.is_null() {
            errno!() = EINVAL;
            return -1;
        }

        iov.iov_base = (*msgbuf).rbuf.add((*msgbuf).roff).cast();
        iov.iov_len = IBUF_READ_SIZE - (*msgbuf).roff;
        msg.msg_iov = &raw mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = &raw mut cmsgbuf.buf as _;
        msg.msg_controllen = size_of_val(&cmsgbuf.buf) as _;

        let mut n: isize;
        'again: loop {
            n = recvmsg(fd, &raw mut msg, 0);
            if n == -1 {
                if errno!() == EINTR {
                    continue 'again;
                }
                if errno!() == EMSGSIZE {
                    // Not enough fd slots: retry to receive without the fd.
                    continue 'again;
                }
                if errno!() == EAGAIN {
                    return 1; // lets retry later again
                }
                return -1;
            }
            break 'again;
        }
        if n == 0 {
            return 0; // connection closed
        }

        (*msgbuf).roff += n as usize;

        let mut cmsg: *mut cmsghdr = CMSG_FIRSTHDR(&raw const msg);
        while !cmsg.is_null() {
            if (*cmsg).cmsg_level == SOL_SOCKET && (*cmsg).cmsg_type == SCM_RIGHTS {
                // We only accept one fd; padding may leave more, which we close.
                let j = ((cmsg as *mut u8).add((*cmsg).cmsg_len as usize).addr()
                    - CMSG_DATA(cmsg).addr())
                    / size_of::<c_int>();
                for k in 0..j {
                    let f = *(CMSG_DATA(cmsg) as *mut c_int).add(k);
                    if k == 0 {
                        fdpass = f;
                    } else {
                        close(f);
                    }
                }
            }
            cmsg = CMSG_NXTHDR(&raw const msg, cmsg);
        }

        ibuf_read_process(msgbuf, fdpass)
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:985`: `static void msgbuf_drain(struct msgbuf *msgbuf, size_t n)`
unsafe fn msgbuf_drain(msgbuf: *mut msgbuf, mut n: usize) {
    unsafe {
        let mut buf;
        while {
            buf = tailq_first(&raw mut (*msgbuf).bufs.bufs);
            !buf.is_null()
        } {
            if n >= ibuf_size(buf) {
                n -= ibuf_size(buf);
                tailq_remove(&raw mut (*msgbuf).bufs.bufs, buf);
                (*msgbuf).bufs.queued -= 1;
                ibuf_free(buf);
            } else {
                (*buf).rpos += n;
                return;
            }
        }
    }
}

// --- ibufqueue: a queue of ibufs plus its length (imsg-buffer.c:1003) ---

/// C `vendor/tmux/compat/imsg-buffer.c:1003`: `static void ibufq_init(struct ibufqueue *bufq)`
unsafe fn ibufq_init(bufq: *mut ibufqueue) {
    unsafe {
        tailq_init(&raw mut (*bufq).bufs);
        (*bufq).queued = 0;
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:1010`: `struct ibufqueue *ibufq_new(void)`
pub unsafe fn ibufq_new() -> *mut ibufqueue {
    unsafe {
        let bufq = calloc(1, size_of::<ibufqueue>()) as *mut ibufqueue;
        if bufq.is_null() {
            return null_mut();
        }
        ibufq_init(bufq);
        bufq
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:1021`: `void ibufq_free(struct ibufqueue *bufq)`
pub unsafe fn ibufq_free(bufq: *mut ibufqueue) {
    unsafe {
        if bufq.is_null() {
            return;
        }
        ibufq_flush(bufq);
        free(bufq as *mut c_void);
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:1030`: `struct ibuf *ibufq_pop(struct ibufqueue *bufq)`
pub unsafe fn ibufq_pop(bufq: *mut ibufqueue) -> *mut ibuf {
    unsafe {
        let buf = tailq_first(&raw mut (*bufq).bufs);
        if buf.is_null() {
            return null_mut();
        }
        tailq_remove(&raw mut (*bufq).bufs, buf);
        (*bufq).queued -= 1;
        buf
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:1042`: `void ibufq_push(struct ibufqueue *bufq, struct ibuf *buf)`
pub unsafe fn ibufq_push(bufq: *mut ibufqueue, buf: *mut ibuf) {
    unsafe {
        // if buf lives on the stack abort before causing more harm
        if (*buf).fd == IBUF_FD_MARK_ON_STACK {
            abort();
        }
        tailq_insert_tail::<_, _>(&raw mut (*bufq).bufs, buf);
        (*bufq).queued += 1;
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:1052`: `uint32_t ibufq_queuelen(struct ibufqueue *bufq)`
pub unsafe fn ibufq_queuelen(bufq: *mut ibufqueue) -> u32 {
    unsafe { (*bufq).queued }
}

/// C `vendor/tmux/compat/imsg-buffer.c:1058`: `void ibufq_concat(struct ibufqueue *to, struct ibufqueue *from)`
pub unsafe fn ibufq_concat(to: *mut ibufqueue, from: *mut ibufqueue) {
    unsafe {
        (*to).queued += (*from).queued;
        // TAILQ_CONCAT: move every element of `from` onto the tail of `to`.
        let mut buf;
        while {
            buf = tailq_first(&raw mut (*from).bufs);
            !buf.is_null()
        } {
            tailq_remove(&raw mut (*from).bufs, buf);
            tailq_insert_tail::<_, _>(&raw mut (*to).bufs, buf);
        }
        (*from).queued = 0;
    }
}

/// C `vendor/tmux/compat/imsg-buffer.c:1066`: `void ibufq_flush(struct ibufqueue *bufq)`
pub unsafe fn ibufq_flush(bufq: *mut ibufqueue) {
    unsafe {
        let mut buf;
        while {
            buf = tailq_first(&raw mut (*bufq).bufs);
            !buf.is_null()
        } {
            tailq_remove(&raw mut (*bufq).bufs, buf);
            ibuf_free(buf);
        }
        (*bufq).queued = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Read the readable region [rpos, wpos) of an ibuf as a byte slice.
    // Mirrors ibuf_data()/ibuf_size() from vendor/tmux/compat/imsg-buffer.c:403,409.
    unsafe fn readable(buf: *const ibuf) -> Vec<u8> {
        unsafe {
            let p = ibuf_data(buf) as *const u8;
            let n = ibuf_size(buf);
            std::slice::from_raw_parts(p, n).to_vec()
        }
    }

    // ibufq_push then ibufq_pop returns buffers FIFO and tracks queuelen; a
    // stack buffer (fd == IBUF_FD_MARK_ON_STACK) must never be enqueued.
    #[test]
    fn test_ibufq_push_pop_fifo() {
        unsafe {
            let mut q: ibufqueue = std::mem::zeroed();
            ibufq_init(&raw mut q);
            assert_eq!(ibufq_queuelen(&raw mut q), 0);

            let a = ibuf_dynamic(4, 64);
            let b = ibuf_dynamic(4, 64);
            ibufq_push(&raw mut q, a);
            ibufq_push(&raw mut q, b);
            assert_eq!(ibufq_queuelen(&raw mut q), 2);

            assert_eq!(ibufq_pop(&raw mut q), a);
            assert_eq!(ibufq_pop(&raw mut q), b);
            assert_eq!(ibufq_queuelen(&raw mut q), 0);
            assert!(ibufq_pop(&raw mut q).is_null());

            ibuf_free(a);
            ibuf_free(b);
        }
    }

    // C `ibuf_open` (imsg-buffer.c:69) allocates a fixed buffer with
    // size == max == len and fd == -1. NOTE: the Rust port additionally
    // rejects len == 0 with EINVAL (upstream C allows a zero-length open),
    // so we assert the port's actual behavior here.
    #[test]
    fn test_ibuf_open_basic_and_zero() {
        unsafe {
            let buf = ibuf_open(16);
            assert!(!buf.is_null());
            // freshly opened: nothing written yet, so size() == 0.
            assert_eq!(ibuf_size(buf), 0);
            assert_eq!((*buf).max, 16);
            assert_eq!((*buf).size, 16);
            assert_eq!((*buf).wpos, 0);
            assert_eq!((*buf).rpos, 0);
            assert_eq!((*buf).fd, -1);
            // ibuf_left == max - wpos (imsg-buffer.c:415).
            assert_eq!(ibuf_left(buf), 16);
            ibuf_free(buf);

            // Port-specific: len == 0 -> NULL with EINVAL.
            errno!() = 0;
            let z = ibuf_open(0);
            assert!(z.is_null());
            assert_eq!(errno!(), EINVAL);
        }
    }

    // C `ibuf_dynamic` (imsg-buffer.c:100): rejects max == 0 or max < len; sets
    // size = len and max = max.
    #[test]
    fn test_ibuf_dynamic_errors_and_alloc() {
        unsafe {
            // max == 0 -> EINVAL (matches C).
            errno!() = 0;
            let a = ibuf_dynamic(0, 0);
            assert!(a.is_null());
            assert_eq!(errno!(), EINVAL);

            // max < len -> EINVAL (shared with C).
            errno!() = 0;
            let b = ibuf_dynamic(8, 4);
            assert!(b.is_null());
            assert_eq!(errno!(), EINVAL);

            // len == 0 with a positive max is a valid empty growable buffer.
            let empty = ibuf_dynamic(0, 16);
            assert!(!empty.is_null());
            assert_eq!((*empty).size, 0);
            assert_eq!((*empty).max, 16);
            ibuf_free(empty);

            // Valid allocation: nothing written yet.
            let buf = ibuf_dynamic(4, 16);
            assert!(!buf.is_null());
            assert_eq!(ibuf_size(buf), 0);
            assert_eq!((*buf).size, 4);
            assert_eq!((*buf).max, 16);
            assert_eq!((*buf).fd, -1);
            // Adding up to the initial size succeeds.
            let src = [0xAAu8, 0xBB, 0xCC, 0xDD];
            assert_eq!(ibuf_add(buf, src.as_ptr() as *const c_void, 4), 0);
            assert_eq!(ibuf_size(buf), 4);
            assert_eq!(readable(buf), src);
            ibuf_free(buf);
        }
    }

    // C `ibuf_add` (imsg-buffer.c:149) + ibuf_data/ibuf_size round trip.
    #[test]
    fn test_ibuf_add_bytes_and_data() {
        unsafe {
            let buf = ibuf_open(16);
            let first = [1u8, 2, 3, 4];
            let second = [5u8, 6];
            assert_eq!(ibuf_add(buf, first.as_ptr() as *const c_void, 4), 0);
            assert_eq!(ibuf_size(buf), 4);
            assert_eq!(ibuf_add(buf, second.as_ptr() as *const c_void, 2), 0);
            assert_eq!(ibuf_size(buf), 6);
            assert_eq!(readable(buf), [1, 2, 3, 4, 5, 6]);
            // wpos advanced by total bytes written.
            assert_eq!((*buf).wpos, 6);
            // Adding zero bytes is a no-op that still succeeds.
            assert_eq!(ibuf_add(buf, first.as_ptr() as *const c_void, 0), 0);
            assert_eq!(ibuf_size(buf), 6);
            ibuf_free(buf);
        }
    }

    // C `ibuf_reserve` (imsg-buffer.c:113): returns a writable pointer and
    // advances wpos; refuses to grow past max with ERANGE.
    #[test]
    fn test_ibuf_reserve_and_backfill() {
        unsafe {
            let buf = ibuf_open(8);
            // Reserve 4 bytes at the front (a length prefix placeholder).
            let hdr = ibuf_reserve(buf, 4);
            assert!(!hdr.is_null());
            assert_eq!(ibuf_size(buf), 4);
            assert_eq!((*buf).wpos, 4);
            // Append a 2-byte body after the placeholder.
            let body = [0x41u8, 0x42];
            assert_eq!(ibuf_add(buf, body.as_ptr() as *const c_void, 2), 0);
            assert_eq!(ibuf_size(buf), 6);
            // Backfill the reserved region directly through the pointer.
            std::ptr::copy_nonoverlapping([0xDEu8, 0xAD, 0xBE, 0xEF].as_ptr(), hdr as *mut u8, 4);
            assert_eq!(readable(buf), [0xDE, 0xAD, 0xBE, 0xEF, 0x41, 0x42]);
            ibuf_free(buf);
        }
    }

    // C `ibuf_reserve` growth path: for a fixed ibuf (size == max), a reserve
    // that exceeds max fails with ERANGE and returns NULL.
    #[test]
    fn test_ibuf_reserve_over_max() {
        unsafe {
            let buf = ibuf_open(4);
            errno!() = 0;
            let p = ibuf_reserve(buf, 8);
            assert!(p.is_null());
            assert_eq!(errno!(), ERANGE);
            // wpos untouched on failure.
            assert_eq!((*buf).wpos, 0);
            ibuf_free(buf);
        }
    }

    // C fixed-size add helpers (imsg-buffer.c:170-246). The n* variants store
    // big-endian, the h* variants store host order. We round-trip through the
    // matching get helpers so the assertions are endianness-independent.
    #[test]
    fn test_ibuf_add_get_n_and_h_round_trip() {
        unsafe {
            let buf = ibuf_open(64);
            assert_eq!(ibuf_add_n8(buf, 0x12), 0);
            assert_eq!(ibuf_add_n16(buf, 0x1234), 0);
            assert_eq!(ibuf_add_n32(buf, 0x1234_5678), 0);
            assert_eq!(ibuf_add_n64(buf, 0x1234_5678_9abc_def0), 0);
            assert_eq!(ibuf_add_h16(buf, 0xBEEF), 0);
            assert_eq!(ibuf_add_h32(buf, 0xDEAD_BEEF), 0);
            assert_eq!(ibuf_add_h64(buf, 0x0011_2233_4455_6677), 0);
            assert_eq!(ibuf_size(buf), 1 + 2 + 4 + 8 + 2 + 4 + 8);

            let mut v8: u8 = 0;
            let mut v16: u16 = 0;
            let mut v32: u32 = 0;
            let mut v64: u64 = 0;
            assert_eq!(ibuf_get_n8(buf, &mut v8), 0);
            assert_eq!(v8, 0x12);
            assert_eq!(ibuf_get_n16(buf, &mut v16), 0);
            assert_eq!(v16, 0x1234);
            assert_eq!(ibuf_get_n32(buf, &mut v32), 0);
            assert_eq!(v32, 0x1234_5678);
            assert_eq!(ibuf_get_n64(buf, &mut v64), 0);
            assert_eq!(v64, 0x1234_5678_9abc_def0);
            assert_eq!(ibuf_get_h16(buf, &mut v16), 0);
            assert_eq!(v16, 0xBEEF);
            assert_eq!(ibuf_get_h32(buf, &mut v32), 0);
            assert_eq!(v32, 0xDEAD_BEEF);
            assert_eq!(ibuf_get_h64(buf, &mut v64), 0);
            assert_eq!(v64, 0x0011_2233_4455_6677);

            // Everything consumed: size back to 0.
            assert_eq!(ibuf_size(buf), 0);
            ibuf_free(buf);
        }
    }

    // n16 stores big-endian, so the raw bytes are [hi, lo] regardless of host.
    #[test]
    fn test_ibuf_add_n16_is_big_endian() {
        unsafe {
            let buf = ibuf_open(2);
            assert_eq!(ibuf_add_n16(buf, 0x0102), 0);
            assert_eq!(readable(buf), [0x01, 0x02]);
            ibuf_free(buf);
        }
    }

    // Overflow guards in the add helpers (imsg-buffer.c:175,188,201).
    #[test]
    fn test_ibuf_add_n_overflow() {
        unsafe {
            let buf = ibuf_open(16);
            errno!() = 0;
            assert_eq!(ibuf_add_n8(buf, 0x100), -1);
            assert_eq!(errno!(), EINVAL);
            errno!() = 0;
            assert_eq!(ibuf_add_n16(buf, 0x1_0000), -1);
            assert_eq!(errno!(), EINVAL);
            errno!() = 0;
            assert_eq!(ibuf_add_n32(buf, 0x1_0000_0000), -1);
            assert_eq!(errno!(), EINVAL);
            // No bytes were written by the failed calls.
            assert_eq!(ibuf_size(buf), 0);
            ibuf_free(buf);
        }
    }

    // C `ibuf_add_zero` (imsg-buffer.c:248): reserves and zeroes.
    #[test]
    fn test_ibuf_add_zero() {
        unsafe {
            let buf = ibuf_open(8);
            let pre = [0xFFu8, 0xFF];
            assert_eq!(ibuf_add(buf, pre.as_ptr() as *const c_void, 2), 0);
            assert_eq!(ibuf_add_zero(buf, 3), 0);
            assert_eq!(ibuf_size(buf), 5);
            assert_eq!(readable(buf), [0xFF, 0xFF, 0x00, 0x00, 0x00]);
            ibuf_free(buf);
        }
    }

    // C `ibuf_seek` (imsg-buffer.c:281): only allows [rpos, wpos); ERANGE
    // otherwise. Used together with ibuf_set for backfilling.
    #[test]
    fn test_ibuf_seek_and_set() {
        unsafe {
            let buf = ibuf_open(16);
            let src = [0u8, 1, 2, 3, 4, 5, 6, 7];
            assert_eq!(ibuf_add(buf, src.as_ptr() as *const c_void, 8), 0);

            // Seek into the readable region and read via the returned pointer.
            let p = ibuf_seek(buf, 2, 4);
            assert!(!p.is_null());
            let seen = std::slice::from_raw_parts(p as *const u8, 4);
            assert_eq!(seen, [2, 3, 4, 5]);

            // ibuf_set overwrites at an offset without changing size.
            let patch = [0xAAu8, 0xBB];
            assert_eq!(ibuf_set(buf, 4, patch.as_ptr() as *const c_void, 2), 0);
            assert_eq!(ibuf_size(buf), 8);
            assert_eq!(readable(buf), [0, 1, 2, 3, 0xAA, 0xBB, 6, 7]);

            // Out-of-range seeks: pos + len past wpos -> ERANGE/NULL.
            errno!() = 0;
            assert!(ibuf_seek(buf, 6, 4).is_null());
            assert_eq!(errno!(), ERANGE);
            errno!() = 0;
            assert!(ibuf_seek(buf, 9, 0).is_null());
            assert_eq!(errno!(), ERANGE);
            // ibuf_set fails the same way.
            errno!() = 0;
            assert_eq!(ibuf_set(buf, 7, patch.as_ptr() as *const c_void, 2), -1);
            assert_eq!(errno!(), ERANGE);
            ibuf_free(buf);
        }
    }

    // C `ibuf_set_n32` (imsg-buffer.c:335) writes a big-endian value at pos.
    // Classic length-prefix backfill: reserve space, then set the length.
    #[test]
    fn test_ibuf_set_n32_backfill() {
        unsafe {
            let buf = ibuf_open(8);
            // 4-byte length placeholder + 4-byte body.
            assert_eq!(ibuf_add_zero(buf, 4), 0);
            let body = [0xDEu8, 0xAD, 0xBE, 0xEF];
            assert_eq!(ibuf_add(buf, body.as_ptr() as *const c_void, 4), 0);
            // Backfill the length prefix in host-independent big-endian form.
            assert_eq!(ibuf_set_n32(buf, 0, 4), 0);
            assert_eq!(readable(buf), [0x00, 0x00, 0x00, 0x04, 0xDE, 0xAD, 0xBE, 0xEF]);

            // Read the length back big-endian.
            let mut len: u32 = 0;
            assert_eq!(ibuf_get_n32(buf, &mut len), 0);
            assert_eq!(len, 4);
            ibuf_free(buf);
        }
    }

    // ibuf_set_n8 overflow guard (imsg-buffer.c:313).
    #[test]
    fn test_ibuf_set_n8_overflow() {
        unsafe {
            let buf = ibuf_open(4);
            assert_eq!(ibuf_add_zero(buf, 1), 0);
            errno!() = 0;
            assert_eq!(ibuf_set_n8(buf, 0, 0x100), -1);
            assert_eq!(errno!(), EINVAL);
            ibuf_free(buf);
        }
    }

    // C `ibuf_data`/`ibuf_size` track rpos: consuming via ibuf_get advances
    // rpos so the visible data window slides forward (imsg-buffer.c:403-411,465).
    #[test]
    fn test_ibuf_data_moves_with_rpos() {
        unsafe {
            let buf = ibuf_open(8);
            let src = [10u8, 20, 30, 40];
            assert_eq!(ibuf_add(buf, src.as_ptr() as *const c_void, 4), 0);
            let base = ibuf_data(buf) as usize;

            let mut two = [0u8; 2];
            assert_eq!(ibuf_get(buf, two.as_mut_ptr() as *mut c_void, 2), 0);
            assert_eq!(two, [10, 20]);
            assert_eq!(ibuf_size(buf), 2);
            assert_eq!(ibuf_data(buf) as usize, base + 2);
            assert_eq!(readable(buf), [30, 40]);

            // Reading past the end fails with EBADMSG and does not advance.
            errno!() = 0;
            let mut big = [0u8; 8];
            assert_eq!(ibuf_get(buf, big.as_mut_ptr() as *mut c_void, 8), -1);
            assert_eq!(errno!(), EBADMSG);
            assert_eq!(ibuf_size(buf), 2);
            ibuf_free(buf);
        }
    }

    // C `ibuf_truncate` (imsg-buffer.c:423): truncating below the current size
    // just moves wpos down; truncating above pads with zeros up to `len` for a
    // heap buffer that still has room under `max`.
    #[test]
    fn test_ibuf_truncate_down_then_grow_zero() {
        unsafe {
            let buf = ibuf_open(16);
            let src = [0u8, 1, 2, 3, 4, 5, 6, 7];
            assert_eq!(ibuf_add(buf, src.as_ptr() as *const c_void, 8), 0);
            assert_eq!(ibuf_truncate(buf, 4), 0);
            assert_eq!(ibuf_size(buf), 4);
            assert_eq!(readable(buf), [0, 1, 2, 3]);
            // Grow back with zero padding.
            assert_eq!(ibuf_truncate(buf, 6), 0);
            assert_eq!(ibuf_size(buf), 6);
            assert_eq!(readable(buf), [0, 1, 2, 3, 0, 0]);
            ibuf_free(buf);
        }
    }

    // A stack buffer (ibuf_from_buffer, max == 0) can only be truncated down;
    // trying to grow it fails with ERANGE (imsg-buffer.c:429-434).
    #[test]
    fn test_ibuf_truncate_stack_cannot_grow() {
        unsafe {
            let mut backing = [9u8, 8, 7, 6, 5];
            let mut sb: ibuf = std::mem::zeroed();
            ibuf_from_buffer(&raw mut sb, backing.as_mut_ptr() as *mut c_void, 5);
            assert_eq!(sb.max, 0); // stack marker in the port
            assert_eq!(ibuf_size(&raw const sb), 5);
            assert_eq!(ibuf_truncate(&raw mut sb, 3), 0);
            assert_eq!(ibuf_size(&raw const sb), 3);
            errno!() = 0;
            assert_eq!(ibuf_truncate(&raw mut sb, 10), -1);
            assert_eq!(errno!(), ERANGE);
            // Never ibuf_free a stack buffer (max == 0 aborts); backing is ours.
        }
    }

    // C `ibuf_skip` (imsg-buffer.c:466): advances rpos, sliding the window;
    // skipping more than is readable errors with EBADMSG and does not advance.
    #[test]
    fn test_ibuf_skip_and_underflow() {
        unsafe {
            let buf = ibuf_open(16);
            let src = [10u8, 20, 30, 40, 50, 60];
            assert_eq!(ibuf_add(buf, src.as_ptr() as *const c_void, 6), 0);
            assert_eq!(ibuf_skip(buf, 2), 0);
            assert_eq!(ibuf_size(buf), 4);
            assert_eq!(readable(buf), [30, 40, 50, 60]);
            errno!() = 0;
            assert_eq!(ibuf_skip(buf, 5), -1);
            assert_eq!(errno!(), EBADMSG);
            assert_eq!(ibuf_size(buf), 4);
            ibuf_free(buf);
        }
    }

    // C `ibuf_get_ibuf` (imsg-buffer.c:479): carves a `len`-byte sub-view off
    // the front and advances the source rpos; asking for more than available
    // errors with EBADMSG.
    #[test]
    fn test_ibuf_get_ibuf_view_and_error() {
        unsafe {
            let buf = ibuf_open(16);
            let src = [1u8, 2, 3, 4, 5, 6];
            assert_eq!(ibuf_add(buf, src.as_ptr() as *const c_void, 6), 0);

            let mut view: ibuf = std::mem::zeroed();
            assert_eq!(ibuf_get_ibuf(buf, 4, &raw mut view), 0);
            assert_eq!(ibuf_size(&raw const view), 4);
            assert_eq!(readable(&raw const view), [1, 2, 3, 4]);
            // Source advanced by 4.
            assert_eq!(ibuf_size(buf), 2);
            assert_eq!(readable(buf), [5, 6]);

            errno!() = 0;
            let mut v2: ibuf = std::mem::zeroed();
            assert_eq!(ibuf_get_ibuf(buf, 3, &raw mut v2), -1);
            assert_eq!(errno!(), EBADMSG);
            // view is a stack ibuf; do not free it.
            ibuf_free(buf);
        }
    }

    // C `ibuf_add_ibuf` (imsg-buffer.c:165) appends another ibuf's readable
    // region; ibuf_add_buf is an alias for the same operation.
    #[test]
    fn test_ibuf_add_ibuf_concatenates() {
        unsafe {
            let dst = ibuf_open(16);
            let src = ibuf_open(8);
            assert_eq!(ibuf_add(dst, [0xAAu8, 0xBB].as_ptr() as *const c_void, 2), 0);
            assert_eq!(ibuf_add(src, [0x01u8, 0x02, 0x03].as_ptr() as *const c_void, 3), 0);
            assert_eq!(ibuf_add_ibuf(dst, src), 0);
            assert_eq!(readable(dst), [0xAA, 0xBB, 0x01, 0x02, 0x03]);
            // add_buf appends the same source region again (src unchanged).
            assert_eq!(ibuf_add_buf(dst, src), 0);
            assert_eq!(ibuf_size(dst), 8);
            assert_eq!(readable(dst), [0xAA, 0xBB, 0x01, 0x02, 0x03, 0x01, 0x02, 0x03]);
            ibuf_free(src);
            ibuf_free(dst);
        }
    }

    // C `ibuf_left` (imsg-buffer.c:415) == max - wpos; `ibuf_rewind`
    // (imsg-buffer.c:439) resets rpos to 0 without touching wpos.
    #[test]
    fn test_ibuf_left_and_rewind() {
        unsafe {
            let buf = ibuf_open(16);
            let src = [1u8, 2, 3, 4, 5];
            assert_eq!(ibuf_add(buf, src.as_ptr() as *const c_void, 5), 0);
            assert_eq!(ibuf_left(buf), 11);
            let mut two = [0u8; 2];
            assert_eq!(ibuf_get(buf, two.as_mut_ptr() as *mut c_void, 2), 0);
            assert_eq!(ibuf_size(buf), 3);
            ibuf_rewind(buf);
            assert_eq!(ibuf_size(buf), 5);
            assert_eq!(readable(buf), [1, 2, 3, 4, 5]);
            // rewind does not change wpos, so left is unchanged.
            assert_eq!(ibuf_left(buf), 11);
            ibuf_free(buf);
        }
    }

    // C fd helpers (imsg-buffer.c:610/616/629): fd_avail reflects presence,
    // fd_get returns the fd once and resets it to -1.
    #[test]
    fn test_ibuf_fd_set_get_avail() {
        unsafe {
            let buf = ibuf_dynamic(4, 4);
            assert!(!buf.is_null());
            assert_eq!(ibuf_fd_avail(buf), 0);
            let fd = libc::dup(2);
            assert!(fd >= 0);
            ibuf_fd_set(buf, fd);
            assert_eq!(ibuf_fd_avail(buf), 1);
            let got = ibuf_fd_get(buf);
            assert_eq!(got, fd);
            assert_eq!(ibuf_fd_avail(buf), 0);
            // We took ownership of the fd; close it and free the (now fd-less) buf.
            libc::close(fd);
            ibuf_free(buf);
        }
    }

    // C msgbuf enqueue path (imsg-buffer.c:445 ibuf_close -> ibuf_enqueue):
    // queued/queuelen track the FIFO, and msgbuf_clear drains it.
    #[test]
    fn test_msgbuf_enqueue_queuelen_clear() {
        unsafe {
            let mut mb: msgbuf = std::mem::zeroed();
            ibufq_init(&raw mut mb.bufs);
            ibufq_init(&raw mut mb.rbufs);
            assert_eq!(msgbuf_queuelen(&raw mut mb), 0);
            for k in 0..3u8 {
                let b = ibuf_dynamic(4, 4);
                assert!(!b.is_null());
                assert_eq!(ibuf_add(b, [k, k, k, k].as_ptr() as *const c_void, 4), 0);
                ibuf_close(&raw mut mb, b);
            }
            assert_eq!(mb.bufs.queued, 3);
            assert_eq!(msgbuf_queuelen(&raw mut mb), 3);
            // FIFO: first enqueued sits at the front.
            let first = tailq_first(&raw mut mb.bufs.bufs);
            assert!(!first.is_null());
            assert_eq!(readable(first), [0, 0, 0, 0]);
            msgbuf_clear(&raw mut mb);
            assert_eq!(mb.bufs.queued, 0);
            assert!(tailq_first(&raw mut mb.bufs.bufs).is_null());
        }
    }

    // C `ibuf_set_n16`/`ibuf_set_n64` (imsg-buffer.c:322/348) write big-endian
    // at an offset; the raw bytes and the n-getters agree.
    #[test]
    fn test_ibuf_set_n16_n64_backfill() {
        unsafe {
            let buf = ibuf_open(16);
            assert_eq!(ibuf_add_zero(buf, 10), 0);
            assert_eq!(ibuf_set_n16(buf, 0, 0x0102), 0);
            assert_eq!(ibuf_set_n64(buf, 2, 0x1122_3344_5566_7788), 0);
            let r = readable(buf);
            assert_eq!(&r[0..2], &[0x01, 0x02]);
            assert_eq!(
                &r[2..10],
                &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]
            );
            let mut v16 = 0u16;
            assert_eq!(ibuf_get_n16(buf, &mut v16), 0);
            assert_eq!(v16, 0x0102);
            let mut v64 = 0u64;
            assert_eq!(ibuf_get_n64(buf, &mut v64), 0);
            assert_eq!(v64, 0x1122_3344_5566_7788);
            ibuf_free(buf);
        }
    }

    // C `ibuf_set_h32` (imsg-buffer.c:368) writes host order; raw bytes are the
    // native representation and get_h32 round-trips.
    #[test]
    fn test_ibuf_set_h32_round_trip() {
        unsafe {
            let buf = ibuf_open(8);
            assert_eq!(ibuf_add_zero(buf, 4), 0);
            assert_eq!(ibuf_set_h32(buf, 0, 0xDEAD_BEEF), 0);
            let r = readable(buf);
            assert_eq!(r, 0xDEAD_BEEFu32.to_ne_bytes());
            let mut v = 0u32;
            assert_eq!(ibuf_get_h32(buf, &mut v), 0);
            assert_eq!(v, 0xDEAD_BEEF);
            ibuf_free(buf);
        }
    }

    // C `ibuf_dynamic` (imsg-buffer.c:100): max is the growth ceiling, so a
    // dynamic buffer grows from its initial `size` up to `max`, and only a write
    // that would exceed `max` fails with ERANGE.
    #[test]
    fn test_ibuf_dynamic_grows_up_to_max() {
        unsafe {
            let buf = ibuf_dynamic(4, 8);
            assert!(!buf.is_null());
            assert_eq!((*buf).max, 8);
            assert_eq!((*buf).size, 4);
            // Fill the initial size, then grow into the headroom up to max.
            assert_eq!(ibuf_add(buf, [1u8, 2, 3, 4].as_ptr() as *const c_void, 4), 0);
            assert_eq!(ibuf_add(buf, [5u8, 6, 7, 8].as_ptr() as *const c_void, 4), 0);
            assert_eq!(ibuf_size(buf), 8);
            assert_eq!(readable(buf), [1, 2, 3, 4, 5, 6, 7, 8]);
            // A 9th byte would exceed max -> ERANGE, buffer unchanged.
            errno!() = 0;
            assert_eq!(ibuf_add(buf, [9u8].as_ptr() as *const c_void, 1), -1);
            assert_eq!(errno!(), ERANGE);
            assert_eq!(ibuf_size(buf), 8);
            ibuf_free(buf);
        }
    }

    // C `ibuf_from_ibuf` (imsg-buffer.c:460) makes a stack view over another
    // ibuf's current readable window (respecting the source rpos).
    #[test]
    fn test_ibuf_from_ibuf_view() {
        unsafe {
            let src = ibuf_open(16);
            assert_eq!(ibuf_add(src, [7u8, 8, 9, 10].as_ptr() as *const c_void, 4), 0);
            let mut one = [0u8; 1];
            assert_eq!(ibuf_get(src, one.as_mut_ptr() as *mut c_void, 1), 0); // rpos -> 1
            let mut view: ibuf = std::mem::zeroed();
            ibuf_from_ibuf(&raw mut view, src);
            assert_eq!(ibuf_size(&raw const view), 3);
            assert_eq!(readable(&raw const view), [8, 9, 10]);
            assert_eq!(view.max, 0); // stack marker
            ibuf_free(src); // view is a stack buffer; do not free it
        }
    }

    // C `ibuf_from_buffer` (imsg-buffer.c:451) wraps external memory as a stack
    // buffer: max == 0, fd == IBUF_FD_MARK_ON_STACK (-2), wpos == size == len,
    // rpos == 0, no space left.
    #[test]
    fn test_ibuf_from_buffer_stack_props() {
        unsafe {
            let mut backing = [0x11u8, 0x22, 0x33];
            let mut b: ibuf = std::mem::zeroed();
            ibuf_from_buffer(&raw mut b, backing.as_mut_ptr() as *mut c_void, 3);
            assert_eq!(b.max, 0);
            assert_eq!(b.fd, IBUF_FD_MARK_ON_STACK);
            assert_eq!(b.wpos, 3);
            assert_eq!(b.size, 3);
            assert_eq!(b.rpos, 0);
            assert_eq!(ibuf_size(&raw const b), 3);
            assert_eq!(ibuf_left(&raw const b), 0);
            assert_eq!(readable(&raw const b), [0x11, 0x22, 0x33]);
        }
    }

    // C `ibuf_reserve` (imsg-buffer.c:113) can fill a fixed buffer exactly to
    // its size; one more byte then exceeds max and fails with ERANGE.
    #[test]
    fn test_ibuf_reserve_exact_to_size() {
        unsafe {
            let buf = ibuf_open(8);
            let p = ibuf_reserve(buf, 8);
            assert!(!p.is_null());
            assert_eq!(ibuf_size(buf), 8);
            assert_eq!((*buf).wpos, 8);
            // The whole reserved region is writable.
            std::ptr::write_bytes(p as *mut u8, 0xCD, 8);
            assert_eq!(readable(buf), [0xCD; 8]);
            errno!() = 0;
            assert!(ibuf_reserve(buf, 1).is_null());
            assert_eq!(errno!(), ERANGE);
            ibuf_free(buf);
        }
    }
}
