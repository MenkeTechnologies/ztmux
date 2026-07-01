// vendor/tmux/compat/systemd.c:45  systemd_create_socket()
pub fn systemd_create_socket(flags: i32, cause: *mut *mut u8) -> i32 {
    unsafe extern "C" {
        // vendor/tmux/compat/systemd.c:45  systemd_create_socket()
        fn systemd_create_socket(flags: i32, cause: *mut *mut u8) -> i32;
    }
    unsafe { systemd_create_socket(flags, cause) }
}
