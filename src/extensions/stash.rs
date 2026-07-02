//! `ztmux stash` — repositories with stashed work behind a pane.
//!
//! For each live pane in a git repo it counts the entries in `git stash list`
//! and reports only the repos that have any — the stashed work it is easy to
//! forget across a wall of panes and many checkouts. Where [`super::changes`]
//! counts uncommitted files in the working tree and [`super::ahead`] counts
//! unpushed commits, `stash` surfaces the third bucket of "unfinished" state:
//! work you shelved and moved on from. Repos with no stashes (and non-repo
//! panes) are omitted; each unique directory is resolved once. Rows are sorted
//! by stash count, most first. With `-o json` / `--json` it emits the same rows
//! as a machine-readable array.

use std::collections::HashMap;
use std::io::IsTerminal;

use super::gitcmd::{git_out, repo_name};
use super::tmux_query::{Pane, poll};

/// The stash state of one repo directory.
#[derive(Clone)]
struct Stash {
    root: String,
    count: i64,
}

/// One output row: a pane and how many stashes its repo holds.
struct Row {
    id: String,
    location: String,
    repo: String,
    count: i64,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux stash: {e}");
        return 1;
    }
    let info = resolve_all(&snap.panes);
    let rows = build_rows(&snap.panes, &info);
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

/// Resolve each unique live-pane directory once to its stash state (or `None`
/// when it is not a repo), caching the miss too.
fn resolve_all(panes: &[Pane]) -> HashMap<String, Option<Stash>> {
    let mut map: HashMap<String, Option<Stash>> = HashMap::new();
    for p in panes {
        if p.dead || p.path.is_empty() || map.contains_key(&p.path) {
            continue;
        }
        map.insert(p.path.clone(), resolve_stash(&p.path));
    }
    map
}

/// Query git for the repo root and stash count. Returns `None` when the
/// directory is not inside a repository.
fn resolve_stash(path: &str) -> Option<Stash> {
    let root = git_out(path, &["rev-parse", "--show-toplevel"])?;
    let list = git_out(path, &["stash", "list"]).unwrap_or_default();
    Some(Stash {
        root,
        count: count_stashes(&list),
    })
}

/// Count the entries in `git stash list` output — one non-blank line per stash.
fn count_stashes(list: &str) -> i64 {
    list.lines().filter(|l| !l.trim().is_empty()).count() as i64
}

/// One row per live pane whose repo has at least one stash, most first; ties
/// break by location.
fn build_rows(panes: &[Pane], info: &HashMap<String, Option<Stash>>) -> Vec<Row> {
    let mut rows: Vec<Row> = panes
        .iter()
        .filter(|p| !p.dead)
        .filter_map(|p| {
            let s = info.get(&p.path).and_then(|o| o.as_ref())?;
            if s.count == 0 {
                return None;
            }
            Some(Row {
                id: p.id.clone(),
                location: location(p),
                repo: repo_name(&s.root),
                count: s.count,
            })
        })
        .collect();
    rows.sort_by(|a, b| b.count.cmp(&a.count).then(a.location.cmp(&b.location)));
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
            &format!("{:>7} {:<16} {}", "STASHES", "REPO", "LOCATION"),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!("{:>7} {:<16} {}\n", r.count, r.repo, r.location));
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
                "repo": r.repo,
                "stashes": r.count,
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
    fn counts_non_blank_stash_lines() {
        let list = "stash@{0}: WIP on main: abc\nstash@{1}: On dev: def\n";
        assert_eq!(count_stashes(list), 2);
    }

    #[test]
    fn no_stashes_counts_zero() {
        assert_eq!(count_stashes(""), 0);
        assert_eq!(count_stashes("\n"), 0);
    }

    fn pane(id: &str, idx: i64, path: &str) -> Pane {
        Pane {
            id: id.into(),
            session: "a".into(),
            window: 0,
            index: idx,
            path: path.into(),
            ..Default::default()
        }
    }

    #[test]
    fn most_stashes_first_and_empty_repos_omitted() {
        let panes = vec![
            pane("%1", 0, "/a"),
            pane("%2", 1, "/b"),
            pane("%3", 2, "/none"),
        ];
        let mut info: HashMap<String, Option<Stash>> = HashMap::new();
        info.insert(
            "/a".into(),
            Some(Stash {
                root: "/a".into(),
                count: 1,
            }),
        );
        info.insert(
            "/b".into(),
            Some(Stash {
                root: "/b".into(),
                count: 4,
            }),
        );
        info.insert(
            "/none".into(),
            Some(Stash {
                root: "/none".into(),
                count: 0,
            }),
        );
        let rows = build_rows(&panes, &info);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].repo, "b");
        assert_eq!(rows[0].count, 4);
        assert_eq!(rows[1].repo, "a");
    }

    #[test]
    fn json_carries_stash_count_and_repo() {
        let panes = vec![pane("%1", 0, "/a")];
        let mut info: HashMap<String, Option<Stash>> = HashMap::new();
        info.insert(
            "/a".into(),
            Some(Stash {
                root: "/home/u/a".into(),
                count: 3,
            }),
        );
        let rows = build_rows(&panes, &info);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["repo"], "a");
        assert_eq!(v[0]["stashes"], 3);
    }
}
