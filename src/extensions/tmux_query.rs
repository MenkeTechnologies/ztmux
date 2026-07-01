//! Shared read-only query layer for the extension TUIs (`dashboard`, `switch`).
//!
//! Re-invokes our own binary (`ztmux -S <socket> list-* -o json`) and
//! deserializes the machine-readable output, so the TUIs need no linkage
//! against the server internals and always target the selected socket.
use std::process::Command;

use serde::Deserialize;
use serde::de::DeserializeOwned;

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub(crate) struct Session {
    pub(crate) name: String,
    pub(crate) id: String,
    pub(crate) windows: i64,
    pub(crate) created: i64,
    pub(crate) attached: bool,
    pub(crate) group: String,
    pub(crate) activity: i64,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub(crate) struct Window {
    pub(crate) session: String,
    pub(crate) index: i64,
    pub(crate) name: String,
    pub(crate) id: String,
    pub(crate) active: bool,
    pub(crate) panes: i64,
    pub(crate) width: i64,
    pub(crate) height: i64,
    pub(crate) layout: String,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub(crate) struct Pane {
    pub(crate) session: String,
    pub(crate) window: i64,
    pub(crate) index: i64,
    pub(crate) id: String,
    pub(crate) active: bool,
    pub(crate) dead: bool,
    pub(crate) pid: i64,
    pub(crate) tty: String,
    pub(crate) command: String,
    pub(crate) path: String,
    pub(crate) title: String,
    pub(crate) width: i64,
    pub(crate) height: i64,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub(crate) struct Client {
    pub(crate) name: String,
    pub(crate) tty: String,
    pub(crate) session: String,
    pub(crate) width: i64,
    pub(crate) height: i64,
    pub(crate) termname: String,
    pub(crate) pid: i64,
}

#[derive(Default)]
pub(crate) struct Snapshot {
    pub(crate) sessions: Vec<Session>,
    pub(crate) windows: Vec<Window>,
    pub(crate) panes: Vec<Pane>,
    pub(crate) clients: Vec<Client>,
    pub(crate) error: Option<String>,
}

/// Build a `ztmux -S <socket> <args…>` command against our own binary.
pub(crate) fn ztmux_cmd(socket: &str, args: &[&str]) -> Command {
    let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("ztmux"));
    let mut c = Command::new(exe);
    if !socket.is_empty() {
        c.arg("-S").arg(socket);
    }
    c.args(args);
    c
}

/// Run a list command and parse its JSON stdout into `Vec<T>`.
pub(crate) fn run_json<T: DeserializeOwned>(socket: &str, args: &[&str]) -> Result<Vec<T>, String> {
    let out = ztmux_cmd(socket, args)
        .output()
        .map_err(|e| format!("spawn: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("parse {args:?}: {e}"))
}

/// Fetch the whole server tree. On a hard error (e.g. no server), the sessions
/// error is surfaced in `Snapshot::error` and the rest is left empty.
pub(crate) fn poll(socket: &str) -> Snapshot {
    let mut snap = Snapshot::default();
    match run_json::<Session>(socket, &["list-sessions", "-o", "json"]) {
        Ok(v) => snap.sessions = v,
        Err(e) => {
            snap.error = Some(e);
            return snap;
        }
    }
    snap.windows =
        run_json::<Window>(socket, &["list-windows", "-a", "-o", "json"]).unwrap_or_default();
    snap.panes = run_json::<Pane>(socket, &["list-panes", "-a", "-o", "json"]).unwrap_or_default();
    snap.clients = run_json::<Client>(socket, &["list-clients", "-o", "json"]).unwrap_or_default();
    snap
}
