// Copyright (c) 2026 Nicholas Marriott <nicholas.marriott@gmail.com>
//
// Permission to use, copy, modify, and distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF MIND, USE, DATA OR PROFITS, WHETHER
// IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING
// OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

//! The command prompt's own settings.
//!
//! Upstream `prompt.c` owns a whole `struct prompt` object shared by the status
//! line, the tree modes and the switch mode. ztmux still keeps the prompt state
//! on the client (`status.rs`), so what lives here is the part of `prompt.c`
//! that resolves a prompt's appearance from the session options — which is the
//! piece the `prompt-cursor-*` options drive.

use crate::options_::*;
use crate::*;

/// Resolve the cursor a prompt shows from the session's options, once when the
/// prompt opens rather than on every redraw.
///
/// Two cursors are resolved, not one: with `status-keys vi` the prompt has a
/// command mode, and upstream lets it be styled separately so the mode is
/// visible at a glance. The colour options are read through `style_apply`
/// (rather than as plain colours) because they are `OPTIONS_TABLE_IS_COLOUR`
/// entries whose empty default means "leave the terminal's own cursor colour
/// alone" — which is what a `fg` of -1 says downstream.
///
/// C `vendor/tmux/prompt.c:110`: `void prompt_set_options(struct
/// prompt_create_data *pd, struct session *s)`
pub unsafe fn prompt_set_options(c: *mut client, s: *mut session) {
    unsafe {
        let oo = if !s.is_null() {
            (*s).options
        } else {
            GLOBAL_S_OPTIONS
        };
        let mut gc: grid_cell = zeroed();

        let n = options_get_number_(oo, "prompt-cursor-style") as u32;
        screen_set_cursor_style(
            n,
            &raw mut (*c).prompt_cstyle,
            &raw mut (*c).prompt_cmode,
        );
        let n = options_get_number_(oo, "prompt-command-cursor-style") as u32;
        screen_set_cursor_style(
            n,
            &raw mut (*c).prompt_command_cstyle,
            &raw mut (*c).prompt_command_cmode,
        );

        style_apply(&raw mut gc, oo, c!("prompt-cursor-colour"), null_mut());
        (*c).prompt_ccolour = gc.fg;
        style_apply(
            &raw mut gc,
            oo,
            c!("prompt-command-cursor-colour"),
            null_mut(),
        );
        (*c).prompt_command_ccolour = gc.fg;
    }
}
