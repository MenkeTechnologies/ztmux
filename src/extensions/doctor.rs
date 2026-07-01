//! `ztmux doctor` — a one-shot environment / server health check.
//!
//! Like [`super::tree`] it is a client subcommand: it inspects the build, the
//! terminal environment, the resolved socket, and (if one is running) the
//! server reached over the same `list-*`/`show-options` path the other
//! extensions use — so it needs no linkage against the server internals. It is
//! deliberately safe to run with no server up: server-dependent checks degrade
//! to a warning instead of starting one.
//!
//! Output is a grouped, coloured report (plain when stdout is not a TTY) or, with
//! `-o json` / `--json`, a machine-readable object. The process exit code is the
//! worst severity seen: 0 = all ok, 1 = warnings, 2 = errors — so `ztmux doctor`
//! drops straight into a CI gate or a shell `if`.

use std::ffi::CStr;
use std::io::IsTerminal;
use std::os::unix::fs::FileTypeExt;

use super::tmux_query::{poll, ztmux_cmd};

/// Severity of a single check, ordered so `max` picks the worst.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Sev {
    Ok,
    Warn,
    Err,
}

impl Sev {
    fn symbol(self) -> &'static str {
        match self {
            Sev::Ok => "✓",
            Sev::Warn => "⚠",
            Sev::Err => "✗",
        }
    }
    fn color(self) -> &'static str {
        match self {
            Sev::Ok => "32",   // green
            Sev::Warn => "33", // yellow
            Sev::Err => "31",  // red
        }
    }
    fn word(self) -> &'static str {
        match self {
            Sev::Ok => "ok",
            Sev::Warn => "warn",
            Sev::Err => "error",
        }
    }
}

/// One diagnostic line: which section it belongs to, a short label, the observed
/// value, and (for non-ok checks) a one-line remediation hint.
struct Check {
    group: &'static str,
    name: &'static str,
    sev: Sev,
    value: String,
    hint: String,
}

impl Check {
    fn ok(group: &'static str, name: &'static str, value: impl Into<String>) -> Self {
        Check {
            group,
            name,
            sev: Sev::Ok,
            value: value.into(),
            hint: String::new(),
        }
    }
    fn warn(
        group: &'static str,
        name: &'static str,
        value: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Check {
            group,
            name,
            sev: Sev::Warn,
            value: value.into(),
            hint: hint.into(),
        }
    }
    fn err(
        group: &'static str,
        name: &'static str,
        value: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Check {
            group,
            name,
            sev: Sev::Err,
            value: value.into(),
            hint: hint.into(),
        }
    }
}

/// Entry point for the `ztmux doctor` subcommand. `socket` is the resolved
/// server socket path (from `-S`/`-L`/default). Returns a process exit code.
pub(crate) fn run(socket: &str) -> i32 {
    let checks = collect(socket);
    let json = std::env::args().any(|a| a == "--json")
        || std::env::args()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|w| w[0] == "-o" && w[1] == "json");

    if json {
        print!("{}", render_json(&checks));
    } else {
        print!("{}", render_text(&checks, std::io::stdout().is_terminal()));
    }

    match checks.iter().map(|c| c.sev).max() {
        Some(Sev::Err) => 2,
        Some(Sev::Warn) => 1,
        _ => 0,
    }
}

// ─── the checks ───────────────────────────────────────────────────────────────

fn collect(socket: &str) -> Vec<Check> {
    let mut c = Vec::new();
    build_checks(&mut c);
    terminal_checks(&mut c);
    server_checks(&mut c, socket);
    limit_checks(&mut c);
    c
}

fn build_checks(c: &mut Vec<Check>) {
    c.push(Check::ok("build", "ztmux", crate::tmux::getversion()));

    // libevent is linked in; report the runtime version. (We deliberately do
    // NOT call event_get_method() here: it dereferences the current event base,
    // which does not exist this early in a client subcommand, and crashes.)
    // SAFETY: event_get_version() returns a static, NUL-terminated C string.
    let ev_ver = unsafe {
        let v = crate::event_::event_get_version();
        if v.is_null() {
            String::from("?")
        } else {
            CStr::from_ptr(v.cast()).to_string_lossy().into_owned()
        }
    };
    c.push(Check::ok("build", "libevent", ev_ver));
    c.push(Check::ok(
        "build",
        "platform",
        format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
    ));
}

