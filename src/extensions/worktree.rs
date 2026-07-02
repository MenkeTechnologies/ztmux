//! `ztmux worktree` — panes sitting in a linked git worktree.
//!
//! `git worktree add` lets one repository have several working trees checked out
//! at once, each on its own branch. A linked worktree looks like an ordinary
//! checkout, but it shares history with a main repo elsewhere. `worktree` finds
//! the panes in one: for each live pane in a repo it compares the worktree's
//! git-dir against the shared common git-dir — they differ only for a *linked*
//! worktree — and reports those. Where [`super::git`] reports branch/dirty state
//! and [`super::remote`] the repo identity, `worktree` answers "which panes are
//! in a side checkout rather than the main tree". The main worktree, non-repo
//! panes, and submodules (whose dirs match) are omitted; each unique directory
//! is resolved once. With `-o json` / `--json` it emits the same rows as a
//! machine-readable array.

use std::collections::HashMap;
use std::io::IsTerminal;

use super::gitcmd::{git_out, repo_name};
use super::tmux_query::{Pane, poll};

/// The worktree state of one repo directory (only the linked ones are kept).
#[derive(Clone)]
struct Worktree {
    root: String,
}

/// One output row: a pane sitting in a linked worktree.
struct Row {
    id: String,
    location: String,
    worktree: String, // root's basename
    root: String,     // full worktree root
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux worktree: {e}");
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

/// Resolve each unique live-pane directory once. `Some` only when the directory
/// is inside a *linked* worktree; `None` for the main tree, submodules, or
/// non-repos. The miss is cached too.
fn resolve_all(panes: &[Pane]) -> HashMap<String, Option<Worktree>> {
    let mut map: HashMap<String, Option<Worktree>> = HashMap::new();
    for p in panes {
        if p.dead || p.path.is_empty() || map.contains_key(&p.path) {
            continue;
        }
        map.insert(p.path.clone(), resolve_worktree(&p.path));
    }
    map
}

/// Query git for the worktree root and its git-dir / common-dir pair. Returns
/// `Some` only when the two dirs differ (a linked worktree).
fn resolve_worktree(path: &str) -> Option<Worktree> {
    let root = git_out(path, &["rev-parse", "--show-toplevel"])?;
    // One call returns both dirs in the same format so they compare cleanly.
    let dirs = git_out(path, &["rev-parse", "--git-dir", "--git-common-dir"])?;
    let (git_dir, common_dir) = parse_dirs(&dirs)?;
    if !is_linked(&git_dir, &common_dir) {
        return None;
    }
    Some(Worktree { root })
}

/// Split the two lines `git rev-parse --git-dir --git-common-dir` prints into
/// `(git_dir, common_dir)`.
fn parse_dirs(out: &str) -> Option<(String, String)> {
    let mut it = out.lines();
    let git_dir = it.next()?.trim().to_string();
    let common_dir = it.next()?.trim().to_string();
    Some((git_dir, common_dir))
}

/// A pane is in a linked worktree exactly when its git-dir differs from the
/// shared common git-dir. They are equal for the main worktree and for a
/// submodule.
fn is_linked(git_dir: &str, common_dir: &str) -> bool {
    git_dir != common_dir
}

/// One row per live pane in a linked worktree, sorted by worktree root then
/// location.
fn build_rows(panes: &[Pane], info: &HashMap<String, Option<Worktree>>) -> Vec<Row> {
    let mut rows: Vec<Row> = panes
        .iter()
        .filter(|p| !p.dead)
        .filter_map(|p| {
            let w = info.get(&p.path).and_then(|o| o.as_ref())?;
            Some(Row {
                id: p.id.clone(),
                location: location(p),
                worktree: repo_name(&w.root),
                root: w.root.clone(),
            })
        })
        .collect();
    rows.sort_by(|a, b| a.root.cmp(&b.root).then(a.location.cmp(&b.location)));
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
                "{:<8} {:<16} {:<16} {}",
                "PANE", "LOCATION", "WORKTREE", "ROOT"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<8} {:<16} {:<16} {}\n",
            r.id, r.location, r.worktree, r.root,
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
                "worktree": r.worktree,
                "root": r.root,
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
    fn linked_when_dirs_differ() {
        assert!(is_linked("/repo/.git/worktrees/feature", "/repo/.git"));
    }

    #[test]
    fn not_linked_for_main_tree_or_submodule() {
        // Main worktree: identical.
        assert!(!is_linked(".git", ".git"));
        // Submodule: git-dir == common-dir (both under modules/).
        assert!(!is_linked(
            "/super/.git/modules/sub",
            "/super/.git/modules/sub"
        ));
    }

    #[test]
    fn parse_dirs_splits_two_lines() {
        assert_eq!(
            parse_dirs("/a/.git/worktrees/x\n/a/.git\n"),
            Some(("/a/.git/worktrees/x".to_string(), "/a/.git".to_string()))
        );
        assert_eq!(parse_dirs("only-one-line"), None);
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
    fn build_rows_keeps_linked_and_sorts_by_root() {
        let panes = vec![
            pane("%1", 0, "/wt-z"),
            pane("%2", 1, "/wt-a"),
            pane("%3", 2, "/main"),
        ];
        let mut info: HashMap<String, Option<Worktree>> = HashMap::new();
        info.insert(
            "/wt-z".into(),
            Some(Worktree {
                root: "/wt-z".into(),
            }),
        );
        info.insert(
            "/wt-a".into(),
            Some(Worktree {
                root: "/wt-a".into(),
            }),
        );
        info.insert("/main".into(), None); // main tree -> omitted
        let rows = build_rows(&panes, &info);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].root, "/wt-a");
        assert_eq!(rows[1].root, "/wt-z");
    }

    #[test]
    fn json_carries_worktree_and_root() {
        let panes = vec![pane("%1", 0, "/wt")];
        let mut info: HashMap<String, Option<Worktree>> = HashMap::new();
        info.insert(
            "/wt".into(),
            Some(Worktree {
                root: "/home/u/wt".into(),
            }),
        );
        let rows = build_rows(&panes, &info);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["worktree"], "wt");
        assert_eq!(v[0]["root"], "/home/u/wt");
    }
}
