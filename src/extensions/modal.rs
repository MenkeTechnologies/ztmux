//! `ztmux modal` — zellij-style modal keybindings.
//!
//! Zellij's defining interaction: instead of a prefix, you enter a *mode* and
//! then press single keys. `Ctrl-p` pane mode, `Ctrl-t` tab mode, `Ctrl-n`
//! resize mode, `Ctrl-o` session mode, `Ctrl-s` scroll (copy) mode, `Ctrl-g`
//! lock. Each mode is a tmux key table whose bindings re-assert the table so it
//! stays "sticky" (Enter/Escape/`q` leave it); the `Ctrl-*` entry keys live in
//! the root table so they work with no prefix, exactly like zellij.
//!
//! It is opt-in (`modal on`) because those root `Ctrl-*` keys are intercepted
//! globally — the zellij trade-off. `modal off` removes the entry keys and
//! restores the prefix. The mode tables are installed with `source-file` (the
//! only way to issue sticky `{ … }` command groups programmatically), and the
//! current mode is shown by the ratatui hint bar, which lists the active key
//! table's keys — so `modal on` turns the hint bar on and `modal off` restores
//! its prior state.
//!
//! Subcommands: `toggle` (default), `on`, `off`.

use super::tmux_query::query_lines;

const MODAL_OPT: &str = "@ztmux-modal";
/// Saved prior `@ztmux-hint` value, so `off` can put it back.
const HINT_SAVE: &str = "@ztmux-modal-saved-hint";

/// The root-table entry keys the modal system installs (removed on `off`).
const ENTRY_KEYS: &[&str] = &["C-p", "C-t", "C-n", "C-s", "C-o", "C-g"];

/// The full mode-table + entry-key config, sourced in on `on`. Sticky modes
/// re-assert their table; Enter/Escape/`q` return to the root table.
const CONFIG: &str = r#"# ztmux zellij-style modal keybindings (ztmux modal on)
# --- Pane mode (Ctrl-p) ---
bind -T pane -N 'move left'   h { select-pane -L ; switch-client -T pane }
bind -T pane -N 'move down'   j { select-pane -D ; switch-client -T pane }
bind -T pane -N 'move up'     k { select-pane -U ; switch-client -T pane }
bind -T pane -N 'move right'  l { select-pane -R ; switch-client -T pane }
bind -T pane -N 'new pane'    n { split-window ; switch-client -T root }
bind -T pane -N 'split right' d { split-window -h ; switch-client -T root }
bind -T pane -N 'close pane'  x { kill-pane }
bind -T pane -N 'fullscreen'  z { resize-pane -Z ; switch-client -T pane }
bind -T pane -N 'rename pane' r { command-prompt -p 'pane name:' 'select-pane -T "%%"' }
bind -T pane -N 'exit' Enter  { switch-client -T root }
bind -T pane -N 'exit' Escape { switch-client -T root }
bind -T pane -N 'exit' q { switch-client -T root }
# --- Tab (window) mode (Ctrl-t) ---
bind -T tab -N 'previous' h { previous-window ; switch-client -T tab }
bind -T tab -N 'next'     l { next-window ; switch-client -T tab }
bind -T tab -N 'new tab'  n { new-window }
bind -T tab -N 'close tab' x { kill-window }
bind -T tab -N 'rename tab' r { command-prompt -p 'tab name:' 'rename-window "%%"' }
bind -T tab -N 'exit' Enter  { switch-client -T root }
bind -T tab -N 'exit' Escape { switch-client -T root }
bind -T tab -N 'exit' q { switch-client -T root }
# --- Resize mode (Ctrl-n) ---
bind -T resize -N 'left'  h { resize-pane -L 5 ; switch-client -T resize }
bind -T resize -N 'down'  j { resize-pane -D 5 ; switch-client -T resize }
bind -T resize -N 'up'    k { resize-pane -U 5 ; switch-client -T resize }
bind -T resize -N 'right' l { resize-pane -R 5 ; switch-client -T resize }
bind -T resize -N 'exit' Enter  { switch-client -T root }
bind -T resize -N 'exit' Escape { switch-client -T root }
bind -T resize -N 'exit' q { switch-client -T root }
# --- Session mode (Ctrl-o) ---
bind -T session -N 'session manager' w { display-popup -E -w 80% -h 70% 'ztmux -S "${TMUX%%,*}" sessions' }
bind -T session -N 'detach' d { detach-client }
bind -T session -N 'exit' Enter  { switch-client -T root }
bind -T session -N 'exit' Escape { switch-client -T root }
bind -T session -N 'exit' q { switch-client -T root }
# --- Locked mode (Ctrl-g) — every key passes through until Ctrl-g ---
bind -T locked -N 'unlock' C-g { switch-client -T root }
# --- Entry keys (root table, no prefix) ---
bind -n -N 'ztmux: pane mode'    C-p { switch-client -T pane }
bind -n -N 'ztmux: tab mode'     C-t { switch-client -T tab }
bind -n -N 'ztmux: resize mode'  C-n { switch-client -T resize }
bind -n -N 'ztmux: scroll mode'  C-s { copy-mode }
bind -n -N 'ztmux: session mode' C-o { switch-client -T session }
bind -n -N 'ztmux: lock'         C-g { switch-client -T locked }
"#;

pub(crate) fn run(socket: &str) -> i32 {
    match op_arg().as_deref() {
        Some("on") => set_modal(socket, true),
        Some("off") => set_modal(socket, false),
        _ => set_modal(socket, !is_on(socket)),
    }
    0
}

/// The subcommand word after `modal` (`ztmux modal on`).
fn op_arg() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let i = args.iter().position(|a| a == "modal")?;
    args.get(i + 1).filter(|s| !s.starts_with('-')).cloned()
}

fn is_on(socket: &str) -> bool {
    query_lines(socket, &["show-options", "-gqv", MODAL_OPT])
        .first()
        .is_some_and(|v| v.trim() == "1")
}

fn set_modal(socket: &str, on: bool) {
    if on {
        // Install the tables + entry keys via a temp config file (source-file is
        // the only way to issue the sticky `{ … }` command groups), then remember
        // the hint bar's prior state and turn it on as the mode indicator.
        let path = std::env::temp_dir().join(format!("ztmux-modal-{}.conf", std::process::id()));
        if std::fs::write(&path, CONFIG).is_err() {
            return;
        }
        let p = path.to_string_lossy().to_string();
        let _ = query_lines(socket, &["source-file", &p]);
        let _ = std::fs::remove_file(&path);
        let _ = query_lines(
            socket,
            &[
                "set-option",
                "-gF",
                HINT_SAVE,
                "#{@ztmux-hint}",
                ";",
                "set-option",
                "-g",
                "@ztmux-hint",
                "on",
                ";",
                "set-option",
                "-g",
                MODAL_OPT,
                "1",
            ],
        );
    } else {
        // Remove the entry keys (the mode tables become unreachable), restore the
        // hint bar's saved state, and clear the markers.
        let mut argv: Vec<&str> = Vec::new();
        for (i, k) in ENTRY_KEYS.iter().enumerate() {
            if i != 0 {
                argv.push(";");
            }
            argv.extend(["unbind-key", "-n", k]);
        }
        argv.extend([
            ";",
            "set-option",
            "-gF",
            "@ztmux-hint",
            "#{@ztmux-modal-saved-hint}",
            ";",
            "set-option",
            "-gu",
            HINT_SAVE,
            ";",
            "set-option",
            "-gu",
            MODAL_OPT,
        ]);
        let _ = query_lines(socket, &argv);
    }
}
