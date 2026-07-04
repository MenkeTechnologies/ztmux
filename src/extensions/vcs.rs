//! `ztmux vcs` — which version-control system each pane's directory is under.
//!
//! For each live pane it walks up from the working directory to the nearest
//! version-control marker — `.git`, `.hg`, `.svn`, `.jj`, and friends — and
//! reports which system it found and where its root is. Where [`super::git`] and
//! its siblings assume git and report git-specific state, `vcs` answers the
//! prior question: *is* this a checkout, and of what. In a polyglot tree with a
//! Mercurial repo here and a Subversion checkout there, it is the one view that
//! names each. It is filesystem-only (marker lookups, no subprocess) and resolves
//! each unique directory once. Panes not under any recognised VCS are omitted.
//! With `-o json` / `--json` it emits the same rows as a machine-readable array;
//! sorted by system, then root, then location.

use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use super::tmux_query::{Pane, poll};

/// VCS markers in priority order: the first that exists in a directory names the
/// system and fixes the checkout root.
const MARKERS: &[(&str, &str)] = &[
    (".git", "Git"),
    (".hg", "Mercurial"),
    (".svn", "Subversion"),
    (".jj", "Jujutsu"),
    (".bzr", "Bazaar"),
    ("_darcs", "Darcs"),
    (".fslckout", "Fossil"),
    ("CVS", "CVS"),
];

/// What the marker walk found for one directory.
#[derive(Clone)]
struct VcsInfo {
    root: String,
    system: String,
}

/// One output row: a pane and the VCS checkout it is sitting in.
struct Row {
    id: String,
    location: String,
    command: String,
    system: String,
    root: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux vcs: {e}");
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

/// Resolve each unique live-pane directory once to its VCS (or `None`), caching
/// the miss too so repeated non-VCS paths are not re-walked.
fn resolve_all(panes: &[Pane]) -> HashMap<String, Option<VcsInfo>> {
    let mut map: HashMap<String, Option<VcsInfo>> = HashMap::new();
    for p in panes {
        if p.dead || p.path.is_empty() || map.contains_key(&p.path) {
            continue;
        }
        map.insert(p.path.clone(), detect(&p.path));
    }
    map
}

/// Walk up from `start` to the nearest VCS marker, against the real filesystem.
fn detect(start: &str) -> Option<VcsInfo> {
    detect_with(start, std::path::Path::exists)
}

/// The marker walk, parameterised over an existence check so it is testable
/// without touching the filesystem.
fn detect_with<F: Fn(&Path) -> bool>(start: &str, exists: F) -> Option<VcsInfo> {
    let mut dir = PathBuf::from(start);
    loop {
        for (marker, system) in MARKERS {
            if exists(&dir.join(marker)) {
                return Some(VcsInfo {
                    root: dir.to_string_lossy().into_owned(),
                    system: (*system).to_string(),
                });
            }
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// The last path component of a checkout root (`/home/u/proj` → `proj`).
fn root_name(root: &str) -> String {
    Path::new(root)
        .file_name()
        .map_or_else(|| root.to_string(), |n| n.to_string_lossy().into_owned())
}

/// One row per live pane whose directory resolved to a VCS, sorted by system,
/// then root, then location.
fn build_rows(panes: &[Pane], info: &HashMap<String, Option<VcsInfo>>) -> Vec<Row> {
    let mut rows: Vec<Row> = panes
        .iter()
        .filter(|p| !p.dead)
        .filter_map(|p| {
            let vi = info.get(&p.path).and_then(|o| o.as_ref())?;
            Some(Row {
                id: p.id.clone(),
                location: location(p),
                command: p.command.clone(),
                system: vi.system.clone(),
                root: vi.root.clone(),
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        a.system
            .cmp(&b.system)
            .then(a.root.cmp(&b.root))
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
                "{:<12} {:<8} {:<16} {:<16} {}",
                "VCS", "PANE", "LOCATION", "ROOT", "PATH"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<12} {:<8} {:<16} {:<16} {}\n",
            r.system,
            r.id,
            r.location,
            root_name(&r.root),
            r.root,
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
                "vcs": r.system,
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
    use std::collections::HashSet;

    fn fake_fs(present: &[&str]) -> impl Fn(&Path) -> bool {
        let set: HashSet<String> = present
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        move |p: &Path| set.contains(p.to_string_lossy().as_ref())
    }

    #[test]
    fn walks_up_to_the_nearest_git_marker() {
        let fs = fake_fs(&["/home/u/proj/.git"]);
        let vi = detect_with("/home/u/proj/src", fs).unwrap();
        assert_eq!(vi.root, "/home/u/proj");
        assert_eq!(vi.system, "Git");
    }

    #[test]
    fn detects_mercurial_and_subversion() {
        assert_eq!(
            detect_with("/h", fake_fs(&["/h/.hg"])).unwrap().system,
            "Mercurial"
        );
        assert_eq!(
            detect_with("/s/x", fake_fs(&["/s/.svn"])).unwrap().system,
            "Subversion"
        );
    }

    #[test]
    fn nearest_marker_wins_over_a_further_one() {
        // An inner hg repo inside an outer git tree resolves to Mercurial.
        let fs = fake_fs(&["/ws/.git", "/ws/sub/.hg"]);
        let vi = detect_with("/ws/sub/deep", fs).unwrap();
        assert_eq!(vi.root, "/ws/sub");
        assert_eq!(vi.system, "Mercurial");
    }

    #[test]
    fn no_marker_anywhere_yields_none() {
        assert!(detect_with("/tmp/scratch", fake_fs(&["/unrelated"])).is_none());
    }

    #[test]
    fn json_carries_vcs_and_root() {
        let panes = vec![Pane {
            id: "%1".into(),
            session: "a".into(),
            window: 0,
            index: 0,
            path: "/repo".into(),
            command: "zsh".into(),
            ..Default::default()
        }];
        let mut info: HashMap<String, Option<VcsInfo>> = HashMap::new();
        info.insert(
            "/repo".into(),
            Some(VcsInfo {
                root: "/repo".into(),
                system: "Git".into(),
            }),
        );
        let rows = build_rows(&panes, &info);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["vcs"], "Git");
        assert_eq!(v[0]["root"], "/repo");
    }
}
