#[cfg(target_os = "linux")]
// vendor/tmux/compat/getprogname.c:25  getprogname()
pub unsafe fn getprogname() -> *const u8 {
    unsafe extern "C" {
        static mut program_invocation_short_name: *mut u8;
    }

    unsafe { program_invocation_short_name }
}

#[cfg(target_os = "macos")]
// vendor/tmux/compat/getprogname.c:25  getprogname()
pub unsafe fn getprogname() -> *const u8 {
    c"tmux".as_ptr().cast()
}
