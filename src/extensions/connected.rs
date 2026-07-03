//! `ztmux connected` — attached clients ranked by how long they have been
//! connected.
//!
//! Each client records when it attached. `connected` reads that and ranks the
//! clients by connection age, longest-connected first: the `attach` you left
//! open on a server three days ago, the paired session someone joined this
//! morning. Where [`super::idle`] ranks by time since *last activity* (staleness
//! of use), `connected` ranks by time since *attach* (age of the connection) —
//! two different questions about the same client. A compact relative age is shown
//! for the terminal; with `-o json` / `--json` the raw unix attach timestamp and
//! uptime seconds are emitted.

use std::io::IsTerminal;
use std::time::{SystemTime, UNIX_EPOCH};

use super::tmux_query::query_lines;

/// The `\x1f`-delimited per-client format the attach time is read through.
const FORMAT: &str = "#{client_name}\u{1f}#{client_session}\u{1f}#{client_created}";

/// One output row: a client and when it attached.
struct Row {
    client: String,
    session: String,
    created: i64, // unix seconds the client attached
}

pub(crate) fn run(socket: &str) -> i32 {
    let lines = query_lines(socket, &["list-clients", "-F", FORMAT]);
    let rows = build_rows(&lines);
    let json = std::env::args().any(|a| a == "--json")
        || std::env::args()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|w| w[0] == "-o" && w[1] == "json");
    if json {
        print!("{}", render_json(&rows, now_unix()));
    } else {
        print!(
            "{}",
            render_text(&rows, now_unix(), std::io::stdout().is_terminal())
        );
    }
    0
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// Parse one formatted line into `(client, session, created_unix)`.
fn parse_line(line: &str) -> Option<(String, String, i64)> {
    let mut it = line.split('\u{1f}');
    let client = it.next()?;
    let session = it.next()?;
    let created: i64 = it.next()?.parse().ok()?;
    Some((client.to_string(), session.to_string(), created))
}

/// One row per attached client, longest-connected (oldest attach) first; ties
/// break by client name.
fn build_rows(lines: &[String]) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .filter_map(|l| parse_line(l))
        .map(|(client, session, created)| Row {
            client,
            session,
            created,
        })
        .collect();
    rows.sort_by(|a, b| a.created.cmp(&b.created).then(a.client.cmp(&b.client)));
    rows
}

/// Format a duration in seconds as a compact relative age. Matches the
/// single-unit rendering used by [`super::recent`].
fn ago(secs: i64) -> String {
    if secs <= 0 {
        return "just now".to_string();
    }
    let d = secs / 86_400;
    let h = secs / 3_600;
    let m = secs / 60;
    if d > 0 {
        format!("{d}d")
    } else if h > 0 {
        format!("{h}h")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{secs}s")
    }
}

fn render_text(rows: &[Row], now: i64, color: bool) -> String {
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
            &format!("{:<18} {:<12} {}", "CLIENT", "SESSION", "UPTIME"),
            "1"
        )
    ));
    for r in rows {
        let uptime = if r.created <= 0 {
            "-".to_string()
        } else {
            ago(now - r.created)
        };
        out.push_str(&format!("{:<18} {:<12} {}\n", r.client, r.session, uptime));
    }
    out
}

fn render_json(rows: &[Row], now: i64) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "client": r.client,
                "session": r.session,
                "created": r.created,
                "uptime": if r.created > 0 { (now - r.created).max(0) } else { -1 },
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
        let (c, s, created) = parse_line("/dev/ttys1\u{1f}work\u{1f}1700000000").unwrap();
        assert_eq!(c, "/dev/ttys1");
        assert_eq!(s, "work");
        assert_eq!(created, 1_700_000_000);
    }

    #[test]
    fn longest_connected_sorts_first() {
        let lines = vec![
            "/dev/ttys1\u{1f}a\u{1f}9000".to_string(), // attached later
            "/dev/ttys2\u{1f}b\u{1f}1000".to_string(), // attached earlier
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows[0].client, "/dev/ttys2"); // oldest attach first
        assert_eq!(rows[1].client, "/dev/ttys1");
    }

    #[test]
    fn text_renders_relative_uptime_against_now() {
        let lines = vec!["/dev/ttys1\u{1f}a\u{1f}9000".to_string()];
        let rows = build_rows(&lines);
        // now = 12_600 -> 3_600s -> "1h".
        let s = render_text(&rows, 12_600, false);
        assert!(s.contains("CLIENT") && s.contains("UPTIME"));
        assert!(
            s.lines()
                .any(|l| l.contains("/dev/ttys1") && l.contains("1h"))
        );
    }

    #[test]
    fn json_carries_created_and_uptime() {
        let lines = vec!["/dev/ttys1\u{1f}a\u{1f}9000".to_string()];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows, 12_600)).unwrap();
        assert_eq!(v[0]["created"], 9_000);
        assert_eq!(v[0]["uptime"], 3_600);
    }
}
