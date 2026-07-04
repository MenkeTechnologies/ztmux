//! `ztmux remote` — the git remote each pane's repository points at.
//!
//! For every live pane sitting in a git repo it reads the `origin` remote URL
//! and prints it in a compact `host/owner/repo` form. Where [`super::git`]
//! reports branch and dirty state (the working-tree condition) and
//! [`super::ahead`] reports sync distance, `remote` answers *which* repository a
//! pane is in — the identity, the GitHub/GitLab slug — so a wall of panes maps
//! to the actual projects behind them. It degrades quietly: panes outside a repo
//! or with no `origin` are omitted, and each unique directory is resolved once.
//! With `-o json` / `--json` it emits the same rows (including the raw URL) as a
//! machine-readable array; sorted by remote, then location.

use std::collections::HashMap;
use std::io::IsTerminal;

use super::gitcmd::git_out;
use super::tmux_query::{Pane, poll};

/// One output row: a pane and the remote its repo points at.
struct Row {
    id: String,
    location: String,
    remote: String, // compact host/owner/repo
    url: String,    // raw origin URL
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux remote: {e}");
        return 1;
    }
    let urls = resolve_all(&snap.panes);
    let rows = build_rows(&snap.panes, &urls);
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

/// Resolve each unique live-pane directory once to its `origin` URL (or `None`),
/// caching the miss too so non-repo paths are not re-spawned.
fn resolve_all(panes: &[Pane]) -> HashMap<String, Option<String>> {
    let mut map: HashMap<String, Option<String>> = HashMap::new();
    for p in panes {
        if p.dead || p.path.is_empty() || map.contains_key(&p.path) {
            continue;
        }
        map.insert(p.path.clone(), origin_url(&p.path));
    }
    map
}

/// `git -C <path> remote get-url origin`, trimmed, or `None` on any failure
/// (including an empty result).
fn origin_url(path: &str) -> Option<String> {
    git_out(path, &["remote", "get-url", "origin"]).filter(|u| !u.is_empty())
}

/// Normalise a git remote URL to a compact `host/owner/repo`, covering the
/// common `scp`-like SSH (`git@host:owner/repo.git`), `ssh://`, and `https://`
/// forms. Anything that does not parse is returned trimmed of a trailing
/// `.git`, so an unusual URL still shows something meaningful.
fn short_remote(url: &str) -> String {
    let trimmed = url.trim();
    // scp-like SSH: git@host:owner/repo(.git)
    if let Some(rest) = trimmed.strip_prefix("git@")
        && let Some((host, path)) = rest.split_once(':')
    {
        return format!("{host}/{}", strip_git(path));
    }
    // scheme://[user@]host[:port]/owner/repo(.git)
    for scheme in ["ssh://", "https://", "http://", "git://"] {
        if let Some(rest) = trimmed.strip_prefix(scheme) {
            let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
            let host = authority.rsplit('@').next().unwrap_or(authority);
            let host = host.split(':').next().unwrap_or(host); // drop :port
            return format!("{host}/{}", strip_git(path));
        }
    }
    strip_git(trimmed).to_string()
}

/// Drop a single trailing `.git` and any leading slash from a repo path.
fn strip_git(path: &str) -> &str {
    path.trim_start_matches('/')
        .strip_suffix(".git")
        .unwrap_or_else(|| path.trim_start_matches('/'))
}

/// One row per live pane whose directory has an `origin`, sorted by remote then
/// location for a stable board.
fn build_rows(panes: &[Pane], urls: &HashMap<String, Option<String>>) -> Vec<Row> {
    let mut rows: Vec<Row> = panes
        .iter()
        .filter(|p| !p.dead)
        .filter_map(|p| {
            let url = urls.get(&p.path).and_then(|o| o.as_ref())?;
            Some(Row {
                id: p.id.clone(),
                location: location(p),
                remote: short_remote(url),
                url: url.clone(),
            })
        })
        .collect();
    rows.sort_by(|a, b| a.remote.cmp(&b.remote).then(a.location.cmp(&b.location)));
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
                "{:<8} {:<16} {:<30} {}",
                "PANE", "LOCATION", "REMOTE", "URL"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<8} {:<16} {:<30} {}\n",
            r.id, r.location, r.remote, r.url,
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
                "remote": r.remote,
                "url": r.url,
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
    fn normalises_scp_like_ssh_url() {
        assert_eq!(
            short_remote("git@github.com:MenkeTechnologies/ztmux.git"),
            "github.com/MenkeTechnologies/ztmux"
        );
    }

    #[test]
    fn normalises_https_url() {
        assert_eq!(
            short_remote("https://github.com/MenkeTechnologies/ztmux.git"),
            "github.com/MenkeTechnologies/ztmux"
        );
    }

    #[test]
    fn normalises_ssh_scheme_with_user_and_port() {
        assert_eq!(
            short_remote("ssh://git@gitlab.com:22/group/sub/proj.git"),
            "gitlab.com/group/sub/proj"
        );
    }

    #[test]
    fn keeps_something_useful_for_unusual_urls() {
        // No recognised scheme: just drop the trailing .git.
        assert_eq!(short_remote("/srv/git/bare.git"), "srv/git/bare");
    }

    #[test]
    fn build_rows_omits_non_repo_panes_and_sorts_by_remote() {
        let panes = vec![
            Pane {
                id: "%1".into(),
                session: "a".into(),
                window: 0,
                index: 0,
                path: "/z".into(),
                ..Default::default()
            },
            Pane {
                id: "%2".into(),
                session: "a".into(),
                window: 0,
                index: 1,
                path: "/a".into(),
                ..Default::default()
            },
            Pane {
                id: "%3".into(),
                session: "a".into(),
                window: 0,
                index: 2,
                path: "/none".into(),
                ..Default::default()
            },
        ];
        let mut urls: HashMap<String, Option<String>> = HashMap::new();
        urls.insert("/z".into(), Some("git@github.com:o/zzz.git".into()));
        urls.insert("/a".into(), Some("git@github.com:o/aaa.git".into()));
        urls.insert("/none".into(), None);
        let rows = build_rows(&panes, &urls);
        assert_eq!(rows.len(), 2);
        // Sorted by remote: aaa before zzz.
        assert_eq!(rows[0].remote, "github.com/o/aaa");
        assert_eq!(rows[1].remote, "github.com/o/zzz");
    }

    #[test]
    fn json_carries_remote_and_raw_url() {
        let panes = vec![Pane {
            id: "%1".into(),
            session: "a".into(),
            window: 0,
            index: 0,
            path: "/a".into(),
            ..Default::default()
        }];
        let mut urls: HashMap<String, Option<String>> = HashMap::new();
        urls.insert("/a".into(), Some("https://github.com/o/r.git".into()));
        let rows = build_rows(&panes, &urls);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["remote"], "github.com/o/r");
        assert_eq!(v[0]["url"], "https://github.com/o/r.git");
    }
}
