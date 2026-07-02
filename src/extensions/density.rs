//! `ztmux density` — windows ranked by how many panes they hold, most first.
//!
//! Where [`super::size`] ranks individual *panes* by cell area (smallest first)
//! and [`super::layouts`] prints each window's raw layout string, `density`
//! ranks *windows* by their pane count, most-split first — the "which window did
//! I cram the most panes into" view, the companion to `size` for spotting the
//! window that has grown unwieldy and wants a `break-pane` or a fresh layout.
//! With `-o json` / `--json` it emits the same rows as a machine-readable array.

use std::io::IsTerminal;

use super::tmux_query::{Snapshot, Window, poll};

/// One output row: a window and its pane count.
struct Row {
    location: String, // session:index
    name: String,
    panes: i64,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux density: {e}");
        return 1;
    }
    let rows = build_rows(&snap);
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

fn location(w: &Window) -> String {
    format!("{}:{}", w.session, w.index)
}

/// One row per window, most panes first; ties break by location for a stable
/// order.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .windows
        .iter()
        .map(|w| Row {
            location: location(w),
            name: w.name.clone(),
            panes: w.panes,
        })
        .collect();
    rows.sort_by(|a, b| b.panes.cmp(&a.panes).then(a.location.cmp(&b.location)));
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
        paint(&format!("{:>5} {:<12} {}", "PANES", "WINDOW", "NAME"), "1")
    ));
    for r in rows {
        out.push_str(&format!("{:>5} {:<12} {}\n", r.panes, r.location, r.name));
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
                "panes": r.panes,
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

    fn window(sess: &str, idx: i64, name: &str, panes: i64) -> Window {
        Window {
            session: sess.into(),
            index: idx,
            name: name.into(),
            panes,
            ..Default::default()
        }
    }

    fn snap(windows: Vec<Window>) -> Snapshot {
        Snapshot {
            windows,
            ..Default::default()
        }
    }

    #[test]
    fn most_panes_sorts_first() {
        let rows = build_rows(&snap(vec![
            window("a", 0, "edit", 2),
            window("a", 1, "run", 6),
            window("a", 2, "logs", 1),
        ]));
        assert_eq!(rows[0].location, "a:1");
        assert_eq!(rows[0].panes, 6);
        assert_eq!(rows[2].location, "a:2");
    }

    #[test]
    fn equal_pane_counts_break_ties_by_location() {
        let rows = build_rows(&snap(vec![window("z", 9, "b", 3), window("a", 0, "a", 3)]));
        assert_eq!(rows[0].location, "a:0");
        assert_eq!(rows[1].location, "z:9");
    }

    #[test]
    fn text_renders_header_count_and_window() {
        let rows = build_rows(&snap(vec![window("a", 0, "edit", 4)]));
        let s = render_text(&rows, false);
        assert!(s.contains("PANES") && s.contains("WINDOW") && s.contains("NAME"));
        assert!(s.lines().any(|l| l.contains("a:0") && l.contains("edit")));
    }

    #[test]
    fn json_carries_window_name_and_pane_count() {
        let rows = build_rows(&snap(vec![window("a", 0, "edit", 4)]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["window"], "a:0");
        assert_eq!(v[0]["name"], "edit");
        assert_eq!(v[0]["panes"], 4);
    }
}
