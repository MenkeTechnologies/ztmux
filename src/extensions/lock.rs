//! `ztmux lock` — sessions set to lock themselves after idle time.
//!
//! With `lock-after-time` set to N seconds, a session locks its screen (running
//! `lock-command`) after that long with no input — a small security measure for
//! a session left on a shared or unattended machine. It is off (`0`) by default
//! and easy to forget you set. `lock` reports the sessions where it is armed and
//! after how long, soonest-locking first. Nothing else surfaces the setting.
//! Sessions with no lock timeout are omitted. With `-o json` / `--json` it emits
//! the same rows (timeout in seconds) as a machine-readable array; a server with
//! none armed prints just the header.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// The `\x1f`-delimited per-session format the option is read through.
const FORMAT: &str = "#{session_name}\u{1f}#{lock-after-time}";

/// One output row: a session and its idle-lock timeout.
struct Row {
    session: String,
    seconds: i64,
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

/// Parse one formatted line into `(session, lock_after_seconds)`. `0` means the
/// option is off.
fn parse_line(line: &str) -> Option<(String, i64)> {
    let mut it = line.split('\u{1f}');
    let session = it.next()?;
    let seconds: i64 = it.next()?.parse().ok()?;
    Some((session.to_string(), seconds))
}

/// Format a duration in seconds as a compact single unit (`45s`, `5m`, `2h`).
fn human(secs: i64) -> String {
    if secs >= 3_600 {
        format!("{}h", secs / 3_600)
    } else if secs >= 60 {
        format!("{}m", secs / 60)
    } else {
        format!("{secs}s")
    }
}

/// One row per session with a lock timeout, soonest-locking (smallest timeout)
/// first; ties break by session name.
fn build_rows(lines: &[String]) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .filter_map(|l| parse_line(l))
        .filter(|(_, secs)| *secs > 0)
        .map(|(session, seconds)| Row { session, seconds })
        .collect();
    rows.sort_by(|a, b| a.seconds.cmp(&b.seconds).then(a.session.cmp(&b.session)));
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
    let mut out = String::new();
    out.push_str(&format!(
        "{}\n",
        paint(&format!("{:>10} {}", "LOCKS-AFTER", "SESSION"), "1")
    ));
    for r in rows {
        out.push_str(&format!("{:>10} {}\n", human(r.seconds), r.session));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "session": r.session,
                "seconds": r.seconds,
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
    fn parses_a_formatted_line() {
        assert_eq!(parse_line("work\u{1f}300"), Some(("work".to_string(), 300)));
        assert_eq!(parse_line("bad"), None);
    }

    #[test]
    fn human_scales_units() {
        assert_eq!(human(45), "45s");
        assert_eq!(human(300), "5m");
        assert_eq!(human(7200), "2h");
    }

    #[test]
    fn only_armed_sessions_soonest_first() {
        let lines = vec![
            "off\u{1f}0".to_string(),
            "slow\u{1f}600".to_string(),
            "fast\u{1f}60".to_string(),
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].session, "fast"); // 60s locks soonest
        assert_eq!(rows[1].session, "slow");
    }

    #[test]
    fn none_armed_renders_header_only() {
        let lines = vec!["a\u{1f}0".to_string()];
        let rows = build_rows(&lines);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("SESSION") && s.contains("LOCKS-AFTER"));
    }

    #[test]
    fn json_carries_seconds() {
        let lines = vec!["work\u{1f}300".to_string()];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["session"], "work");
        assert_eq!(v[0]["seconds"], 300);
    }
}