fn terminal_checks(c: &mut Vec<Check>) {
    match std::env::var("TERM") {
        Ok(term) if !term.is_empty() => {
            c.push(Check::ok("terminal", "TERM", &term));
            terminfo_check(c, &term);
            let colorterm = std::env::var("COLORTERM").unwrap_or_default();
            let truecolor = colorterm.contains("truecolor") || colorterm.contains("24bit");
            if term.contains("256color") || truecolor {
                c.push(Check::ok(
                    "terminal",
                    "color",
                    if truecolor {
                        "24-bit (COLORTERM)"
                    } else {
                        "256"
                    },
                ));
            } else {
                c.push(Check::warn(
                    "terminal",
                    "color",
                    "8/16",
                    "for 24-bit color set COLORTERM=truecolor or a *-256color TERM",
                ));
            }
        }
        _ => c.push(Check::err(
            "terminal",
            "TERM",
            "(unset)",
            "export TERM=xterm-256color (or your terminal's terminfo entry)",
        )),
    }

    let locale = ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .unwrap_or_default();
    if locale.to_ascii_lowercase().contains("utf-8") || locale.to_ascii_lowercase().contains("utf8")
    {
        c.push(Check::ok("terminal", "locale", &locale));
    } else {
        c.push(Check::warn(
            "terminal",
            "locale",
            if locale.is_empty() {
                "(unset)".into()
            } else {
                locale
            },
            "set LANG/LC_ALL to a UTF-8 locale (e.g. en_US.UTF-8) for wide-char handling",
        ));
    }
}

/// Best-effort: does a terminfo entry for `term` exist in any standard database?
fn terminfo_check(c: &mut Vec<Check>, term: &str) {
    let Some(first) = term.chars().next() else {
        return;
    };
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(d) = std::env::var("TERMINFO") {
        dirs.push(d.into());
    }
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(std::path::Path::new(&home).join(".terminfo"));
    }
    for d in [
        "/usr/share/terminfo",
        "/etc/terminfo",
        "/lib/terminfo",
        "/usr/lib/terminfo",
    ] {
        dirs.push(d.into());
    }
    // Entries are stored under either the first character or its two-digit hex
    // code (the macOS/ncurses layout differs across installs); try both.
    let hex = format!("{:x}", first as u32);
    for d in &dirs {
        for sub in [first.to_string(), hex.clone()] {
            if d.join(&sub).join(term).exists() {
                c.push(Check::ok(
                    "terminal",
                    "terminfo",
                    d.join(sub).join(term).display().to_string(),
                ));
                return;
            }
        }
    }
    c.push(Check::warn(
        "terminal",
        "terminfo",
        format!("no entry for '{term}'"),
        "install the terminfo entry or set TERM to one present (e.g. xterm-256color)",
    ));
}

