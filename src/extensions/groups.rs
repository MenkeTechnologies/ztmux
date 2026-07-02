//! `ztmux groups` — sessions clustered by their session group.
//!
//! tmux session *groups* (created with `new-session -t <existing>`) share one
//! set of windows across several independently-attachable sessions. Nothing
//! else in the toolchain surfaces that relation: [`super::recent`] ranks every
//! session by activity and [`super::tree`] prints structure, but neither shows
//! which sessions are yoked together. `groups` lists only the grouped sessions,
//! sorted by group then name so members cluster, and is the "which of my
//! sessions are really the same windows" view. Sessions with no group are
//! omitted. With `-o json` / `--json` it emits one object per group.

use std::io::IsTerminal;

use super::tmux_query::{Snapshot, poll};

/// One output row: a grouped session and its window count / attach state.
struct Row {
    group: String,
    session: String,
    windows: i64,
    attached: bool,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux groups: {e}");
        return 1;
    }
    let rows = build_rows(&snap);
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

/// Keep only sessions that belong to a group, ordered by group then session
/// name so members of the same group sit together for a stable, greppable list.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .sessions
        .iter()
        .filter(|s| !s.group.is_empty())
        .map(|s| Row {
            group: s.group.clone(),
            session: s.name.clone(),
            windows: s.windows,
            attached: s.attached,
        })
        .collect();
    rows.sort_by(|a, b| a.group.cmp(&b.group).then(a.session.cmp(&b.session)));
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
                "{:<16} {:<20} {:>7} {:>8}",
                "GROUP", "SESSION", "WINDOWS", "ATTACHED"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<16} {:<20} {:>7} {:>8}\n",
            r.group,
            r.session,
            r.windows,
            if r.attached { "yes" } else { "no" },
        ));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    // Fold the flat rows back into one object per group, preserving order.
    let mut groups: Vec<serde_json::Value> = Vec::new();
    let mut current: Option<(String, Vec<serde_json::Value>)> = None;
    let flush = |cur: Option<(String, Vec<serde_json::Value>)>,
                 out: &mut Vec<serde_json::Value>| {
        if let Some((name, members)) = cur {
            out.push(serde_json::json!({
                "group": name,
                "count": members.len(),
                "sessions": members,
            }));
        }
    };
    for r in rows {
        let member = serde_json::json!({
            "name": r.session,
            "windows": r.windows,
            "attached": r.attached,
        });
        match &mut current {
            Some((name, members)) if *name == r.group => members.push(member),
            _ => {
                flush(current.take(), &mut groups);
                current = Some((r.group.clone(), vec![member]));
            }
        }
    }
    flush(current.take(), &mut groups);
    format!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::Value::Array(groups)).unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::super::tmux_query::Session;
    use super::*;

    fn sess(name: &str, group: &str, windows: i64, attached: bool) -> Session {
        Session {
            name: name.into(),
            group: group.into(),
            windows,
            attached,
            ..Default::default()
        }
    }

    fn snap(sessions: Vec<Session>) -> Snapshot {
        Snapshot {
            sessions,
            ..Default::default()
        }
    }

    #[test]
    fn ungrouped_sessions_are_omitted() {
        let rows = build_rows(&snap(vec![
            sess("solo", "", 1, true),
            sess("a", "dev", 3, true),
        ]));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].session, "a");
        assert_eq!(rows[0].group, "dev");
    }

    #[test]
    fn rows_cluster_by_group_then_name() {
        let rows = build_rows(&snap(vec![
            sess("z", "dev", 1, false),
            sess("a", "ops", 1, false),
            sess("a", "dev", 1, false),
        ]));
        // dev group first (a, z), then ops (a).
        assert_eq!(
            rows.iter()
                .map(|r| (r.group.as_str(), r.session.as_str()))
                .collect::<Vec<_>>(),
            vec![("dev", "a"), ("dev", "z"), ("ops", "a")],
        );
    }

    #[test]
    fn text_has_header_and_attach_state() {
        let rows = build_rows(&snap(vec![sess("a", "dev", 3, true)]));
        let s = render_text(&rows, false);
        assert!(s.contains("GROUP") && s.contains("ATTACHED"));
        assert!(s.lines().any(|l| l.contains("dev") && l.contains("yes")));
    }

    #[test]
    fn json_folds_rows_into_one_object_per_group() {
        let rows = build_rows(&snap(vec![
            sess("a", "dev", 1, true),
            sess("b", "dev", 2, false),
            sess("c", "ops", 1, true),
        ]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 2);
        assert_eq!(v[0]["group"], "dev");
        assert_eq!(v[0]["count"], 2);
        assert_eq!(v[0]["sessions"][1]["name"], "b");
        assert_eq!(v[1]["group"], "ops");
        assert_eq!(v[1]["count"], 1);
    }
}
