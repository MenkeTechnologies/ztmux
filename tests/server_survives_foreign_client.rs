//! Crash gate: the server must survive a client built from a different tmux release.
//!
//! A client with `$TMUX` set connects to whatever socket it names, so a ztmux server
//! is routinely reached by a stock tmux binary (`if-shell "tmux has-session"` in a
//! shared `.tmux.conf` does exactly that during config load). Both speak protocol 8,
//! but a newer tmux sends identify flag bits and message types this build has never
//! heard of. C ORs the flags in raw (`c->flags |= longflags`) and its dispatch `switch`
//! has no default, so unknown input is carried or ignored. The port had
//! `from_bits(..).expect(..)` and `try_from(..).expect(..)`, which panicked and took
//! the server down with "server exited unexpectedly" on every client.
//!
//! The test speaks raw imsg on a private socket, so no real server is touched.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_ztmux");

/// `vendor/tmux/tmux-protocol.h`
const PROTOCOL_VERSION: u32 = 8;
const MSG_IDENTIFY_LONGFLAGS: u32 = 111;

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

fn server_alive(sock: &str) -> bool {
    ztmux(sock, &["has-session", "-t", "base"]).status.success()
}

/// Boot a server and return its socket path as the server itself reports it.
fn boot(sock: &str) -> String {
    assert!(
        ztmux(
            sock,
            &["-f", "/dev/null", "new-session", "-d", "-s", "base"]
        )
        .status
        .success(),
        "could not start a server on socket {sock}"
    );
    let out = ztmux(sock, &["display-message", "-p", "#{socket_path}"]);
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

/// Send one imsg (header: type, len, peerid = version, pid) and hang up.
fn send_imsg(path: &str, msgtype: u32, data: &[u8]) {
    let mut stream = UnixStream::connect(path).expect("connect to server socket");
    let mut buf = Vec::new();
    buf.extend_from_slice(&msgtype.to_ne_bytes());
    buf.extend_from_slice(&((16 + data.len()) as u32).to_ne_bytes());
    buf.extend_from_slice(&PROTOCOL_VERSION.to_ne_bytes());
    buf.extend_from_slice(&u32::MAX.to_ne_bytes());
    buf.extend_from_slice(data);
    stream.write_all(&buf).expect("write imsg");
}

fn assert_survives(tag: &str, msgtype: u32, data: &[u8]) {
    let sock = socket(tag);
    let path = boot(&sock);
    send_imsg(&path, msgtype, data);
    let alive = server_alive(&sock);
    let _ = ztmux(&sock, &["kill-server"]);
    assert!(
        alive,
        "server died after msgtype {msgtype} payload {data:?}"
    );
}

#[test]
fn identify_longflags_with_an_unknown_bit_is_kept_not_fatal() {
    assert_survives(
        "longflags",
        MSG_IDENTIFY_LONGFLAGS,
        &(1u64 << 63).to_ne_bytes(),
    );
}

#[test]
fn unknown_message_type_is_ignored_not_fatal() {
    assert_survives("msgtype", 9999, &[]);
}