fn server_checks(c: &mut Vec<Check>, socket: &str) {
    c.push(Check::ok(
        "server",
        "socket",
        if socket.is_empty() {
            "(default namespace)".into()
        } else {
            socket.to_string()
        },
    ));

    // A ztmux socket lives in its own `ztmux-<uid>` namespace; a stray $TMUX means
    // the user is inside a *real* tmux client, a classic source of confusion.
    match std::env::var("ZTMUX") {
        Ok(v) if !v.is_empty() => c.push(Check::ok("server", "$ZTMUX", v)),
        _ => c.push(Check::ok("server", "$ZTMUX", "(unset — not attached)")),
    }
    if let Ok(v) = std::env::var("TMUX")
        && !v.is_empty()
    {
        // $TMUX is `socket,pid,session`. If its socket lives in the ztmux
        // namespace it is ours; a foreign path means a real tmux client, which
        // is the classic point of confusion worth flagging.
        let sock_part = v.split(',').next().unwrap_or(&v);
        if sock_part.contains("ztmux-") {
            c.push(Check::ok("server", "$TMUX", v));
        } else {
            c.push(Check::warn(
                "server",
                "$TMUX",
                v,
                "you are inside a real tmux client; ztmux uses its own ztmux-<uid> namespace",
            ));
        }
    }

    if !socket.is_empty() {
        match std::fs::metadata(socket) {
            Ok(m) if m.file_type().is_socket() => {}
            Ok(_) => c.push(Check::err(
                "server",
                "socket file",
                socket,
                "path exists but is not a socket; remove it or pick another with -S",
            )),
            Err(_) => c.push(Check::warn(
                "server",
                "socket file",
                "missing",
                "no server on this socket yet — start one with `ztmux new-session`",
            )),
        }
    }

    let snap = poll(socket);
    match snap.error {
        Some(e) => c.push(Check::warn(
            "server",
            "reachable",
            e,
            "start one with `ztmux new-session`, then re-run `ztmux doctor`",
        )),
        None => {
            c.push(Check::ok(
                "server",
                "reachable",
                format!(
                    "{} sessions · {} windows · {} panes · {} clients",
                    snap.sessions.len(),
                    snap.windows.len(),
                    snap.panes.len(),
                    snap.clients.len()
                ),
            ));
            // Only touch the server for options once we know it is up, so doctor
            // never *starts* a server as a side effect.
            if let Some(dt) = show_option(socket, "default-terminal") {
                c.push(Check::ok("server", "default-terminal", dt));
            }
        }
    }
}

/// Read a single global server option value, or None on any failure.
fn show_option(socket: &str, name: &str) -> Option<String> {
    let out = ztmux_cmd(socket, &["show-options", "-g", name])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&out.stdout);
    // Format is `name value` or `name "quoted value"`.
    let rest = line.trim().strip_prefix(name)?.trim();
    Some(rest.trim_matches('"').to_string())
}

fn limit_checks(c: &mut Vec<Check>) {
    // SAFETY: getrlimit into a fully-initialised rlimit is always sound.
    let mut rl = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let rc = unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut rl) };
    if rc != 0 {
        return;
    }
    let soft = rl.rlim_cur;
    let hard = rl.rlim_max;
    let value = format!("soft={soft} hard={hard}");
    // A pathologically high soft limit is what made pane spawn hang on macOS
    // (the closefrom() loop walked millions of fds) — see docs/BUGS.md.
    if soft > 1_048_576 {
        c.push(Check::warn(
            "limits",
            "open files",
            value,
            "a very high RLIMIT_NOFILE can slow pane spawn; consider `ulimit -n 65536`",
        ));
    } else {
        c.push(Check::ok("limits", "open files", value));
    }
}

// ─── rendering ─────────────────────────────────────────────────────────────────

fn render_text(checks: &[Check], color: bool) -> String {
    let paint = |s: &str, code: &str| -> String {
        if color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };

    let mut out = String::new();
    out.push_str(&format!("{}\n", paint("ztmux doctor", "1;36")));

    let mut last_group = "";
    for c in checks {
        if c.group != last_group {
            out.push_str(&format!("\n{}\n", paint(c.group, "1")));
            last_group = c.group;
        }
        let sym = paint(c.sev.symbol(), c.sev.color());
        out.push_str(&format!("  {sym} {:<16} {}\n", c.name, c.value));
        if !c.hint.is_empty() {
            out.push_str(&format!("      {}\n", paint(&format!("↳ {}", c.hint), "2")));
        }
    }

    let warns = checks.iter().filter(|c| c.sev == Sev::Warn).count();
    let errs = checks.iter().filter(|c| c.sev == Sev::Err).count();
    let (summary, code) = if errs > 0 {
        (format!("{errs} error(s), {warns} warning(s)"), "31")
    } else if warns > 0 {
        (format!("{warns} warning(s)"), "33")
    } else {
        ("all checks passed".to_string(), "32")
    };
    out.push_str(&format!("\n{}\n", paint(&summary, code)));
    out
}

