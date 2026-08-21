//! Gate: an extension's flags and positionals must reach the extension.
//!
//! Extensions used to find their own arguments by scanning `argv` for their own
//! name (`position(|a| a == "clear")`) and taking what followed. Three verbs are
//! dispatched under a name that is not their module's: `clearall` (`clear.rs`),
//! `revive` (`respawn.rs`) and `finder` (`find.rs`). For those the scan never
//! matched, so the argument list came back empty:
//!
//!   * `clearall -f` and `revive -f` silently stayed in dry-run mode — the
//!     documented way to run them at all did nothing, and `-s <session>` was
//!     ignored, so both printed panes from every session.
//!   * `finder <query>` never saw its query and always exited 2 with a usage
//!     line, which made the verb unusable.
//!
//! Every extension now reads `extensions::verb_args()`, the argument list the
//! CLI dispatch actually consumed, so the verb name is out of the equation.
//! These tests drive the real binary, which is the only place the wiring is
//! observable.

use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_ztmux");

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

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn kill(sock: &str) {
    let _ = ztmux(sock, &["kill-server"]);
}

/// Poll a `display-message` format until `ready` accepts it, so tests never race
/// the server. Returns the last value seen.
fn wait_until(sock: &str, target: &str, format: &str, ready: impl Fn(&str) -> bool) -> String {
    let mut last = String::new();
    for _ in 0..60 {
        last = out(&ztmux(
            sock,
            &["display-message", "-p", "-t", target, format],
        ))
        .trim()
        .to_string();
        if ready(&last) {
            return last;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    last
}

/// Poll until the format equals `want`.
fn wait_for(sock: &str, target: &str, format: &str, want: &str) -> String {
    wait_until(sock, target, format, |v| v == want)
}

/// `finder <query>` must search on the query rather than reject the invocation.
#[test]
fn finder_reads_its_query() {
    let sock = socket("finder");
    assert!(
        ztmux(
            &sock,
            &["new-session", "-d", "-s", "base", "sh", "-c", "sleep 30"],
        )
        .status
        .success(),
        "could not start a server"
    );

    // `finder` matches pane metadata — command, path, title, window name — so
    // query the window name, which `new-session -s base` leaves as the command.
    assert!(
        ztmux(&sock, &["rename-window", "-t", "base", "needle-window"])
            .status
            .success()
    );
    let hit = ztmux(&sock, &["finder", "needle-window"]);
    assert!(
        !err(&hit).contains("usage:"),
        "finder rejected its own query: {}",
        err(&hit)
    );
    assert!(
        hit.status.success() && out(&hit).contains("base:"),
        "finder found nothing for a window that exists: {:?} {:?}",
        out(&hit),
        err(&hit)
    );

    // A query that matches nothing still parses, and exits non-zero so the verb
    // composes in shell `if`.
    let miss = ztmux(&sock, &["finder", "zzz-no-such-pane"]);
    assert!(!miss.status.success(), "an empty result must exit non-zero");
    assert!(
        !err(&miss).contains("usage:"),
        "a non-matching query is not a usage error: {}",
        err(&miss)
    );

    // No query at all is still the usage error, named after the verb the user
    // typed.
    let bare = ztmux(&sock, &["finder"]);
    assert!(
        err(&bare).contains("usage: ztmux finder"),
        "expected a usage line naming `finder`, got {:?}",
        err(&bare)
    );

    kill(&sock);
}

/// `clearall -f` must clear, and `-s` must scope it to one session.
#[test]
fn clearall_honours_force_and_session() {
    let sock = socket("clearall");
    // Each pane prints more than a screenful and then holds, so both have
    // scrollback to clear without needing an interactive shell.
    for name in ["keep", "wipe"] {
        assert!(
            ztmux(
                &sock,
                &[
                    "new-session",
                    "-d",
                    "-s",
                    name,
                    "sh",
                    "-c",
                    "seq 1 200; sleep 30",
                ],
            )
            .status
            .success(),
            "could not start session {name}"
        );
    }
    for target in ["keep", "wipe"] {
        let lines = wait_until(&sock, target, "#{history_size}", |v| v != "0");
        assert_ne!(lines, "0", "{target} never accumulated scrollback");
    }

    // Without -f nothing is touched, whatever the session filter says.
    let dry = ztmux(&sock, &["clearall", "-s", "wipe"]);
    assert!(
        out(&dry).contains("dry-run"),
        "clearall without -f must stay a dry run: {:?}",
        out(&dry)
    );
    assert!(
        !out(&dry).contains("keep:"),
        "`-s wipe` listed panes from another session: {:?}",
        out(&dry)
    );

    let done = ztmux(&sock, &["clearall", "-s", "wipe", "-f"]);
    assert!(
        out(&done).starts_with("cleared"),
        "clearall -f stayed in dry-run: {:?}",
        out(&done)
    );
    assert_eq!(
        wait_for(&sock, "wipe", "#{history_size}", "0"),
        "0",
        "clearall -f did not clear the targeted session"
    );
    assert_ne!(
        out(&ztmux(
            &sock,
            &["display-message", "-p", "-t", "keep", "#{history_size}"]
        ))
        .trim(),
        "0",
        "clearall -s wipe cleared another session too"
    );

    kill(&sock);
}

/// `revive -f` must respawn dead panes instead of only listing them.
#[test]
fn revive_honours_force() {
    let sock = socket("revive");
    assert!(
        ztmux(
            &sock,
            &["new-session", "-d", "-s", "base", "sh", "-c", "sleep 30"],
        )
        .status
        .success(),
        "could not start a server"
    );
    assert!(
        ztmux(&sock, &["set-option", "-g", "remain-on-exit", "on"])
            .status
            .success()
    );
    assert!(
        ztmux(
            &sock,
            &["new-window", "-d", "-t", "base", "sh", "-c", "exit 3"]
        )
        .status
        .success()
    );
    assert_eq!(
        wait_for(&sock, "base:1", "#{pane_dead}", "1"),
        "1",
        "the pane never died, so there is nothing to revive"
    );

    let dry = ztmux(&sock, &["revive"]);
    assert!(
        out(&dry).contains("dry-run"),
        "revive without -f must stay a dry run: {:?}",
        out(&dry)
    );

    let before = out(&ztmux(
        &sock,
        &["display-message", "-p", "-t", "base:1", "#{pane_pid}"],
    ))
    .trim()
    .to_string();

    let done = ztmux(&sock, &["revive", "-f"]);
    assert!(
        out(&done).starts_with("respawned"),
        "revive -f stayed in dry-run: {:?}",
        out(&done)
    );
    // The command really ran again: a respawn records a new pid. (It is
    // `exit 3`, so with remain-on-exit still on the pane goes straight back to
    // dead — the new pid is what proves the respawn happened.)
    let after = out(&ztmux(
        &sock,
        &["display-message", "-p", "-t", "base:1", "#{pane_pid}"],
    ))
    .trim()
    .to_string();
    assert_ne!(
        before, after,
        "revive -f did not respawn the dead pane (pid unchanged)"
    );

    kill(&sock);
}
