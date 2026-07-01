use core::ffi::{c_char, c_int};
unsafe extern "C" {
    pub unsafe fn utempter_add_record(master_fd: c_int, hostname: *const c_char) -> c_int;
    pub unsafe fn utempter_remove_added_record() -> c_int;
    pub unsafe fn utempter_remove_record(master_fd: c_int) -> c_int;
    pub unsafe fn utempter_set_helper(pathname: *const c_char);
}
