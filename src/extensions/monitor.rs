//! `ztmux monitor` — windows armed to alert on activity or silence.
//!
//! A window can be set to watch itself: `monitor-activity` raises an alert when
//! any output appears, `monitor-silence` when none appears for N seconds. Where
//! [`super::alerts`] shows the windows that have *already* fired, `monitor` shows
//! the ones *armed* to fire — the standing watches you set and may have forgotten
//! about. It reads the two options for every window and reports the ones with
//! either armed (the always-on-by-default bell monitor is deliberately left out,
//! since it would match nearly everything). It is the "what am I still watching"
//! view. With `-o json` / `--json` it emits the same rows as a machine-readable
//! array; a server with nothing armed prints just the header.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// The `\x1f`-delimited per-window format the options are read through.
const FORMAT: &str = "#{session_name}\u{1f}#{window_index}\u{1f}#{window_name}\u{1f}#{monitor-activity}\u{1f}#{monitor-silence}";

/// One output row: a window with a standing activity/silence monitor.
struct Row {
    location: String, // session:index
    name: String,
    activity: bool,
    silence: i64, // seconds, 0 = off
}

pub(crate) fn run(socket: &str) -> i32 {
    let lines = query_lines(socket, &["list-windows", "-a", "-F", FORMAT]);
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

/// Parse one formatted line into `(location, name, activity, silence_seconds)`.
/// `monitor-activity` is `1`/`0`; `monitor-silence` is a second count (`0` when
/// off).
fn parse_line(line: &str) -> Option<(String, String, bool, i64)> {
    let mut it = line.split('\u{1f}');
    let session = it.next()?;
    let index = it.next()?;
    let name = it.next()?;
    let activity = it.next()? == "1";
    let silence: i64 = it.next()?.parse().unwrap_or(0);
    Some((
        format!("{session}:{index}"),
        name.to_string(),
        activity,
        silence,
    ))
}

/// One row per window with activity or silence monitoring armed, ordered by
/// location.
fn build_rows(lines: &[String]) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .filter_map(|l| parse_line(l))
        .filter(|(_, _, activity, silence)| *activity || *silence > 0)
        .map(|(location, name, activity, silence)| Row {
            location,
            name,
            activity,
            silence,
        })
        .collect();
    rows.sort_by(|a, b| a.location.cmp(&b.location));
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
        paint(
            &format!(
                "{:<12} {:<16} {:<9} {}",
                "WINDOW", "NAME", "ACTIVITY", "SILENCE"
            ),
            "1"
        )
    ));
    for r in rows {
        let silence = if r.silence > 0 {
            format!("{}s", r.silence)
        } else {
            "-".to_string()
        };
        out.push_str(&format!(
            "{:<12} {:<16} {:<9} {}\n",
            r.location,
            r.name,
            if r.activity { "yes" } else { "no" },
            silence,
        ));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "window": r.location,
                "name": r.name,
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
    fn parses_activity_and_silence() {
        let (loc, name, act, sil) = parse_line("a\u{1f}2\u{1f}build\u{1f}1\u{1f}30").unwrap();
        assert_eq!(loc, "a:2");
        assert_eq!(name, "build");
        assert!(act);
        assert_eq!(sil, 30);
    }

    #[test]
    fn only_armed_windows_are_kept() {
        let lines = vec![
            "a\u{1f}0\u{1f}idle\u{1f}0\u{1f}0".to_string(), // nothing armed
            "a\u{1f}1\u{1f}act\u{1f}1\u{1f}0".to_string(),  // activity armed
            "a\u{1f}2\u{1f}sil\u{1f}0\u{1f}15".to_string(), // silence armed
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].location, "a:1");
        assert_eq!(rows[1].location, "a:2");
        assert_eq!(rows[1].silence, 15);
    }

    #[test]
    fn nothing_armed_renders_header_only() {
        let lines = vec!["a\u{1f}0\u{1f}idle\u{1f}0\u{1f}0".to_string()];
        let rows = build_rows(&lines);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("WINDOW") && s.contains("SILENCE"));
    }

    #[test]
    fn json_carries_activity_and_silence() {
        let lines = vec!["a\u{1f}1\u{1f}act\u{1f}1\u{1f}30".to_string()];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["window"], "a:1");
        assert_eq!(v[0]["activity"], true);
        assert_eq!(v[0]["silence"], 30);
    }
}
