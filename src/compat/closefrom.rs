// Copyright (c) 2004-2005 Todd C. Miller <Todd.Miller@courtesan.com>
//
// Permission to use, copy, modify, and distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

#[cfg(target_os = "linux")]
unsafe extern "C" {
    // vendor/tmux/compat/closefrom.c:90  closefrom()
    pub fn closefrom(__lowfd: i32);
}

#[cfg(target_os = "macos")]
// vendor/tmux/compat/closefrom.c:63  closefrom_fallback()
fn closefrom_fallback(lowfd: i32) {
    unsafe {
        let mut maxfd = libc::sysconf(libc::_SC_OPEN_MAX);
        if maxfd < 0 {
            maxfd = 256; // OPEN_MAX
        }
        let mut fd = lowfd as libc::c_long;
        while fd < maxfd {
            libc::close(fd as i32);
            fd += 1;
        }
    }
}

#[cfg(target_os = "macos")]
/// Closes all file descriptors >= `lowfd`.
// vendor/tmux/compat/closefrom.c:98  closefrom() — the HAVE_LIBPROC_H variant.
//
// Enumerate the process's OPEN fds with proc_pidinfo(PROC_PIDLISTFDS) and close
// only those >= lowfd. The previous port looped close() from lowfd up to
// getdtablesize() — i.e. the (server-raised) RLIMIT_NOFILE, which can be
// millions — so the pane child hung here between fork and exec and never reached
// execvp (pane_current_command then reported the server binary, not the child).
pub fn closefrom(lowfd: i32) {
    unsafe {
        let pid = libc::getpid();
        let sz = libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, std::ptr::null_mut(), 0);
        if sz == 0 {
            return; // no fds, really?
        }
        if sz < 0 {
            return closefrom_fallback(lowfd);
        }
        let fdinfo_buf = libc::malloc(sz as usize) as *mut libc::proc_fdinfo;
        if fdinfo_buf.is_null() {
            return closefrom_fallback(lowfd);
        }
        let r = libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, fdinfo_buf.cast(), sz);
        if r < 0 || r > sz {
            libc::free(fdinfo_buf.cast());
            return closefrom_fallback(lowfd);
        }
        let count = r as usize / size_of::<libc::proc_fdinfo>();
        for i in 0..count {
            let fd = (*fdinfo_buf.add(i)).proc_fd;
            if fd >= lowfd {
                libc::close(fd);
            }
        }
        libc::free(fdinfo_buf.cast());
    }
}
