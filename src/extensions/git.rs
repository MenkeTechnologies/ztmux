//! `ztmux git` — the git branch and dirty state of every pane sitting in a repo.
//!
//! For each live pane's working directory it asks git for the repo root, the
//! current branch (or short SHA when detached), and whether the tree is dirty,
//! then prints one row per pane that is inside a repository. Where
//! [`super::dedup`] groups panes by raw `(cwd, command)` and [`super::ports`]
//! attributes listening sockets, `git` answers "which pane is on which branch,
//! and is it dirty" across the whole server — the multi-repo, multi-pane status
//! board. It degrades quietly: panes outside a repo (or when `git` is missing)
//! are simply omitted. Unique directories are resolved once. Output is a table
//! (coloured on a TTY) or a JSON array with `-o json` / `--json`, sorted by
//! repo, branch, then location.

use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::Path;
use std::process::Command;

use super::tmux_query::{Pane, poll};

/// What git reports about one working directory.
#[derive(Clone)]
struct GitInfo {
    root: String,
    branch: String,
    dirty: bool,
}

/// One output row: a pane and the repo/branch it is sitting in.
struct Row {
    id: String,
    location: String,
    command: String,
    repo: String,
    branch: String,
    dirty: bool,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux git: {e}");
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

/// Resolve every unique live-pane directory exactly once, mapping it to its
/// git state (or `None` when the directory is not inside a repository). The
/// `None` is cached too, so repeated non-repo paths do not re-spawn git.
fn resolve_all(panes: &[Pane]) -> HashMap<String, Option<GitInfo>> {
    let mut map: HashMap<String, Option<GitInfo>> = HashMap::new();
    for p in panes {
        if p.dead || p.path.is_empty() || map.contains_key(&p.path) {
            continue;
        }
        map.insert(p.path.clone(), resolve_git(&p.path));
    }
    map
}

/// Run git against one directory. Returns `None` on any failure (not a repo,
/// git missing, etc.) so the extension degrades to fewer rows rather than an
/// error.
fn resolve_git(path: &str) -> Option<GitInfo> {
    let root = git_out(path, &["rev-parse", "--show-toplevel"])?;
    let mut branch = git_out(path, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    // Detached HEAD reports the literal "HEAD"; show the short SHA instead.
    if branch == "HEAD" {
        branch = git_out(path, &["rev-parse", "--short", "HEAD"]).unwrap_or(branch);
    }
    let dirty = !git_out(path, &["status", "--porcelain"])
        .unwrap_or_default()
        .is_empty();
    Some(GitInfo {
        root,
        branch,
        dirty,
    })
}

/// One `git -C <path> <args…>` invocation; the trimmed stdout on success, else
/// `None`.
fn git_out(path: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The last path component of a repo root (`/home/u/proj` → `proj`), for the
/// compact REPO column.
fn repo_name(root: &str) -> String {
    Path::new(root)
        .file_name()
        .map_or_else(|| root.to_string(), |n| n.to_string_lossy().into_owned())
}

/// One row per live pane whose directory resolved to a repo, sorted by repo,
/// branch, then location for a stable, greppable board.
fn build_rows(panes: &[Pane], info: &HashMap<String, Option<GitInfo>>) -> Vec<Row> {
    let mut rows: Vec<Row> = panes
        .iter()
        .filter(|p| !p.dead)
        .filter_map(|p| {
            let gi = info.get(&p.path).and_then(|o| o.as_ref())?;
            Some(Row {
                id: p.id.clone(),
                location: location(p),
                command: p.command.clone(),
                repo: repo_name(&gi.root),
                branch: gi.branch.clone(),
                dirty: gi.dirty,
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        a.repo
            .cmp(&b.repo)
            .then(a.branch.cmp(&b.branch))
            .then(a.location.cmp(&b.location))
    });
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
                "{:<8} {:<16} {:<12} {:<16} {:<16} {}",
                "PANE", "LOCATION", "COMMAND", "REPO", "BRANCH", "DIRTY"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<8} {:<16} {:<12} {:<16} {:<16} {}\n",
            r.id,
            r.location,
            r.command,
            r.repo,
            r.branch,
            if r.dirty { "yes" } else { "no" },
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
                "repo": r.repo,
                "branch": r.branch,
                "dirty": r.dirty,
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

    fn info(pairs: &[(&str, Option<(&str, &str, bool)>)]) -> HashMap<String, Option<GitInfo>> {
        pairs
            .iter()
            .map(|(path, gi)| {
                (
                    (*path).to_string(),
                    gi.map(|(root, branch, dirty)| GitInfo {
                        root: root.to_string(),
                        branch: branch.to_string(),
                        dirty,
                    }),
                )
            })
            .collect()
    }

    #[test]
    fn repo_name_takes_the_last_component() {
        assert_eq!(repo_name("/home/u/proj"), "proj");
        assert_eq!(repo_name("/repo"), "repo");
    }

    #[test]
    fn panes_outside_a_repo_are_omitted() {
        let panes = vec![
            pane("%1", "a", 0, 0, "zsh", "/home/u/proj"),
            pane("%2", "a", 1, 0, "zsh", "/tmp"),
        ];
        let map = info(&[
            ("/home/u/proj", Some(("/home/u/proj", "main", false))),
            ("/tmp", None),
        ]);
        let rows = build_rows(&panes, &map);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%1");
        assert_eq!(rows[0].repo, "proj");
        assert_eq!(rows[0].branch, "main");
        assert!(!rows[0].dirty);
    }

    #[test]
    fn dead_and_pathless_panes_drop_out() {
        let mut dead = pane("%2", "a", 1, 0, "zsh", "/home/u/proj");
        dead.dead = true;
        let panes = vec![
            pane("%1", "a", 0, 0, "zsh", "/home/u/proj"),
            dead,
            pane("%3", "a", 2, 0, "zsh", ""),
        ];
        let map = info(&[("/home/u/proj", Some(("/home/u/proj", "main", true)))]);
        let rows = build_rows(&panes, &map);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%1");
        assert!(rows[0].dirty);
    }

    #[test]
    fn rows_sorted_by_repo_then_branch_then_location() {
        let panes = vec![
            pane("%1", "z", 9, 0, "zsh", "/r/beta"),
            pane("%2", "a", 0, 0, "zsh", "/r/alpha"),
            pane("%3", "b", 0, 0, "zsh", "/r/alpha-dev"),
        ];
        let map = info(&[
            ("/r/beta", Some(("/r/beta", "main", false))),
            ("/r/alpha", Some(("/r/alpha", "main", false))),
            ("/r/alpha-dev", Some(("/r/alpha", "feature", false))),
        ]);
        let rows = build_rows(&panes, &map);
        // Both /r/alpha* map to repo "alpha"; "feature" < "main" by branch.
        assert_eq!(
            rows.iter()
                .map(|r| (r.repo.as_str(), r.branch.as_str()))
                .collect::<Vec<_>>(),
            vec![("alpha", "feature"), ("alpha", "main"), ("beta", "main")],
        );
    }

    #[test]
    fn text_has_header_and_dirty_flag() {
        let panes = vec![pane("%1", "a", 0, 0, "zsh", "/r/x")];
        let map = info(&[("/r/x", Some(("/r/x", "main", true)))]);
        let s = render_text(&build_rows(&panes, &map), false);
        assert!(s.contains("REPO") && s.contains("BRANCH") && s.contains("DIRTY"));
        assert!(
            s.lines()
                .any(|l| l.contains("main") && l.trim_end().ends_with("yes"))
        );
    }

    #[test]
    fn json_carries_repo_branch_dirty() {
        let panes = vec![pane("%1", "a", 0, 0, "nvim", "/r/x")];
        let map = info(&[("/r/x", Some(("/r/x", "dev", true)))]);
        let v: serde_json::Value =
            serde_json::from_str(&render_json(&build_rows(&panes, &map))).unwrap();
        assert_eq!(v[0]["repo"], "x");
        assert_eq!(v[0]["branch"], "dev");
        assert_eq!(v[0]["dirty"], true);
        assert_eq!(v[0]["command"], "nvim");
    }
}
