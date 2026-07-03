//! `ztmux piped` — panes with an active `pipe-pane` capture.
//!
//! `pipe-pane` tees a pane's output to a shell command — logging a session to a
//! file, feeding a monitor, mirroring a build. Once running it is invisible: the
//! pane looks ordinary while everything it prints is also going somewhere else.
//! `piped` finds those panes by reading the `pane_pipe` flag and reports the ones
//! where a pipe is active — the "what am I still logging, and where" check.
//! Panes with no pipe are omitted. With `-o json` / `--json` it emits the same
//! rows as a machine-readable array; a server with none piped prints just the
//! header.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// The `\x1f`-delimited per-pane format the flag is read through.
const FORMAT: &str = "#{pane_id}\u{1f}#{session_name}\u{1f}#{window_index}\u{1f}#{pane_index}\u{1f}#{pane_current_command}\u{1f}#{pane_pipe}";

/// One output row: a pane with an active pipe.
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

/// Parse one formatted line into `(id, location, command, piped)`.
fn parse_line(line: &str) -> Option<(String, String, String, bool)> {
    let mut it = line.split('\u{1f}');
    let id = it.next()?;
    let session = it.next()?;
    let window = it.next()?;
    let pane = it.next()?;
    let command = it.next()?;
    let piped = it.next()? == "1";
    Some((
        id.to_string(),
        format!("{session}:{window}.{pane}"),
        command.to_string(),
        piped,
    ))
}

/// One row per piped pane, ordered by location.
fn build_rows(lines: &[String]) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .filter_map(|l| parse_line(l))
        .filter(|(_, _, _, piped)| *piped)
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
        let (id, loc, cmd, piped) =
            parse_line("%3\u{1f}work\u{1f}1\u{1f}2\u{1f}tail\u{1f}1").unwrap();
        assert_eq!(id, "%3");
        assert_eq!(loc, "work:1.2");
        assert_eq!(cmd, "tail");
        assert!(piped);
    }

    #[test]
    fn only_piped_panes_are_kept() {
        let lines = vec![
            "%0\u{1f}a\u{1f}0\u{1f}0\u{1f}zsh\u{1f}0".to_string(),
            "%1\u{1f}a\u{1f}0\u{1f}1\u{1f}tail\u{1f}1".to_string(),
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%1");
        assert_eq!(rows[0].command, "tail");
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
    fn none_piped_renders_header_only() {
        let lines = vec!["%0\u{1f}a\u{1f}0\u{1f}0\u{1f}zsh\u{1f}0".to_string()];
        let rows = build_rows(&lines);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("PANE") && s.contains("COMMAND"));
    }

    #[test]
    fn json_carries_id_location_command() {
        let lines = vec!["%1\u{1f}a\u{1f}0\u{1f}1\u{1f}tail\u{1f}1".to_string()];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["id"], "%1");
        assert_eq!(v[0]["location"], "a:0.1");
        assert_eq!(v[0]["command"], "tail");
    }
}
