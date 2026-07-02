//! `ztmux tag` — the tagged-version context of every pane's repository.
//!
//! For each live pane in a git repo it runs `git describe --tags` and reports
//! the result: the nearest tag, and how many commits the checkout is past it
//! (e.g. `v1.2.3-4-gabc123` — four commits beyond `v1.2.3`). Where
//! [`super::commit`] shows the *last commit* and [`super::ahead`] shows distance
//! from *upstream*, `tag` shows distance from the last *release* — the "which
//! version is each working tree near, and how far past it" board. A checkout
//! sitting exactly on a tag is flagged. Repos with no tags reachable, and
//! non-repo panes, are omitted; each unique directory is resolved once. With
//! `-o json` / `--json` it emits the same rows as a machine-readable array.

use std::collections::HashMap;
use std::io::IsTerminal;

use super::gitcmd::{git_out, repo_name};
use super::tmux_query::{Pane, poll};

/// The describe result for one repo directory.
#[derive(Clone)]
struct Tag {
    root: String,
    describe: String,
    exact: bool,
}

/// One output row: a pane and its repo's tagged-version context.
struct Row {
    id: String,
    location: String,
    repo: String,
    describe: String,
    exact: bool,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux tag: {e}");
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

/// Resolve each unique live-pane directory once to its describe result (or
/// `None` when it is not a repo or has no reachable tag), caching the miss too.
fn resolve_all(panes: &[Pane]) -> HashMap<String, Option<Tag>> {
    let mut map: HashMap<String, Option<Tag>> = HashMap::new();
    for p in panes {
        if p.dead || p.path.is_empty() || map.contains_key(&p.path) {
            continue;
        }
        map.insert(p.path.clone(), resolve_tag(&p.path));
    }
    map
}

/// Query git for the repo root and `git describe --tags`. Returns `None` when
/// the directory is not a repo, or has no tag reachable from `HEAD`.
fn resolve_tag(path: &str) -> Option<Tag> {
    let root = git_out(path, &["rev-parse", "--show-toplevel"])?;
    // Without `--always`, describe fails (→ None) when no tag is reachable,
    // which is exactly the "omit this repo" case we want.
    let describe = git_out(path, &["describe", "--tags"])?;
    if describe.is_empty() {
        return None;
    }
    let exact = is_exact_tag(&describe);
    Some(Tag {
        root,
        describe,
        exact,
    })
}

/// `true` when a `git describe --tags` string names a commit sitting exactly on
/// a tag — i.e. it lacks the `-<n>-g<sha>` "commits past the tag" suffix that
/// git appends when `HEAD` is ahead of the nearest tag.
fn is_exact_tag(describe: &str) -> bool {
    // git appends "-<count>-g<abbrev>" when past a tag. Detect that suffix:
    // split on '-' and check whether the last two fields are <number> and
    // g<hex>. A tag containing '-' (e.g. "v1.0-rc1") is fine — only the trailing
    // count/hash pair marks "past the tag".
    let parts: Vec<&str> = describe.rsplitn(3, '-').collect();
    // rsplitn(3) yields [last, secondlast, rest] for 3+ segments.
    if parts.len() < 3 {
        return true;
    }
    let g_abbrev = parts[0]; // e.g. "gabc1234"
    let count = parts[1]; // e.g. "4"
    let looks_like_past = g_abbrev.starts_with('g')
        && g_abbrev.len() > 1
        && g_abbrev[1..].chars().all(|c| c.is_ascii_hexdigit())
        && !count.is_empty()
        && count.chars().all(|c| c.is_ascii_digit());
    !looks_like_past
}

/// One row per live pane whose repo has a reachable tag, sorted by repo then
/// location.
fn build_rows(panes: &[Pane], info: &HashMap<String, Option<Tag>>) -> Vec<Row> {
    let mut rows: Vec<Row> = panes
        .iter()
        .filter(|p| !p.dead)
        .filter_map(|p| {
            let t = info.get(&p.path).and_then(|o| o.as_ref())?;
            Some(Row {
                id: p.id.clone(),
                location: location(p),
                repo: repo_name(&t.root),
                describe: t.describe.clone(),
                exact: t.exact,
            })
        })
        .collect();
    rows.sort_by(|a, b| a.repo.cmp(&b.repo).then(a.location.cmp(&b.location)));
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
            &format!("{:<24} {:<6} {:<16} {}", "TAG", "EXACT", "REPO", "LOCATION"),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<24} {:<6} {:<16} {}\n",
            r.describe,
            if r.exact { "yes" } else { "no" },
            r.repo,
            r.location,
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
                "describe": r.describe,
                "exact": r.exact,
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
    fn exact_tag_has_no_past_suffix() {
        assert!(is_exact_tag("v1.2.3"));
        assert!(is_exact_tag("v1.0-rc1")); // hyphen in tag, but no count/hash
        assert!(is_exact_tag("release"));
    }

    #[test]
    fn describe_past_a_tag_is_not_exact() {
        assert!(!is_exact_tag("v1.2.3-4-gabc1234"));
        assert!(!is_exact_tag("v1.0-rc1-10-gdeadbee"));
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
    fn rows_sorted_by_repo_and_non_tagged_omitted() {
        let panes = vec![
            pane("%1", 0, "/z"),
            pane("%2", 1, "/a"),
            pane("%3", 2, "/none"),
        ];
        let mut info: HashMap<String, Option<Tag>> = HashMap::new();
        info.insert(
            "/z".into(),
            Some(Tag {
                root: "/z".into(),
                describe: "v2.0".into(),
                exact: true,
            }),
        );
        info.insert(
            "/a".into(),
            Some(Tag {
                root: "/a".into(),
                describe: "v1.0-3-gabc".into(),
                exact: false,
            }),
        );
        info.insert("/none".into(), None);
        let rows = build_rows(&panes, &info);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].repo, "a");
        assert_eq!(rows[1].repo, "z");
    }

    #[test]
    fn json_carries_describe_and_exact() {
        let panes = vec![pane("%1", 0, "/r")];
        let mut info: HashMap<String, Option<Tag>> = HashMap::new();
        info.insert(
            "/r".into(),
            Some(Tag {
                root: "/home/u/r".into(),
                describe: "v1.2.3".into(),
                exact: true,
            }),
        );
        let rows = build_rows(&panes, &info);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["repo"], "r");
        assert_eq!(v[0]["describe"], "v1.2.3");
        assert_eq!(v[0]["exact"], true);
    }
}
