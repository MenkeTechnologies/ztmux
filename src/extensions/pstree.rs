//! `ztmux pstree` — the process tree running under every pane.
//!
//! Where [`super::ps`] shows one row per pane (its shell), this walks the OS
//! process table (via [`super::proctree`]) and prints the full descendant tree
//! rooted at each pane's pid — the "what is actually running in here" view. A
//! pane whose process has exited shows `(no process)`. Output is an indented
//! tree (pane headers coloured when stdout is a TTY) or a nested JSON document
//! with `-o json` / `--json`.

use std::collections::HashMap;
use std::io::IsTerminal;

use super::proctree::{Proc, children_map, table};
use super::tmux_query::{Pane, Snapshot, poll};

/// A node in a pane's process tree.
struct Node {
    pid: i64,
    comm: String,
    children: Vec<Node>,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux pstree: {e}");
        return 1;
    }
    let procs = table();
    let forest = build_forest(&snap, &procs);
    let json = std::env::args().any(|a| a == "--json")
        || std::env::args()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|w| w[0] == "-o" && w[1] == "json");
    if json {
        print!("{}", render_json(&forest));
    } else {
        print!("{}", render_text(&forest, std::io::stdout().is_terminal()));
    }
    0
}

/// One entry per pane: its identity plus the process tree rooted at its pid
/// (`None` when that pid is not in the process table, i.e. the process exited).
struct PaneTree {
    id: String,
    loc: String,
    pid: i64,
    root: Option<Node>,
}

fn build_forest(snap: &Snapshot, procs: &[Proc]) -> Vec<PaneTree> {
    let cmap = children_map(procs);
    let comm: HashMap<i64, &str> = procs.iter().map(|p| (p.pid, p.comm.as_str())).collect();
    let mut panes: Vec<&Pane> = snap.panes.iter().collect();
    panes.sort_by_key(|p| loc(p));
    panes
        .into_iter()
        .map(|p| PaneTree {
            id: p.id.clone(),
            loc: loc(p),
            pid: p.pid,
            root: comm
                .contains_key(&p.pid)
                .then(|| build_node(p.pid, &cmap, &comm)),
        })
        .collect()
}

/// Build the subtree rooted at `pid`. Depth-guarded against a pathological
/// (cyclic) process table.
fn build_node(pid: i64, cmap: &HashMap<i64, Vec<i64>>, comm: &HashMap<i64, &str>) -> Node {
    build_node_depth(pid, cmap, comm, 0)
}

fn build_node_depth(
    pid: i64,
    cmap: &HashMap<i64, Vec<i64>>,
    comm: &HashMap<i64, &str>,
    depth: usize,
) -> Node {
    let children = if depth >= 128 {
        Vec::new()
    } else {
        cmap.get(&pid)
            .map(|kids| {
                kids.iter()
                    .filter(|&&c| c != pid)
                    .map(|&c| build_node_depth(c, cmap, comm, depth + 1))
                    .collect()
            })
            .unwrap_or_default()
    };
    Node {
        pid,
        comm: comm.get(&pid).copied().unwrap_or("?").to_string(),
        children,
    }
}

fn loc(p: &Pane) -> String {
    format!("{}:{}.{}", p.session, p.window, p.index)
}

fn render_text(forest: &[PaneTree], color: bool) -> String {
    let paint = |s: &str, code: &str| -> String {
        if color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };
    let mut out = String::new();
    for pt in forest {
        out.push_str(&format!(
            "{}\n",
            paint(&format!("── {} {} ──", pt.id, pt.loc), "1;36")
        ));
        match &pt.root {
            None => out.push_str("  (no process)\n"),
            Some(root) => {
                out.push_str(&format!("  {} {}\n", root.pid, root.comm));
                render_children(&root.children, "  ", &mut out);
            }
        }
    }
    out
}

