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
use core::ffi::c_void;
use core::ptr::null_mut;

/// C `vendor/tmux/compat/recallocarray.c:33`: `void *recallocarray(void *ptr, size_t oldnmemb, size_t newnmemb, size_t size)`
pub unsafe fn recallocarray(
    ptr: *mut c_void,
    oldnmemb: usize,
    newnmemb: usize,
    size: usize,
) -> *mut c_void {
    const MUL_NO_OVERFLOW: usize = 1usize << (size_of::<usize>() * 4);

    unsafe extern "C" {
        fn getpagesize() -> i32;
    }

    unsafe {
        if ptr.is_null() {
            return libc::calloc(newnmemb, size);
        }

        if (newnmemb >= MUL_NO_OVERFLOW || size >= MUL_NO_OVERFLOW)
            && newnmemb > 0
            && usize::MAX / newnmemb < size
        {
            crate::errno!() = libc::ENOMEM;
            return null_mut();
        }
        let newsize = newnmemb * size;

        if (oldnmemb >= MUL_NO_OVERFLOW || size >= MUL_NO_OVERFLOW)
            && oldnmemb > 0
            && usize::MAX / oldnmemb < size
        {
            crate::errno!() = libc::EINVAL;
            return null_mut();
        }
        let oldsize = oldnmemb * size;

        // Don't bother too much if we're shrinking just a bit,
        // we do not shrink for series of small steps, oh well.
        if newsize <= oldsize {
            let d = oldsize - newsize;

            if d < oldsize / 2 && d < getpagesize() as usize {
                libc::memset((ptr as *mut u8).add(newsize).cast(), 0, d);
                return ptr;
            }
        }

        let newptr = libc::malloc(newsize);
        if newptr.is_null() {
            return null_mut();
        }

        if newsize > oldsize {
            libc::memcpy(newptr, ptr, oldsize);
            libc::memset(
                (newptr as *mut u8).add(oldsize).cast(),
                0,
                newsize - oldsize,
            );
        } else {
            libc::memcpy(newptr, ptr, newsize);
        }

        libc::memset(ptr, 0, oldsize);
        libc::free(ptr);

        newptr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected behavior derived from vendor/tmux/compat/recallocarray.c.

    #[test]
    fn test_recallocarray_null_ptr_is_calloc() {
        // C: `if (ptr == NULL) return calloc(newnmemb, size);`
        // Result is newnmemb*size bytes, all zeroed.
        unsafe {
            let p = recallocarray(null_mut(), 0, 5, 4) as *mut u8;
            assert!(!p.is_null());
            for i in 0..20 {
                assert_eq!(*p.add(i), 0, "calloc'd byte {i} not zero");
            }
            libc::free(p.cast());
        }
    }

    #[test]
    fn test_recallocarray_grow_zeroes_new_and_preserves_old() {
        // Growing goes through the malloc/memcpy/memset path:
        // new bytes [oldsize..newsize) must be zeroed, old bytes copied.
        unsafe {
            let p = libc::malloc(4) as *mut u8;
            assert!(!p.is_null());
            for i in 0..4 {
                *p.add(i) = 0xAA;
            }

            let q = recallocarray(p.cast(), 4, 8, 1) as *mut u8;
            assert!(!q.is_null());
            // Original bytes preserved.
            for i in 0..4 {
                assert_eq!(*q.add(i), 0xAA, "old byte {i} not preserved");
            }
            // Newly grown bytes zeroed.
            for i in 4..8 {
                assert_eq!(*q.add(i), 0, "grown byte {i} not zeroed");
            }
            libc::free(q.cast());
        }
    }

    #[test]
    fn test_recallocarray_small_shrink_zeroes_tail_in_place() {
        // C: when newsize <= oldsize and the delta is small (d < oldsize/2 and
        // d < getpagesize()), the same ptr is returned with the freed tail
        // [newsize..oldsize) zeroed in place.
        unsafe {
            let p = libc::malloc(100) as *mut u8;
            assert!(!p.is_null());
            libc::memset(p.cast(), 0xFF, 100);

            let q = recallocarray(p.cast(), 100, 90, 1) as *mut u8;
            assert!(!q.is_null());
            // Same allocation returned (small-shrink fast path).
            assert_eq!(q, p);
            // Retained region unchanged.
            for i in 0..90 {
                assert_eq!(*q.add(i), 0xFF, "retained byte {i} changed");
            }
            // Freed tail zeroed.
            for i in 90..100 {
                assert_eq!(*q.add(i), 0, "shrunk tail byte {i} not zeroed");
            }
            libc::free(q.cast());
        }
    }

    #[test]
    fn test_recallocarray_large_shrink_copies_prefix() {
        // Shrinking by a large delta (d >= oldsize/2) takes the malloc/memcpy
        // path copying only `newsize` bytes.
        unsafe {
            let p = libc::malloc(100) as *mut u8;
            assert!(!p.is_null());
            libc::memset(p.cast(), 0x5A, 100);

            let q = recallocarray(p.cast(), 100, 10, 1) as *mut u8;
            assert!(!q.is_null());
            for i in 0..10 {
                assert_eq!(*q.add(i), 0x5A, "copied prefix byte {i} wrong");
            }
            libc::free(q.cast());
        }
    }

    #[test]
    fn test_recallocarray_overflow_returns_null() {
        // C: when the newnmemb*size multiplication would overflow, set ENOMEM
        // and return NULL (the original ptr is NOT freed in that case).
        unsafe {
            let p = libc::malloc(8);
            assert!(!p.is_null());

            crate::errno!() = 0;
            let r = recallocarray(p, 1, usize::MAX, 2);
            assert!(r.is_null(), "overflow should return NULL");
            assert_eq!(crate::errno!(), libc::ENOMEM);

            // ptr was not freed by recallocarray on the overflow path.
            libc::free(p);
        }
    }

    // C: the oldnmemb*size multiplication overflow check sets EINVAL (not
    // ENOMEM) and returns NULL without freeing ptr (recallocarray.c:48-50).
    #[test]
    fn test_recallocarray_oldnmemb_overflow_einval() {
        unsafe {
            let p = libc::malloc(8);
            assert!(!p.is_null());
            crate::errno!() = 0;
            // newnmemb*size is fine (4*2); oldnmemb*size overflows.
            let r = recallocarray(p, usize::MAX, 4, 2);
            assert!(r.is_null(), "old overflow should return NULL");
            assert_eq!(crate::errno!(), libc::EINVAL);
            // ptr not freed on this error path.
            libc::free(p);
        }
    }

    // newsize == oldsize: delta is 0, so the small-shrink fast path returns the
    // same pointer with contents untouched (recallocarray.c:55-61).
    #[test]
    fn test_recallocarray_same_size_returns_same_ptr() {
        unsafe {
            let p = libc::malloc(16) as *mut u8;
            assert!(!p.is_null());
            libc::memset(p.cast(), 0x5A, 16);
            let q = recallocarray(p.cast(), 16, 16, 1) as *mut u8;
            assert_eq!(q, p, "no-op resize should keep the same allocation");
            for i in 0..16 {
                assert_eq!(*q.add(i), 0x5A, "byte {i} changed");
            }
            libc::free(q.cast());
        }
    }

    // Growing with a non-1 element size zeroes exactly the new region and
    // preserves the old bytes (oldsize = 2*4, newsize = 4*4).
    #[test]
    fn test_recallocarray_grow_with_size_field() {
        unsafe {
            let p = libc::malloc(8) as *mut u8;
            assert!(!p.is_null());
            for i in 0..8 {
                *p.add(i) = 0xAA;
            }
            let q = recallocarray(p.cast(), 2, 4, 4) as *mut u8;
            assert!(!q.is_null());
            for i in 0..8 {
                assert_eq!(*q.add(i), 0xAA, "old byte {i} not preserved");
            }
            for i in 8..16 {
                assert_eq!(*q.add(i), 0, "grown byte {i} not zeroed");
            }
            libc::free(q.cast());
        }
    }

    // Large shrink with a non-1 element size copies only `newsize` bytes
    // (oldsize = 50*2 = 100, newsize = 5*2 = 10).
    #[test]
    fn test_recallocarray_large_shrink_with_size_field() {
        unsafe {
            let p = libc::malloc(100) as *mut u8;
            assert!(!p.is_null());
            libc::memset(p.cast(), 0x33, 100);
            let q = recallocarray(p.cast(), 50, 5, 2) as *mut u8;
            assert!(!q.is_null());
            for i in 0..10 {
                assert_eq!(*q.add(i), 0x33, "copied prefix byte {i} wrong");
            }
            libc::free(q.cast());
        }
    }

    // NULL ptr with a non-1 element size behaves as calloc(newnmemb, size):
    // 3*8 == 24 zeroed bytes (recallocarray.c:31-32).
    #[test]
    fn test_recallocarray_null_ptr_size_field_zeroed() {
        unsafe {
            let p = recallocarray(null_mut(), 0, 3, 8) as *mut u8;
            assert!(!p.is_null());
            for i in 0..24 {
                assert_eq!(*p.add(i), 0, "calloc'd byte {i} not zero");
            }
            libc::free(p.cast());
        }
    }
}
