//! `ztmux triggers` — content-triggered automation, no upstream tmux counterpart.
//!
//! tmux once had `monitor-content`: match a pattern in a pane and raise an
//! alert. It was removed in 2.0. `triggers` revives and generalises the idea —
//! from "raise an alert" to "run any ztmux command" — closing the loop between
//! the read-only sensing extensions ([`super::events`], [`super::monitor`],
//! [`super::alerts`], [`super::hooks`]) and *acting* on what they see.
//!
//! A rule is `{ match: <regex>, action: <ztmux command> }` scoped to a pane
//! glob. Rules live in `~/.ztmux/triggers.json`. Sensing is real-time and
//! poll-free: `arm` pipes every in-scope pane's output (via ported `pipe-pane`)
//! into a per-pane matcher (`triggers __match`), which regex-tests each line and
//! dispatches the action when it fires. Fires are debounced per rule and logged
//! to `~/.ztmux/triggers.log`.
//!
//! Subcommands: `list` (default), `arm`, `disarm`, `test <text>`, and the
//! internal `__match <target>` streaming matcher spawned by `pipe-pane`.

use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use super::diagnostics;
use super::tmux_query::{poll, ztmux_cmd};
use crate::libc::{REG_EXTENDED, REG_ICASE, REG_NOSUB, regcomp, regex_t, regexec, regfree};

/// One trigger rule as stored in `~/.ztmux/triggers.json`.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct Rule {
    /// Label used for display, logging, and the per-rule debounce state file.
    name: String,
    /// Glob (only `*` wildcards) matched against a pane's `session:window.index`
    /// target. Defaults to `*` (every pane).
    #[serde(default = "star")]
    pane: String,
    /// POSIX extended regex tested against each (ANSI-stripped) output line.
    #[serde(rename = "match")]
    pattern: String,
    /// ztmux command line run when the regex fires, e.g.
    /// `split-window -h 'less +F ~/.ztmux/build.log'`.
    action: String,
    /// Case-insensitive matching.
    #[serde(default)]
    ignore_case: bool,
    /// Minimum gap between fires of this rule, in milliseconds (0 = no
    /// debounce). Guards against a burst of matching lines re-firing.
    #[serde(default = "default_debounce")]
    debounce_ms: u64,
}

fn star() -> String {
    "*".into()
}
fn default_debounce() -> u64 {
    3000
}

#[derive(Debug, Default, Deserialize, serde::Serialize)]
struct Config {
    #[serde(default)]
    triggers: Vec<Rule>,
}

/// Path to the rule file: `~/.ztmux/triggers.json`.
fn config_path() -> PathBuf {
    diagnostics::dir().join("triggers.json")
}

/// Load and parse the rule file. A missing file is an empty rule set; a parse
/// error is reported to stderr and also treated as empty so a bad edit can
/// never wedge the matcher.
fn load_rules() -> Vec<Rule> {
    let path = config_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    match serde_json::from_str::<Config>(&text) {
        Ok(c) => c.triggers,
        Err(e) => {
            eprintln!("ztmux triggers: {}: {e}", path.display());
            Vec::new()
        }
    }
}

pub(crate) fn run(socket: &str) -> i32 {
    let args: Vec<String> = std::env::args().collect();
    match sub_after(&args, "triggers").as_deref() {
        None | Some("list") | Some("ls") => list(),
        Some("arm") => arm(socket),
        Some("disarm") | Some("off") => disarm(socket),
        Some("add") | Some("new") => add(&args, socket),
        Some("wizard") => wizard(socket),
        Some("test") => test_cmd(&args),
        Some("__match") => match_stream(socket, &args),
        Some(other) => {
            eprintln!("ztmux triggers: unknown subcommand {other:?}");
            eprintln!(
                "usage: ztmux triggers [list|arm|disarm|add <name> <pane> <match> <action>|test <text>]"
            );
            2
        }
    }
}

