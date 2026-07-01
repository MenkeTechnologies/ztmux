/// The `strlcpy()` function copies up to size - 1 characters from the NUL-terminated string src to dst,
/// NUL-terminating the result.
// vendor/tmux/compat/strlcpy.c:30  size_t strlcpy(char *dst, const char *src, size_t siz)
pub unsafe fn strlcpy(dst: *mut u8, src: *const u8, siz: usize) -> usize {
    unsafe {
        let len = crate::libc::strnlen(src, siz);
        core::ptr::copy_nonoverlapping(src, dst, len);
        *dst.add(len) = b'\0';

        len
    }
}
