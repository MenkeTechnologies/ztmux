//! `ztmux changes` — how much uncommitted work sits in every pane's repo.
//!
//! Where [`super::git`] reports a repo's dirty state as a yes/no and
//! [`super::ahead`] counts unpushed commits, `changes` counts the *files* with
//! uncommitted edits — the `git status --porcelain` line count — and ranks the
//! repos by it, dirtiest first. It is the "which repo have I left with the most
//! unsaved work" board, the pipeable primitive behind deciding where to commit
//! next. Clean repos (and non-repo panes) are omitted; each unique directory is
//! resolved once. With `-o json` / `--json` it emits the same rows as a
//! machine-readable array.

use std::collections::HashMap;
use std::io::IsTerminal;

use super::gitcmd::{git_out, repo_name};
use super::tmux_query::{Pane, poll};

/// The uncommitted-work state of one repo directory.
#[derive(Clone)]
struct Changes {
    root: String,
    branch: String,
    files: i64,
}

/// One output row: a pane and how many uncommitted files its repo carries.
struct Row {
    id: String,
    location: String,
    repo: String,
    branch: String,
    files: i64,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux changes: {e}");
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

/// Resolve each unique live-pane directory once to its change state (or `None`
/// when it is not a repo), caching the miss too.
fn resolve_all(panes: &[Pane]) -> HashMap<String, Option<Changes>> {
    let mut map: HashMap<String, Option<Changes>> = HashMap::new();
    for p in panes {
        if p.dead || p.path.is_empty() || map.contains_key(&p.path) {
            continue;
        }
        map.insert(p.path.clone(), resolve_changes(&p.path));
    }
    map
}

/// Query git for the repo root, branch, and porcelain change count. Returns
/// `None` when the directory is not inside a repository.
fn resolve_changes(path: &str) -> Option<Changes> {
    let root = git_out(path, &["rev-parse", "--show-toplevel"])?;
    let porcelain = git_out(path, &["status", "--porcelain"]).unwrap_or_default();
    let files = count_changes(&porcelain);
    let mut branch = git_out(path, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    if branch == "HEAD" || branch.is_empty() {
        branch = git_out(path, &["rev-parse", "--short", "HEAD"]).unwrap_or(branch);
    }
    Some(Changes {
        root,
        branch,
        files,
    })
}

/// Count the changed files in `git status --porcelain` output — one non-blank
/// line per changed path.
fn count_changes(porcelain: &str) -> i64 {
    porcelain.lines().filter(|l| !l.trim().is_empty()).count() as i64
}

/// One row per live pane whose repo has uncommitted files, dirtiest first; ties
/// break by location.
fn build_rows(panes: &[Pane], info: &HashMap<String, Option<Changes>>) -> Vec<Row> {
    let mut rows: Vec<Row> = panes
        .iter()
        .filter(|p| !p.dead)
        .filter_map(|p| {
            let c = info.get(&p.path).and_then(|o| o.as_ref())?;
            if c.files == 0 {
                return None;
            }
            Some(Row {
                id: p.id.clone(),
                location: location(p),
                repo: repo_name(&c.root),
                branch: c.branch.clone(),
                files: c.files,
            })
        })
        .collect();
    rows.sort_by(|a, b| b.files.cmp(&a.files).then(a.location.cmp(&b.location)));
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
                "{:>6} {:<20} {:<16} {}",
                "FILES", "BRANCH", "REPO", "LOCATION"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:>6} {:<20} {:<16} {}\n",
            r.files, r.branch, r.repo, r.location,
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
                "files": r.files,
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
    fn counts_non_blank_porcelain_lines() {
        let porcelain = " M src/a.rs\n?? new.txt\nA  staged.rs\n";
        assert_eq!(count_changes(porcelain), 3);
    }

    #[test]
    fn clean_repo_counts_zero() {
        assert_eq!(count_changes(""), 0);
        assert_eq!(count_changes("\n  \n"), 0);
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
    fn dirtiest_repo_sorts_first_and_clean_repos_omitted() {
        let panes = vec![
            pane("%1", 0, "/a"),
            pane("%2", 1, "/b"),
            pane("%3", 2, "/clean"),
        ];
        let mut info: HashMap<String, Option<Changes>> = HashMap::new();
        info.insert(
            "/a".into(),
            Some(Changes {
                root: "/a".into(),
                branch: "main".into(),
                files: 2,
            }),
        );
        info.insert(
            "/b".into(),
            Some(Changes {
                root: "/b".into(),
                branch: "dev".into(),
                files: 9,
            }),
        );
        info.insert(
            "/clean".into(),
            Some(Changes {
                root: "/clean".into(),
                branch: "main".into(),
                files: 0,
            }),
        );
        let rows = build_rows(&panes, &info);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].repo, "b");
        assert_eq!(rows[0].files, 9);
        assert_eq!(rows[1].repo, "a");
    }

    #[test]
    fn json_carries_files_branch_and_repo() {
        let panes = vec![pane("%1", 0, "/a")];
        let mut info: HashMap<String, Option<Changes>> = HashMap::new();
        info.insert(
            "/a".into(),
            Some(Changes {
                root: "/home/u/a".into(),
                branch: "main".into(),
                files: 4,
            }),
        );
        let rows = build_rows(&panes, &info);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["repo"], "a");
        assert_eq!(v[0]["branch"], "main");
        assert_eq!(v[0]["files"], 4);
    }
}