/// The token immediately following `key` in `args`, if any.
fn sub_after(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1).cloned())
}

// ---------------------------------------------------------------------------
// add — append a rule without hand-editing the JSON (drives the inline wizard)
// ---------------------------------------------------------------------------

/// `ztmux triggers add <name> <pane-glob> <match-regex> <action>` appends a rule
/// to `~/.ztmux/triggers.json` and re-arms. Empty positional fields fall back to
/// sensible defaults (auto name, `*` pane); an empty match or action is an
/// error. This is what the `command-prompt` wizard binding calls.
fn add(args: &[String], socket: &str) -> i32 {
    let Some(start) = args
        .iter()
        .position(|a| a == "add" || a == "new")
        .map(|i| i + 1)
    else {
        return 2;
    };
    let pos = &args[start.min(args.len())..];
    let field = |i: usize| pos.get(i).map(|s| s.trim().to_string()).unwrap_or_default();

    let pattern = field(2);
    let action = field(3);
    if pattern.is_empty() || action.is_empty() {
        eprintln!("ztmux triggers add: both a match regex and an action are required");
        return 2;
    }
    let mut rules = load_rules();
    let name = {
        let n = field(0);
        if n.is_empty() {
            format!("rule{}", rules.len() + 1)
        } else {
            n
        }
    };
    let pane = {
        let p = field(1);
        if p.is_empty() { star() } else { p }
    };

    rules.push(Rule {
        name: name.clone(),
        pane,
        pattern,
        action,
        ignore_case: false,
        debounce_ms: default_debounce(),
    });

    let json = match serde_json::to_string_pretty(&Config { triggers: rules }) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("ztmux triggers add: {e}");
            return 1;
        }
    };
    if let Err(e) = std::fs::write(config_path(), json) {
        eprintln!("ztmux triggers add: {}: {e}", config_path().display());
        return 1;
    }
    println!("added trigger '{name}'");
    // Re-arm so the new rule takes effect immediately.
    arm(socket)
}

