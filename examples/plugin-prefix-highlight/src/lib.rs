//! `plugin-prefix-highlight` — tmux-prefix-highlight, without the shell.
//!
//! The original is a `*.tmux` script that rewrites `status-right` at load,
//! substituting its own `#{prefix_highlight}` placeholder for a long
//! `#{?client_prefix,…,}` conditional built out of the user's `@`-options. It
//! works, and it is the shape every status-line plugin has to take when the
//! only extension point is a shell script: compute a format STRING once, and
//! hope the conditions it encodes are the ones you meant.
//!
//! Here the same feature is a **format provider**: `#{plugin_prefix_highlight}`
//! is resolved by this code every time the status line is drawn, so it answers
//! from the live client rather than from a string baked in at load.
//!
//! ```tmux
//! znative load path:examples/plugin-prefix-highlight
//! set -g status-right '#{plugin_prefix_highlight} %H:%M'
//! ```
//!
//! The `@`-options are the ones tmux-prefix-highlight already uses, so an
//! existing config keeps working:
//!
//! | Option | Default | Meaning |
//! | --- | --- | --- |
//! | `@prefix_highlight_fg` | `colour231` | text colour |
//! | `@prefix_highlight_bg` | `colour04` | background colour |
//! | `@prefix_highlight_output_prefix` | `[` | put before the label |
//! | `@prefix_highlight_output_suffix` | `]` | put after it |
//! | `@prefix_highlight_prefix_prompt` | `Wait` | shown while the prefix is held |
//! | `@prefix_highlight_show_copy_mode` | `off` | also light up in copy mode |
//! | `@prefix_highlight_copy_prompt` | `Copy` | the copy-mode label |
//! | `@prefix_highlight_copy_mode_attr` | `fg=default,bg=yellow` | the copy-mode style |
//! | `@prefix_highlight_show_sync_mode` | `off` | also light up with synchronize-panes |
//! | `@prefix_highlight_sync_prompt` | `Sync` | the sync label |
//! | `@prefix_highlight_sync_mode_attr` | `fg=default,bg=green` | the sync style |
//! | `@prefix_highlight_empty_prompt` | *(empty)* | shown when nothing is active |
//! | `@prefix_highlight_empty_attr` | `fg=default,bg=default` | the idle style |
//!
//! What it demonstrates: **a format provider that reads the expansion it is
//! part of**. `#{client_prefix}`, `#{pane_in_mode}` and `#{synchronize-panes}`
//! are per-client, per-pane state, so the provider expands them through its
//! [`Ctx`] — the same tree the status line is being drawn from. Without that
//! (ABI v1) a provider could only report globals, which is not enough to write
//! this plugin at all.

use std::os::raw::c_int;

// The ABI, copied in. A plugin outside this repo copies
// `plugin-abi/ztnative.rs` into its own `src/`; this one points at the
// in-tree original so the examples cannot drift from it.
#[path = "../../../plugin-abi/ztnative.rs"]
mod ztnative;

use crate::ztnative::{Args, Ctx, Host};

/// Read an `@`-option, falling back to the same default the shell plugin uses.
fn opt(host: &Host, name: &str, default: &str) -> String {
    host.get_option(name)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// True when an option is set to something tmux would call on.
fn flag(host: &Host, name: &str) -> bool {
    matches!(
        host.get_option(name).as_deref(),
        Some("on" | "yes" | "1" | "true")
    )
}

/// Expand `fmt` against the tree being drawn and test it the way tmux's own
/// `#{?…}` does: non-empty and not `0`.
fn truthy(host: &Host, ctx: &Ctx, fmt: &str) -> bool {
    match host.format_expand(ctx, fmt) {
        Some(v) => !v.is_empty() && v != "0",
        None => false,
    }
}

/// `#{plugin_prefix_highlight}` — the highlight itself.
fn highlight(host: &Host, ctx: &Ctx, _key: &str) -> Option<String> {
    let fg = opt(host, "@prefix_highlight_fg", "colour231");
    let bg = opt(host, "@prefix_highlight_bg", "colour04");
    let open = opt(host, "@prefix_highlight_output_prefix", "[");
    let close = opt(host, "@prefix_highlight_output_suffix", "]");

    // Order matters and matches the original: the prefix wins over copy mode,
    // copy mode over synchronize-panes.
    let (label, style) = if truthy(host, ctx, "#{client_prefix}") {
        (
            opt(host, "@prefix_highlight_prefix_prompt", "Wait"),
            format!("fg={fg},bg={bg}"),
        )
    } else if flag(host, "@prefix_highlight_show_copy_mode") && truthy(host, ctx, "#{pane_in_mode}")
    {
        (
            opt(host, "@prefix_highlight_copy_prompt", "Copy"),
            opt(host, "@prefix_highlight_copy_mode_attr", "fg=default,bg=yellow"),
        )
    } else if flag(host, "@prefix_highlight_show_sync_mode")
        && truthy(host, ctx, "#{synchronize-panes}")
    {
        (
            opt(host, "@prefix_highlight_sync_prompt", "Sync"),
            opt(host, "@prefix_highlight_sync_mode_attr", "fg=default,bg=green"),
        )
    } else {
        // Idle. The original still paints the empty prompt in its own style so
        // a fixed-width status bar does not jump when the prefix is pressed.
        let empty = opt(host, "@prefix_highlight_empty_prompt", "");
        if empty.is_empty() {
            return Some(String::new());
        }
        (
            empty,
            opt(host, "@prefix_highlight_empty_attr", "fg=default,bg=default"),
        )
    };

    Some(format!("#[{style}]{open}{label}{close}#[default]"))
}

/// `prefix-highlight` — print what the provider would render right now, for a
/// config that wants to check its options without staring at the status bar.
fn show(host: &Host, ctx: &Ctx, _args: &Args) -> c_int {
    match highlight(host, ctx, "plugin_prefix_highlight") {
        Some(v) if v.is_empty() => host.print(ctx, "(idle, empty prompt unset)"),
        Some(v) => host.print(ctx, &v),
        None => host.print(ctx, "(nothing)"),
    }
    0
}

declare_plugin! {
    name: "prefix-highlight",
    version: "0.1.0",
    commands: {
        "prefix-highlight" => { template: "", usage: "", handler: show },
    },
    formats: { "plugin_prefix_highlight" => highlight },
}
