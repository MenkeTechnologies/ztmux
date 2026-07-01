//! `ztmux events` — stream server lifecycle events as JSONL.
//!
//! A client subcommand that turns the polling `list-* -o json` query layer into
//! a push-style feed: it snapshots the server on an interval, diffs each tick
//! against the previous one, and prints one JSON object per change
//! (session/window/pane created, closed, renamed, layout- or command-changed,
//! pane died, client attached/detached). It lets a client subscribe to changes
//! instead of re-polling everything — something only the server side can see.
//!
//! Runs until interrupted. `-n <ms>` sets the poll interval (default 500);
//! `--count <N>` exits after N events (handy for scripts/tests).

use std::collections::HashMap;
use std::io::Write;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::tmux_query::{Snapshot, poll};

pub(crate) fn run(socket: &str) -> i32 {
    let interval =
        Duration::from_millis(arg_value("-n").and_then(|s| s.parse().ok()).unwrap_or(500));
    let max: Option<u64> = arg_value("--count").and_then(|s| s.parse().ok());

    let mut prev = poll(socket);
    if let Some(e) = &prev.error {
        eprintln!("ztmux events: {e}");
        return 1;
    }

    let mut emitted: u64 = 0;
    let stdout = std::io::stdout();
    loop {
        std::thread::sleep(interval);
        let cur = poll(socket);
        if let Some(e) = &cur.error {
            let mut m = serde_json::json!({ "event": "server_unreachable", "detail": e });
            stamp(&mut m);
            emit(&stdout, &m);
            return 0;
        }
        for mut ev in diff(&prev, &cur) {
            stamp(&mut ev);
            emit(&stdout, &ev);
            emitted += 1;
            if let Some(n) = max
                && emitted >= n
            {
                return 0;
            }
        }
        prev = cur;
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn stamp(ev: &mut serde_json::Value) {
    if let Some(obj) = ev.as_object_mut() {
        obj.insert("ts".into(), serde_json::json!(now_unix()));
    }
}

fn emit(stdout: &std::io::Stdout, ev: &serde_json::Value) {
    let mut h = stdout.lock();
    let _ = writeln!(h, "{ev}");
    let _ = h.flush();
}

/// Read the value following `flag` in argv, if present.
fn arg_value(flag: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

/// Diff two snapshots into a list of event objects (without the `ts` stamp,
/// which the caller adds — keeps this pure and testable).
fn diff(prev: &Snapshot, cur: &Snapshot) -> Vec<serde_json::Value> {
    let mut out = Vec::new();

    // Sessions, keyed by id.
    let ps: HashMap<&str, _> = prev.sessions.iter().map(|s| (s.id.as_str(), s)).collect();
    let cs: HashMap<&str, _> = cur.sessions.iter().map(|s| (s.id.as_str(), s)).collect();
    for (id, s) in &cs {
        match ps.get(id) {
            None => out.push(serde_json::json!({
                "event": "session_created", "id": id, "name": s.name })),
            Some(old) if old.name != s.name => out.push(serde_json::json!({
                "event": "session_renamed", "id": id, "from": old.name, "to": s.name })),
            _ => {}
        }
    }
    for (id, s) in &ps {
        if !cs.contains_key(id) {
            out.push(serde_json::json!({ "event": "session_closed", "id": id, "name": s.name }));
        }
    }

    // Windows, keyed by id.
    let pw: HashMap<&str, _> = prev.windows.iter().map(|w| (w.id.as_str(), w)).collect();
    let cw: HashMap<&str, _> = cur.windows.iter().map(|w| (w.id.as_str(), w)).collect();
    for (id, w) in &cw {
        match pw.get(id) {
            None => out.push(serde_json::json!({
                "event": "window_created", "id": id,
                "session": w.session, "index": w.index, "name": w.name })),
            Some(old) => {
                if old.name != w.name {
                    out.push(serde_json::json!({
                        "event": "window_renamed", "id": id, "from": old.name, "to": w.name }));
                }
                if old.layout != w.layout && !w.layout.is_empty() {
                    out.push(serde_json::json!({
                        "event": "window_layout_changed", "id": id, "layout": w.layout }));
                }
            }
        }
    }
    for (id, w) in &pw {
        if !cw.contains_key(id) {
            out.push(serde_json::json!({
                "event": "window_closed", "id": id, "session": w.session, "index": w.index }));
        }
    }

    // Panes, keyed by id.
    let pp: HashMap<&str, _> = prev.panes.iter().map(|p| (p.id.as_str(), p)).collect();
    let cp: HashMap<&str, _> = cur.panes.iter().map(|p| (p.id.as_str(), p)).collect();
    for (id, p) in &cp {
        match pp.get(id) {
            None => out.push(serde_json::json!({
                "event": "pane_created", "id": id,
                "session": p.session, "window": p.window, "command": p.command })),
            Some(old) => {
                if old.command != p.command {
                    out.push(serde_json::json!({
                        "event": "pane_command_changed", "id": id,
                        "from": old.command, "to": p.command }));
                }
                if !old.dead && p.dead {
                    out.push(serde_json::json!({ "event": "pane_died", "id": id }));
                }
            }
        }
    }
    for (id, p) in &pp {
        if !cp.contains_key(id) {
            out.push(serde_json::json!({
                "event": "pane_closed", "id": id, "session": p.session, "window": p.window }));
        }
    }

    // Clients, keyed by name.
    let pc: HashMap<&str, _> = prev.clients.iter().map(|c| (c.name.as_str(), c)).collect();
    let cc: HashMap<&str, _> = cur.clients.iter().map(|c| (c.name.as_str(), c)).collect();
    for (name, c) in &cc {
        if !pc.contains_key(name) {
            out.push(serde_json::json!({
                "event": "client_attached", "name": name, "session": c.session }));
        }
    }
    for (name, c) in &pc {
        if !cc.contains_key(name) {
            out.push(serde_json::json!({
                "event": "client_detached", "name": name, "session": c.session }));
        }
    }

    // Stable order so output/tests are deterministic regardless of HashMap.
    out.sort_by_key(|e| {
        (
            e["event"].as_str().unwrap_or("").to_string(),
            e["id"].as_str().unwrap_or("").to_string(),
        )
    });
    out
}

#[cfg(test)]
mod tests {
    use super::super::tmux_query::{Client, Pane, Session, Window};
    use super::*;

    fn ev_kinds(evs: &[serde_json::Value]) -> Vec<String> {
        evs.iter()
            .map(|e| e["event"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn detects_created_and_closed_and_renamed() {
        let prev = Snapshot {
            sessions: vec![Session {
                id: "$0".into(),
                name: "a".into(),
                ..Default::default()
            }],
            windows: vec![Window {
                id: "@0".into(),
                session: "a".into(),
                name: "w".into(),
                ..Default::default()
            }],
            panes: vec![Pane {
                id: "%0".into(),
                command: "zsh".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let cur = Snapshot {
            sessions: vec![Session {
                id: "$0".into(),
                name: "b".into(),
                ..Default::default()
            }],
            windows: vec![
                Window {
                    id: "@0".into(),
                    session: "b".into(),
                    name: "w".into(),
                    ..Default::default()
                },
                Window {
                    id: "@1".into(),
                    session: "b".into(),
                    name: "x".into(),
                    ..Default::default()
                },
            ],
            panes: vec![
                Pane {
                    id: "%0".into(),
                    command: "nvim".into(),
                    ..Default::default()
                },
                Pane {
                    id: "%1".into(),
                    command: "less".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let k = ev_kinds(&diff(&prev, &cur));
        assert!(k.contains(&"session_renamed".to_string()));
        assert!(k.contains(&"window_created".to_string()));
        assert!(k.contains(&"pane_created".to_string()));
        assert!(k.contains(&"pane_command_changed".to_string()));
    }

    #[test]
    fn detects_pane_death_and_client_churn() {
        let prev = Snapshot {
            panes: vec![Pane {
                id: "%0".into(),
                dead: false,
                ..Default::default()
            }],
            clients: vec![Client {
                name: "c0".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let cur = Snapshot {
            panes: vec![Pane {
                id: "%0".into(),
                dead: true,
                ..Default::default()
            }],
            clients: vec![Client {
                name: "c1".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let k = ev_kinds(&diff(&prev, &cur));
        assert!(k.contains(&"pane_died".to_string()));
        assert!(k.contains(&"client_attached".to_string())); // c1
        assert!(k.contains(&"client_detached".to_string())); // c0
    }

    #[test]
    fn no_change_yields_no_events() {
        let s = Snapshot {
            sessions: vec![Session {
                id: "$0".into(),
                name: "a".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(diff(&s, &s).is_empty());
    }

    // window_layout_changed fires only when the layout differs AND the new
    // layout is non-empty (empty means "not reported this tick").
    #[test]
    fn layout_change_emitted_only_for_nonempty_new_layout() {
        let mk = |layout: &str| Snapshot {
            windows: vec![Window {
                id: "@0".into(),
                session: "s".into(),
                name: "w".into(),
                layout: layout.into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let k = ev_kinds(&diff(&mk(""), &mk("abc")));
        assert!(k.contains(&"window_layout_changed".to_string()));
        let k = ev_kinds(&diff(&mk("abc"), &mk("")));
        assert!(!k.contains(&"window_layout_changed".to_string()));
    }

    // Objects present in prev but gone in cur produce *_closed events.
    #[test]
    fn detects_closed_sessions_windows_and_panes() {
        let prev = Snapshot {
            sessions: vec![Session {
                id: "$0".into(),
                name: "a".into(),
                ..Default::default()
            }],
            windows: vec![Window {
                id: "@0".into(),
                session: "a".into(),
                ..Default::default()
            }],
            panes: vec![Pane {
                id: "%0".into(),
                session: "a".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let k = ev_kinds(&diff(&prev, &Snapshot::default()));
        assert!(k.contains(&"session_closed".to_string()));
        assert!(k.contains(&"window_closed".to_string()));
        assert!(k.contains(&"pane_closed".to_string()));
    }

    // Output is sorted by (event kind, id), so two creations of the same kind
    // come out in id order regardless of the snapshot's vec order.
    #[test]
    fn events_are_sorted_by_kind_then_id() {
        let cur = Snapshot {
            panes: vec![
                Pane {
                    id: "%2".into(),
                    ..Default::default()
                },
                Pane {
                    id: "%1".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let evs = diff(&Snapshot::default(), &cur);
        let ids: Vec<&str> = evs.iter().map(|e| e["id"].as_str().unwrap()).collect();
        assert_eq!(ids, vec!["%1", "%2"]);
    }
}
