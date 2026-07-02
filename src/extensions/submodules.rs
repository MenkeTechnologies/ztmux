//! `ztmux submodules` — repositories with submodules, and how many are off.
//!
//! For each live pane in a git repo it runs `git submodule status` and reports
//! the repos that have any submodules, with a count of how many are *out of
//! sync* — not initialised, or checked out at a different commit than the
//! superproject records. Where [`super::changes`] and [`super::ahead`] describe
//! the superproject's own state, `submodules` describes its dependencies: the
//! nested checkouts that quietly drift and break a build. Repos with no
//! submodules (and non-repo panes) are omitted; those with drift sort first.
//! Each unique directory is resolved once. With `-o json` / `--json` it emits the
//! same rows as a machine-readable array.

use std::collections::HashMap;
use std::io::IsTerminal;

use super::gitcmd::{git_out, repo_name};
use super::tmux_query::{Pane, poll};

/// The submodule state of one repo directory.
#[derive(Clone)]
struct Submodules {
    root: String,
    total: i64,
    out_of_sync: i64,
}

/// One output row: a pane and its repo's submodule state.
struct Row {
    id: String,
    location: String,
    repo: String,
    total: i64,
    out_of_sync: i64,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux submodules: {e}");
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

/// Resolve each unique live-pane directory once to its submodule state (or
/// `None` when it is not a repo), caching the miss too.
fn resolve_all(panes: &[Pane]) -> HashMap<String, Option<Submodules>> {
    let mut map: HashMap<String, Option<Submodules>> = HashMap::new();
    for p in panes {
        if p.dead || p.path.is_empty() || map.contains_key(&p.path) {
            continue;
        }
        map.insert(p.path.clone(), resolve_submodules(&p.path));
    }
    map
}

/// Query git for the repo root and `git submodule status`. Returns `None` when
/// the directory is not inside a repository.
fn resolve_submodules(path: &str) -> Option<Submodules> {
    let root = git_out(path, &["rev-parse", "--show-toplevel"])?;
    let status = git_out(path, &["submodule", "status"]).unwrap_or_default();
    let (total, out_of_sync) = parse_status(&status);
    Some(Submodules {
        root,
        total,
        out_of_sync,
    })
}

/// Parse `git submodule status` into `(total, out_of_sync)`. Each non-blank line
/// is one submodule; its first character is the state: a space means in sync,
/// anything else (`-` uninitialised, `+` different commit, `U` conflicts) is out
/// of sync.
fn parse_status(status: &str) -> (i64, i64) {
    let mut total = 0;
    let mut out = 0;
    for line in status.lines() {
        if line.trim().is_empty() {
            continue;
        }
        total += 1;
        if !line.starts_with(' ') {
            out += 1;
        }
    }
    (total, out)
}

/// One row per live pane whose repo has submodules, most-out-of-sync first, then
/// most submodules, then location.
fn build_rows(panes: &[Pane], info: &HashMap<String, Option<Submodules>>) -> Vec<Row> {
    let mut rows: Vec<Row> = panes
        .iter()
        .filter(|p| !p.dead)
        .filter_map(|p| {
            let s = info.get(&p.path).and_then(|o| o.as_ref())?;
            if s.total == 0 {
                return None;
            }
            Some(Row {
                id: p.id.clone(),
                location: location(p),
                repo: repo_name(&s.root),
                total: s.total,
                out_of_sync: s.out_of_sync,
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        b.out_of_sync
            .cmp(&a.out_of_sync)
            .then(b.total.cmp(&a.total))
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
                "{:>6} {:>7} {:<16} {}",
                "TOTAL", "OUTSYNC", "REPO", "LOCATION"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:>6} {:>7} {:<16} {}\n",
            r.total, r.out_of_sync, r.repo, r.location,
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
                "total": r.total,
                "out_of_sync": r.out_of_sync,
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
    fn parses_in_sync_and_out_of_sync_submodules() {
        // A leading space = in sync; '-' = uninitialised; '+' = different commit.
        let status = " abc123 vendor/a (v1.0)\n-def456 vendor/b\n+aaa111 vendor/c (v2.0-3-g…)\n";
        assert_eq!(parse_status(status), (3, 2));
    }

    #[test]
    fn no_submodules_counts_zero() {
        assert_eq!(parse_status(""), (0, 0));
        assert_eq!(parse_status("\n  \n"), (0, 0));
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
    fn out_of_sync_repos_sort_first_and_submodule_less_repos_omitted() {
        let panes = vec![
            pane("%1", 0, "/clean"),
            pane("%2", 1, "/drift"),
            pane("%3", 2, "/none"),
        ];
        let mut info: HashMap<String, Option<Submodules>> = HashMap::new();
        info.insert(
            "/clean".into(),
            Some(Submodules {
                root: "/clean".into(),
                total: 3,
                out_of_sync: 0,
            }),
        );
        info.insert(
            "/drift".into(),
            Some(Submodules {
                root: "/drift".into(),
                total: 2,
                out_of_sync: 2,
            }),
        );
        info.insert(
            "/none".into(),
            Some(Submodules {
                root: "/none".into(),
                total: 0,
                out_of_sync: 0,
            }),
        );
        let rows = build_rows(&panes, &info);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].repo, "drift"); // 2 out of sync
        assert_eq!(rows[1].repo, "clean");
    }

    #[test]
    fn json_carries_totals() {
        let panes = vec![pane("%1", 0, "/a")];
        let mut info: HashMap<String, Option<Submodules>> = HashMap::new();
        info.insert(
            "/a".into(),
            Some(Submodules {
                root: "/home/u/a".into(),
                total: 4,
                out_of_sync: 1,
            }),
        );
        let rows = build_rows(&panes, &info);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["repo"], "a");
        assert_eq!(v[0]["total"], 4);
        assert_eq!(v[0]["out_of_sync"], 1);
    }
}
