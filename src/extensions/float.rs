//! ztmux extension: zellij-style auto-hiding of floating panes.
//!
//! Upstream tmux keeps a floating pane on screen when a tiled pane takes focus,
//! and clips the tiled pane's output around it. zellij instead treats floating
//! panes as a mode: they vanish when you focus a tiled pane and come back when
//! you toggle them on again.
//!
//! Setting `@ztmux-float-autohide` on the window (or globally with `set -wg`)
//! selects the zellij behaviour. It is off by default, so ztmux behaves like
//! upstream tmux unless asked otherwise.
use crate::*;

/// Window option enabling the zellij behaviour.
pub(crate) const AUTOHIDE_OPT: &str = "@ztmux-float-autohide";

/// Whether `wp` is a floating pane that should currently be hidden: the option
/// is on for its window and the active pane is a tiled one.
///
/// Consulted from `window_pane_visible`, so a hidden float is skipped by the
/// redraw and drops out of the visible-range clipping too — a hidden pane must
/// not carve holes in the panes below it.
pub(crate) unsafe fn is_hidden(wp: *mut window_pane) -> bool {
    unsafe {
        if window_pane_is_floating(wp) == 0 {
            return false;
        }
        let w = (*wp).window;
        let active = (*w).active;
        if active.is_null() || window_pane_is_floating(active) != 0 {
            return false;
        }
        autohide_enabled(w)
    }
}

/// Read `@ztmux-float-autohide` from the window, falling back to the global
/// window options. Absent or a false-ish value means off.
unsafe fn autohide_enabled(w: *mut window) -> bool {
    unsafe {
        for oo in [(*w).options, GLOBAL_W_OPTIONS] {
            if oo.is_null() || options_::options_get_only_const(oo, AUTOHIDE_OPT).is_null() {
                continue;
            }
            let s = cstr_to_str(options_::options_get_string_(oo, AUTOHIDE_OPT));
            let s = s.trim();
            return !(s.is_empty()
                || s.eq_ignore_ascii_case("0")
                || s.eq_ignore_ascii_case("off")
                || s.eq_ignore_ascii_case("false")
                || s.eq_ignore_ascii_case("no"));
        }
        false
    }
}
