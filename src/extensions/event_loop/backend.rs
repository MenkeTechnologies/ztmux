//! Readiness polling for the event loop.
//!
//! Which syscall is used mirrors what tmux forces libevent to use, for the same
//! reasons: `select` on macOS, where kqueue and poll do not work on anything
//! but sockets (tmux sets `EVENT_NOKQUEUE`/`EVENT_NOPOLL` in
//! `osdep-darwin.c`), and `poll` everywhere else (tmux sets `EVENT_NOEPOLL` in
//! `osdep-linux.c` because epoll cannot watch `/dev/null`). The tty, the pane
//! ptys and `/dev/null` are all non-sockets, so those constraints are not
//! historical trivia — they decide whether the loop works at all.
use std::ffi::c_int;
use std::io;
use std::time::Duration;

use crate::event_::{EV_READ, EV_WRITE};

/// One file descriptor to watch, and what came back for it.
#[derive(Clone, Copy)]
pub struct Watch {
    pub fd: c_int,
    /// `EV_READ` and/or `EV_WRITE`.
    pub want: i16,
    /// Subset of `want` that is ready; filled in by [`wait`].
    pub got: i16,
}

impl Watch {
    pub fn new(fd: c_int, want: i16) -> Self {
        Self { fd, want, got: 0 }
    }
}

/// Block until at least one watch is ready or `timeout` elapses; `None` waits
/// forever. An interrupted wait returns `Ok(())` with nothing ready, which the
/// loop treats like a spurious wakeup.
pub fn wait(watches: &mut [Watch], timeout: Option<Duration>) -> io::Result<()> {
    for w in watches.iter_mut() {
        w.got = 0;
    }
    wait_impl(watches, timeout)
}

/// `poll(2)`: one entry per watch, so duplicate fds with different callbacks
/// stay independent.
#[cfg(not(target_os = "macos"))]
fn wait_impl(watches: &mut [Watch], timeout: Option<Duration>) -> io::Result<()> {
    let mut fds: Vec<libc::pollfd> = watches
        .iter()
        .map(|w| libc::pollfd {
            fd: w.fd,
            events: (if w.want & EV_READ != 0 {
                libc::POLLIN
            } else {
                0
            }) | (if w.want & EV_WRITE != 0 {
                libc::POLLOUT
            } else {
                0
            }),
            revents: 0,
        })
        .collect();

    // poll takes whole milliseconds; round up so a sub-millisecond timer is not
    // turned into a busy spin.
    let ms: c_int = match timeout {
        None => -1,
        Some(d) => c_int::try_from(d.as_millis().min(c_int::MAX as u128))
            .unwrap_or(c_int::MAX)
            .max(c_int::from(!d.is_zero())),
    };

    let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, ms) };
    if n < 0 {
        let err = io::Error::last_os_error();
        return if err.kind() == io::ErrorKind::Interrupted {
            Ok(())
        } else {
            Err(err)
        };
    }

    for (w, p) in watches.iter_mut().zip(fds) {
        // An error or hangup is reported as readiness for whatever the caller
        // asked for, so its callback runs and sees the real error from read or
        // write. This is what libevent's poll backend does.
        if p.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
            w.got = w.want;
            continue;
        }
        if p.revents & libc::POLLIN != 0 {
            w.got |= EV_READ;
        }
        if p.revents & libc::POLLOUT != 0 {
            w.got |= EV_WRITE;
        }
    }
    Ok(())
}

/// `select(2)` with descriptor sets sized to the highest fd in use, the way
/// libevent's select backend does it. The fixed `fd_set` type tops out at
/// `FD_SETSIZE` (1024) descriptors, which a busy server with many panes, jobs
/// and clients can exceed, and writing past it would be silent memory
/// corruption.
#[cfg(target_os = "macos")]
fn wait_impl(watches: &mut [Watch], timeout: Option<Duration>) -> io::Result<()> {
    /// One word of an `fd_set`. Darwin's `fd_set` is an array of `__int32_t`
    /// (`__DARWIN_NFDBITS` is 32), so the bit for descriptor `fd` is bit
    /// `fd % 32` of word `fd / 32`.
    type FdMask = i32;
    /// Bits per word of an `fd_set`.
    const BITS: usize = FdMask::BITS as usize;

    let max_fd = watches.iter().map(|w| w.fd).max().unwrap_or(-1);
    if max_fd < 0 {
        // Nothing to watch: this is just a sleep.
        return sleep(timeout);
    }
    let nfds = max_fd + 1;
    let words = (nfds as usize).div_ceil(BITS);

    let mut rset = vec![0 as FdMask; words];
    let mut wset = vec![0 as FdMask; words];
    let set = |bits: &mut [FdMask], fd: c_int| {
        bits[fd as usize / BITS] |= 1 << (fd as usize % BITS);
    };
    let isset =
        |bits: &[FdMask], fd: c_int| bits[fd as usize / BITS] & (1 << (fd as usize % BITS)) != 0;

    for w in watches.iter() {
        if w.want & EV_READ != 0 {
            set(&mut rset, w.fd);
        }
        if w.want & EV_WRITE != 0 {
            set(&mut wset, w.fd);
        }
    }

    let mut tv = timeout.map(|d| libc::timeval {
        tv_sec: d.as_secs().min(libc::time_t::MAX as u64) as libc::time_t,
        tv_usec: d.subsec_micros() as libc::suseconds_t,
    });
    let tvp = tv.as_mut().map_or(std::ptr::null_mut(), std::ptr::from_mut);

    let n = unsafe {
        libc::select(
            nfds,
            rset.as_mut_ptr().cast::<libc::fd_set>(),
            wset.as_mut_ptr().cast::<libc::fd_set>(),
            std::ptr::null_mut(),
            tvp,
        )
    };
    if n < 0 {
        let err = io::Error::last_os_error();
        return if err.kind() == io::ErrorKind::Interrupted {
            Ok(())
        } else {
            Err(err)
        };
    }

    for w in watches.iter_mut() {
        if w.want & EV_READ != 0 && isset(&rset, w.fd) {
            w.got |= EV_READ;
        }
        if w.want & EV_WRITE != 0 && isset(&wset, w.fd) {
            w.got |= EV_WRITE;
        }
    }
    Ok(())
}

/// Wait out `timeout` with no descriptors to watch.
#[cfg(target_os = "macos")]
fn sleep(timeout: Option<Duration>) -> io::Result<()> {
    let Some(d) = timeout else {
        // No descriptors and no deadline would block forever with nothing able
        // to wake us; the loop treats this as a spurious wakeup and re-checks.
        return Ok(());
    };
    let req = libc::timespec {
        tv_sec: d.as_secs().min(libc::time_t::MAX as u64) as libc::time_t,
        tv_nsec: d.subsec_nanos() as _,
    };
    unsafe { libc::nanosleep(&req, std::ptr::null_mut()) };
    Ok(())
}