/// Render a node's children with box-drawing connectors, `prefix` carrying the
/// indentation/guides accumulated from ancestors.
fn render_children(children: &[Node], prefix: &str, out: &mut String) {
    let last = children.len().saturating_sub(1);
    for (i, node) in children.iter().enumerate() {
        let is_last = i == last;
        let connector = if is_last { "└─ " } else { "├─ " };
        out.push_str(&format!("{prefix}{connector}{} {}\n", node.pid, node.comm));
        let child_prefix = format!("{prefix}{}", if is_last { "   " } else { "│  " });
        render_children(&node.children, &child_prefix, out);
    }
}

fn render_json(forest: &[PaneTree]) -> String {
    let arr: Vec<serde_json::Value> = forest
        .iter()
        .map(|pt| {
            serde_json::json!({
                "pane": pt.id,
                "location": pt.loc,
                "pid": pt.pid,
                "tree": pt.root.as_ref().map(node_json),
            })
        })
        .collect();
    format!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::Value::Array(arr)).unwrap_or_default()
    )
}

fn node_json(node: &Node) -> serde_json::Value {
    serde_json::json!({
        "pid": node.pid,
        "command": node.comm,
        "children": node.children.iter().map(node_json).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn procs() -> Vec<Proc> {
        vec![
            Proc {
                pid: 100,
                ppid: 1,
                comm: "zsh".into(),
            },
            Proc {
                pid: 200,
                ppid: 100,
                comm: "npm".into(),
            },
            Proc {
                pid: 300,
                ppid: 200,
                comm: "node".into(),
            },
            Proc {
                pid: 400,
                ppid: 100,
                comm: "vim".into(),
            },
        ]
    }

    fn snap() -> Snapshot {
        Snapshot {
            panes: vec![
                Pane {
                    id: "%0".into(),
                    session: "w".into(),
                    window: 0,
                    index: 0,
                    pid: 100,
                    ..Default::default()
                },
                Pane {
                    id: "%1".into(),
                    session: "w".into(),
                    window: 0,
                    index: 1,
                    pid: 999,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn builds_a_tree_rooted_at_the_pane_pid() {
        let forest = build_forest(&snap(), &procs());
        let p0 = &forest[0];
        let root = p0.root.as_ref().unwrap();
        assert_eq!(root.pid, 100);
        assert_eq!(root.comm, "zsh");
        // zsh has two children: npm (200) and vim (400), sorted by pid.
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].pid, 200);
        assert_eq!(root.children[0].children[0].comm, "node"); // npm → node
        assert_eq!(root.children[1].comm, "vim");
    }

    #[test]
    fn pane_with_no_live_process_has_none_root() {
        let forest = build_forest(&snap(), &procs());
        assert!(forest[1].root.is_none());
    }

    #[test]
    fn text_renders_connectors_and_no_process() {
        let forest = build_forest(&snap(), &procs());
        let s = render_text(&forest, false);
        assert!(s.contains("── %0 w:0.0 ──"));
        assert!(s.contains("100 zsh"));
        assert!(s.contains("├─ 200 npm"));
        assert!(s.contains("└─ 400 vim"));
        assert!(s.contains("(no process)"));
    }

    #[test]
    fn json_nests_children_and_marks_dead_pane_tree_null() {
        let forest = build_forest(&snap(), &procs());
        let v: serde_json::Value = serde_json::from_str(&render_json(&forest)).unwrap();
        assert_eq!(v[0]["tree"]["command"], "zsh");
        assert_eq!(
            v[0]["tree"]["children"][0]["children"][0]["command"],
            "node"
        );
        assert!(v[1]["tree"].is_null());
    }

    // A self-referential parent (pid == ppid appearing as its own child) must
    // not cause infinite recursion.
    #[test]
    fn self_parent_does_not_recurse_forever() {
        let procs = vec![
            Proc {
                pid: 100,
                ppid: 1,
                comm: "zsh".into(),
            },
            Proc {
                pid: 100,
                ppid: 100,
                comm: "zsh".into(),
            },
        ];
        let forest = build_forest(&snap(), &procs);
        // Completes and roots at the pane pid without hanging.
        assert_eq!(forest[0].root.as_ref().unwrap().pid, 100);
    }
}
