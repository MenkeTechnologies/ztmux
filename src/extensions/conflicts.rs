//! `ztmux conflicts` — repositories stuck with unresolved merge conflicts.
//!
//! For each live pane in a git repo it counts the *unmerged* paths in
//! `git status --porcelain` — the files a merge, rebase, or cherry-pick left in
//! conflict — and reports only the repos that have any. Where [`super::changes`]
//! counts *all* uncommitted files, `conflicts` isolates the subset that blocks
//! progress: the repo is mid-operation and needs a resolution before anything
//! else. It is the "which checkout did I leave half-merged" board. Repos with no
//! conflicts (and non-repo panes) are omitted; each unique directory is resolved
//! once. Rows are sorted by conflict count, most first. With `-o json` /
//! `--json` it emits the same rows as a machine-readable array.

use std::collections::HashMap;
use std::io::IsTerminal;

use super::gitcmd::{git_out, repo_name};
use super::tmux_query::{Pane, poll};

/// The conflict state of one repo directory.
#[derive(Clone)]
struct Conflicts {
    root: String,
    branch: String,
    count: i64,
}

/// One output row: a pane and how many conflicted files its repo carries.
struct Row {
    id: String,
    location: String,
    repo: String,
    branch: String,
    count: i64,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux conflicts: {e}");
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

/// Resolve each unique live-pane directory once to its conflict state (or `None`
/// when it is not a repo), caching the miss too.
fn resolve_all(panes: &[Pane]) -> HashMap<String, Option<Conflicts>> {
    let mut map: HashMap<String, Option<Conflicts>> = HashMap::new();
    for p in panes {
        if p.dead || p.path.is_empty() || map.contains_key(&p.path) {
            continue;
        }
        map.insert(p.path.clone(), resolve_conflicts(&p.path));
    }
    map
}

/// Query git for the repo root, branch, and unmerged-path count. Returns `None`
/// when the directory is not inside a repository.
fn resolve_conflicts(path: &str) -> Option<Conflicts> {
    let root = git_out(path, &["rev-parse", "--show-toplevel"])?;
    let porcelain = git_out(path, &["status", "--porcelain"]).unwrap_or_default();
    let count = count_conflicts(&porcelain);
    let mut branch = git_out(path, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    if branch == "HEAD" || branch.is_empty() {
        branch = git_out(path, &["rev-parse", "--short", "HEAD"]).unwrap_or(branch);
    }
    Some(Conflicts {
        root,
        branch,
        count,
    })
}

/// `true` when a two-character porcelain status code marks an *unmerged* path.
/// Per `git status` porcelain: an entry is unmerged if either side is `U`, or it
/// is `DD` (both deleted) or `AA` (both added).
fn is_conflict(xy: &str) -> bool {
    let mut ch = xy.chars();
    let (Some(x), Some(y)) = (ch.next(), ch.next()) else {
        return false;
    };
    x == 'U' || y == 'U' || (x == 'D' && y == 'D') || (x == 'A' && y == 'A')
}

/// Count the unmerged paths in `git status --porcelain` output. The first two
/// characters of each non-blank line are the XY status code.
fn count_conflicts(porcelain: &str) -> i64 {
    porcelain
        .lines()
        .filter(|l| l.len() >= 2 && is_conflict(&l[..2]))
        .count() as i64
}

/// One row per live pane whose repo has conflicts, most first; ties break by
/// location.
fn build_rows(panes: &[Pane], info: &HashMap<String, Option<Conflicts>>) -> Vec<Row> {
    let mut rows: Vec<Row> = panes
        .iter()
        .filter(|p| !p.dead)
        .filter_map(|p| {
            let c = info.get(&p.path).and_then(|o| o.as_ref())?;
            if c.count == 0 {
                return None;
            }
            Some(Row {
                id: p.id.clone(),
                location: location(p),
                repo: repo_name(&c.root),
                branch: c.branch.clone(),
                count: c.count,
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
            &format!(
                "{:>9} {:<20} {:<16} {}",
                "CONFLICTS", "BRANCH", "REPO", "LOCATION"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:>9} {:<20} {:<16} {}\n",
            r.count, r.branch, r.repo, r.location,
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
                "repo": r.repo,
                "branch": r.branch,
                "conflicts": r.count,
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
    fn detects_unmerged_status_codes() {
        assert!(is_conflict("UU")); // both modified
        assert!(is_conflict("AA")); // both added
        assert!(is_conflict("DD")); // both deleted
        assert!(is_conflict("AU")); // added by us
        assert!(is_conflict("UD")); // deleted by them
        // Non-conflicts:
        assert!(!is_conflict(" M")); // modified, not unmerged
        assert!(!is_conflict("??")); // untracked
        assert!(!is_conflict("A ")); // staged add
        assert!(!is_conflict("MM"));
    }

    #[test]
    fn counts_only_conflicted_lines() {
        let porcelain = "UU src/a.rs\n M src/b.rs\n?? c.txt\nAA d.rs\n";
        assert_eq!(count_conflicts(porcelain), 2);
    }

    #[test]
    fn clean_or_merely_dirty_repo_counts_zero() {
        assert_eq!(count_conflicts(""), 0);
        assert_eq!(count_conflicts(" M a\n M b\n?? c\n"), 0);
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
    fn most_conflicted_first_and_clean_repos_omitted() {
        let panes = vec![
            pane("%1", 0, "/a"),
            pane("%2", 1, "/b"),
            pane("%3", 2, "/clean"),
        ];
        let mut info: HashMap<String, Option<Conflicts>> = HashMap::new();
        info.insert(
            "/a".into(),
            Some(Conflicts {
                root: "/a".into(),
                branch: "main".into(),
                count: 1,
            }),
        );
        info.insert(
            "/b".into(),
            Some(Conflicts {
                root: "/b".into(),
                branch: "merge".into(),
                count: 5,
            }),
        );
        info.insert(
            "/clean".into(),
            Some(Conflicts {
                root: "/clean".into(),
                branch: "main".into(),
                count: 0,
            }),
        );
        let rows = build_rows(&panes, &info);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].repo, "b");
        assert_eq!(rows[0].count, 5);
        assert_eq!(rows[1].repo, "a");
    }

    #[test]
    fn json_carries_conflicts_branch_and_repo() {
        let panes = vec![pane("%1", 0, "/a")];
        let mut info: HashMap<String, Option<Conflicts>> = HashMap::new();
        info.insert(
            "/a".into(),
            Some(Conflicts {
                root: "/home/u/a".into(),
                branch: "merge".into(),
                count: 3,
            }),
        );
        let rows = build_rows(&panes, &info);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["repo"], "a");
        assert_eq!(v[0]["branch"], "merge");
        assert_eq!(v[0]["conflicts"], 3);
    }
}
