/// C `vendor/tmux/compat/getpeereid.c:30`: `int getpeereid(int s, uid_t *uid, gid_t *gid)`
///
/// tmux's compat file selects one of three implementations at build time. The
/// Rust port mirrors that with `cfg`: Linux/Android use `SO_PEERCRED`
/// (`HAVE_SO_PEERCRED`), the BSDs/macOS delegate to the system `getpeereid`
/// (`HAVE_GETPEEREID`), and everything else falls back to the process's own
/// effective ids (the C `#else` branch).
#[cfg(any(target_os = "linux", target_os = "android"))]
pub unsafe fn getpeereid(s: i32, uid: *mut libc::uid_t, gid: *mut libc::gid_t) -> i32 {
    unsafe {
        let mut uc: libc::ucred = core::mem::zeroed();
        let mut len = size_of::<libc::ucred>() as libc::socklen_t;
        if libc::getsockopt(
            s,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut uc).cast(),
            &raw mut len,
        ) == -1
        {
            return -1;
        }
        *uid = uc.uid;
        *gid = uc.gid;
        0
    }
}

/// C `vendor/tmux/compat/getpeereid.c:30`: `int getpeereid(int s, uid_t *uid, gid_t *gid)`
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly",
))]
pub unsafe fn getpeereid(s: i32, uid: *mut libc::uid_t, gid: *mut libc::gid_t) -> i32 {
    unsafe { libc::getpeereid(s, uid, gid) }
}

/// C `vendor/tmux/compat/getpeereid.c:30`: `int getpeereid(int s, uid_t *uid, gid_t *gid)`
#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly",
)))]
pub unsafe fn getpeereid(_s: i32, uid: *mut libc::uid_t, gid: *mut libc::gid_t) -> i32 {
    unsafe {
        *uid = libc::geteuid();
        *gid = libc::getegid();
    }
    0
}
