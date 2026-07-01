// Copyright (c) 2008, 2017 Otto Moerbeek <otto@drijf.net>
//
// Permission to use, copy, modify, and distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

#[cfg(target_os = "macos")]
/// C `vendor/tmux/compat/reallocarray.c:32`: `void *reallocarray(void *optr, size_t nmemb, size_t size)`
pub unsafe fn reallocarray(
    optr: *mut core::ffi::c_void,
    nmemb: usize,
    size: usize,
) -> *mut core::ffi::c_void {
    const MUL_NO_OVERFLOW: usize = 1usize << (size_of::<usize>() * 4);

    unsafe {
        if (nmemb >= MUL_NO_OVERFLOW || size >= MUL_NO_OVERFLOW)
            && nmemb > 0
            && usize::MAX / nmemb < size
        {
            crate::errno!() = libc::ENOMEM;
            return core::ptr::null_mut();
        }
        libc::realloc(optr, size * nmemb)
    }
}

#[cfg(target_os = "linux")]
pub(crate) use libc::reallocarray;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reallocarray_alloc_usable() {
        // 4 * 4 = 16 bytes: non-null and writable/readable.
        unsafe {
            let p = reallocarray(core::ptr::null_mut(), 4, 4).cast::<u8>();
            assert!(!p.is_null());
            for i in 0..16usize {
                *p.add(i) = i as u8;
            }
            for i in 0..16usize {
                assert_eq!(*p.add(i), i as u8);
            }
            libc::free(p.cast());
        }
    }

    #[test]
    fn test_reallocarray_overflow_returns_null() {
        // nmemb * size overflows usize -> ENOMEM, null pointer.
        unsafe {
            let p = reallocarray(core::ptr::null_mut(), usize::MAX, 2);
            assert!(p.is_null());
        }
    }

    // Growing preserves the existing prefix (realloc semantics), and the whole
    // grown region is writable.
    #[test]
    fn test_reallocarray_grow_preserves_prefix() {
        unsafe {
            let p = reallocarray(core::ptr::null_mut(), 4, 1).cast::<u8>();
            assert!(!p.is_null());
            for i in 0..4usize {
                *p.add(i) = i as u8;
            }
            let q = reallocarray(p.cast(), 8, 1).cast::<u8>();
            assert!(!q.is_null());
            for i in 0..4usize {
                assert_eq!(*q.add(i), i as u8, "grown byte {i} not preserved");
            }
            for i in 0..8usize {
                *q.add(i) = (i * 2) as u8;
            }
            libc::free(q.cast());
        }
    }

    // Shrinking keeps the retained prefix intact.
    #[test]
    fn test_reallocarray_shrink_keeps_prefix() {
        unsafe {
            let p = reallocarray(core::ptr::null_mut(), 16, 1).cast::<u8>();
            assert!(!p.is_null());
            for i in 0..16usize {
                *p.add(i) = (0x40 + i) as u8;
            }
            let q = reallocarray(p.cast(), 4, 1).cast::<u8>();
            assert!(!q.is_null());
            for i in 0..4usize {
                assert_eq!(*q.add(i), (0x40 + i) as u8, "retained byte {i} wrong");
            }
            libc::free(q.cast());
        }
    }

    // nmemb * size is the real allocation size: 3 * 4 == 12 usable bytes.
    #[test]
    fn test_reallocarray_size_is_nmemb_times_size() {
        unsafe {
            let p = reallocarray(core::ptr::null_mut(), 3, 4).cast::<u8>();
            assert!(!p.is_null());
            for i in 0..12usize {
                *p.add(i) = i as u8;
            }
            for i in 0..12usize {
                assert_eq!(*p.add(i), i as u8);
            }
            libc::free(p.cast());
        }
    }

    // Both operands below MUL_NO_OVERFLOW (2^32 on 64-bit) short-circuit the
    // overflow guard even though the product is large but valid (1e6 bytes).
    #[test]
    fn test_reallocarray_below_threshold_allocates() {
        unsafe {
            let p = reallocarray(core::ptr::null_mut(), 1000, 1000).cast::<u8>();
            assert!(!p.is_null());
            // Touch both endpoints of the 1,000,000-byte region.
            *p = 0xAB;
            *p.add(999_999) = 0xCD;
            assert_eq!(*p, 0xAB);
            assert_eq!(*p.add(999_999), 0xCD);
            libc::free(p.cast());
        }
    }

    // Overflow with the large operand in the `size` position (mirrors the
    // nmemb-large case) also returns NULL.
    #[test]
    fn test_reallocarray_size_operand_overflow() {
        unsafe {
            let p = reallocarray(core::ptr::null_mut(), 2, usize::MAX);
            assert!(p.is_null());
        }
    }
}
