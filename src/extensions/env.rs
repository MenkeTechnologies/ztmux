//! `ztmux env` — the environment variables set per-session, over the global one.
//!
//! Each session carries its own environment (seeded by `update-environment` on
//! attach and edited with `set-environment -t`). `env` compares every session's
//! environment against the server's global environment and prints only the
//! variables that differ — the per-session overrides. It is the "why does this
//! session's shell behave differently" view; nothing else in the toolchain
//! surfaces the environment. Output is a table (coloured on a TTY) or a JSON
//! array with `-o json` / `--json`, sorted by session then variable.

use std::collections::HashMap;
use std::io::IsTerminal;

use super::tmux_query::{poll, ztmux_cmd};

/// One output row: a session's override of one environment variable.
struct Row {
    session: String,
    var: String,
    value: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux env: {e}");
        return 1;
    }
    let global = env_map(socket, &["show-environment", "-g"]);
    let mut rows: Vec<Row> = Vec::new();
    for s in &snap.sessions {
        let senv = env_map(socket, &["show-environment", "-t", &s.name]);
        rows.extend(diff_rows(&s.name, &global, &senv));
    }
    rows.sort_by(|a, b| a.session.cmp(&b.session).then(a.var.cmp(&b.var)));

    let json = std::env::args().any(|a| a == "--json")
        || std::env::args()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|w| w[0] == "-o" && w[1] == "json");
    if json {
        print!("{}", render_json(&rows));
    } else {
        print!("{}", render_text(&rows, std::io::stdout().is_terminal()));
    }
    0
}

/// Run `show-environment` against our own binary and parse its output. Empty on
/// any failure so the extension degrades to fewer rows.
fn env_map(socket: &str, args: &[&str]) -> HashMap<String, String> {
    let Ok(out) = ztmux_cmd(socket, args).output() else {
        return HashMap::new();
    };
    if !out.status.success() {
        return HashMap::new();
    }
    parse_env(&String::from_utf8_lossy(&out.stdout))
}

/// Parse `show-environment` output: `VAR=value` lines become entries; `-VAR`
/// (unset markers) and lines without `=` are ignored. The value keeps any `=`
/// after the first.
fn parse_env(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter(|l| !l.starts_with('-'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// The variables in `session` whose value differs from `global` (including those
/// absent from the global environment) — the session's overrides.
fn diff_rows(
    session: &str,
    global: &HashMap<String, String>,
    senv: &HashMap<String, String>,
) -> Vec<Row> {
    senv.iter()
        .filter(|(k, v)| global.get(*k) != Some(*v))
        .map(|(k, v)| Row {
            session: session.to_string(),
            var: k.clone(),
            value: v.clone(),
        })
        .collect()
}

fn render_text(rows: &[Row], color: bool) -> String {
    let paint = |s: &str, code: &str| -> String {
        if color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };
    let mut out = String::new();
    out.push_str(&format!(
        "{}\n",
        paint(
            &format!("{:<20} {:<24} {}", "SESSION", "VARIABLE", "VALUE"),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!("{:<20} {:<24} {}\n", r.session, r.var, r.value));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "session": r.session,
                "variable": r.var,
                "value": r.value,
            })
        })
        .collect();
    format!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::Value::Array(arr)).unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn parse_env_keeps_set_vars_and_skips_unset_markers() {
        let m = parse_env("PATH=/usr/bin\n-TMOUT\nEDITOR=nvim\nnoequals\n");
        assert_eq!(m.len(), 2);
        assert_eq!(m.get("PATH").map(String::as_str), Some("/usr/bin"));
        assert_eq!(m.get("EDITOR").map(String::as_str), Some("nvim"));
        assert!(!m.contains_key("TMOUT"));
    }

    #[test]
    fn parse_env_value_keeps_later_equals() {
        let m = parse_env("OPTS=a=b=c\n");
        assert_eq!(m.get("OPTS").map(String::as_str), Some("a=b=c"));
    }

    #[test]
    fn diff_reports_only_changed_or_new_vars() {
        let global = map(&[("PATH", "/usr/bin"), ("LANG", "en_US")]);
        let senv = map(&[
            ("PATH", "/usr/bin"), // same → excluded
            ("LANG", "de_DE"),    // changed → included
            ("PROJECT", "ztmux"), // new → included
        ]);
        let mut rows = diff_rows("dev", &global, &senv);
        rows.sort_by(|a, b| a.var.cmp(&b.var));
        assert_eq!(
            rows.iter()
                .map(|r| (r.var.as_str(), r.value.as_str()))
                .collect::<Vec<_>>(),
            vec![("LANG", "de_DE"), ("PROJECT", "ztmux")],
        );
        assert!(rows.iter().all(|r| r.session == "dev"));
    }

    #[test]
    fn identical_environment_yields_no_rows() {
        let g = map(&[("A", "1"), ("B", "2")]);
        assert!(diff_rows("s", &g, &g).is_empty());
    }

    #[test]
    fn text_has_header_and_override() {
        let global = map(&[("LANG", "en_US")]);
        let senv = map(&[("LANG", "de_DE")]);
        let s = render_text(&diff_rows("dev", &global, &senv), false);
        assert!(s.contains("SESSION") && s.contains("VARIABLE") && s.contains("VALUE"));
        assert!(
            s.lines()
                .any(|l| l.contains("dev") && l.contains("LANG") && l.contains("de_DE"))
        );
    }

    #[test]
    fn json_carries_override_fields() {
        let global = map(&[("LANG", "en_US")]);
        let senv = map(&[("PROJECT", "ztmux")]);
        let v: serde_json::Value =
            serde_json::from_str(&render_json(&diff_rows("dev", &global, &senv))).unwrap();
        assert_eq!(v[0]["session"], "dev");
        assert_eq!(v[0]["variable"], "PROJECT");
        assert_eq!(v[0]["value"], "ztmux");
    }
}
