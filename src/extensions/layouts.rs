//! `ztmux layouts` — the current layout string of every window.
//!
//! Where [`super::layout`] *applies* a named preset to a window and
//! [`super::equalize`] *re-balances* every window, `layouts` only *reports* the
//! layout each window is in right now: its pane count and the raw tmux layout
//! string (the checksum-prefixed geometry `select-layout` consumes). It is the
//! read-only companion to those two — the view you capture before changing a
//! layout so you can restore it, or diff to see which windows drifted off a
//! preset. With `-o json` / `--json` it emits the same rows as a machine-readable
//! array.

use std::io::IsTerminal;

use super::tmux_query::{Snapshot, Window, poll};

/// One output row: a window and the layout it is currently in.
struct Row {
    location: String, // session:index
    name: String,
    panes: i64,
    layout: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux layouts: {e}");
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

/// One row per window, ordered by session then window index for a stable table.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .windows
        .iter()
        .map(|w| Row {
            location: location(w),
            name: w.name.clone(),
            panes: w.panes,
            layout: w.layout.clone(),
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
                "{:<12} {:<16} {:>5} {}",
                "WINDOW", "NAME", "PANES", "LAYOUT"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<12} {:<16} {:>5} {}\n",
            r.location, r.name, r.panes, r.layout,
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
                "panes": r.panes,
                "layout": r.layout,
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

    fn window(sess: &str, idx: i64, name: &str, panes: i64, layout: &str) -> Window {
        Window {
            session: sess.into(),
            index: idx,
            name: name.into(),
            panes,
            layout: layout.into(),
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
    fn one_row_per_window_sorted_by_location() {
        let rows = build_rows(&snap(vec![
            window("z", 9, "logs", 1, "abcd,80x24,0,0,0"),
            window("a", 0, "edit", 2, "efgh,80x24,0,0{...}"),
            window("a", 1, "run", 3, "ijkl,80x24,0,0[...]"),
        ]));
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].location, "a:0");
        assert_eq!(rows[1].location, "a:1");
        assert_eq!(rows[2].location, "z:9");
    }

    #[test]
    fn text_renders_location_name_panes_and_layout() {
        let rows = build_rows(&snap(vec![window("a", 0, "edit", 2, "efgh,80x24,0,0")]));
        let s = render_text(&rows, false);
        assert!(s.contains("WINDOW") && s.contains("LAYOUT"));
        assert!(
            s.lines()
                .any(|l| l.contains("a:0") && l.contains("edit") && l.contains("efgh,80x24,0,0"))
        );
    }

    #[test]
    fn json_carries_window_name_panes_and_layout() {
        let rows = build_rows(&snap(vec![window("a", 0, "edit", 2, "efgh,80x24,0,0")]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["window"], "a:0");
        assert_eq!(v[0]["name"], "edit");
        assert_eq!(v[0]["panes"], 2);
        assert_eq!(v[0]["layout"], "efgh,80x24,0,0");
    }
}
