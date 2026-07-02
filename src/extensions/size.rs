//! `ztmux size` — every pane's geometry, smallest cell-area first.
//!
//! Where [`super::layout`] *applies* a named layout preset and [`super::ps`]
//! reports the *process* in each pane, `size` reports pure geometry: the
//! `width×height` of every pane and its cell area, sorted smallest-first so the
//! cramped panes — the 12-column sliver you forgot about — sort to the top. It
//! is the "which pane is too small to work in" view. With `-o json` / `--json`
//! it emits the same rows as a machine-readable array.

use std::io::IsTerminal;

use super::tmux_query::{Pane, Snapshot, poll};

/// One output row: a pane with its dimensions and derived cell area.
struct Row {
    id: String,
    location: String,
    command: String,
    width: i64,
    height: i64,
    cells: i64,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux size: {e}");
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

fn location(p: &Pane) -> String {
    format!("{}:{}.{}", p.session, p.window, p.index)
}

/// Build one row per live pane, ordered smallest cell-area first; ties break by
/// location for a stable order. Dead panes are skipped — their last-known size
/// is not a pane you can type into.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .panes
        .iter()
        .filter(|p| !p.dead)
        .map(|p| Row {
            id: p.id.clone(),
            location: location(p),
            command: p.command.clone(),
            width: p.width,
            height: p.height,
            cells: p.width * p.height,
        })
        .collect();
    rows.sort_by(|a, b| a.cells.cmp(&b.cells).then(a.location.cmp(&b.location)));
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
                "{:<8} {:<16} {:<12} {:>9} {:>7}",
                "PANE", "LOCATION", "COMMAND", "SIZE", "CELLS"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<8} {:<16} {:<12} {:>9} {:>7}\n",
            r.id,
            r.location,
            r.command,
            format!("{}x{}", r.width, r.height),
            r.cells,
        ));
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
                "width": r.width,
                "height": r.height,
                "cells": r.cells,
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

    fn pane(id: &str, sess: &str, win: i64, idx: i64, w: i64, h: i64) -> Pane {
        Pane {
            session: sess.into(),
            window: win,
            index: idx,
            id: id.into(),
            command: "zsh".into(),
            width: w,
            height: h,
            ..Default::default()
        }
    }

    fn snap(panes: Vec<Pane>) -> Snapshot {
        Snapshot {
            panes,
            ..Default::default()
        }
    }

    #[test]
    fn smallest_cell_area_sorts_first() {
        let rows = build_rows(&snap(vec![
            pane("%1", "a", 0, 0, 200, 50), // 10000 cells
            pane("%2", "a", 0, 1, 12, 24),  //   288 cells
            pane("%3", "a", 0, 2, 80, 24),  //  1920 cells
        ]));
        assert_eq!(rows[0].id, "%2");
        assert_eq!(rows[0].cells, 288);
        assert_eq!(rows[1].id, "%3");
        assert_eq!(rows[2].id, "%1");
    }

    #[test]
    fn equal_area_breaks_ties_by_location() {
        let rows = build_rows(&snap(vec![
            pane("%9", "z", 9, 0, 40, 20),
            pane("%1", "a", 0, 0, 20, 40), // same 800 cells
        ]));
        assert_eq!(rows[0].location, "a:0.0");
        assert_eq!(rows[1].location, "z:9.0");
    }

    #[test]
    fn dead_panes_are_skipped() {
        let mut dead = pane("%2", "a", 0, 1, 80, 24);
        dead.dead = true;
        let rows = build_rows(&snap(vec![pane("%1", "a", 0, 0, 80, 24), dead]));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%1");
    }

    #[test]
    fn text_renders_wxh_and_header() {
        let rows = build_rows(&snap(vec![pane("%1", "a", 0, 0, 80, 24)]));
        let s = render_text(&rows, false);
        assert!(s.contains("PANE") && s.contains("SIZE") && s.contains("CELLS"));
        assert!(s.lines().any(|l| l.contains("80x24") && l.contains("1920")));
    }

    #[test]
    fn json_carries_dimensions_and_derived_cells() {
        let rows = build_rows(&snap(vec![pane("%1", "a", 0, 0, 80, 24)]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["id"], "%1");
        assert_eq!(v[0]["width"], 80);
        assert_eq!(v[0]["height"], 24);
        assert_eq!(v[0]["cells"], 1920);
    }
}
