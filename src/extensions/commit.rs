//! `ztmux commit` — the last commit in every pane's repository.
//!
//! For each live pane in a git repo it reads `git log -1` and reports the short
//! SHA, how long ago the commit landed, and its subject line, most-recent commit
//! first. Where [`super::git`] shows branch/dirty state, [`super::ahead`] shows
//! unpushed commits, and [`super::changes`] shows uncommitted files, `commit`
//! shows what was last *done* in each repo — the "what did I touch here, and
//! when" board across a wall of panes. Panes outside a repo are omitted; each
//! unique directory is resolved once. With `-o json` / `--json` it emits the
//! same rows (commit time as raw unix seconds) as a machine-readable array.

use std::collections::HashMap;
use std::io::IsTerminal;

use super::gitcmd::{git_out, repo_name};
use super::tmux_query::{Pane, poll};

/// The last commit of one repo directory.
#[derive(Clone)]
struct Commit {
    root: String,
    sha: String,
    when: i64, // committer time, unix seconds
    ago: String,
    subject: String,
}

/// One output row: a pane and its repo's last commit.
struct Row {
    id: String,
    location: String,
    repo: String,
    sha: String,
    ago: String,
    subject: String,
    when: i64,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux commit: {e}");
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

/// Resolve each unique live-pane directory once to its last commit (or `None`
/// when it is not a repo), caching the miss too.
fn resolve_all(panes: &[Pane]) -> HashMap<String, Option<Commit>> {
    let mut map: HashMap<String, Option<Commit>> = HashMap::new();
    for p in panes {
        if p.dead || p.path.is_empty() || map.contains_key(&p.path) {
            continue;
        }
        map.insert(p.path.clone(), resolve_commit(&p.path));
    }
    map
}

/// Query git for the repo root and the last commit's short SHA, committer time,
/// relative age, and subject. Returns `None` when the directory is not a repo
/// (or the repo has no commits yet).
fn resolve_commit(path: &str) -> Option<Commit> {
    let root = git_out(path, &["rev-parse", "--show-toplevel"])?;
    // %x1f is a literal unit-separator byte git inserts between fields.
    let line = git_out(path, &["log", "-1", "--format=%h%x1f%ct%x1f%cr%x1f%s"])?;
    let (sha, when, ago, subject) = parse_log(&line)?;
    Some(Commit {
        root,
        sha,
        when,
        ago,
        subject,
    })
}

/// Parse the unit-separator-delimited `git log -1` line into
/// `(sha, committer_unix_time, relative_age, subject)`.
fn parse_log(line: &str) -> Option<(String, i64, String, String)> {
    let mut it = line.split('\u{1f}');
    let sha = it.next()?.to_string();
    let when: i64 = it.next()?.parse().ok()?;
    let ago = it.next()?.to_string();
    let subject = it.next().unwrap_or("").to_string();
    if sha.is_empty() {
        return None;
    }
    Some((sha, when, ago, subject))
}

/// One row per live pane whose directory resolved to a repo, most-recent commit
/// first; ties break by location.
fn build_rows(panes: &[Pane], info: &HashMap<String, Option<Commit>>) -> Vec<Row> {
    let mut rows: Vec<Row> = panes
        .iter()
        .filter(|p| !p.dead)
        .filter_map(|p| {
            let c = info.get(&p.path).and_then(|o| o.as_ref())?;
            Some(Row {
                id: p.id.clone(),
                location: location(p),
                repo: repo_name(&c.root),
                sha: c.sha.clone(),
                ago: c.ago.clone(),
                subject: c.subject.clone(),
                when: c.when,
            })
        })
        .collect();
    rows.sort_by(|a, b| b.when.cmp(&a.when).then(a.location.cmp(&b.location)));
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
            &format!("{:<10} {:>14} {:<16} {}", "SHA", "AGE", "REPO", "SUBJECT"),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<10} {:>14} {:<16} {}\n",
            r.sha, r.ago, r.repo, r.subject,
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
                "sha": r.sha,
                "when": r.when,
                "ago": r.ago,
                "subject": r.subject,
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
    fn parses_unit_separated_log_line() {
        let line = "abc123\u{1f}1700000000\u{1f}2 days ago\u{1f}Fix the parser";
        let (sha, when, ago, subject) = parse_log(line).unwrap();
        assert_eq!(sha, "abc123");
        assert_eq!(when, 1_700_000_000);
        assert_eq!(ago, "2 days ago");
        assert_eq!(subject, "Fix the parser");
    }

    #[test]
    fn subject_may_be_empty_but_sha_required() {
        // Empty subject is allowed.
        assert!(parse_log("abc\u{1f}100\u{1f}now\u{1f}").is_some());
        // Missing/empty sha is rejected.
        assert!(parse_log("\u{1f}100\u{1f}now\u{1f}x").is_none());
        assert!(parse_log("garbage").is_none());
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

    fn commit(root: &str, sha: &str, when: i64, subject: &str) -> Commit {
        Commit {
            root: root.into(),
            sha: sha.into(),
            when,
            ago: "x".into(),
            subject: subject.into(),
        }
    }

    #[test]
    fn most_recent_commit_sorts_first_and_non_repo_omitted() {
        let panes = vec![
            pane("%1", 0, "/old"),
            pane("%2", 1, "/new"),
            pane("%3", 2, "/x"),
        ];
        let mut info: HashMap<String, Option<Commit>> = HashMap::new();
        info.insert("/old".into(), Some(commit("/old", "aaa", 100, "old work")));
        info.insert("/new".into(), Some(commit("/new", "bbb", 900, "new work")));
        info.insert("/x".into(), None);
        let rows = build_rows(&panes, &info);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].repo, "new");
        assert_eq!(rows[1].repo, "old");
    }

    #[test]
    fn json_carries_sha_when_and_subject() {
        let panes = vec![pane("%1", 0, "/r")];
        let mut info: HashMap<String, Option<Commit>> = HashMap::new();
        info.insert(
            "/r".into(),
            Some(commit("/home/u/r", "deadbee", 1_700_000_000, "Do a thing")),
        );
        let rows = build_rows(&panes, &info);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["repo"], "r");
        assert_eq!(v[0]["sha"], "deadbee");
        assert_eq!(v[0]["when"], 1_700_000_000);
        assert_eq!(v[0]["subject"], "Do a thing");
    }
}
