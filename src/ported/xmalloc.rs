// Author: Tatu Ylonen <ylo@cs.hut.fi>
// Copyright (c) 1995 Tatu Ylonen <ylo@cs.hut.fi>, Espoo, Finland
//                    All rights reserved
// Versions of malloc and friends that check their results, and never return
// failure (they call fatalx if they encounter an error).
//
// As far as I am concerned, the code I have written for this software
// can be used freely for any purpose.  Any derived versions of this
// software must be clearly marked as such, and if the derived work is
// incompatible with the protocol description in the RFC file, it must be
// called by a name other than "ssh" or "Secure Shell".
#![expect(clippy::panic)]
use std::{
    ffi::{CStr, c_void},
    mem::MaybeUninit,
    ptr::NonNull,
};

use crate::{
    compat::{reallocarray::reallocarray, recallocarray::recallocarray},
    fatalx,
};

/// C `vendor/tmux/xmalloc.c:27`: `void *xmalloc(size_t size)`
pub fn xmalloc(size: usize) -> NonNull<c_void> {
    debug_assert_ne!(size, 0, "xmalloc: zero size");

    // Allocate using max_align_t to have the same allignment as malloc.
    // We allocate a bit too much when size is not a multiple of max_align_t.
    let count = size.div_ceil(size_of::<libc::max_align_t>());
    let alloc = vec![MaybeUninit::<libc::max_align_t>::uninit(); count].into_boxed_slice();
    NonNull::new(Box::into_raw(alloc))
        .expect("box pointer is not null")
        .cast()
}

/// C `vendor/tmux/xmalloc.c:41`: `void *xcalloc(size_t nmemb, size_t size)`
pub fn xcalloc(nmemb: usize, size: usize) -> NonNull<c_void> {
    debug_assert!(size != 0 && nmemb != 0, "xcalloc: zero size");

    NonNull::new(unsafe { ::libc::calloc(nmemb, size) })
        .unwrap_or_else(|| panic!("xcalloc: allocating {nmemb} * {size}"))
}

pub fn xcalloc_<T>(nmemb: usize) -> NonNull<T> {
    xcalloc(nmemb, size_of::<T>()).cast()
}

pub unsafe fn xcalloc1<'a, T>() -> &'a mut T {
    let mut ptr: NonNull<T> = xcalloc(1, size_of::<T>()).cast();
    unsafe { ptr.as_mut() }
}

/// C `vendor/tmux/xmalloc.c:55`: `void *xrealloc(void *ptr, size_t size)`
pub unsafe fn xrealloc(ptr: *mut c_void, size: usize) -> NonNull<c_void> {
    unsafe { xrealloc_(ptr, size) }
}

pub unsafe fn xrealloc_<T>(ptr: *mut T, size: usize) -> NonNull<T> {
    unsafe { xreallocarray_old(ptr, 1, size) }
}

/// C `vendor/tmux/xmalloc.c:61`: `void *xreallocarray(void *ptr, size_t nmemb, size_t size)`
pub unsafe fn xreallocarray(ptr: *mut c_void, nmemb: usize, size: usize) -> NonNull<c_void> {
    unsafe { xreallocarray_old(ptr, nmemb, size) }
}

pub unsafe fn xreallocarray_old<T>(ptr: *mut T, nmemb: usize, size: usize) -> NonNull<T> {
    unsafe {
        if nmemb == 0 || size == 0 {
            fatalx("xreallocarray: zero size");
        }

        match NonNull::new(reallocarray(ptr as _, nmemb, size)) {
            None => fatalx("xreallocarray: allocating "),
            Some(new_ptr) => new_ptr.cast(),
        }
    }
}

pub unsafe fn xreallocarray_<T>(ptr: *mut T, nmemb: usize) -> NonNull<T> {
    let size = size_of::<T>();
    unsafe {
        if nmemb == 0 || size == 0 {
            fatalx("xreallocarray: zero size");
        }

        match NonNull::new(reallocarray(ptr as _, nmemb, size)) {
            None => fatalx("xreallocarray: allocating"),
            Some(new_ptr) => new_ptr.cast(),
        }
    }
}

/// C `vendor/tmux/xmalloc.c:75`: `void *xrecallocarray(void *ptr, size_t oldnmemb, size_t nmemb, size_t size)`
pub unsafe fn xrecallocarray(
    ptr: *mut c_void,
    oldnmemb: usize,
    nmemb: usize,
    size: usize,
) -> NonNull<c_void> {
    unsafe { xrecallocarray_(ptr, oldnmemb, nmemb, size) }
}

