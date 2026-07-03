//! `ztmux input` — panes that are ignoring keyboard input.
//!
//! A pane's input can be turned off — a read-only observer, a pane a script left
//! with `pane_input_off` set — so it displays output but silently drops every
//! keystroke. That is baffling from the keyboard: you type and nothing happens.
//! `input` finds those panes by reading the `pane_input_off` flag and reports the
//! ones where input is disabled — the "why won't this pane take my typing" check.
//! Panes accepting input are omitted. With `-o json` / `--json` it emits the same
//! rows as a machine-readable array; a server where every pane accepts input
//! prints just the header.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// The `\x1f`-delimited per-pane format the flag is read through.
const FORMAT: &str = "#{pane_id}\u{1f}#{session_name}\u{1f}#{window_index}\u{1f}#{pane_index}\u{1f}#{pane_current_command}\u{1f}#{pane_input_off}";

/// One output row: a pane with input disabled.
struct Row {
    id: String,
    location: String,
    command: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let lines = query_lines(socket, &["list-panes", "-a", "-F", FORMAT]);
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

/// Parse one formatted line into `(id, location, command, input_off)`.
fn parse_line(line: &str) -> Option<(String, String, String, bool)> {
    let mut it = line.split('\u{1f}');
    let id = it.next()?;
    let session = it.next()?;
    let window = it.next()?;
    let pane = it.next()?;
    let command = it.next()?;
    let input_off = it.next()? == "1";
    Some((
        id.to_string(),
        format!("{session}:{window}.{pane}"),
        command.to_string(),
        input_off,
    ))
}

/// One row per input-disabled pane, ordered by location.
fn build_rows(lines: &[String]) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .filter_map(|l| parse_line(l))
        .filter(|(_, _, _, input_off)| *input_off)
        .map(|(id, location, command, _)| Row {
            id,
            location,
            command,
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
            &format!("{:<8} {:<16} {}", "PANE", "LOCATION", "COMMAND"),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!("{:<8} {:<16} {}\n", r.id, r.location, r.command));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "location": r.location,
                "command": r.command,
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
        let (id, loc, cmd, off) = parse_line("%2\u{1f}a\u{1f}1\u{1f}0\u{1f}less\u{1f}1").unwrap();
        assert_eq!(id, "%2");
        assert_eq!(loc, "a:1.0");
        assert_eq!(cmd, "less");
        assert!(off);
    }

    #[test]
    fn only_input_disabled_panes_are_kept() {
        let lines = vec![
            "%0\u{1f}a\u{1f}0\u{1f}0\u{1f}zsh\u{1f}0".to_string(),
            "%1\u{1f}a\u{1f}0\u{1f}1\u{1f}less\u{1f}1".to_string(),
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%1");
    }

    #[test]
    fn rows_sorted_by_location() {
        let lines = vec![
            "%9\u{1f}z\u{1f}9\u{1f}0\u{1f}a\u{1f}1".to_string(),
            "%1\u{1f}a\u{1f}0\u{1f}0\u{1f}b\u{1f}1".to_string(),
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows[0].location, "a:0.0");
        assert_eq!(rows[1].location, "z:9.0");
    }

    #[test]
    fn none_disabled_renders_header_only() {
        let lines = vec!["%0\u{1f}a\u{1f}0\u{1f}0\u{1f}zsh\u{1f}0".to_string()];
        let rows = build_rows(&lines);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("PANE") && s.contains("COMMAND"));
    }

    #[test]
    fn json_carries_id_location_command() {
        let lines = vec!["%1\u{1f}a\u{1f}0\u{1f}1\u{1f}less\u{1f}1".to_string()];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["id"], "%1");
        assert_eq!(v[0]["location"], "a:0.1");
        assert_eq!(v[0]["command"], "less");
    }
}
