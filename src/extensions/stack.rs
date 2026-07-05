//! `ztmux stack` — zellij-style pane stacks, ported from zellij's `StackedPanes`
//! (`zellij-server/src/panes/tiled_panes/stacked_panes.rs`).
//!
//! Zellij's model: the panes of a stack form a vertical column sharing an x and
//! width; exactly one is the *flexible* pane (its `rows` is percentage-based and
//! fills the whole stack minus the others), and every other pane is collapsed to
//! `Dimension::fixed(1)` — a one-row title bar. Focusing a pane makes it the
//! flexible one and collapses the previously-flexible pane (zellij's
//! `expand_pane()` / `make_*_pane_in_stack_flexible()`).
//!
//! Zellij owns its own layout engine (`PaneGeom` / `Dimension`), which ztmux — a
//! from-source *tmux* port — does not have. So this ports the model and geometry
//! logic and realises it through tmux's own layout: equalise the column
//! (`select-layout even-vertical`), then grow the flexible (active) pane to the
//! full window height (`resize-pane -y 999`), which forces every other pane down
//! to its one-row minimum — exactly zellij's geometry (flexible pane = H-(N-1),
//! all others fixed(1)). Growing the active pane is order-independent, unlike
//! shrinking each non-active pane in turn (tmux's `resize-pane` hands freed rows
//! to a *neighbour*, so per-pane shrinks cascade height into whichever pane sits
//! next to the last one collapsed rather than into the active pane).
//!
//! Subcommands: `toggle` (default), `refocus` (re-collapse to the active pane so
//! navigating the stack expands the newly-focused pane), and `off`.
//!
//! This module is the reference implementation, reachable as a top-level command
//! (`ztmux stack …`) and via `:stack` (a `display-popup` that runs it as its own
//! process). The `prefix +` binding and the `window-pane-changed` refocus hook
//! do NOT call it — they inline the same geometry in pure tmux
//! (`select-layout even-vertical ; resize-pane -y 999`). A hook or key binding
//! that shelled out to `ztmux stack` would spawn a subprocess that connects back
//! into the server mid-command (reentrant), and the nested queries fail; pure
//! tmux avoids both the reentrancy and a per-focus process spawn.

use super::tmux_query::query_lines;

/// Per-window flag marking the window as stacked (zellij's `stacked: Some(id)`).
const STACKED_OPT: &str = "@ztmux-stacked";

pub(crate) fn run(socket: &str) -> i32 {
    let win = target_window();
    let win = win.as_deref().unwrap_or("");
    match op_arg().as_deref() {
        Some("off") | Some("unstack") => set_stacked(socket, win, false),
        // Re-establish the stack geometry around whichever pane is now flexible
        // (active). zellij does this in expand_pane() on focus.
        Some("refocus") | Some("expand") => {
            if is_stacked(socket, win) {
                collapse_to_flexible(socket, win);
            }
        }
        // Default: toggle the whole window in/out of stacked mode.
        _ => set_stacked(socket, win, !is_stacked(socket, win)),
    }
    0
}

/// The subcommand word after `stack` (`ztmux stack refocus -t @3`).
fn op_arg() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let i = args.iter().position(|a| a == "stack")?;
    args.get(i + 1).filter(|s| !s.starts_with('-')).cloned()
}

/// The `-t <window>` the stack acts on (the binding/hook passes `#{window_id}`).
fn target_window() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let i = args.iter().position(|a| a == "-t")?;
    args.get(i + 1).cloned()
}

/// Whether the window is currently stacked.
fn is_stacked(socket: &str, win: &str) -> bool {
    query_lines(socket, &tgt(win, &["show-options", "-wqv", STACKED_OPT]))
        .first()
        .is_some_and(|v| v.trim() == "1")
}

/// Enter or leave stacked mode for the window.
fn set_stacked(socket: &str, win: &str, on: bool) {
    let _ = query_lines(
        socket,
        &tgt(
            win,
            &["set-option", "-w", STACKED_OPT, if on { "1" } else { "0" }],
        ),
    );
    // Reset to an even column either way (zellij lays the stack out top-to-bottom).
    let _ = query_lines(socket, &tgt(win, &["select-layout", "even-vertical"]));
    if on {
        collapse_to_flexible(socket, win);
    }
}

/// Collapse every non-active pane in the window to a single row (zellij's
/// `Dimension::fixed(1)`), leaving the active pane flexible (it absorbs the rest
/// of the height, = H-(N-1)). Equalise first so the geometry is deterministic
/// regardless of the previous layout, then grow the active pane to the full
/// height — `resize-pane -t <window>` targets that window's active pane, and
/// `-y 999` clamps to the window height, squeezing all others to one row. Both
/// commands go in ONE tmux invocation (separated by a literal `;` argument).
fn collapse_to_flexible(socket: &str, win: &str) {
    let mut argv: Vec<String> = vec!["select-layout".into()];
    if !win.is_empty() {
        argv.push("-t".into());
        argv.push(win.into());
    }
    argv.push("even-vertical".into());
    argv.push(";".into());
    argv.push("resize-pane".into());
    if !win.is_empty() {
        argv.push("-t".into());
        argv.push(win.into());
    }
    argv.extend(["-y".into(), "999".into()]);
    let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
    let _ = query_lines(socket, &refs);
}

/// Build an argv with a `-t <win>` inserted after the command word (the first
/// arg), so `tgt("@3", &["select-layout", "even-vertical"])` becomes
/// `select-layout -t @3 even-vertical`.
fn tgt<'a>(win: &'a str, args: &[&'a str]) -> Vec<&'a str> {
    let mut out: Vec<&str> = Vec::with_capacity(args.len() + 2);
    out.push(args[0]);
    if !win.is_empty() {
        out.push("-t");
        out.push(win);
    }
    out.extend_from_slice(&args[1..]);
    out
}
