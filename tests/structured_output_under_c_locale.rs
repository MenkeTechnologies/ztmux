//! `-o json` and friends must stay parseable for a client that has not declared
//! UTF-8.
//!
//! `server_client_print` (`vendor/tmux/server-client.c:3040`) runs
//! `utf8_sanitize` over a message when the client lacks `CLIENT_UTF8`, and
//! `utf8_sanitize` (`vendor/tmux/utf8.c:784`) replaces every byte outside
//! `0x20..=0x7e` with `_`. The structured-output extension used to hand its whole
//! document to a single `cmdq_print`, so for such a client every newline in it
//! became an underscore and the document collapsed to one unparseable line.
//!
//! Two conditions are needed to be that client, and missing either one hides the
//! bug: a non-UTF-8 locale, AND no `$TMUX` in the environment — `tmux.c:485-492`
//! assumes UTF-8 when `$TMUX` is set, which is why this reproduces from a bare
//! shell but not from inside a ztmux pane. The tests below strip `$TMUX`
//! deliberately; do not "simplify" that away.

use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_ztmux");

fn socket(tag: &str) -> String {
    format!("ztmux-test-{}-{tag}", std::process::id())
}

/// A client with no UTF-8 anywhere: C locale, and no `$TMUX` to imply otherwise.
fn ztmux_c_locale(sock: &str, args: &[&str]) -> Output {
    Command::new(BIN)
        .arg("-L")
        .arg(sock)
        .args(args)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .env_remove("LC_CTYPE")
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .output()
        .expect("failed to run ztmux")
}

fn stdout_of(out: Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn boot(sock: &str) {
    assert!(
        ztmux_c_locale(
            sock,
            &[
                "-f",
                "/dev/null",
                "new-session",
                "-d",
                "-x",
                "80",
                "-y",
                "24",
                "sleep 600"
            ]
        )
        .status
        .success(),
        "could not start a server"
    );
    assert!(
        ztmux_c_locale(sock, &["new-window", "-d", "sleep 600"])
            .status
            .success(),
        "could not add a second window"
    );
}

fn kill(sock: &str) {
    let _ = ztmux_c_locale(sock, &["kill-server"]);
}

#[test]
fn json_keeps_its_line_structure_for_a_non_utf8_client() {
    let sock = socket("json-c-locale");
    boot(&sock);
    let out = stdout_of(ztmux_c_locale(&sock, &["list-windows", "-o", "json"]));
    kill(&sock);

    let lines: Vec<&str> = out.trim_end_matches('\n').split('\n').collect();
    // `[`, one object per window, `]` — four lines for two windows. The defect
    // produced exactly one line, with `_` where each newline had been.
    assert_eq!(lines.len(), 4, "expected 4 lines, got {lines:?}");
    assert_eq!(lines[0], "[");
    assert_eq!(lines[3], "]");
    assert!(lines[1].starts_with('{'), "first row: {}", lines[1]);
    assert!(lines[1].ends_with("},"), "first row: {}", lines[1]);
    assert!(
        lines[2].starts_with('{') && lines[2].ends_with('}'),
        "second row: {}",
        lines[2]
    );
    assert!(
        !out.contains("[_"),
        "newline was sanitized into a separator: {out}"
    );
}

#[test]
fn csv_emits_one_line_per_row_for_a_non_utf8_client() {
    let sock = socket("csv-c-locale");
    boot(&sock);
    let out = stdout_of(ztmux_c_locale(&sock, &["list-windows", "-o", "csv"]));
    kill(&sock);

    // Header plus one line per window. Commas are printable ASCII, so unlike
    // `-o tsv` (whose tabs the sanitizer eats) csv survives this client intact.
    let lines: Vec<&str> = out.trim_end_matches('\n').split('\n').collect();
    assert_eq!(lines.len(), 3, "expected header + 2 rows, got {lines:?}");
    assert!(
        lines[0].starts_with("session,index,name,"),
        "header: {}",
        lines[0]
    );
    assert!(lines[1].contains(','), "row: {}", lines[1]);
}

#[test]
fn non_ascii_content_is_still_sanitized_for_a_non_utf8_client() {
    let sock = socket("sanitize-c-locale");
    boot(&sock);
    assert!(
        ztmux_c_locale(&sock, &["rename-window", "héllo"])
            .status
            .success(),
        "could not rename the window"
    );
    let out = stdout_of(ztmux_c_locale(&sock, &["list-windows", "-o", "json"]));
    kill(&sock);

    // The fix restores the document's line structure; it must NOT smuggle
    // non-ASCII past `utf8_sanitize`, because that is the client telling the
    // server it cannot render it. `list-windows -F` gets the same treatment.
    assert!(
        out.contains(r#""name":"h_llo""#),
        "expected sanitized name in: {out}"
    );
    assert!(
        !out.contains("héllo"),
        "raw UTF-8 reached a non-UTF-8 client: {out}"
    );
}
