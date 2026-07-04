//! `ztmux pick <op>` — batch operations over the multi-pane mark set.
//!
//! tmux only ever has a single *marked* pane (`select-pane -m`) and its
//! `synchronize-panes` is all-or-nothing per window. ztmux lets you mark several
//! panes (`prefix + m`, which sets the per-pane user option `@ztmux_sel`) and
//! then act on the whole set. `pick` reads that flag across the server:
//!
//! * `sync` — `synchronize-panes on` for every marked pane, then drop the marks
//!   (so *only* the chosen panes broadcast typing, and the loud "synced" border
//!   style replaces the "marked" one).
//! * `unmark` — drop every mark, leaving sync untouched.
//! * `clear` — full reset: unmark everything and unsync every pane.
//! * `list` — (default) print the current mark set.
//!
//! Invoked from the `prefix + m`/`M` bindings and the pane menu via `run-shell`,
//! or directly as `ztmux pick <op>`.
use super::tmux_query::query_lines;

/// Per-pane user option that marks a pane as part of the set.
const SEL_OPT: &str = "@ztmux_sel";

pub(crate) fn run(socket: &str) -> i32 {
    let op = op_arg();
    let marked = marked_panes(socket);
    match op.as_deref() {
        Some("sync") => {
            // Turn the marks into an active selective sync: every marked pane
            // gets `synchronize-panes on`, then the (transient) mark is dropped
            // so the loud "synced" border style takes over from the "marked" one.
            for id in &marked {
                let _ = query_lines(
                    socket,
                    &["set-option", "-p", "-t", id, "synchronize-panes", "on"],
                );
                let _ = query_lines(socket, &["set-option", "-pu", "-t", id, SEL_OPT]);
            }
        }
        Some("unmark") => {
            // Abort the selection: drop every mark, leave sync untouched.
            for id in &all_panes(socket) {
                let _ = query_lines(socket, &["set-option", "-pu", "-t", id, SEL_OPT]);
            }
        }
        Some("clear") => {
            // Full reset: unmark everything and unsync every pane.
            for id in &all_panes(socket) {
                let _ = query_lines(socket, &["set-option", "-pu", "-t", id, SEL_OPT]);
                let _ = query_lines(
                    socket,
                    &["set-option", "-p", "-t", id, "synchronize-panes", "off"],
                );
            }
        }
        _ => {
            if marked.is_empty() {
                println!("no panes marked (mark panes with prefix + m)");
            } else {
                println!("{} pane(s) marked:", marked.len());
                for id in &marked {
                    println!("  {id}");
                }
            }
        }
    }
    0
}

/// The operation word following the `pick` subcommand (`ztmux pick sync`).
fn op_arg() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let i = args.iter().position(|a| a == "pick")?;
    args.get(i + 1).cloned()
}

/// Every pane id whose `@ztmux_sel` flag is set, across all sessions.
fn marked_panes(socket: &str) -> Vec<String> {
    query_lines(
        socket,
        &[
            "list-panes",
            "-a",
            "-F",
            &format!("#{{pane_id}} #{{{SEL_OPT}}}"),
        ],
    )
    .iter()
    .filter_map(|l| {
        let (id, sel) = l.split_once(' ')?;
        (sel == "1").then(|| id.to_string())
    })
    .collect()
}

/// Every pane id on the server.
fn all_panes(socket: &str) -> Vec<String> {
    query_lines(socket, &["list-panes", "-a", "-F", "#{pane_id}"])
}
