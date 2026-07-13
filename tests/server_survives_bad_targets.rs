//! Crash gate: the server must survive commands that resolve a target to nothing.
//!
//! Every bug this pins had the same root cause — a C idiom that is silently unsafe
//! once ported to Rust:
//!
//!   * C's `TAILQ_FOREACH`/`RB_FOREACH` leave the loop variable NULL when the loop
//!     runs to completion, and callers branch on that NULL. A Rust `for` loop that
//!     assigns each element instead retains the *last* one visited, so "not found"
//!     silently became "some arbitrary element". That returned a session-less client
//!     from `cmd_find_client` (`lock-client -t nosuch` then dereferenced
//!     `c->session` and killed the server), and made `session_group_synchronize_to`
//!     sync a lone session from itself, wiping its own window list.
//!   * C builds a throwaway stack struct as an RB_FIND key. In Rust, assigning into
//!     that uninitialized memory *drops* the garbage already there, so a lookup could
//!     call free() on a pointer Rust never allocated.
//!
//! These all present as a dead server, so that is what we assert. Every command runs
//! on a private socket, so this never touches a real tmux/ztmux server.

use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_ztmux");

/// Unique per test so concurrent test threads never share a server.
fn socket(tag: &str) -> String {
    format!("ztmux-test-{}-{tag}", std::process::id())
}

fn ztmux(sock: &str, args: &[&str]) -> Output {
    Command::new(BIN)
        .arg("-L")
        .arg(sock)
        .args(args)
        .output()
        .expect("failed to run ztmux")
}

/// The server answering a trivial query is the liveness signal: if it crashed, the
/// client reports "server exited unexpectedly" and this is false.
fn server_alive(sock: &str) -> bool {
    ztmux(sock, &["has-session", "-t", "base"]).status.success()
}

fn boot(sock: &str) {
    assert!(
        ztmux(sock, &["new-session", "-d", "-s", "base"])
            .status
            .success(),
        "could not start a server on socket {sock}"
    );
}

fn kill(sock: &str) {
    let _ = ztmux(sock, &["kill-server"]);
}

/// `lock-client -t <unknown>` must report an error, not take down the server.
///
/// `cmd_find_client` returned the last client in the list rather than NULL, so the
/// command ran against a client with no session and `server_lock_client` dereferenced
/// `c->session`. Any CMD_CLIENT_TFLAG command (detach-client, suspend-client, …) shared
/// the bug, so they are checked too — a wrong-client match there is silent data loss,
/// not just a crash.
#[test]
fn unknown_client_target_errors_and_server_survives() {
    let sock = socket("client");
    boot(&sock);

    for cmd in ["lock-client", "detach-client", "suspend-client"] {
        let out = ztmux(&sock, &[cmd, "-t", "nosuchclient"]);
        let err = String::from_utf8_lossy(&out.stderr);

        assert!(
            err.contains("can't find client"),
            "{cmd} -t nosuchclient: expected a can't-find-client error, got stderr {err:?}"
        );
        assert!(
            !out.status.success(),
            "{cmd} -t nosuchclient unexpectedly succeeded"
        );
        assert!(
            server_alive(&sock),
            "server died running {cmd} -t nosuchclient"
        );
    }

    kill(&sock);
}