pub unsafe fn xrecallocarray_<T>(
    ptr: *mut T,
    oldnmemb: usize,
    nmemb: usize,
    size: usize,
) -> NonNull<T> {
    if nmemb == 0 || size == 0 {
        panic!("xrecallocarray: zero size");
    }

    NonNull::new(unsafe { recallocarray(ptr as *mut c_void, oldnmemb, nmemb, size) })
        .unwrap_or_else(|| panic!("xrecallocarray: allocating {nmemb} * {size}"))
        .cast()
}

pub unsafe fn xrecallocarray__<T>(ptr: *mut T, oldnmemb: usize, nmemb: usize) -> NonNull<T> {
    let size = size_of::<T>();
    if nmemb == 0 || size == 0 {
        panic!("xrecallocarray: zero size");
    }

    NonNull::new(unsafe { recallocarray(ptr as *mut c_void, oldnmemb, nmemb, size) })
        .unwrap_or_else(|| panic!("xrecallocarray: allocating {nmemb} * {size}"))
        .cast()
}

/// C `vendor/tmux/xmalloc.c:89`: `char *xstrdup(const char *str)`
pub unsafe fn xstrdup(str: *const u8) -> NonNull<u8> {
    NonNull::new(unsafe { crate::libc::strdup(str) }).unwrap()
}

pub fn xstrdup_(str: &CStr) -> NonNull<u8> {
    unsafe { xstrdup(str.as_ptr().cast()) }
}

pub fn xstrdup__(str: &str) -> *mut u8 {
    let mut out = str.to_string();
    out.push('\0');
    out.leak().as_mut_ptr()
}
pub fn xstrdup___(str: Option<&str>) -> *mut u8 {
    let Some(str) = str else {
        return std::ptr::null_mut();
    };
    xstrdup__(str)
}

/// C `vendor/tmux/xmalloc.c:99`: `char *xstrndup(const char *str, size_t maxlen)`
pub unsafe fn xstrndup(str: *const u8, maxlen: usize) -> NonNull<u8> {
    NonNull::new(unsafe { crate::libc::strndup(str, maxlen) }).unwrap()
}

macro_rules! format_nul {
   ($fmt:literal $(, $args:expr)* $(,)?) => {
        crate::xmalloc::format_nul_(format_args!($fmt $(, $args)*))
    };
}
pub(crate) use format_nul;
pub(crate) fn format_nul_(args: std::fmt::Arguments) -> *mut u8 {
    let mut s = args.to_string();
    s.push('\0');
    s.leak().as_mut_ptr()
}

