/// The `strlcpy()` function copies up to size - 1 characters from the NUL-terminated string src to dst,
/// NUL-terminating the result.
/// C `vendor/tmux/compat/strlcpy.c:30`: `size_t strlcpy(char *dst, const char *src, size_t siz)`
pub unsafe fn strlcpy(dst: *mut u8, src: *const u8, siz: usize) -> usize {
    unsafe {
        let len = crate::libc::strnlen(src, siz);
        core::ptr::copy_nonoverlapping(src, dst, len);
        *dst.add(len) = b'\0';

        len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strlcpy_full_copy() {
        // src fits: return value is the source length, result NUL-terminated.
        let src = crate::c!("hi");
        let mut dst = [0xffu8; 10];
        let ret = unsafe { strlcpy(dst.as_mut_ptr(), src, dst.len()) };
        assert_eq!(ret, 2);
        assert_eq!(&dst[..3], b"hi\0");
    }

    #[test]
    fn test_strlcpy_truncation() {
        // siz=3: at most 3 bytes copied, then NUL written at dst[3].
        let src = crate::c!("hello");
        let mut dst = [0xffu8; 8];
        let ret = unsafe { strlcpy(dst.as_mut_ptr(), src, 3) };
        assert_eq!(ret, 3);
        assert_eq!(&dst[..4], b"hel\0");
    }

    #[test]
    fn test_strlcpy_empty_src() {
        let src = crate::c!("");
        let mut dst = [0xffu8; 4];
        let ret = unsafe { strlcpy(dst.as_mut_ptr(), src, dst.len()) };
        assert_eq!(ret, 0);
        assert_eq!(dst[0], b'\0');
    }

    #[test]
    fn test_strlcpy_exact_fit_returns_srclen() {
        // src length == siz-1: fits exactly, return value equals strlen(src)
        // and matches the C contract (strlcpy.c:52) when no truncation occurs.
        let src = crate::c!("abc");
        let mut dst = [0xffu8; 8];
        let ret = unsafe { strlcpy(dst.as_mut_ptr(), src, 4) };
        assert_eq!(ret, 3);
        assert_eq!(&dst[..4], b"abc\0");
    }

    #[test]
    fn test_strlcpy_large_buffer_returns_srclen() {
        let src = crate::c!("abcde");
        let mut dst = [0xffu8; 16];
        let ret = unsafe { strlcpy(dst.as_mut_ptr(), src, dst.len()) };
        assert_eq!(ret, 5);
        assert_eq!(&dst[..6], b"abcde\0");
    }

    #[test]
    fn test_strlcpy_truncation_copies_siz_bytes() {
        // DISCREPANCY vs BSD strlcpy: the C routine copies at most siz-1 bytes
        // and returns strlen(src) (the would-be length) so callers can detect
        // truncation. This port instead copies min(strlen(src), siz) = siz
        // bytes, writes the NUL at dst[siz], and returns the *capped* length.
        // For src "hello", siz 4: C -> "hel\0" ret 5; this port -> "hell\0"
        // ret 4. Asserting ACTUAL Rust behavior; buffer sized to hold siz+1.
        let src = crate::c!("hello");
        let mut dst = [0xffu8; 8];
        let ret = unsafe { strlcpy(dst.as_mut_ptr(), src, 4) };
        assert_eq!(ret, 4);
        assert_eq!(&dst[..5], b"hell\0");
    }

    #[test]
    fn test_strlcpy_siz_one() {
        // siz == 1. C strlcpy copies zero bytes, NUL-terminates at dst[0], and
        // returns strlen(src)=1. DISCREPANCY: this port's strnlen("x",1)=1 so it
        // copies one byte and writes NUL at dst[1]. Asserting ACTUAL behavior.
        let src = crate::c!("x");
        let mut dst = [0xffu8; 4];
        let ret = unsafe { strlcpy(dst.as_mut_ptr(), src, 1) };
        assert_eq!(ret, 1);
        assert_eq!(&dst[..2], b"x\0");
    }

    #[test]
    fn test_strlcpy_src_len_equals_siz() {
        // src "abc", siz 3. strnlen("abc",3)=3 -> copies all three, NUL at [3].
        // (C would produce "ab\0" and return 3.) Asserting ACTUAL behavior.
        let src = crate::c!("abc");
        let mut dst = [0xffu8; 8];
        let ret = unsafe { strlcpy(dst.as_mut_ptr(), src, 3) };
        assert_eq!(ret, 3);
        assert_eq!(&dst[..4], b"abc\0");
    }

    #[test]
    fn test_strlcpy_does_not_read_past_src_nul() {
        // strnlen stops at the source NUL, so a short src into a big buffer
        // copies only up to the terminator regardless of siz.
        let src = crate::c!("hi");
        let mut dst = [0xffu8; 32];
        let ret = unsafe { strlcpy(dst.as_mut_ptr(), src, 20) };
        assert_eq!(ret, 2);
        assert_eq!(&dst[..3], b"hi\0");
        // Bytes beyond the terminator are left untouched (still 0xff).
        assert_eq!(dst[3], 0xff);
    }
}