/// `new-session -t <name>` with no such session groups the new session under that
/// name (tmux creates `<name>-0`). This drove two separate crashes: the RB_FIND key
/// struct in `session_group_find` freed uninitialized garbage, and
/// `session_group_synchronize_to` then synced the lone new session from itself and
/// emptied its window list, so `RB_MIN(&s->windows)` returned NULL.
#[test]
fn new_session_into_fresh_group_survives() {
    let sock = socket("group");
    boot(&sock);

    assert!(
        ztmux(&sock, &["new-session", "-d", "-t", "freshgrp"])
            .status
            .success(),
        "new-session -t freshgrp failed"
    );
    assert!(
        server_alive(&sock),
        "server died creating a fresh session group"
    );

    // tmux names the session `<group>-<id>`; the id depends on how many sessions came
    // before, so match the group rather than pinning an index.
    let out = ztmux(
        &sock,
        &["list-sessions", "-F", "#{session_name}:#{session_group}"],
    );
    let listing = String::from_utf8_lossy(&out.stdout);
    let grouped = listing
        .lines()
        .find_map(|l| l.strip_suffix(":freshgrp"))
        .unwrap_or_else(|| panic!("no session in group freshgrp, got {listing:?}"));
    assert!(
        grouped.starts_with("freshgrp-"),
        "grouped session should be named freshgrp-<id>, got {grouped:?}"
    );

    // The grouped session must keep the window it was created with.
    let out = ztmux(&sock, &["list-windows", "-t", grouped]);
    assert!(
        !String::from_utf8_lossy(&out.stdout).trim().is_empty(),
        "grouped session lost its windows — synchronize_to self-synced it"
    );

    // Grouping onto a session that *does* exist must still share its windows.
    assert!(
        ztmux(&sock, &["new-session", "-d", "-t", "base"])
            .status
            .success(),
        "new-session -t base (existing session) failed"
    );
    assert!(
        server_alive(&sock),
        "server died grouping onto an existing session"
    );

    kill(&sock);
}

/// Stacking distinct modes on one pane must create one entry per mode.
///
/// `window_pane_set_mode` scanned `wp->modes` for an entry matching the requested
/// mode and created a new one when there was none. The ported loop retained the last
/// entry visited, so a pane already in some other mode reused that entry — binding the
/// new mode to the previous mode's `data`, a type confusion.
#[test]
fn stacking_distinct_pane_modes_keeps_one_entry_each() {
    let sock = socket("modes");
    boot(&sock);

    for mode in ["copy-mode", "clock-mode", "customize-mode"] {
        assert!(
            ztmux(&sock, &[mode, "-t", "base"]).status.success(),
            "{mode} failed"
        );
        assert!(server_alive(&sock), "server died entering {mode}");
    }

    // One entry per distinct mode entered — reusing a wrong-mode entry would undercount.
    let out = ztmux(
        &sock,
        &["display-message", "-p", "-t", "base", "#{pane_in_mode}"],
    );
    let n = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    assert_eq!(
        n, "3",
        "expected 3 stacked mode entries, got {n:?} — a mode entry was reused"
    );

    kill(&sock);
}

/// wait-for owns its channels through `Box`/`Drop` rather than xstrdup + free, and
/// looks them up without fabricating a key struct. Exercise the full add/find/remove
/// cycle: a channel is created, found again by name, and torn down.
#[test]
fn wait_for_channel_lifecycle_survives() {
    let sock = socket("waitfor");
    boot(&sock);

    // Signal with no waiters: creates the channel, marks it woken, then removes it.
    assert!(ztmux(&sock, &["wait-for", "-S", "chan"]).status.success());
    assert!(server_alive(&sock), "server died signalling a wait channel");

    // Lock/unlock round-trips the same channel through find-by-name.
    assert!(ztmux(&sock, &["wait-for", "-L", "lk"]).status.success());
    assert!(ztmux(&sock, &["wait-for", "-U", "lk"]).status.success());
    assert!(
        server_alive(&sock),
        "server died locking/unlocking a wait channel"
    );

    // Unlocking a channel that is gone reports an error rather than dying.
    let out = ztmux(&sock, &["wait-for", "-U", "lk"]);
    assert!(
        !out.status.success(),
        "unlocking a released channel should fail"
    );
    assert!(
        server_alive(&sock),
        "server died unlocking a released channel"
    );

    // Enough distinct channels that lookups must descend the tree, not hit the root.
    for i in 0..20 {
        assert!(
            ztmux(&sock, &["wait-for", "-L", &format!("ch{i}")])
                .status
                .success(),
            "locking ch{i} failed"
        );
    }
    for i in (0..20).rev() {
        assert!(
            ztmux(&sock, &["wait-for", "-U", &format!("ch{i}")])
                .status
                .success(),
            "ch{i} was not found by name — tree descent is wrong"
        );
    }
    assert!(server_alive(&sock), "server died cycling 20 wait channels");

    kill(&sock);
}
