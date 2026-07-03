//! `ztmux visual` — sessions that show alerts visually instead of just beeping.
//!
//! When a window raises a bell, activity, or silence alert, a session can show a
//! message on the status line rather than (or as well as) passing a terminal
//! bell through — controlled by `visual-bell`, `visual-activity`, and
//! `visual-silence`, all off by default. `visual` reports the sessions where any
//! of these is on, and which. Where [`super::monitor`] shows which windows are
//! *armed* to alert and [`super::alerts`] shows which have *fired*, `visual`
//! shows how a session *presents* an alert when it fires. Sessions using the
//! default (silent/bell-only) are omitted. With `-o json` / `--json` it emits the
//! same rows as a machine-readable array; a server with none set prints just the
//! header.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// The `\x1f`-delimited per-session format the three options are read through.
const FORMAT: &str =
    "#{session_name}\u{1f}#{visual-bell}\u{1f}#{visual-activity}\u{1f}#{visual-silence}";

/// One output row: a session and which alerts it shows visually.
struct Row {
    session: String,
    bell: bool,
    activity: bool,
    silence: bool,
}

pub(crate) fn run(socket: &str) -> i32 {
    let lines = query_lines(socket, &["list-sessions", "-F", FORMAT]);
    let rows = build_rows(&lines);
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

/// Interpret a `visual-*` option value: `on` (or `both`) counts as visual, `off`
/// does not.
fn is_on(value: &str) -> bool {
    value == "on" || value == "both"
}

/// Parse one formatted line into `(session, bell, activity, silence)`.
fn parse_line(line: &str) -> Option<(String, bool, bool, bool)> {
    let mut it = line.split('\u{1f}');
    let session = it.next()?;
    let bell = is_on(it.next()?);
    let activity = is_on(it.next()?);
    let silence = is_on(it.next()?);
    Some((session.to_string(), bell, activity, silence))
}

/// One row per session with at least one `visual-*` option on, ordered by
/// session name.
fn build_rows(lines: &[String]) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .filter_map(|l| parse_line(l))
        .filter(|(_, b, a, s)| *b || *a || *s)
        .map(|(session, bell, activity, silence)| Row {
            session,
            bell,
            activity,
            silence,
        })
        .collect();
    rows.sort_by(|a, b| a.session.cmp(&b.session));
    rows
}

fn render_text(rows: &[Row], color: bool) -> String {
    let paint = |s: &str, code: &str| -> String {
        if color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };
    let yn = |b: bool| if b { "yes" } else { "no" };
    let mut out = String::new();
    out.push_str(&format!(
        "{}\n",
        paint(
            &format!(
                "{:<12} {:<5} {:<9} {}",
                "SESSION", "BELL", "ACTIVITY", "SILENCE"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<12} {:<5} {:<9} {}\n",
            r.session,
            yn(r.bell),
            yn(r.activity),
            yn(r.silence),
        ));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "session": r.session,
                "bell": r.bell,
                "activity": r.activity,
                "silence": r.silence,
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

    #[test]
    fn on_and_both_count_as_visual() {
        assert!(is_on("on"));
        assert!(is_on("both"));
        assert!(!is_on("off"));
    }

    #[test]
    fn parses_a_formatted_line() {
        let (s, b, a, sil) = parse_line("work\u{1f}on\u{1f}off\u{1f}both").unwrap();
        assert_eq!(s, "work");
        assert!(b);
        assert!(!a);
        assert!(sil);
    }

    #[test]
    fn only_sessions_with_any_visual_are_kept() {
        let lines = vec![
            "none\u{1f}off\u{1f}off\u{1f}off".to_string(),
            "bell\u{1f}on\u{1f}off\u{1f}off".to_string(),
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].session, "bell");
        assert!(rows[0].bell);
    }

    #[test]
    fn none_set_renders_header_only() {
        let lines = vec!["a\u{1f}off\u{1f}off\u{1f}off".to_string()];
        let rows = build_rows(&lines);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("SESSION") && s.contains("SILENCE"));
    }

    #[test]
    fn json_carries_flags() {
        let lines = vec!["a\u{1f}on\u{1f}on\u{1f}off".to_string()];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["bell"], true);
        assert_eq!(v[0]["activity"], true);
        assert_eq!(v[0]["silence"], false);
    }
}
