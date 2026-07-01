// Copyright (c) 2017 Nicholas Marriott <nicholas.marriott@gmail.com>
//
// Permission to use, copy, modify, and distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF MIND, USE, DATA OR PROFITS, WHETHER
// IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING
// OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
use core::ffi::c_void;

/// C `vendor/tmux/compat/freezero.c:25`: `void freezero(void *ptr, size_t size)`
pub unsafe fn freezero(ptr: *mut c_void, size: usize) {
    unsafe {
        if !ptr.is_null() {
            libc::memset(ptr, 0, size);
            libc::free(ptr);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // C source (vendor/tmux/compat/freezero.c): freezero zeroes `size` bytes
    // then frees `ptr`; a NULL ptr is a no-op. There is no return value, so
    // the observable contract is "does not crash / no double-free".

    #[test]
    fn test_freezero_frees_allocation() {
        unsafe {
            let p = libc::malloc(64);
            assert!(!p.is_null());
            // Write into the block so the memset has something to clear.
            libc::memset(p, 0xAB, 64);
            freezero(p, 64);
        }
    }

    #[test]
    fn test_freezero_null_is_noop() {
        unsafe {
            // Must not dereference or free a NULL pointer.
            freezero(core::ptr::null_mut(), 0);
            freezero(core::ptr::null_mut(), 128);
        }
    }

    #[test]
    fn test_freezero_zero_size_allocation() {
        unsafe {
            // A zero-length memset over a live allocation is valid.
            let p = libc::malloc(16);
            assert!(!p.is_null());
            freezero(p, 0);
        }
    }

    // Exercises freezero across a range of allocation sizes; each block is
    // written first so the internal memset has live bytes to clear before free.
    #[test]
    fn test_freezero_various_sizes() {
        unsafe {
            for &sz in &[1usize, 7, 64, 1024, 4096] {
                let p = libc::malloc(sz);
                assert!(!p.is_null(), "malloc({sz}) failed");
                libc::memset(p, 0xCD, sz);
                freezero(p, sz);
            }
        }
    }

    // freezero clears only `size` bytes; passing a size smaller than the
    // allocation is valid (it zeroes the prefix, then frees the whole block).
    #[test]
    fn test_freezero_size_smaller_than_alloc() {
        unsafe {
            let p = libc::malloc(32);
            assert!(!p.is_null());
            libc::memset(p, 0xEE, 32);
            // Only the first 16 bytes are zeroed before free; no crash / no leak.
            freezero(p, 16);
        }
    }
}
