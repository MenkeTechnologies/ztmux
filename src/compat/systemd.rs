// vendor/tmux/compat/systemd.c:45  int systemd_create_socket(int flags, char **cause)
pub fn systemd_create_socket(flags: i32, cause: *mut *mut u8) -> i32 {
    unsafe extern "C" {
        // vendor/tmux/compat/systemd.c:45  int systemd_create_socket(int flags, char **cause)
        fn systemd_create_socket(flags: i32, cause: *mut *mut u8) -> i32;
    }
    unsafe { systemd_create_socket(flags, cause) }
}
