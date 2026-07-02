//! `ztmux dedup` — find redundant live panes: two or more panes running the
//! same command in the same working directory.
//!
//! Where [`super::prune`] reclaims *dead* or *empty* server objects and
//! [`super::find`] *searches* pane metadata for a query, `dedup` computes a
//! relation the others don't: it groups every live pane by `(path, command)`
//! and reports only the groups with more than one member — the "you left three
//! zsh panes open in the same repo, close two" view. Each duplicate group gets
//! a stable integer id so the flat table stays greppable
//! (`ztmux dedup | grep '^    2 '`). With `-o json` / `--json` it emits one
//! object per group.

use std::io::IsTerminal;

use super::tmux_query::{Pane, Snapshot, poll};

/// A set of panes that share the same working directory and command.
struct Group {
    id: usize,
    path: String,
    command: String,
    panes: Vec<Member>,
}

/// One pane within a duplicate group.
struct Member {
    id: String,       // pane id (%N)
    location: String, // session:window.pane
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux dedup: {e}");
        return 1;
    }
    let groups = build_groups(&snap);
    let json = std::env::args().any(|a| a == "--json")
        || std::env::args()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|w| w[0] == "-o" && w[1] == "json");
    if json {
        print!("{}", render_json(&groups));
    } else {
        print!("{}", render_text(&groups, std::io::stdout().is_terminal()));
    }
    0
}

fn location(p: &Pane) -> String {
    format!("{}:{}.{}", p.session, p.window, p.index)
}

/// Group live (non-dead) panes by `(path, command)`, keeping only groups with
/// two or more members. A [`std::collections::BTreeMap`] gives a deterministic
/// `(path, command)` order; a stable size-descending sort then puts the biggest
/// duplicate cluster first while leaving equal-sized groups in that path/command
/// order. Panes carrying no working directory are skipped (a bare `path` can't
/// prove two panes are in the same place).
fn build_groups(snap: &Snapshot) -> Vec<Group> {
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<(String, String), Vec<Member>> = BTreeMap::new();
    for p in &snap.panes {
        if p.dead || p.path.is_empty() {
            continue;
        }
        buckets
            .entry((p.path.clone(), p.command.clone()))
            .or_default()
            .push(Member {
                id: p.id.clone(),
                location: location(p),
            });
    }
    let mut groups: Vec<Group> = buckets
        .into_iter()
        .filter(|(_, m)| m.len() > 1)
        .map(|((path, command), mut panes)| {
            panes.sort_by(|a, b| a.location.cmp(&b.location));
            Group {
                id: 0,
                path,
                command,
                panes,
            }
        })
        .collect();
    // Largest group first; ties keep the map's (path, command) order (stable sort).
    groups.sort_by_key(|g| std::cmp::Reverse(g.panes.len()));
    for (i, g) in groups.iter_mut().enumerate() {
        g.id = i + 1;
    }
    groups
}

fn render_text(groups: &[Group], color: bool) -> String {
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
                "{:>5} {:>5} {:<8} {:<16} {:<12} {}",
                "GROUP", "COUNT", "PANE", "LOCATION", "COMMAND", "PATH"
            ),
            "1"
        )
    ));
    for g in groups {
        let count = g.panes.len();
        for m in &g.panes {
            out.push_str(&format!(
                "{:>5} {:>5} {:<8} {:<16} {:<12} {}\n",
                g.id, count, m.id, m.location, g.command, g.path,
            ));
        }
    }
    out
}

