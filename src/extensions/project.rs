//! `ztmux project` — the project kind and root behind every pane's directory.
//!
//! For each live pane it walks up from the pane's working directory to the
//! nearest project marker — `Cargo.toml`, `go.mod`, `package.json`,
//! `pyproject.toml`, `Gemfile`, and so on, falling back to a bare `.git` — and
//! reports what kind of project that pane is sitting in and where its root is.
//! Where [`super::git`] answers branch/dirty state and [`super::cwd`] groups by
//! raw directory, `project` answers "what am I working *on* in each pane": Rust
//! here, a Node package there. It is filesystem-only (marker lookups, no
//! subprocess) and resolves each unique directory once. Panes not inside any
//! recognised project are omitted. With `-o json` / `--json` it emits the same
//! rows as a machine-readable array; sorted by kind, then project, then location.

use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use super::tmux_query::{Pane, poll};

/// Project markers in priority order: the first that exists in a directory wins,
/// so `Cargo.toml` beats a co-located `Makefile`, and a bare `.git` is the
/// last-resort "some repo" fallback.
const MARKERS: &[(&str, &str)] = &[
    ("Cargo.toml", "Rust"),
    ("go.mod", "Go"),
    ("pyproject.toml", "Python"),
    ("setup.py", "Python"),
    ("package.json", "Node"),
    ("deno.json", "Deno"),
    ("Gemfile", "Ruby"),
    ("mix.exs", "Elixir"),
    ("pom.xml", "Java"),
    ("build.gradle", "Gradle"),
    ("build.gradle.kts", "Gradle"),
    ("composer.json", "PHP"),
    ("CMakeLists.txt", "CMake"),
    ("Makefile", "Make"),
    (".git", "Git"),
];

/// What the marker walk found for one directory.
#[derive(Clone)]
struct ProjectInfo {
    root: String,
    kind: String,
}