/// Open the inline trigger wizard: a chained `command-prompt` that collects the
/// four fields (name, pane glob, match regex, action) through the floating
/// prompt, then calls `triggers add`. No JSON editing. Targets the current
/// client, so run it from a shell inside the session (or bind a key to it).
fn wizard(socket: &str) -> i32 {
    let template = format!("run-shell \"ztmux -S '{socket}' triggers add '%1' '%2' '%3' '%4'\"");
    match ztmux_cmd(
        socket,
        &[
            "command-prompt",
            "-p",
            "trigger name:,pane glob (*):,match regex:,action:",
            &template,
        ],
    )
    .status()
    {
        Ok(s) if s.success() => 0,
        _ => {
            eprintln!("ztmux triggers wizard: could not open the prompt (need an attached client)");
            1
        }
    }
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

fn list() -> i32 {
    let rules = load_rules();
    let path = config_path();
    if rules.is_empty() {
        println!("no triggers configured");
        println!("create {} — for example:", path.display());
        println!("{EXAMPLE}");
        return 0;
    }
    let color = std::io::stdout().is_terminal();
    let bold = |s: &str| {
        if color {
            format!("\x1b[1m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };
    println!(
        "{}",
        bold(&format!(
            "{:<16} {:<14} {:>8}  {}",
            "NAME", "PANE", "DEBOUNCE", "MATCH → ACTION"
        ))
    );
    for r in &rules {
        println!(
            "{:<16} {:<14} {:>7}s  {} → {}",
            r.name,
            r.pane,
            r.debounce_ms / 1000,
            r.pattern,
            r.action
        );
    }
    0
}

const EXAMPLE: &str = r#"{
  "triggers": [
    {
      "name": "build-fail",
      "pane": "*",
      "match": "Error|FAILED|panicked",
      "action": "split-window -h 'less +F ~/.ztmux/triggers.log'",
      "ignore_case": false,
      "debounce_ms": 5000
    }
  ]
}"#;

// ---------------------------------------------------------------------------
// arm / disarm
// ---------------------------------------------------------------------------

/// The `session:window.index` target for a pane, used both as the `pipe-pane`
/// target and as the string the pane globs are matched against.
fn pane_target(p: &super::tmux_query::Pane) -> String {
    format!("{}:{}.{}", p.session, p.window, p.index)
}

fn arm(socket: &str) -> i32 {
    let rules = load_rules();
    if rules.is_empty() {
        println!("no triggers configured ({})", config_path().display());
        return 0;
    }
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux triggers: {e}");
        return 1;
    }
    let mut armed = 0;
    for p in &snap.panes {
        let target = pane_target(p);
        if !rules.iter().any(|r| glob_match(&r.pane, &target)) {
            continue;
        }
        let matcher = matcher_cmd(socket, &target);
        let ok = ztmux_cmd(socket, &["pipe-pane", "-t", &target, &matcher])
            .status()
            .is_ok_and(|s| s.success());
        if ok {
            armed += 1;
        }
    }
    println!("armed {armed} pane(s) against {} rule(s)", rules.len());
    0
}

fn disarm(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux triggers: {e}");
        return 1;
    }
    let mut off = 0;
    for p in &snap.panes {
        let target = pane_target(p);
        // `pipe-pane` with no command closes any existing pipe on the pane.
        if ztmux_cmd(socket, &["pipe-pane", "-t", &target])
            .status()
            .is_ok_and(|s| s.success())
        {
            off += 1;
        }
    }
    println!("disarmed {off} pane(s)");
    0
}

/// The shell command `pipe-pane` runs for a pane: our own binary re-invoked as
/// the per-pane streaming matcher. `pipe-pane` feeds the pane's output to this
/// command's stdin.
fn matcher_cmd(socket: &str, target: &str) -> String {
    let exe = std::env::current_exe()
        .map_or_else(|_| "ztmux".into(), |p| p.to_string_lossy().into_owned());
    if socket.is_empty() {
        format!("{} triggers __match {}", shq(&exe), shq(target))
    } else {
        format!(
            "{} -S {} triggers __match {}",
            shq(&exe),
            shq(socket),
            shq(target)
        )
    }
}

// ---------------------------------------------------------------------------
// __match — the per-pane streaming matcher (spawned by pipe-pane)
// ---------------------------------------------------------------------------

fn match_stream(socket: &str, args: &[String]) -> i32 {
    let target = sub_after(args, "__match").unwrap_or_default();
    // Only rules whose glob matches this pane; compile each regex once.
    let compiled: Vec<(Rule, Regex)> = load_rules()
        .into_iter()
        .filter(|r| glob_match(&r.pane, &target))
        .filter_map(|r| Regex::compile(&r.pattern, r.ignore_case).map(|re| (r, re)))
        .collect();
    if compiled.is_empty() {
        return 0;
    }
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut buf: Vec<u8> = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break, // pane closed → stream ended
            Ok(_) => {
                let line = strip_ansi(&buf);
                for (rule, re) in &compiled {
                    if re.is_match(&line) && debounce_ok(rule) {
                        dispatch(socket, rule, &target);
                    }
                }
            }
            Err(_) => break,
        }
    }
    0
}

/// Run a rule's action as a detached ztmux command and append a line to
/// `~/.ztmux/triggers.log`. The action is a ztmux command line, tokenised by
/// `/bin/sh` (matching how `pipe-pane`/`run-shell` treat commands), and
/// backgrounded so a slow action never stalls the output stream.
fn dispatch(socket: &str, rule: &Rule, target: &str) {
    let exe = std::env::current_exe()
        .map_or_else(|_| "ztmux".into(), |p| p.to_string_lossy().into_owned());
    let prefix = if socket.is_empty() {
        shq(&exe)
    } else {
        format!("{} -S {}", shq(&exe), shq(socket))
    };
    let line = format!("{prefix} {} &", rule.action);
    let _ = Command::new("/bin/sh").arg("-c").arg(&line).status();

    let entry = format!(
        "{} {} [{}] fired: {}\n",
        now_ms(),
        rule.name,
        target,
        rule.action
    );
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(diagnostics::dir().join("triggers.log"))
    {
        let _ = f.write_all(entry.as_bytes());
    }
}

