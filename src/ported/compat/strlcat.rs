use crate::libc;

/// The `strlcat()` function appends the NUL-terminated string src to the end of dst.
/// It will append at most size - strlen(dst) - 1 bytes, NUL-terminating the result.
/// C `vendor/tmux/compat/strlcat.c:32`: `size_t strlcat(char *dst, const char *src, size_t siz)`
pub unsafe fn strlcat(dst: *mut u8, src: *const u8, size: usize) -> usize {
    unsafe {
        let dst_strlen = libc::strnlen(dst, size);
        let src_strlen = libc::strnlen(src, size.saturating_sub(dst_strlen).saturating_sub(1));

        core::ptr::copy_nonoverlapping(src, dst.add(dst_strlen), src_strlen);
        *dst.add(dst_strlen + src_strlen) = b'\0';

        dst_strlen + src_strlen
    }
}

#[expect(clippy::disallowed_methods)]
pub unsafe fn strlcat_(dst: *mut u8, src: &str, size: usize) -> usize {
    unsafe {
        let dst_strlen = libc::strnlen(dst, size);
        let src_strlen = src
            .len()
            .min(size.saturating_sub(dst_strlen).saturating_sub(1));

        core::ptr::copy_nonoverlapping(src.as_ptr(), dst.add(dst_strlen), src_strlen);
        *dst.add(dst_strlen + src_strlen) = b'\0';

        dst_strlen + src_strlen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strlcat_append() {
        // "foo" + "bar" fits: return value is combined length, NUL-terminated.
        let mut dst = [0u8; 10];
        dst[..3].copy_from_slice(b"foo");
        let src = crate::c!("bar");
        let ret = unsafe { strlcat(dst.as_mut_ptr(), src, dst.len()) };
        assert_eq!(ret, 6);
        assert_eq!(&dst[..7], b"foobar\0");
    }

    #[test]
    fn test_strlcat_truncation() {
        // size=5, dst holds "foo": only 5-3-1=1 byte of src is appended.
        let mut dst = [0u8; 10];
        dst[..3].copy_from_slice(b"foo");
        let src = crate::c!("bar");
        let ret = unsafe { strlcat(dst.as_mut_ptr(), src, 5) };
        assert_eq!(ret, 4);
        assert_eq!(&dst[..5], b"foob\0");
    }

    #[test]
    fn test_strlcat_empty_dst() {
        let mut dst = [0u8; 10];
        let src = crate::c!("hi");
        let ret = unsafe { strlcat(dst.as_mut_ptr(), src, dst.len()) };
        assert_eq!(ret, 2);
        assert_eq!(&dst[..3], b"hi\0");
    }

    #[test]
    fn test_strlcat_empty_src() {
        // Appending "" leaves dst unchanged and returns its existing length.
        let mut dst = [0u8; 10];
        dst[..2].copy_from_slice(b"hi");
        let src = crate::c!("");
        let ret = unsafe { strlcat(dst.as_mut_ptr(), src, dst.len()) };
        assert_eq!(ret, 2);
        assert_eq!(&dst[..3], b"hi\0");
    }

    #[test]
    fn test_strlcat_exact_fit() {
        // dst "ab" + src "cd" into siz 5: 2+2 bytes plus NUL fills the buffer
        // exactly, no truncation.
        let mut dst = [0u8; 10];
        dst[..2].copy_from_slice(b"ab");
        let src = crate::c!("cd");
        let ret = unsafe { strlcat(dst.as_mut_ptr(), src, 5) };
        assert_eq!(ret, 4);
        assert_eq!(&dst[..5], b"abcd\0");
    }

    #[test]
    fn test_strlcat_truncation_returns_copied_len() {
        // DISCREPANCY vs BSD strlcat: C returns MIN(siz, strlen(dst)) +
        // strlen(src) — the would-be length that signals truncation
        // (strlcat.c:56, "if retval >= siz, truncation occurred"). This port
        // returns dst_len + the *truncated* src length actually written.
        // dst "foo", siz 5, src "barbaz": only 5-3-1=1 byte of src is copied.
        // C -> ret 3+6=9; this port -> ret 4. Asserting ACTUAL behavior.
        let mut dst = [0u8; 16];
        dst[..3].copy_from_slice(b"foo");
        let src = crate::c!("barbaz");
        let ret = unsafe { strlcat(dst.as_mut_ptr(), src, 5) };
        assert_eq!(ret, 4);
        assert_eq!(&dst[..5], b"foob\0");
    }

    #[test]
    fn test_strlcat_no_room_left() {
        // dst already occupies the whole size window (no NUL within `siz`):
        // nothing is appended. C returns siz + strlen(src) and writes nothing;
        // DISCREPANCY: this port returns siz and writes a NUL at dst[siz].
        let mut dst = [b'x'; 16];
        let src = crate::c!("y");
        let ret = unsafe { strlcat(dst.as_mut_ptr(), src, 3) };
        assert_eq!(ret, 3);
        // Port terminates at index siz.
        assert_eq!(dst[3], b'\0');
    }

    #[test]
    fn test_strlcat_size_zero() {
        // siz 0. C strlcat writes nothing and returns strlen(src).
        // DISCREPANCY: this port computes dst_len=0, copies 0 bytes, and writes
        // a NUL at dst[0], returning 0. Asserting ACTUAL behavior (buffer has
        // room for the stray NUL).
        let mut dst = [b'z'; 8];
        let src = crate::c!("abc");
        let ret = unsafe { strlcat(dst.as_mut_ptr(), src, 0) };
        assert_eq!(ret, 0);
        assert_eq!(dst[0], b'\0');
    }

    #[test]
    fn test_strlcat_repeated_append() {
        // Two successive appends accumulate, each returning the running length.
        let mut dst = [0u8; 16];
        let ret1 = unsafe { strlcat(dst.as_mut_ptr(), crate::c!("ab"), dst.len()) };
        assert_eq!(ret1, 2);
        let ret2 = unsafe { strlcat(dst.as_mut_ptr(), crate::c!("cde"), dst.len()) };
        assert_eq!(ret2, 5);
        assert_eq!(&dst[..6], b"abcde\0");
    }
}