/// One output row: a pane and the project it is sitting in.
struct Row {
    id: String,
    location: String,
    command: String,
    kind: String,
    project: String, // root's basename
    root: String,    // full root path
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux project: {e}");
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

/// Resolve each unique live-pane directory once to its project (or `None`),
/// caching the miss too so repeated non-project paths are not re-walked.
fn resolve_all(panes: &[Pane]) -> HashMap<String, Option<ProjectInfo>> {
    let mut map: HashMap<String, Option<ProjectInfo>> = HashMap::new();
    for p in panes {
        if p.dead || p.path.is_empty() || map.contains_key(&p.path) {
            continue;
        }
        let found = detect(&p.path);
        map.insert(p.path.clone(), found);
    }
    map
}

/// Walk up from `start` to the nearest directory holding a project marker,
/// against the real filesystem.
fn detect(start: &str) -> Option<ProjectInfo> {
    detect_with(start, |p| p.exists())
}

/// The marker walk, parameterised over an existence check so it is testable
/// without touching the filesystem. Starting at `start`, each directory is
/// checked for the markers in priority order; the first hit fixes the project
/// root and kind. If no ancestor holds a marker the walk reaches the filesystem
/// root and yields `None`.
fn detect_with<F: Fn(&Path) -> bool>(start: &str, exists: F) -> Option<ProjectInfo> {
    let mut dir = PathBuf::from(start);
    loop {
        for (marker, kind) in MARKERS {
            if exists(&dir.join(marker)) {
                return Some(ProjectInfo {
                    root: dir.to_string_lossy().into_owned(),
                    kind: (*kind).to_string(),
                });
            }
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// The last path component of a project root (`/home/u/proj` → `proj`).
fn project_name(root: &str) -> String {
    Path::new(root)
        .file_name()
        .map_or_else(|| root.to_string(), |n| n.to_string_lossy().into_owned())
}

/// One row per live pane whose directory resolved to a project, sorted by kind,
/// then project, then location.
fn build_rows(panes: &[Pane], info: &HashMap<String, Option<ProjectInfo>>) -> Vec<Row> {
    let mut rows: Vec<Row> = panes
        .iter()
        .filter(|p| !p.dead)
        .filter_map(|p| {
            let pi = info.get(&p.path).and_then(|o| o.as_ref())?;
            Some(Row {
                id: p.id.clone(),
                location: location(p),
                command: p.command.clone(),
                kind: pi.kind.clone(),
                project: project_name(&pi.root),
                root: pi.root.clone(),
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then(a.project.cmp(&b.project))
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
                "{:<8} {:<16} {:<10} {:<16} {}",
                "PANE", "LOCATION", "TYPE", "PROJECT", "ROOT"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<8} {:<16} {:<10} {:<16} {}\n",
            r.id, r.location, r.kind, r.project, r.root,
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
                "kind": r.kind,
                "project": r.project,
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

    /// Build an existence predicate over a fixed set of paths.
    fn fake_fs(present: &[&str]) -> impl Fn(&Path) -> bool {
        let set: HashSet<String> = present.iter().map(|s| s.to_string()).collect();
        move |p: &Path| set.contains(p.to_string_lossy().as_ref())
    }

    #[test]
    fn walks_up_to_the_nearest_marker() {
        let fs = fake_fs(&["/home/u/proj/Cargo.toml"]);
        let pi = detect_with("/home/u/proj/src/bin", fs).unwrap();
        assert_eq!(pi.root, "/home/u/proj");
        assert_eq!(pi.kind, "Rust");
    }

    #[test]
    fn marker_priority_picks_the_higher_ranked_kind() {
        // Cargo.toml outranks a co-located Makefile.
        let fs = fake_fs(&["/p/Cargo.toml", "/p/Makefile"]);
        let pi = detect_with("/p", fs).unwrap();
        assert_eq!(pi.kind, "Rust");
    }

    #[test]
    fn nearest_ancestor_wins_over_a_further_one() {
        // An inner Node package inside an outer Rust workspace resolves to Node.
        let fs = fake_fs(&["/ws/Cargo.toml", "/ws/web/package.json"]);
        let pi = detect_with("/ws/web/src", fs).unwrap();
        assert_eq!(pi.root, "/ws/web");
        assert_eq!(pi.kind, "Node");
    }

    #[test]
    fn bare_git_is_the_fallback() {
        let fs = fake_fs(&["/repo/.git"]);
        let pi = detect_with("/repo/deep/dir", fs).unwrap();
        assert_eq!(pi.kind, "Git");
        assert_eq!(pi.root, "/repo");
    }

    #[test]
    fn no_marker_anywhere_yields_none() {
        let fs = fake_fs(&["/unrelated/file"]);
        assert!(detect_with("/home/u/scratch", fs).is_none());
    }

    #[test]
    fn build_rows_sorts_and_omits_unresolved_panes() {
        let panes = vec![
            Pane {
                id: "%1".into(),
                session: "a".into(),
                window: 0,
                index: 0,
                path: "/rust".into(),
                command: "vim".into(),
                ..Default::default()
            },
            Pane {
                id: "%2".into(),
                session: "a".into(),
                window: 0,
                index: 1,
                path: "/nowhere".into(),
                command: "zsh".into(),
                ..Default::default()
            },
        ];
        let mut info: HashMap<String, Option<ProjectInfo>> = HashMap::new();
        info.insert(
            "/rust".into(),
            Some(ProjectInfo {
                root: "/rust".into(),
                kind: "Rust".into(),
            }),
        );
        info.insert("/nowhere".into(), None);
        let rows = build_rows(&panes, &info);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%1");
        assert_eq!(rows[0].kind, "Rust");
        assert_eq!(rows[0].project, "rust");
    }

    #[test]
    fn json_carries_kind_project_and_root() {
        let panes = vec![Pane {
            id: "%1".into(),
            session: "a".into(),
            window: 0,
            index: 0,
            path: "/home/u/proj".into(),
            command: "cargo".into(),
            ..Default::default()
        }];
        let mut info: HashMap<String, Option<ProjectInfo>> = HashMap::new();
        info.insert(
            "/home/u/proj".into(),
            Some(ProjectInfo {
                root: "/home/u/proj".into(),
                kind: "Rust".into(),
            }),
        );
        let rows = build_rows(&panes, &info);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["kind"], "Rust");
        assert_eq!(v[0]["project"], "proj");
        assert_eq!(v[0]["root"], "/home/u/proj");
    }
}
