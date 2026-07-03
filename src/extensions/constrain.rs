//! `ztmux constrain` — attached clients ranked by screen size, smallest first.
//!
//! When several clients share a session, the *smallest* one caps what everyone
//! sees: with the default `window-size smallest`, the session is sized to the
//! narrowest and shortest attached client, so a phone terminal joined to your
//! session shrinks it for all. `constrain` ranks the clients by screen area,
//! smallest first — the top row is the one holding a session down. Where
//! [`super::who`] lists every client and [`super::viewers`] counts them per
//! session, `constrain` surfaces the size cap. With `-o json` / `--json` it emits
//! the same rows (width and height) as a machine-readable array; a server with no
//! clients prints just the header.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// The `\x1f`-delimited per-client format the dimensions are read through.
const FORMAT: &str =
    "#{client_name}\u{1f}#{client_session}\u{1f}#{client_width}\u{1f}#{client_height}";

/// One output row: a client and its screen size.
struct Row {
    client: String,
    session: String,
    width: i64,
    height: i64,
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
        print!("{}", render_json(&rows));
    } else {
        print!("{}", render_text(&rows, std::io::stdout().is_terminal()));
    }
    0
}

/// Parse one formatted line into `(client, session, width, height)`.
fn parse_line(line: &str) -> Option<(String, String, i64, i64)> {
    let mut it = line.split('\u{1f}');
    let client = it.next()?;
    let session = it.next()?;
    let width: i64 = it.next()?.parse().ok()?;
    let height: i64 = it.next()?.parse().ok()?;
    Some((client.to_string(), session.to_string(), width, height))
}

/// One row per attached client, smallest cell-area first (the constraining
/// client on top); ties break by client name.
fn build_rows(lines: &[String]) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .filter_map(|l| parse_line(l))
        .map(|(client, session, width, height)| Row {
            client,
            session,
            width,
            height,
        })
        .collect();
    rows.sort_by(|a, b| {
        (a.width * a.height)
            .cmp(&(b.width * b.height))
            .then(a.client.cmp(&b.client))
    });
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
            &format!("{:>9} {:<18} {}", "SIZE", "CLIENT", "SESSION"),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:>9} {:<18} {}\n",
            format!("{}x{}", r.width, r.height),
            r.client,
            r.session,
        ));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "client": r.client,
                "session": r.session,
                "width": r.width,
                "height": r.height,
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
        let (c, s, w, h) = parse_line("/dev/ttys1\u{1f}work\u{1f}80\u{1f}24").unwrap();
        assert_eq!(c, "/dev/ttys1");
        assert_eq!(s, "work");
        assert_eq!(w, 80);
        assert_eq!(h, 24);
    }

    #[test]
    fn smallest_client_sorts_first() {
        let lines = vec![
            "/dev/ttys1\u{1f}a\u{1f}200\u{1f}50".to_string(), // 10000 cells
            "/dev/ttys2\u{1f}a\u{1f}80\u{1f}24".to_string(),  // 1920 cells (constrains)
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows[0].client, "/dev/ttys2");
        assert_eq!(rows[0].width, 80);
        assert_eq!(rows[1].client, "/dev/ttys1");
    }

    #[test]
    fn no_clients_renders_header_only() {
        let rows = build_rows(&[]);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("SIZE") && s.contains("CLIENT"));
    }

    #[test]
    fn text_renders_wxh() {
        let lines = vec!["/dev/ttys1\u{1f}a\u{1f}80\u{1f}24".to_string()];
        let rows = build_rows(&lines);
        let s = render_text(&rows, false);
        assert!(s.lines().any(|l| l.contains("80x24")));
    }

    #[test]
    fn json_carries_dimensions() {
        let lines = vec!["/dev/ttys1\u{1f}a\u{1f}80\u{1f}24".to_string()];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["width"], 80);
        assert_eq!(v[0]["height"], 24);
    }
}