// ---------------------------------------------------------------------------
// test
// ---------------------------------------------------------------------------

fn test_cmd(args: &[String]) -> i32 {
    let text = args
        .iter()
        .position(|a| a == "test")
        .map(|i| args[i + 1..].join(" "))
        .unwrap_or_default();
    let rules = load_rules();
    if rules.is_empty() {
        println!("no triggers configured ({})", config_path().display());
        return 0;
    }
    for r in &rules {
        let hit = Regex::compile(&r.pattern, r.ignore_case).is_some_and(|re| re.is_match(&text));
        println!("{} {}", if hit { "MATCH" } else { "  -  " }, r.name);
    }
    0
}

// ---------------------------------------------------------------------------
// debounce
// ---------------------------------------------------------------------------

/// True if `rule` may fire now: the gap since its last fire is at least
/// `debounce_ms`. Records the new fire time. Best-effort and file-backed so it
/// holds across the independent per-pane matcher processes.
fn debounce_ok(rule: &Rule) -> bool {
    if rule.debounce_ms == 0 {
        return true;
    }
    let dir = diagnostics::dir().join("triggers-state");
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join(sanitize(&rule.name));
    let now = now_ms();
    if let Ok(s) = std::fs::read_to_string(&file)
        && let Ok(last) = s.trim().parse::<u64>()
        && now.saturating_sub(last) < rule.debounce_ms
    {
        return false;
    }
    let _ = std::fs::write(&file, now.to_string());
    true
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Reduce a rule name to a safe file-name stem for the debounce state file.
fn sanitize(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() { "_".into() } else { s }
}

// ---------------------------------------------------------------------------
// helpers: glob, shell-quote, ANSI strip, regex
// ---------------------------------------------------------------------------

/// Minimal glob match supporting only `*` (matches any run, including empty).
/// Enough for pane targets like `work:*` or `*`.
fn glob_match(pat: &str, text: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (pat.chars().collect(), text.chars().collect());
    // Classic two-pointer wildcard match with backtracking on the last `*`.
    let (mut pi, mut ti, mut star, mut mark) = (0usize, 0usize, None::<usize>, 0usize);
    while ti < t.len() {
        if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if pi < p.len() && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Single-quote a string for `/bin/sh`.
fn shq(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Strip terminal escape sequences (CSI, OSC, and lone two-byte escapes) so the
/// regex matches the visible text, then trim trailing CR/LF. Pane output from
/// `pipe-pane` is raw, so without this a colour code could sit between the
/// characters a pattern expects.
fn strip_ansi(bytes: &[u8]) -> String {
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'[' => {
                    // CSI: ESC [ ... final byte in 0x40..=0x7e
                    i += 2;
                    while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                        i += 1;
                    }
                    i += 1;
                }
                b']' => {
                    // OSC: ESC ] ... terminated by BEL or ESC \
                    i += 2;
                    while i < bytes.len() && bytes[i] != 0x07 {
                        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                            i += 1;
                            break;
                        }
                        i += 1;
                    }
                    i += 1;
                }
                _ => i += 2, // lone ESC + one byte
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    let s = String::from_utf8_lossy(&out);
    s.trim_end_matches(['\r', '\n']).to_string()
}

/// RAII wrapper over a compiled POSIX extended regex (`regcomp`/`regexec`), the
/// same engine tmux uses for pane search. Compiled with `REG_NOSUB` since we
/// only need a boolean match.
struct Regex {
    re: regex_t,
}

impl Regex {
    fn compile(pattern: &str, ignore_case: bool) -> Option<Regex> {
        let mut cpat: Vec<u8> = pattern
            .bytes()
            .map(|b| if b == 0 { b' ' } else { b })
            .collect();
        cpat.push(0);
        let mut flags = REG_EXTENDED | REG_NOSUB;
        if ignore_case {
            flags |= REG_ICASE;
        }
        // SAFETY: zeroed regex_t is the documented pre-compile state; regcomp
        // fully initialises it on success. cpat is NUL-terminated.
        let mut re: regex_t = unsafe { std::mem::zeroed() };
        if unsafe { regcomp(&raw mut re, cpat.as_ptr(), flags) } != 0 {
            return None;
        }
        Some(Regex { re })
    }

    fn is_match(&self, text: &str) -> bool {
        // Interior NULs would truncate the C string, so map them to spaces.
        let mut c: Vec<u8> = text
            .bytes()
            .map(|b| if b == 0 { b' ' } else { b })
            .collect();
        c.push(0);
        // SAFETY: self.re is a live compiled regex; c is NUL-terminated; no
        // submatches requested (nmatch 0, null pmatch).
        unsafe { regexec(&raw const self.re, c.as_ptr(), 0, std::ptr::null_mut(), 0) == 0 }
    }
}

impl Drop for Regex {
    fn drop(&mut self) {
        // SAFETY: self.re was produced by a successful regcomp.
        unsafe { regfree(&raw mut self.re) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_star_matches() {
        assert!(glob_match("*", "work:1.0"));
        assert!(glob_match("work:*", "work:1.0"));
        assert!(glob_match("*:1.0", "work:1.0"));
        assert!(glob_match("w*k:*.0", "work:1.0"));
        assert!(!glob_match("play:*", "work:1.0"));
        assert!(!glob_match("work", "work:1.0"));
        assert!(glob_match("work", "work"));
    }

    #[test]
    fn shell_quote_escapes_quotes() {
        assert_eq!(shq("abc"), "'abc'");
        assert_eq!(shq("a'b"), r"'a'\''b'");
    }

    #[test]
    fn strip_ansi_removes_csi_and_osc() {
        assert_eq!(strip_ansi(b"\x1b[31mError\x1b[0m\n"), "Error");
        assert_eq!(strip_ansi(b"\x1b]0;title\x07plain"), "plain");
        assert_eq!(strip_ansi(b"just text\r\n"), "just text");
    }

    #[test]
    fn regex_matches_extended_and_case() {
        let re = Regex::compile("Error|FAILED", false).unwrap();
        assert!(re.is_match("build Error here"));
        assert!(re.is_match("FAILED"));
        assert!(!re.is_match("all good"));

        let ci = Regex::compile("error", true).unwrap();
        assert!(ci.is_match("ERROR"));
        assert!(!Regex::compile("error", false).unwrap().is_match("ERROR"));
    }

    #[test]
    fn bad_regex_is_none() {
        assert!(Regex::compile("(unclosed", false).is_none());
    }

    #[test]
    fn config_parses_with_defaults() {
        let cfg: Config = serde_json::from_str(
            r#"{"triggers":[{"name":"x","match":"foo","action":"display-message hi"}]}"#,
        )
        .unwrap();
        assert_eq!(cfg.triggers.len(), 1);
        let r = &cfg.triggers[0];
        assert_eq!(r.pane, "*");
        assert_eq!(r.debounce_ms, 3000);
        assert!(!r.ignore_case);
    }

    #[test]
    fn sanitize_keeps_safe_chars() {
        assert_eq!(sanitize("build-fail_1"), "build-fail_1");
        assert_eq!(sanitize("a/b c"), "a_b_c");
        assert_eq!(sanitize(""), "_");
    }
}
