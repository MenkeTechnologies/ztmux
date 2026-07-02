//! `ztmux linked` — windows that live in more than one session at once.
//!
//! tmux's `link-window` lets a single window appear in several sessions: the
//! same window (same `@id`) shows up under different session/index slots, and an
//! edit in one is an edit in all. That sharing is invisible in the normal tree —
//! a linked window looks like any other. `linked` surfaces it: it groups every
//! window by its id and reports the ones that appear in two or more sessions,
//! with every place they show up. It is the "which windows am I sharing across
//! sessions" view, built purely from the server snapshot. Most-shared first.
//! With `-o json` / `--json` it emits the same rows as a machine-readable array;
//! a server with no linked windows prints just the header.

use std::io::IsTerminal;

use super::tmux_query::{Snapshot, Window, poll};

/// One output row: a window id that is linked into several sessions.
struct Row {
    id: String,
    name: String,
    sessions: usize,
    locations: Vec<String>, // every session:index it appears under
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux linked: {e}");
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

fn location(w: &Window) -> String {
    format!("{}:{}", w.session, w.index)
}

/// Group windows by id and keep only the ids that appear in two or more
/// *distinct* sessions — the linked windows. A [`std::collections::BTreeMap`]
/// gives a deterministic id order; a stable sort by session count then puts the
/// most-shared window first. Windows with an empty id are skipped.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    use std::collections::BTreeMap;
    use std::collections::BTreeSet;
    // id -> (name, locations, distinct sessions)
    let mut buckets: BTreeMap<String, (String, Vec<String>, BTreeSet<String>)> = BTreeMap::new();
    for w in &snap.windows {
        if w.id.is_empty() {
            continue;
        }
        let entry = buckets
            .entry(w.id.clone())
            .or_insert_with(|| (w.name.clone(), Vec::new(), BTreeSet::new()));
        entry.1.push(location(w));
        entry.2.insert(w.session.clone());
    }
    let mut rows: Vec<Row> = buckets
        .into_iter()
        .filter(|(_, (_, _, sessions))| sessions.len() > 1)
        .map(|(id, (name, mut locations, sessions))| {
            locations.sort();
            Row {
                id,
                name,
                sessions: sessions.len(),
                locations,
            }
        })
        .collect();
    // Most-shared first; ties keep the map's id order (stable sort).
    rows.sort_by_key(|r| std::cmp::Reverse(r.sessions));
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
                "{:<8} {:<16} {:>8} {}",
                "WINDOW", "NAME", "SESSIONS", "LOCATIONS"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<8} {:<16} {:>8} {}\n",
            r.id,
            r.name,
            r.sessions,
            r.locations.join(" "),
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
                "name": r.name,
                "sessions": r.sessions,
                "locations": r.locations,
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

    fn window(id: &str, sess: &str, idx: i64, name: &str) -> Window {
        Window {
            id: id.into(),
            session: sess.into(),
            index: idx,
            name: name.into(),
            ..Default::default()
        }
    }

    fn snap(windows: Vec<Window>) -> Snapshot {
        Snapshot {
            windows,
            ..Default::default()
        }
    }

    #[test]
    fn window_in_one_session_is_not_linked() {
        let rows = build_rows(&snap(vec![
            window("@1", "a", 0, "edit"),
            window("@2", "a", 1, "run"),
        ]));
        assert!(rows.is_empty());
    }

    #[test]
    fn window_shared_across_sessions_is_reported() {
        let rows = build_rows(&snap(vec![
            window("@1", "a", 0, "shared"),
            window("@1", "b", 3, "shared"),
            window("@2", "a", 1, "solo"),
        ]));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "@1");
        assert_eq!(rows[0].sessions, 2);
        assert_eq!(rows[0].locations, vec!["a:0", "b:3"]);
    }

    #[test]
    fn same_id_same_session_is_not_counted_as_two_sessions() {
        // Defensive: two rows with the same id in the same session (should not
        // normally happen) count as one session, so not linked.
        let rows = build_rows(&snap(vec![
            window("@1", "a", 0, "x"),
            window("@1", "a", 0, "x"),
        ]));
        assert!(rows.is_empty());
    }

    #[test]
    fn most_shared_window_sorts_first() {
        let rows = build_rows(&snap(vec![
            window("@1", "a", 0, "two"),
            window("@1", "b", 0, "two"),
            window("@2", "a", 1, "three"),
            window("@2", "b", 1, "three"),
            window("@2", "c", 1, "three"),
        ]));
        assert_eq!(rows[0].id, "@2");
        assert_eq!(rows[0].sessions, 3);
        assert_eq!(rows[1].id, "@1");
    }

    #[test]
    fn json_carries_id_sessions_and_locations() {
        let rows = build_rows(&snap(vec![
            window("@1", "a", 0, "shared"),
            window("@1", "b", 2, "shared"),
        ]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["id"], "@1");
        assert_eq!(v[0]["sessions"], 2);
        assert_eq!(v[0]["locations"][1], "b:2");
    }
}
