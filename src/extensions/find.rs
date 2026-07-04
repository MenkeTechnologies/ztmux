//! `ztmux finder` — a one-shot, pipeable search across the whole server.
//!
//! The non-interactive complement to the [`super::switch`] picker: it scans
//! every pane (via the `list-* -o json` query layer) and prints the panes whose
//! command, current path, title, or enclosing window name contains the query.
//! Matching is a case-insensitive substring test — no regex dependency — so it
//! stays dependency-free and predictable. Output is a table (coloured when
//! stdout is a TTY) or a machine-readable array with `-o json` / `--json`. Rows
//! are sorted by location (`session:window.pane`).

use std::io::IsTerminal;

use super::tmux_query::{Snapshot, Window, poll};

/// One matched pane, annotated with which fields the query hit.
struct Hit {
    id: String,
    loc: String,
    command: String,
    title: String,
    path: String,
    fields: Vec<&'static str>,
}

pub(crate) fn run(socket: &str) -> i32 {
    let Some(query) = query_arg() else {
        eprintln!("usage: ztmux find <query> [-o json]");
        return 2;
    };
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux find: {e}");
        return 1;
    }
    let hits = search(&snap, &query);
    let json = std::env::args().any(|a| a == "--json")
        || std::env::args()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|w| w[0] == "-o" && w[1] == "json");
    if json {
        print!("{}", render_json(&hits));
    } else {
        print!("{}", render_text(&hits, std::io::stdout().is_terminal()));
    }
    // Exit non-zero when nothing matched, so `find` is usable in shell `if`.
    i32::from(hits.is_empty())
}

/// The first positional argument after the `find` subcommand. Output flags
/// (`-o json`, `--json`) and the `json` value are skipped so they never get
/// mistaken for the query.
fn query_arg() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let start = args.iter().position(|a| a == "find")? + 1;
    args[start..]
        .iter()
        .find(|a| !a.starts_with('-') && a.as_str() != "json")
        .cloned()
}

fn search(snap: &Snapshot, query: &str) -> Vec<Hit> {
    let needle = query.to_lowercase();
    let contains = |hay: &str| !hay.is_empty() && hay.to_lowercase().contains(&needle);
    let win_name = |p: &super::tmux_query::Pane| -> &str {
        snap.windows
            .iter()
            .find(|w: &&Window| w.session == p.session && w.index == p.window)
            .map_or("", |w| w.name.as_str())
    };

    let mut hits: Vec<Hit> = snap
        .panes
        .iter()
        .filter_map(|p| {
            let mut fields = Vec::new();
            if contains(&p.command) {
                fields.push("command");
            }
            if contains(&p.path) {
                fields.push("path");
            }
            if contains(&p.title) {
                fields.push("title");
            }
            if contains(win_name(p)) {
                fields.push("window");
            }
            if fields.is_empty() {
                return None;
            }
            Some(Hit {
                id: p.id.clone(),
                loc: format!("{}:{}.{}", p.session, p.window, p.index),
                command: p.command.clone(),
                title: p.title.clone(),
                path: p.path.clone(),
                fields,
            })
        })
        .collect();
    hits.sort_by(|a, b| a.loc.cmp(&b.loc));
    hits
}

fn render_text(hits: &[Hit], color: bool) -> String {
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
                "{:<8} {:<16} {:<16} {:<10} {}",
                "PANE", "LOCATION", "COMMAND", "MATCH", "PATH"
            ),
            "1"
        )
    ));
    for h in hits {
        out.push_str(&format!(
            "{:<8} {:<16} {:<16} {:<10} {}\n",
            h.id,
            h.loc,
            h.command,
            h.fields.join(","),
            h.path,
        ));
    }
    out
}

fn render_json(hits: &[Hit]) -> String {
    let arr: Vec<serde_json::Value> = hits
        .iter()
        .map(|h| {
            serde_json::json!({
                "pane": h.id,
                "location": h.loc,
                "command": h.command,
                "title": h.title,
                "path": h.path,
                "matched": h.fields,
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
    use super::super::tmux_query::{Pane, Window};
    use super::*;

    fn snap() -> Snapshot {
        Snapshot {
            windows: vec![Window {
                session: "work".into(),
                index: 0,
                name: "editor".into(),
                ..Default::default()
            }],
            panes: vec![
                Pane {
                    id: "%0".into(),
                    session: "work".into(),
                    window: 0,
                    index: 0,
                    command: "nvim".into(),
                    path: "/home/user/src".into(),
                    title: "main.rs".into(),
                    ..Default::default()
                },
                Pane {
                    id: "%1".into(),
                    session: "work".into(),
                    window: 0,
                    index: 1,
                    command: "zsh".into(),
                    path: "/home/user/docs".into(),
                    title: "shell".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn matches_command_case_insensitively() {
        let hits = search(&snap(), "NVIM");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "%0");
        assert_eq!(hits[0].fields, vec!["command"]);
    }

    #[test]
    fn matches_path_substring_on_multiple_panes() {
        let hits = search(&snap(), "/home/user");
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|h| h.fields.contains(&"path")));
    }

    #[test]
    fn matches_window_name_for_every_pane_in_it() {
        // "editor" is the window name, so both of its panes match on "window".
        let hits = search(&snap(), "editor");
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|h| h.fields.contains(&"window")));
    }

    #[test]
    fn no_match_yields_empty() {
        assert!(search(&snap(), "kubernetes").is_empty());
    }

    // A pane hitting several fields lists them all, in scan order.
    #[test]
    fn records_all_matched_fields() {
        let mut sn = snap();
        // "editor" already matches the window name; make the title match too.
        sn.panes[0].title = "editor-buffer".into();
        let hits = search(&sn, "editor");
        let h = hits.iter().find(|h| h.id == "%0").unwrap();
        assert_eq!(h.fields, vec!["title", "window"]);
    }

    #[test]
    fn json_is_an_array_carrying_match_fields() {
        let hits = search(&snap(), "nvim");
        let v: serde_json::Value = serde_json::from_str(&render_json(&hits)).unwrap();
        assert!(v.is_array());
        assert_eq!(v[0]["pane"], "%0");
        assert_eq!(v[0]["matched"][0], "command");
    }

    #[test]
    fn text_has_header_and_matched_column() {
        let hits = search(&snap(), "nvim");
        let s = render_text(&hits, false);
        assert!(s.contains("PANE") && s.contains("MATCH"));
        assert!(s.contains("%0") && s.contains("command"));
    }
}
