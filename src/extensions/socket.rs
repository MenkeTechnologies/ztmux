//! Resolving the server socket from `$TMUX`, when that socket is ztmux's own.
//!
//! Upstream adopts `$TMUX` outright (`tmux.c:540`). ztmux cannot: nested inside
//! a real tmux pane `$TMUX` points at tmux's socket, and ztmux would speak its
//! protocol at a tmux server. The two are told apart by the directory the
//! socket lives in — ztmux's is `ztmux-<uid>`, tmux's is `tmux-<uid>` — so this
//! is ztmux-original logic rather than a port, and lives here.

use std::ffi::CString;
use std::ptr::null;

use crate::libc::getuid;

/// The socket `$TMUX` names, when it is one of ztmux's own; NULL otherwise.
///
/// A command run inside a pane inherits `$TMUX`, and without this a nested
/// `ztmux set-environment ...` on a `-L pldbg` server would resolve to the
/// *default* socket and quietly act on the wrong server.
///
/// A socket named with `-S` sits wherever the user put it, so it cannot be
/// recognised this way and is not adopted; a nested command there still needs
/// its own `-S`. The returned pointer is a leaked `CString`, matching what
/// `make_label` hands back to the same caller.
pub(crate) unsafe fn socket_from_environment() -> *const u8 {
    let Ok(value) = std::env::var("TMUX") else {
        return null();
    };
    let uid = unsafe { getuid() };
    let Some(socket) = ztmux_socket(&value, uid) else {
        return null();
    };
    CString::new(socket).map_or(null(), |path| path.into_raw().cast())
}

/// The socket path out of a `$TMUX` value (`<path>,<pid>,<session>`), if it
/// lives in ztmux's own `ztmux-<uid>` socket directory.
fn ztmux_socket(value: &str, uid: u32) -> Option<&str> {
    let socket = value.split(',').next().filter(|s| !s.is_empty())?;
    let parent = std::path::Path::new(socket).parent()?.file_name()?;
    (parent == std::ffi::OsStr::new(&format!("ztmux-{uid}"))).then_some(socket)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Regression: a command run inside a `-L pldbg` pane inherits that server's
    // $TMUX, and must resolve back to it rather than to the default socket -
    // but a foreign tmux's $TMUX (`tmux-<uid>`) must still be ignored, or ztmux
    // speaks protocol at a tmux server (bug 2).
    #[test]
    fn ztmux_socket_adopts_only_its_own() {
        assert_eq!(
            ztmux_socket("/private/tmp/ztmux-501/pldbg,28344,0", 501),
            Some("/private/tmp/ztmux-501/pldbg")
        );
        assert_eq!(
            ztmux_socket("/private/tmp/ztmux-501/default,1,0", 501),
            Some("/private/tmp/ztmux-501/default")
        );
        // Real tmux's socket directory, which ztmux must never speak to.
        assert_eq!(ztmux_socket("/private/tmp/tmux-501/default,1,0", 501), None);
        // Another user's ztmux directory is not ours either.
        assert_eq!(ztmux_socket("/private/tmp/ztmux-0/default,1,0", 501), None);
        // A socket loose in a directory of its own name proves nothing.
        assert_eq!(ztmux_socket("/tmp/mysock,1,0", 501), None);
        // Malformed or empty values resolve to nothing rather than to "".
        assert_eq!(ztmux_socket("", 501), None);
        assert_eq!(ztmux_socket(",1,0", 501), None);
        assert_eq!(ztmux_socket("ztmux-501,1,0", 501), None);
    }
}
