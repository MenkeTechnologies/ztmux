use core::ffi::c_int;

use libc::{pid_t, termios, winsize};

/// C `vendor/tmux/compat/fdforkpty.c:24`: `int getptmfd(void)`
pub extern "C" fn getptmfd() -> c_int {
    c_int::MAX
}

/// C `vendor/tmux/compat/fdforkpty.c:30`: `pid_t fdforkpty(__unused int ptmfd, int *master, char *name, struct termios *tio, struct winsize *ws)`
pub unsafe fn fdforkpty(
    _ptmfd: c_int,
    master: *mut c_int,
    name: *mut u8,
    tio: *mut termios,
    ws: *mut winsize,
) -> pid_t {
    unsafe { ::libc::forkpty(master, name.cast(), tio, ws) }
}
