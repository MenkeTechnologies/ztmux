//! `ztmux ahead` — how far each pane's repo is ahead of / behind its upstream.
//!
//! For every live pane in a git repo whose branch tracks an upstream, it asks
//! git how many commits the branch is ahead (unpushed) and behind (unpulled),
//! and reports both. Where [`super::git`] shows branch and dirty state and
//! [`super::remote`] shows the repo identity, `ahead` shows *sync distance* — the
//! "which repos have I left with unpushed commits, and which need a pull" board.
//! Rows are sorted with the most out-of-sync repos first, so the ones needing a
//! push or pull float to the top. Panes outside a repo, or on a branch with no
//! upstream, are omitted; each unique directory is resolved once. With `-o json`
//! / `--json` it emits the same rows as a machine-readable array.

use std::collections::HashMap;
use std::io::IsTerminal;

use super::gitcmd::{git_out, repo_name};
use super::tmux_query::{Pane, poll};

/// Upstream sync state for one repo directory.
#[derive(Clone)]
struct Sync {
    root: String,
    branch: String,
    ahead: i64,
    behind: i64,
}

/// One output row: a pane and its repo's sync distance from upstream.
struct Row {
    id: String,
    location: String,
    repo: String,
    branch: String,
    ahead: i64,
    behind: i64,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux ahead: {e}");
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

/// Resolve each unique live-pane directory once to its sync state (or `None`
/// when it is not a repo or has no upstream), caching the miss too.
fn resolve_all(panes: &[Pane]) -> HashMap<String, Option<Sync>> {
    let mut map: HashMap<String, Option<Sync>> = HashMap::new();
    for p in panes {
        if p.dead || p.path.is_empty() || map.contains_key(&p.path) {
            continue;
        }
        map.insert(p.path.clone(), resolve_sync(&p.path));
    }
    map
}

/// Query git for the repo root, current branch, and ahead/behind counts against
/// `@{upstream}`. Returns `None` on any failure — no repo, or (most commonly) a
/// branch with no upstream configured.
fn resolve_sync(path: &str) -> Option<Sync> {
    let root = git_out(path, &["rev-parse", "--show-toplevel"])?;
    let counts = git_out(
        path,
        &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"],
    )?;
    let (behind, ahead) = parse_counts(&counts)?;
    let mut branch = git_out(path, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    if branch == "HEAD" || branch.is_empty() {
        branch = git_out(path, &["rev-parse", "--short", "HEAD"]).unwrap_or(branch);
    }
    Some(Sync {
        root,
        branch,
        ahead,
        behind,
    })
}

/// Parse the two whitespace-separated integers `git rev-list --left-right
/// --count @{upstream}...HEAD` prints, as `(behind, ahead)` — left is
/// upstream-only commits (behind), right is HEAD-only commits (ahead).
fn parse_counts(s: &str) -> Option<(i64, i64)> {
    let mut it = s.split_whitespace();
    let behind = it.next()?.parse().ok()?;
    let ahead = it.next()?.parse().ok()?;
    Some((behind, ahead))
}

/// One row per live pane whose repo has an upstream, most out-of-sync first
/// (by ahead+behind), ties broken by location.
fn build_rows(panes: &[Pane], info: &HashMap<String, Option<Sync>>) -> Vec<Row> {
    let mut rows: Vec<Row> = panes
        .iter()
        .filter(|p| !p.dead)
        .filter_map(|p| {
            let s = info.get(&p.path).and_then(|o| o.as_ref())?;
            Some(Row {
                id: p.id.clone(),
                location: location(p),
                repo: repo_name(&s.root),
                branch: s.branch.clone(),
                ahead: s.ahead,
                behind: s.behind,
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        (b.ahead + b.behind)
            .cmp(&(a.ahead + a.behind))
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
                "{:>6} {:>6} {:<20} {:<16} {}",
                "AHEAD", "BEHIND", "BRANCH", "REPO", "LOCATION"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:>6} {:>6} {:<20} {:<16} {}\n",
            r.ahead, r.behind, r.branch, r.repo, r.location,
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
                "ahead": r.ahead,
                "behind": r.behind,
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
    fn parses_tab_separated_counts_as_behind_ahead() {
        assert_eq!(parse_counts("3\t5"), Some((3, 5)));
    }

    #[test]
    fn parses_space_separated_and_zero_counts() {
        assert_eq!(parse_counts("0 0"), Some((0, 0)));
        assert_eq!(parse_counts("  12   7  "), Some((12, 7)));
    }

    #[test]
    fn rejects_malformed_counts() {
        assert_eq!(parse_counts("nope"), None);
        assert_eq!(parse_counts("5"), None);
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
    fn most_out_of_sync_repo_sorts_first_and_non_upstream_omitted() {
        let panes = vec![
            pane("%1", 0, "/near"),
            pane("%2", 1, "/far"),
            pane("%3", 2, "/plain"),
        ];
        let mut info: HashMap<String, Option<Sync>> = HashMap::new();
        info.insert(
            "/near".into(),
            Some(Sync {
                root: "/near".into(),
                branch: "main".into(),
                ahead: 1,
                behind: 0,
            }),
        );
        info.insert(
            "/far".into(),
            Some(Sync {
                root: "/far".into(),
                branch: "dev".into(),
                ahead: 4,
                behind: 3,
            }),
        );
        info.insert("/plain".into(), None); // no upstream
        let rows = build_rows(&panes, &info);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].repo, "far"); // 7 total out-of-sync
        assert_eq!(rows[1].repo, "near"); // 1
    }

    #[test]
    fn json_carries_ahead_behind_and_branch() {
        let panes = vec![pane("%1", 0, "/r")];
        let mut info: HashMap<String, Option<Sync>> = HashMap::new();
        info.insert(
            "/r".into(),
            Some(Sync {
                root: "/home/u/r".into(),
                branch: "main".into(),
                ahead: 2,
                behind: 5,
            }),
        );
        let rows = build_rows(&panes, &info);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["repo"], "r");
        assert_eq!(v[0]["branch"], "main");
        assert_eq!(v[0]["ahead"], 2);
        assert_eq!(v[0]["behind"], 5);
    }
}