fn render_json(checks: &[Check]) -> String {
    let arr: Vec<serde_json::Value> = checks
        .iter()
        .map(|c| {
            serde_json::json!({
                "group": c.group,
                "name": c.name,
                "status": c.sev.word(),
                "value": c.value,
                "hint": c.hint,
            })
        })
        .collect();
    let status = match checks.iter().map(|c| c.sev).max() {
        Some(Sev::Err) => "error",
        Some(Sev::Warn) => "warn",
        _ => "ok",
    };
    let v = serde_json::json!({ "status": status, "checks": arr });
    format!("{}\n", serde_json::to_string_pretty(&v).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_severity_is_the_worst_check() {
        let mut c = vec![Check::ok("g", "a", "x")];
        assert_eq!(c.iter().map(|x| x.sev).max(), Some(Sev::Ok));
        c.push(Check::warn("g", "b", "y", "h"));
        assert_eq!(c.iter().map(|x| x.sev).max(), Some(Sev::Warn));
        c.push(Check::err("g", "c", "z", "h"));
        assert_eq!(c.iter().map(|x| x.sev).max(), Some(Sev::Err));
    }

    #[test]
    fn text_report_groups_and_shows_hints() {
        let checks = vec![
            Check::ok("build", "ztmux", "3.7"),
            Check::warn("terminal", "color", "8/16", "set COLORTERM=truecolor"),
        ];
        let s = render_text(&checks, false);
        assert!(s.contains("build"), "group header");
        assert!(s.contains("ztmux") && s.contains("3.7"));
        assert!(s.contains("terminal"));
        assert!(s.contains("↳ set COLORTERM=truecolor"), "hint line");
        assert!(s.contains("1 warning(s)"), "summary");
    }

    #[test]
    fn json_report_is_well_formed() {
        let checks = vec![Check::err("server", "reachable", "no server", "start one")];
        let s = render_json(&checks);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["status"], "error");
        assert_eq!(v["checks"][0]["name"], "reachable");
        assert_eq!(v["checks"][0]["status"], "error");
    }

    #[test]
    fn build_checks_report_real_version_and_libevent() {
        let mut c = Vec::new();
        build_checks(&mut c);
        assert!(c.iter().any(|x| x.name == "ztmux" && !x.value.is_empty()));
        assert!(
            c.iter()
                .any(|x| x.name == "libevent" && !x.value.is_empty())
        );
    }

    // Sev is ordered Ok < Warn < Err (so max picks the worst) and each variant
    // maps to a stable word/symbol/color.
    #[test]
    fn severity_orders_and_maps() {
        assert!(Sev::Ok < Sev::Warn && Sev::Warn < Sev::Err);
        assert_eq!(Sev::Ok.word(), "ok");
        assert_eq!(Sev::Warn.word(), "warn");
        assert_eq!(Sev::Err.word(), "error");
        assert_eq!(Sev::Err.symbol(), "✗");
        assert_eq!(Sev::Ok.color(), "32");
    }

    // With every check ok the JSON status is "ok" and ok checks carry no hint.
    #[test]
    fn json_status_is_ok_when_all_pass() {
        let checks = vec![
            Check::ok("build", "ztmux", "1.0"),
            Check::ok("build", "platform", "x"),
        ];
        let v: serde_json::Value = serde_json::from_str(&render_json(&checks)).unwrap();
        assert_eq!(v["status"], "ok");
        assert_eq!(v["checks"][0]["status"], "ok");
        assert_eq!(v["checks"][0]["hint"], "");
    }

    // An all-ok text report ends with the pass summary and prints no hint lines.
    #[test]
    fn text_all_pass_summary() {
        let checks = vec![Check::ok("g", "a", "v")];
        let s = render_text(&checks, false);
        assert!(s.contains("all checks passed"));
        assert!(!s.contains("↳"));
    }
}