macro_rules! xsnprintf_ {
   ($out:expr, $len:expr, $fmt:literal $(, $args:expr)* $(,)?) => {
        crate::xmalloc::xsnprintf__($out, $len, format_args!($fmt $(, $args)*))
    };
}
pub(crate) use xsnprintf_;
pub(crate) unsafe fn xsnprintf__(
    out: *mut u8,
    len: usize,
    args: std::fmt::Arguments,
) -> std::io::Result<usize> {
    use std::io::Write;

    struct WriteAdapter {
        buffer: *mut u8,
        length: usize,
        written: usize,
    }
    impl WriteAdapter {
        fn new(buffer: *mut u8, length: usize) -> Self {
            Self {
                buffer,
                length,
                written: 0,
            }
        }
    }

    impl std::io::Write for WriteAdapter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let remaining = self.length - self.written;
            let write_amount = buf.len().min(remaining);

            unsafe {
                std::ptr::copy_nonoverlapping(
                    buf.as_ptr(),
                    self.buffer.add(self.written).cast(),
                    write_amount,
                );
            }
            self.written += write_amount;

            Ok(write_amount)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut adapter = WriteAdapter::new(out, len);
    adapter.write_fmt(args)?;
    // C xsnprintf/snprintf return the length EXCLUDING the terminating NUL, and
    // callers (e.g. style_tostring's `off +=`, cmd_display_panes' width check)
    // advance by that. Capture it before writing the NUL, which the previous code
    // counted — the off-by-one wrote each subsequent field past an embedded NUL.
    let written = adapter.written;
    if adapter.write(&[0])? == 0 {
        return Err(std::io::ErrorKind::WriteZero.into());
    }

    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libc::{free_, strcmp, strlen};

    // Read a NUL-terminated C string produced by one of the x* helpers back
    // into a Rust byte slice (excluding the terminating NUL) for assertions.
    unsafe fn as_bytes<'a>(p: *const u8) -> &'a [u8] {
        unsafe { std::slice::from_raw_parts(p, strlen(p)) }
    }

    // C `xmalloc.c:89` xstrdup: strdup(str); the result is a distinct,
    // heap-allocated, byte-for-byte copy of the source string.
    #[test]
    fn test_xstrdup_round_trip() {
        unsafe {
            let src = crate::c!("hello world");
            let dup = xstrdup(src).as_ptr();

            // Distinct allocation from the source literal.
            assert_ne!(dup as *const u8, src);
            // Byte-for-byte identical content and length.
            assert_eq!(strcmp(dup, src), 0);
            assert_eq!(strlen(dup), 11);
            assert_eq!(as_bytes(dup), b"hello world");

            free_(dup);
        }
    }

    #[test]
    fn test_xstrdup_empty() {
        unsafe {
            let dup = xstrdup(crate::c!("")).as_ptr();
            assert_eq!(strlen(dup), 0);
            // The terminating NUL is present.
            assert_eq!(*dup, 0);
            free_(dup);
        }
    }

    // xstrdup_ is the &CStr wrapper around xstrdup.
    #[test]
    fn test_xstrdup_cstr_wrapper() {
        unsafe {
            let s = c"abc";
            let dup = xstrdup_(s).as_ptr();
            assert_eq!(as_bytes(dup), b"abc");
            assert_eq!(strcmp(dup, crate::c!("abc")), 0);
            free_(dup);
        }
    }

    // C `xmalloc.c:99` xstrndup: strndup(str, maxlen). When maxlen is smaller
    // than the source length, the copy is truncated to maxlen bytes and NUL
    // terminated.
    #[test]
    fn test_xstrndup_truncates() {
        unsafe {
            let dup = xstrndup(crate::c!("hello"), 3).as_ptr();
            assert_eq!(strlen(dup), 3);
            assert_eq!(as_bytes(dup), b"hel");
            assert_eq!(*dup.add(3), 0);
            free_(dup);
        }
    }

    // When maxlen >= source length, strndup copies the whole string.
    #[test]
    fn test_xstrndup_full_when_maxlen_large() {
        unsafe {
            let dup = xstrndup(crate::c!("hi"), 10).as_ptr();
            assert_eq!(strlen(dup), 2);
            assert_eq!(as_bytes(dup), b"hi");
            free_(dup);
        }
    }

    // maxlen == 0 yields an empty (but valid, NUL-terminated) string.
    #[test]
    fn test_xstrndup_zero_maxlen() {
        unsafe {
            let dup = xstrndup(crate::c!("hello"), 0).as_ptr();
            assert_eq!(strlen(dup), 0);
            assert_eq!(*dup, 0);
            free_(dup);
        }
    }

    // maxlen exactly equal to the length copies the whole string.
    #[test]
    fn test_xstrndup_exact_len() {
        unsafe {
            let dup = xstrndup(crate::c!("abcd"), 4).as_ptr();
            assert_eq!(as_bytes(dup), b"abcd");
            assert_eq!(strlen(dup), 4);
            free_(dup);
        }
    }

    // xstrdup__ turns a Rust &str into a NUL-terminated C string (Rust-owned,
    // leaked — do not free with libc free).
    #[test]
    fn test_xstrdup_str() {
        unsafe {
            let p = xstrdup__("port");
            assert_eq!(as_bytes(p), b"port");
            assert_eq!(*p.add(4), 0);
            assert_eq!(strcmp(p, crate::c!("port")), 0);
        }
    }

    // xstrdup___ maps None -> NULL and Some(s) -> a NUL-terminated copy.
    #[test]
    fn test_xstrdup_opt() {
        unsafe {
            assert!(xstrdup___(None).is_null());
            let p = xstrdup___(Some("x"));
            assert!(!p.is_null());
            assert_eq!(as_bytes(p), b"x");
        }
    }

    // C `xmalloc.c:41` xcalloc: calloc(nmemb, size) returns zeroed memory.
    #[test]
    fn test_xcalloc_zeroes() {
        unsafe {
            let n = 16usize;
            let p = xcalloc(n, 1).cast::<u8>().as_ptr();
            let slice = std::slice::from_raw_parts(p, n);
            assert!(slice.iter().all(|&b| b == 0));
            free_(p);
        }
    }

    // xcalloc_::<T> allocates nmemb * size_of::<T>() zeroed and typed.
    #[test]
    fn test_xcalloc_typed_zeroes() {
        unsafe {
            let p = xcalloc_::<u32>(4).as_ptr();
            let slice = std::slice::from_raw_parts(p, 4);
            assert_eq!(slice, &[0u32, 0, 0, 0]);
            free_(p);
        }
    }

    // C `xmalloc.c:41`: calloc detects nmemb*size overflow and returns NULL;
    // the port then panics ("xcalloc: allocating ...").
    #[test]
    #[should_panic(expected = "xcalloc")]
    fn test_xcalloc_overflow_panics() {
        // usize::MAX * 2 overflows; calloc must return NULL.
        let _ = xcalloc(usize::MAX, 2);
    }

    // C `xmalloc.c:27` xmalloc: returns usable, writable memory of the
    // requested size.
    #[test]
    fn test_xmalloc_usable() {
        unsafe {
            let n = 32usize;
            let p = xmalloc(n).cast::<u8>().as_ptr();
            for i in 0..n {
                *p.add(i) = (i as u8).wrapping_mul(3);
            }
            for i in 0..n {
                assert_eq!(*p.add(i), (i as u8).wrapping_mul(3));
            }
            free_(p);
        }
    }

    // C `xmalloc.c:61` xreallocarray: grows an allocation while preserving the
    // existing contents.
    #[test]
    fn test_xreallocarray_preserves_contents() {
        unsafe {
            let p = xcalloc(4, 1).cast::<u8>().as_ptr();
            for i in 0..4 {
                *p.add(i) = (i as u8) + 1; // 1,2,3,4
            }
            let grown = xreallocarray(p.cast(), 8, 1).cast::<u8>().as_ptr();
            for i in 0..4 {
                assert_eq!(*grown.add(i), (i as u8) + 1);
            }
            free_(grown);
        }
    }

    // xsnprintf__ formats into the buffer, NUL-terminates it, and (in the port)
    // returns the number of bytes written including the terminating NUL, i.e.
    // the formatted length + 1.
    #[test]
    fn test_xsnprintf_content_and_len() {
        unsafe {
            let mut buf = [0xffu8; 16];
            let ret = xsnprintf_!(buf.as_mut_ptr(), buf.len(), "hi {}", 42).unwrap();
            // "hi 42" is 5 bytes; the return excludes the NUL, like C snprintf.
            assert_eq!(ret, 5);
            assert_eq!(&buf[..5], b"hi 42");
            assert_eq!(buf[5], 0);
            assert_eq!(strlen(buf.as_ptr()), 5);
        }
    }

    // With no arguments the format string is copied verbatim.
    #[test]
    fn test_xsnprintf_plain() {
        unsafe {
            let mut buf = [0u8; 8];
            let ret = xsnprintf_!(buf.as_mut_ptr(), buf.len(), "abc").unwrap();
            assert_eq!(ret, 3); // length excluding the NUL, like C snprintf
            assert_eq!(&buf[..3], b"abc");
            assert_eq!(buf[3], 0);
        }
    }

    // When the buffer cannot hold the formatted text plus its NUL terminator,
    // the port surfaces an error (analogous to C xsnprintf's overflow check)
    // rather than silently truncating without reporting.
    #[test]
    fn test_xsnprintf_overflow_errors() {
        unsafe {
            // "hello" needs 6 bytes (with NUL); give only 3.
            let mut buf = [0u8; 3];
            let ret = xsnprintf_!(buf.as_mut_ptr(), buf.len(), "hello");
            assert!(ret.is_err());
        }
    }

    // A buffer exactly the size of the formatted text (no room for the NUL)
    // must also error.
    #[test]
    fn test_xsnprintf_needs_room_for_nul() {
        unsafe {
            let mut buf = [0u8; 5];
            let ret = xsnprintf_!(buf.as_mut_ptr(), buf.len(), "hello");
            assert!(ret.is_err());
        }
    }

    // format_nul_ formats its arguments and appends a NUL terminator, returning
    // a Rust-owned (leaked) C string.
    #[test]
    fn test_format_nul() {
        unsafe {
            let p = format_nul!("v={}", 7);
            assert_eq!(as_bytes(p), b"v=7");
            assert_eq!(*p.add(3), 0);
            assert_eq!(strcmp(p, crate::c!("v=7")), 0);
        }
    }
}