fn render_json(groups: &[Group]) -> String {
    let arr: Vec<serde_json::Value> = groups
        .iter()
        .map(|g| {
            serde_json::json!({
                "group": g.id,
                "count": g.panes.len(),
                "path": g.path,
                "command": g.command,
                "panes": g
                    .panes
                    .iter()
                    .map(|m| serde_json::json!({ "id": m.id, "location": m.location }))
                    .collect::<Vec<_>>(),
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

    fn pane(id: &str, sess: &str, win: i64, idx: i64, cmd: &str, path: &str) -> Pane {
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
    fn only_duplicate_groups_are_reported() {
        // Two zsh panes in /repo (a dup), one lone nvim in /repo (unique).
        let g = build_groups(&snap(vec![
            pane("%1", "a", 0, 0, "zsh", "/repo"),
            pane("%2", "b", 0, 0, "zsh", "/repo"),
            pane("%3", "a", 1, 0, "nvim", "/repo"),
        ]));
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].command, "zsh");
        assert_eq!(g[0].path, "/repo");
        assert_eq!(g[0].panes.len(), 2);
    }

    #[test]
    fn same_command_different_dir_is_not_a_dup() {
        let g = build_groups(&snap(vec![
            pane("%1", "a", 0, 0, "zsh", "/one"),
            pane("%2", "a", 1, 0, "zsh", "/two"),
        ]));
        assert!(g.is_empty());
    }

    #[test]
    fn dead_and_pathless_panes_are_skipped() {
        let mut dead = pane("%2", "a", 1, 0, "zsh", "/repo");
        dead.dead = true;
        let g = build_groups(&snap(vec![
            pane("%1", "a", 0, 0, "zsh", "/repo"),
            dead,                             // dead: excluded
            pane("%3", "a", 2, 0, "zsh", ""), // no cwd: excluded
        ]));
        // Only one live pane with a path remains → no group of ≥2.
        assert!(g.is_empty());
    }

    #[test]
    fn largest_group_gets_id_one() {
        // /big has 3 zsh panes, /small has 2 vim panes → /big sorts first.
        let g = build_groups(&snap(vec![
            pane("%1", "a", 0, 0, "vim", "/small"),
            pane("%2", "a", 1, 0, "vim", "/small"),
            pane("%3", "a", 2, 0, "zsh", "/big"),
            pane("%4", "a", 3, 0, "zsh", "/big"),
            pane("%5", "a", 4, 0, "zsh", "/big"),
        ]));
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].id, 1);
        assert_eq!(g[0].path, "/big");
        assert_eq!(g[0].panes.len(), 3);
        assert_eq!(g[1].id, 2);
        assert_eq!(g[1].path, "/small");
    }

    #[test]
    fn members_sorted_by_location() {
        let g = build_groups(&snap(vec![
            pane("%9", "z", 5, 0, "zsh", "/repo"),
            pane("%1", "a", 0, 0, "zsh", "/repo"),
        ]));
        assert_eq!(g[0].panes[0].location, "a:0.0");
        assert_eq!(g[0].panes[1].location, "z:5.0");
    }

    #[test]
    fn text_has_header_and_group_id_column() {
        let g = build_groups(&snap(vec![
            pane("%1", "a", 0, 0, "zsh", "/repo"),
            pane("%2", "b", 0, 0, "zsh", "/repo"),
        ]));
        let s = render_text(&g, false);
        assert!(s.contains("GROUP") && s.contains("COUNT") && s.contains("PATH"));
        // Both panes carry the same group id (1) and count (2).
        assert_eq!(s.lines().filter(|l| l.contains("/repo")).count(), 2);
        assert!(s.lines().any(|l| l.contains("%1") && l.contains("a:0.0")));
    }

    #[test]
    fn json_emits_one_object_per_group_with_members() {
        let g = build_groups(&snap(vec![
            pane("%1", "a", 0, 0, "zsh", "/repo"),
            pane("%2", "b", 0, 0, "zsh", "/repo"),
        ]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&g)).unwrap();
        assert_eq!(v[0]["group"], 1);
        assert_eq!(v[0]["count"], 2);
        assert_eq!(v[0]["command"], "zsh");
        assert_eq!(v[0]["panes"].as_array().unwrap().len(), 2);
        assert_eq!(v[0]["panes"][0]["id"], "%1");
    }
}
