//! `ztmux snapshot` — dump the whole server as one nested JSON document.
//!
//! A one-shot client subcommand over the `list-* -o json` query layer. Where
//! `list-*` emit flat per-object arrays, this nests them
//! (sessions → windows → panes) plus the client list into a single document —
//! a convenient primitive for external tooling and inspection. It is NOT a
//! save/restore format (that lives in the GUI client); it is a read-only view.

use super::tmux_query::{Snapshot, poll};

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux snapshot: {e}");
        return 1;
    }
    print!("{}", render(&snap));
    0
}

fn render(snap: &Snapshot) -> String {
    let sessions: Vec<serde_json::Value> = snap
        .sessions
        .iter()
        .map(|s| {
            let mut windows: Vec<&super::tmux_query::Window> = snap
                .windows
                .iter()
                .filter(|w| w.session == s.name)
                .collect();
            windows.sort_by_key(|w| w.index);
            let windows: Vec<serde_json::Value> = windows
                .iter()
                .map(|w| {
                    let mut panes: Vec<&super::tmux_query::Pane> = snap
                        .panes
                        .iter()
                        .filter(|p| p.session == w.session && p.window == w.index)
                        .collect();
                    panes.sort_by_key(|p| p.index);
                    let panes: Vec<serde_json::Value> = panes
                        .iter()
                        .map(|p| {
                            serde_json::json!({
                                "id": p.id,
                                "index": p.index,
                                "active": p.active,
                                "dead": p.dead,
                                "pid": p.pid,
                                "command": p.command,
                                "title": p.title,
                                "path": p.path,
                                "width": p.width,
                                "height": p.height,
                            })
                        })
                        .collect();
                    serde_json::json!({
                        "id": w.id,
                        "index": w.index,
                        "name": w.name,
                        "active": w.active,
                        "layout": w.layout,
                        "width": w.width,
                        "height": w.height,
                        "panes": panes,
                    })
                })
                .collect();
            serde_json::json!({
                "id": s.id,
                "name": s.name,
                "attached": s.attached,
                "created": s.created,
                "activity": s.activity,
                "group": s.group,
                "windows": windows,
            })
        })
        .collect();

    let clients: Vec<serde_json::Value> = snap
        .clients
        .iter()
        .map(|c| {
            serde_json::json!({
                "name": c.name,
                "tty": c.tty,
                "session": c.session,
                "width": c.width,
                "height": c.height,
                "termname": c.termname,
                "pid": c.pid,
            })
        })
        .collect();

    let doc = serde_json::json!({ "sessions": sessions, "clients": clients });
    format!(
        "{}\n",
        serde_json::to_string_pretty(&doc).unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::super::tmux_query::{Client, Pane, Session, Window};
    use super::*;

    fn snap() -> Snapshot {
        Snapshot {
            sessions: vec![Session {
                id: "$0".into(),
                name: "work".into(),
                attached: true,
                ..Default::default()
            }],
            windows: vec![Window {
                id: "@1".into(),
                session: "work".into(),
                index: 0,
                name: "editor".into(),
                ..Default::default()
            }],
            panes: vec![Pane {
                id: "%2".into(),
                session: "work".into(),
                window: 0,
                index: 0,
                command: "nvim".into(),
                ..Default::default()
            }],
            clients: vec![Client {
                name: "c0".into(),
                session: "work".into(),
                ..Default::default()
            }],
            error: None,
        }
    }

    #[test]
    fn nests_sessions_windows_and_panes() {
        let out = render(&snap());
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["sessions"][0]["name"], "work");
        assert_eq!(v["sessions"][0]["windows"][0]["name"], "editor");
        assert_eq!(
            v["sessions"][0]["windows"][0]["panes"][0]["command"],
            "nvim"
        );
        assert_eq!(v["clients"][0]["name"], "c0");
    }

    // Each pane object exposes the full field set (id/pid/size/flags/…).
    #[test]
    fn pane_object_carries_all_fields() {
        let mut sn = snap();
        sn.panes[0].pid = 321;
        sn.panes[0].width = 80;
        sn.panes[0].height = 24;
        sn.panes[0].active = true;
        let v: serde_json::Value = serde_json::from_str(&render(&sn)).unwrap();
        let p = &v["sessions"][0]["windows"][0]["panes"][0];
        assert_eq!(p["id"], "%2");
        assert_eq!(p["pid"], 321);
        assert_eq!(p["width"], 80);
        assert_eq!(p["height"], 24);
        assert_eq!(p["active"], true);
    }

    // Windows nest under their session in index order regardless of input order.
    #[test]
    fn windows_nested_sorted_by_index() {
        let mut sn = snap();
        sn.windows.push(Window {
            id: "@9".into(),
            session: "work".into(),
            index: 2,
            name: "logs".into(),
            ..Default::default()
        });
        sn.windows.insert(
            0,
            Window {
                id: "@5".into(),
                session: "work".into(),
                index: 1,
                name: "build".into(),
                ..Default::default()
            },
        );
        let v: serde_json::Value = serde_json::from_str(&render(&sn)).unwrap();
        let wins = v["sessions"][0]["windows"].as_array().unwrap();
        let idxs: Vec<i64> = wins.iter().map(|w| w["index"].as_i64().unwrap()).collect();
        assert_eq!(idxs, vec![0, 1, 2]);
    }

    // An empty server yields empty (but present) sessions/clients arrays.
    #[test]
    fn empty_snapshot_has_empty_arrays() {
        let v: serde_json::Value = serde_json::from_str(&render(&Snapshot::default())).unwrap();
        assert_eq!(v["sessions"].as_array().unwrap().len(), 0);
        assert_eq!(v["clients"].as_array().unwrap().len(), 0);
    }
}
