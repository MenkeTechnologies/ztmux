//! `ztmux cwd` — every working directory in use, busiest first.
//!
//! Where [`super::dedup`] groups panes by the *pair* `(path, command)` and only
//! reports the collisions, and [`super::find`] *searches* pane metadata for a
//! query, `cwd` answers the plain question "which directories am I working in,
//! and how many panes sit in each" — it groups every live pane by its working
//! directory alone and ranks the directories by pane count. It is the
//! "where is my attention spread" view, the pipeable primitive behind
//! `ztmux cwd | head` to jump to the repo you have the most panes open in.
//! `$HOME` is abbreviated to `~` for display; with `-o json` / `--json` the
//! full path is emitted alongside the pane count, the distinct commands, and
//! every pane location.

use std::io::IsTerminal;

use super::tmux_query::{Pane, Snapshot, poll};

/// One output row: a directory with the panes that sit in it.
struct Row {
    path: String,          // raw working directory (JSON)
    display: String,       // `$HOME`-abbreviated path (text)
    count: usize,          // number of live panes in this directory
    commands: Vec<String>, // distinct commands, sorted
    locations: Vec<String>,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux cwd: {e}");
        return 1;
    }
    let rows = build_rows(&snap, home());
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

fn home() -> Option<String> {
    std::env::var("HOME").ok().filter(|h| !h.is_empty())
}

/// Replace a leading `$HOME` with `~` so the busiest column stays narrow.
fn abbreviate(path: &str, home: Option<&str>) -> String {
    if let Some(h) = home {
        if path == h {
            return "~".to_string();
        }
        if let Some(rest) = path.strip_prefix(h) {
            if rest.starts_with('/') {
                return format!("~{rest}");
            }
        }
    }
    path.to_string()
}

/// Group live (non-dead) panes by working directory, biggest cluster first.
/// A [`std::collections::BTreeMap`] gives a deterministic path order; a stable
/// count-descending sort then puts the busiest directory on top while leaving
/// equal-sized directories in path order. Panes carrying no working directory
/// are skipped — a bare `path` is not a place you can point at.
fn build_rows(snap: &Snapshot, home: Option<String>) -> Vec<Row> {
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<String, (Vec<String>, Vec<String>)> = BTreeMap::new();
    for p in &snap.panes {
        if p.dead || p.path.is_empty() {
            continue;
        }
        let entry = buckets.entry(p.path.clone()).or_default();
        entry.0.push(location(p));
        if !p.command.is_empty() && !entry.1.contains(&p.command) {
            entry.1.push(p.command.clone());
        }
    }
    let mut rows: Vec<Row> = buckets
        .into_iter()
        .map(|(path, (mut locations, mut commands))| {
            locations.sort();
            commands.sort();
            Row {
                display: abbreviate(&path, home.as_deref()),
                count: locations.len(),
                commands,
                locations,
                path,
            }
        })
        .collect();
    // Busiest directory first; ties keep the map's path order (stable sort).
    rows.sort_by_key(|r| std::cmp::Reverse(r.count));
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
        paint(&format!("{:>5} {:<20} {}", "PANES", "COMMANDS", "DIR"), "1")
    ));
    for r in rows {
        out.push_str(&format!(
            "{:>5} {:<20} {}\n",
            r.count,
            r.commands.join(","),
            r.display,
        ));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "path": r.path,
                "panes": r.count,
                "commands": r.commands,
                "locations": r.locations,
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

    fn pane(id: &str, sess: &str, win: i64, idx: i64, path: &str, cmd: &str) -> Pane {
        Pane {
            session: sess.into(),
            window: win,
            index: idx,
            id: id.into(),
            command: cmd.into(),
            path: path.into(),
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
    fn busiest_directory_sorts_first() {
        let rows = build_rows(
            &snap(vec![
                pane("%1", "a", 0, 0, "/tmp", "zsh"),
                pane("%2", "a", 0, 1, "/src", "vim"),
                pane("%3", "a", 0, 2, "/src", "cargo"),
                pane("%4", "a", 0, 3, "/src", "zsh"),
            ]),
            None,
        );
        assert_eq!(rows[0].path, "/src");
        assert_eq!(rows[0].count, 3);
        assert_eq!(rows[1].path, "/tmp");
    }

    #[test]
    fn distinct_commands_are_deduped_and_sorted() {
        let rows = build_rows(
            &snap(vec![
                pane("%1", "a", 0, 0, "/src", "zsh"),
                pane("%2", "a", 0, 1, "/src", "vim"),
                pane("%3", "a", 0, 2, "/src", "zsh"),
            ]),
            None,
        );
        assert_eq!(rows[0].commands, vec!["vim", "zsh"]);
        assert_eq!(rows[0].locations, vec!["a:0.0", "a:0.1", "a:0.2"]);
    }

    #[test]
    fn dead_and_pathless_panes_are_skipped() {
        let mut dead = pane("%2", "a", 0, 1, "/src", "zsh");
        dead.dead = true;
        let rows = build_rows(
            &snap(vec![
                pane("%1", "a", 0, 0, "/src", "zsh"),
                dead,
                pane("%3", "a", 0, 2, "", "zsh"),
            ]),
            None,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].count, 1);
    }

    #[test]
    fn home_is_abbreviated_to_tilde() {
        assert_eq!(abbreviate("/home/u", Some("/home/u")), "~");
        assert_eq!(abbreviate("/home/u/src", Some("/home/u")), "~/src");
        // A prefix that is not a path boundary must not be abbreviated.
        assert_eq!(abbreviate("/home/user2", Some("/home/u")), "/home/user2");
        assert_eq!(abbreviate("/etc", Some("/home/u")), "/etc");
    }

    #[test]
    fn text_renders_header_count_and_dir() {
        let rows = build_rows(
            &snap(vec![pane("%1", "a", 0, 0, "/home/u/src", "vim")]),
            Some("/home/u".into()),
        );
        let s = render_text(&rows, false);
        assert!(s.contains("PANES") && s.contains("COMMANDS") && s.contains("DIR"));
        assert!(s.lines().any(|l| l.contains("~/src") && l.contains("vim")));
    }

    #[test]
    fn json_carries_path_count_commands_and_locations() {
        let rows = build_rows(
            &snap(vec![
                pane("%1", "a", 0, 0, "/src", "zsh"),
                pane("%2", "a", 0, 1, "/src", "vim"),
            ]),
            None,
        );
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["path"], "/src");
        assert_eq!(v[0]["panes"], 2);
        assert_eq!(v[0]["commands"][0], "vim");
        assert_eq!(v[0]["locations"][1], "a:0.1");
    }
}
