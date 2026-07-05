//! `ztmux tabs` — a zellij-style tab bar across the top of the screen.
//!
//! Zellij shows tabs (ztmux windows) as flat coloured segments along a top bar,
//! the active one highlighted. ztmux has no separate decoration row, but tmux's
//! own status line does exactly this job — reserved by the server, resized for
//! free, and rendered with `window-status-*` formats. So this styles the status
//! line into a zellij tab bar: moved to the top, session badge on the left,
//! windows as ` #I #W ` segments with the current one in a highlight colour.
//!
//! It is fully reversible: `tabs on` first saves the prior value of every status
//! option it touches into `@ztmux-tab-saved-*`, and `tabs off` restores them, so
//! a user's custom status line comes back untouched. The bottom stays free for
//! the ratatui info bar (status at the top), matching zellij's top-tabs /
//! bottom-hints layout.
//!
//! Subcommands: `toggle` (default), `on`, `off`.

use super::tmux_query::query_lines;

/// Marks the tab bar as active (and namespaces the saved originals).
const TAB_OPT: &str = "@ztmux-tab-bar";

/// The status options the zellij tab bar owns, each paired with the value it is
/// set to when the bar is on. Every one is saved before being overwritten.
const STYLE: &[(&str, &str)] = &[
    ("status", "on"),
    ("status-position", "top"),
    ("status-justify", "left"),
    ("status-style", "bg=colour235,fg=colour245"),
    (
        "status-left",
        "#[bg=colour108,fg=colour235,bold] #S #[default] ",
    ),
    ("status-left-length", "40"),
    (
        "window-status-format",
        "#[fg=colour245,bg=colour237] #I #W #[default]",
    ),
    (
        "window-status-current-format",
        "#[fg=colour235,bg=colour108,bold] #I #W #[default]",
    ),
    ("window-status-separator", " "),
    (
        // Commas inside a `#[…]` run that sits inside a `#{?…,a,b}` conditional
        // must be escaped `#,`, else tmux reads them as the branch separator.
        "status-right",
        "#{?client_prefix,#[bg=colour214#,fg=colour235] PREFIX #[default] ,}#[fg=colour245]%H:%M ",
    ),
    ("status-right-length", "40"),
];

pub(crate) fn run(socket: &str) -> i32 {
    match op_arg().as_deref() {
        Some("on") => set_tabs(socket, true),
        Some("off") => set_tabs(socket, false),
        _ => set_tabs(socket, !is_on(socket)),
    }
    0
}

/// The subcommand word after `tabs` (`ztmux tabs on`).
fn op_arg() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let i = args.iter().position(|a| a == "tabs")?;
    args.get(i + 1).filter(|s| !s.starts_with('-')).cloned()
}

/// Whether the zellij tab bar is currently on.
fn is_on(socket: &str) -> bool {
    query_lines(socket, &["show-options", "-gqv", TAB_OPT])
        .first()
        .is_some_and(|v| v.trim() == "1")
}

/// The `@ztmux-tab-saved-…` key that holds `name`'s pre-tab-bar value.
fn saved_key(name: &str) -> String {
    format!("@ztmux-tab-saved-{name}")
}

fn set_tabs(socket: &str, on: bool) {
    // Everything runs as ONE batched tmux invocation (commands separated by a
    // literal `;` argument), with NO read-back round-trip: values are saved and
    // restored server-side with `set -gF … "#{option}"` format expansion. That
    // keeps this a single nested `ztmux` call, so it also works when invoked from
    // a `run-shell -b` menu/binding (a read-then-write would double-nest and the
    // inner queries would fail while the outer call is in flight).
    let saved: Vec<String> = STYLE.iter().map(|(k, _)| saved_key(k)).collect();
    // `#{<option>}` / `#{<@saved>}` format strings referencing each option value.
    let read_fmt: Vec<String> = STYLE.iter().map(|(k, _)| format!("#{{{k}}}")).collect();
    let saved_fmt: Vec<String> = saved.iter().map(|s| format!("#{{{s}}}")).collect();

    let mut argv: Vec<&str> = Vec::new();
    if on {
        for (i, (k, v)) in STYLE.iter().enumerate() {
            // Save the current value (via format expansion), then apply ours.
            push_cmd(
                &mut argv,
                &["set-option", "-gF", &saved[i], &read_fmt[i]],
                i == 0,
            );
            push_cmd(&mut argv, &["set-option", "-g", k, v], false);
        }
        push_cmd(&mut argv, &["set-option", "-g", TAB_OPT, "1"], false);
    } else {
        for (i, (k, _)) in STYLE.iter().enumerate() {
            // Restore the saved value (via format expansion), then drop the marker.
            push_cmd(&mut argv, &["set-option", "-gF", k, &saved_fmt[i]], i == 0);
            push_cmd(&mut argv, &["set-option", "-gu", &saved[i]], false);
        }
        push_cmd(&mut argv, &["set-option", "-gu", TAB_OPT], false);
    }
    let _ = query_lines(socket, &argv);
}

/// Append one tmux command to a batched argv, prefixing a `;` separator unless it
/// is the first command in the batch.
fn push_cmd<'a>(argv: &mut Vec<&'a str>, cmd: &[&'a str], first: bool) {
    if !first {
        argv.push(";");
    }
    argv.extend_from_slice(cmd);
}
