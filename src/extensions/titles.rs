//! `ztmux titles` — what every pane is calling itself.
//!
//! A pane's title is set by the program inside it (the `\e]2;…\a` terminal
//! escape) or with `select-pane -T`; ssh and many shells push the current host
//! or command into it, so the title often says more about what a pane is doing
//! than its process name. `titles` prints every live pane's title in one
//! pipeable table — the complement to [`super::ssh`], showing what each pane's
//! terminal announces itself as. Panes with an empty title are omitted. Sorted
//! by location. With `-o json` / `--json` it emits the same rows as an array.

use std::io::IsTerminal;

use super::tmux_query::{Pane, Snapshot, poll};

/// One output row: a pane and the title it is advertising.
struct Row {
    id: String,
    location: String,
    command: String,
    title: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux titles: {e}");
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

/// One row per live pane that advertises a (non-empty) title, ordered by
/// location for a stable, greppable table.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .panes
        .iter()
        .filter(|p| !p.dead && !p.title.is_empty())
        .map(|p| Row {
            id: p.id.clone(),
            location: location(p),
            command: p.command.clone(),
            title: p.title.clone(),
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
                "{:<8} {:<16} {:<12} {}",
                "PANE", "LOCATION", "COMMAND", "TITLE"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<8} {:<16} {:<12} {}\n",
            r.id, r.location, r.command, r.title,
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
                "title": r.title,
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

    fn pane(id: &str, sess: &str, win: i64, idx: i64, cmd: &str, title: &str) -> Pane {
        Pane {
            session: sess.into(),
            window: win,
            index: idx,
            id: id.into(),
            command: cmd.into(),
            title: title.into(),
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
    fn empty_titles_are_omitted() {
        let rows = build_rows(&snap(vec![
            pane("%0", "a", 0, 0, "zsh", ""),           // no title → excluded
            pane("%1", "a", 1, 0, "ssh", "prod-web-1"), // titled → included
        ]));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%1");
        assert_eq!(rows[0].title, "prod-web-1");
    }

    #[test]
    fn dead_panes_are_skipped() {
        let mut dead = pane("%1", "a", 1, 0, "ssh", "prod");
        dead.dead = true;
        assert!(build_rows(&snap(vec![dead])).is_empty());
    }

    #[test]
    fn rows_sorted_by_location() {
        let rows = build_rows(&snap(vec![
            pane("%9", "z", 5, 0, "ssh", "b"),
            pane("%1", "a", 0, 0, "ssh", "a"),
        ]));
        assert_eq!(rows[0].location, "a:0.0");
        assert_eq!(rows[1].location, "z:5.0");
    }

    #[test]
    fn text_shows_title_verbatim() {
        let rows = build_rows(&snap(vec![pane(
            "%1",
            "a",
            0,
            0,
            "ssh",
            "user@host: ~/src",
        )]));
        let s = render_text(&rows, false);
        assert!(s.contains("TITLE"));
        assert!(
            s.lines()
                .any(|l| l.contains("%1") && l.contains("user@host: ~/src"))
        );
    }

    #[test]
    fn json_carries_title() {
        let rows = build_rows(&snap(vec![pane("%1", "a", 0, 0, "ssh", "prod")]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["id"], "%1");
        assert_eq!(v[0]["title"], "prod");
    }
}
