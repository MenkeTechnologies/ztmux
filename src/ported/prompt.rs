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

//! Port of `vendor/tmux/prompt.c`: the editable prompt as a standalone object.
//!
//! The prompt owns its own text, cursor, history index, styles and callbacks,
//! and knows how to draw itself into an arbitrary row of an arbitrary screen.
//! That is what lets the status line, the tree modes and the switch mode each
//! run one without duplicating the editor: they hand `prompt_draw` a row and
//! feed `prompt_key`/`prompt_mouse` the input.

use crate::*;
use crate::options_::*;

/// C `vendor/tmux/prompt.c:29`: `struct prompt`.
#[repr(C)]
pub struct prompt {
    string: *mut u8,
    pub(crate) buffer: *mut utf8_data,
    state: cmd_find_state,
    last: *mut u8,
    pub(crate) index: usize,

    inputcb: prompt_input_cb,
    freecb: prompt_free_cb,
    data: *mut c_void,

    message_format: *mut u8,
    keys: c_int,
    word_separators: *mut u8,
    style: grid_cell,
    command_style: grid_cell,
    cstyle: screen_cursor_style,
    command_cstyle: screen_cursor_style,
    ccolour: c_int,
    command_ccolour: c_int,
    cmode: mode_flag,
    command_cmode: mode_flag,

    type_: prompt_type,
    flags: prompt_flags,
    closed: c_int,

    hindex: [c_uint; PROMPT_NTYPES as usize],
    copied: *mut utf8_data,

    complete_list: *mut *mut u8,
    complete_size: c_uint,
    complete_display: *mut u8,
}

impl prompt {
    /// Borrowed prompt string (the `(command) ` label), for hosts that draw
    /// their own chrome around the prompt rather than calling `prompt_draw`.
    #[inline]
    pub(crate) fn string_ptr(&self) -> *const u8 {
        self.string
    }
}

/// Get prompt flags as a string.
/// C `vendor/tmux/prompt.c:72`: `static const char *prompt_flags_to_string(int flags)`
fn prompt_flags_to_string(flags: prompt_flags) -> String {
    let mut out = String::new();
    for (bit, name) in [
        (prompt_flags::PROMPT_SINGLE, "SINGLE"),
        (prompt_flags::PROMPT_NUMERIC, "NUMERIC"),
        (prompt_flags::PROMPT_INCREMENTAL, "INCREMENTAL"),
        (prompt_flags::PROMPT_NOFORMAT, "NOFORMAT"),
        (prompt_flags::PROMPT_KEY, "KEY"),
        (prompt_flags::PROMPT_ACCEPT, "ACCEPT"),
        (prompt_flags::PROMPT_QUOTENEXT, "QUOTENEXT"),
        (prompt_flags::PROMPT_BSPACE_EXIT, "BSPACE_EXIT"),
        (prompt_flags::PROMPT_NOFREEZE, "NOFREEZE"),
        (prompt_flags::PROMPT_COMMANDMODE, "COMMANDMODE"),
        (prompt_flags::PROMPT_ISPANE, "ISPANE"),
        (prompt_flags::PROMPT_ISMODE, "ISMODE"),
        (prompt_flags::PROMPT_EDITARROWS, "EDITARROWS"),
    ] {
        if flags.intersects(bit) {
            if !out.is_empty() {
                out.push(',');
            }
            out.push_str(name);
        }
    }
    out
}

/// Set prompt options from session options.
/// C `vendor/tmux/prompt.c:110`: `void prompt_set_options(struct prompt_create_data *pd, struct session *s)`
///
/// Everything a prompt's appearance depends on is resolved here, once, when the
/// prompt is created — so a `set-option` while it is open cannot restyle it.
/// The two cursor colours go through `style_apply` rather than plain colour
/// parsing because they are `OPTIONS_TABLE_IS_COLOUR` entries whose empty
/// default means "leave the terminal's own cursor colour alone", which is what a
/// `fg` of -1 says downstream.
pub unsafe fn prompt_set_options(pd: *mut prompt_create_data, s: *mut session) {
    unsafe {
        let oo = if !s.is_null() {
            (*s).options
        } else {
            GLOBAL_S_OPTIONS
        };
        let mut gc: grid_cell = zeroed();

        style_apply(&raw mut (*pd).style, oo, c!("message-style"), null_mut());
        style_apply(
            &raw mut (*pd).command_style,
            oo,
            c!("message-command-style"),
            null_mut(),
        );
        let n = options_get_number_(oo, "prompt-cursor-style") as u32;
        screen_set_cursor_style(n, &raw mut (*pd).cstyle, &raw mut (*pd).cmode);
        let n = options_get_number_(oo, "prompt-command-cursor-style") as u32;
        screen_set_cursor_style(
            n,
            &raw mut (*pd).command_cstyle,
            &raw mut (*pd).command_cmode,
        );
        style_apply(&raw mut gc, oo, c!("prompt-cursor-colour"), null_mut());
        (*pd).ccolour = gc.fg;
        style_apply(
            &raw mut gc,
            oo,
            c!("prompt-command-cursor-colour"),
            null_mut(),
        );
        (*pd).command_ccolour = gc.fg;
        (*pd).message_format = options_get_string_(oo, "message-format");
        (*pd).keys = options_get_number_(oo, "status-keys") as i32;
        (*pd).word_separators = options_get_string_(oo, "word-separators");
    }
}

/// Create prompt.
/// C `vendor/tmux/prompt.c:138`: `struct prompt *prompt_create(const struct prompt_create_data *pd)`
pub unsafe fn prompt_create(pd: *const prompt_create_data) -> *mut prompt {
    unsafe {
        let mut input = (*pd).input;

        let pr: *mut prompt = xcalloc_::<prompt>(1).as_ptr();

        let ft = if !(*pd).fs.is_null() {
            let ft = format_create_from_state(null_mut(), null_mut(), (*pd).fs);
            cmd_find_copy_state(&raw mut (*pr).state, (*pd).fs);
            ft
        } else {
            let ft = format_create_defaults(null_mut(), null_mut(), null_mut(), null_mut(), null_mut());
            cmd_find_clear_state(&raw mut (*pr).state, cmd_find_flags::empty());
            ft
        };

        if input.is_null() {
            input = c!("");
        }
        (*pr).string = xstrdup((*pd).prompt).as_ptr();
        let tmp = if (*pd).flags.intersects(prompt_flags::PROMPT_NOFORMAT) {
            xstrdup(input).as_ptr()
        } else {
            format_expand_time(ft, input)
        };
        if (*pd).flags.intersects(prompt_flags::PROMPT_INCREMENTAL) {
            (*pr).last = xstrdup(tmp).as_ptr();
            (*pr).buffer = utf8_fromcstr(c!(""));
        } else {
            (*pr).last = null_mut();
            (*pr).buffer = utf8_fromcstr(tmp);
        }
        (*pr).index = utf8_strlen((*pr).buffer);
        free_(tmp);

        (*pr).inputcb = (*pd).inputcb;
        (*pr).freecb = (*pd).freecb;
        (*pr).data = (*pd).data;

        (*pr).flags = (*pd).flags;
        (*pr).type_ = (*pd).type_;

        memcpy__(&raw mut (*pr).style, &raw const (*pd).style);
        memcpy__(&raw mut (*pr).command_style, &raw const (*pd).command_style);
        (*pr).cstyle = (*pd).cstyle;
        (*pr).command_cstyle = (*pd).command_cstyle;
        (*pr).ccolour = (*pd).ccolour;
        (*pr).command_ccolour = (*pd).command_ccolour;
        (*pr).cmode = (*pd).cmode;
        (*pr).command_cmode = (*pd).command_cmode;
        (*pr).message_format = xstrdup((*pd).message_format).as_ptr();
        (*pr).keys = (*pd).keys;
        (*pr).word_separators = xstrdup((*pd).word_separators).as_ptr();

        format_free(ft);
        pr
    }
}

/// Free prompt.
/// C `vendor/tmux/prompt.c:198`: `void prompt_free(struct prompt *pr)`
pub unsafe fn prompt_free(pr: *mut prompt) {
    unsafe {
        if pr.is_null() {
            return;
        }
        if let (Some(freecb), Some(data)) = ((*pr).freecb, NonNull::new((*pr).data)) {
            freecb(data);
        }
        free_((*pr).message_format);
        free_((*pr).word_separators);
        free_((*pr).last);
        free_((*pr).string);
        free_((*pr).buffer);
        free_((*pr).copied);
        prompt_clear_complete(pr);
        free_(pr);
    }
}

/// Fire the input callback. Returns one if the prompt is finished or zero if
/// still open.
/// C `vendor/tmux/prompt.c:219`: `static int prompt_fire_callback(struct prompt *pr, const char *s, enum prompt_key_result type, int *redraw)`
unsafe fn prompt_fire_callback(
    pr: *mut prompt,
    s: *const u8,
    type_: prompt_key_result,
    redraw: *mut i32,
) -> i32 {
    unsafe {
        let Some(inputcb) = (*pr).inputcb else {
            return 1;
        };
        let Some(data) = NonNull::new((*pr).data) else {
            return 1;
        };
        if inputcb(data, s, type_) == prompt_result::PROMPT_CLOSE {
            (*pr).closed = 1;
            return 1;
        }
        if !redraw.is_null() {
            *redraw = 1;
        }
        0
    }
}

/// Start incremental prompt.
/// C `vendor/tmux/prompt.c:236`: `void prompt_incremental_start(struct prompt *pr)`
pub unsafe fn prompt_incremental_start(pr: *mut prompt) {
    unsafe {
        if (*pr).flags.intersects(prompt_flags::PROMPT_INCREMENTAL) {
            let tmp = utf8_tocstr((*pr).buffer);
            let cp = format_nul!("={}", _s(tmp));
            prompt_fire_callback(pr, cp, prompt_key_result::PROMPT_KEY_HANDLED, null_mut());
            free_(cp);
            free_(tmp);
        }
    }
}

/// Update prompt.
/// C `vendor/tmux/prompt.c:251`: `void prompt_update(struct prompt *pr, const char *msg, const char *input)`
pub unsafe fn prompt_update(pr: *mut prompt, msg: *const u8, mut input: *const u8) {
    unsafe {
        let ft = if cmd_find_valid_state(&raw const (*pr).state) {
            format_create_from_state(null_mut(), null_mut(), &raw mut (*pr).state)
        } else {
            format_create_defaults(null_mut(), null_mut(), null_mut(), null_mut(), null_mut())
        };

        free_((*pr).string);
        (*pr).string = xstrdup(msg).as_ptr();

        if input.is_null() {
            input = c!("");
        }
        free_((*pr).buffer);
        let tmp = if (*pr).flags.intersects(prompt_flags::PROMPT_NOFORMAT) {
            xstrdup(input).as_ptr()
        } else {
            format_expand_time(ft, input)
        };
        (*pr).buffer = utf8_fromcstr(tmp);
        (*pr).index = utf8_strlen((*pr).buffer);
        free_(tmp);

        (*pr).hindex = [0; PROMPT_NTYPES as usize];
        (*pr).closed = 0;
        prompt_clear_complete(pr);

        format_free(ft);
    }
}

/// Is this prompt closed?
/// C `vendor/tmux/prompt.c:284`: `int prompt_closed(struct prompt *pr)`
pub unsafe fn prompt_closed(pr: *mut prompt) -> i32 {
    unsafe { (*pr).closed }
}

/// Redraw character. Return 1 if can continue redrawing, 0 otherwise.
/// C `vendor/tmux/prompt.c:291`: `static int prompt_redraw_character(struct screen_write_ctx *ctx, u_int offset, u_int pwidth, u_int *width, struct grid_cell *gc, const struct utf8_data *ud)`
unsafe fn prompt_redraw_character(
    ctx: *mut screen_write_ctx,
    offset: u32,
    pwidth: u32,
    width: *mut u32,
    gc: *mut grid_cell,
    ud: *const utf8_data,
) -> i32 {
    unsafe {
        if *width < offset {
            *width += (*ud).width as u32;
            return 1;
        }
        if *width >= offset + pwidth {
            return 0;
        }
        *width += (*ud).width as u32;
        if *width > offset + pwidth {
            return 0;
        }

        let ch = (*ud).data[0];
        if (*ud).size == 1 && (ch <= 0x1f || ch == 0x7f) {
            (*gc).data.data[0] = b'^';
            (*gc).data.data[1] = if ch == 0x7f { b'?' } else { ch | 0x40 };
            (*gc).data.size = 2;
            (*gc).data.have = 2;
            (*gc).data.width = 2;
        } else {
            utf8_copy(&raw mut (*gc).data, ud);
        }
        screen_write_cell(ctx, gc);
        1
    }
}

/// Redraw quote indicator `^` if necessary. Return 1 if can continue redrawing,
/// 0 otherwise.
/// C `vendor/tmux/prompt.c:324`: `static int prompt_redraw_quote(const struct prompt *pr, u_int pcursor, struct screen_write_ctx *ctx, u_int offset, u_int pwidth, u_int *width, struct grid_cell *gc)`
unsafe fn prompt_redraw_quote(
    pr: *const prompt,
    pcursor: u32,
    ctx: *mut screen_write_ctx,
    offset: u32,
    pwidth: u32,
    width: *mut u32,
    gc: *mut grid_cell,
) -> i32 {
    unsafe {
        if (*pr).flags.intersects(prompt_flags::PROMPT_QUOTENEXT)
            && (*(*ctx).s).cx == pcursor + 1
        {
            let mut ud: utf8_data = zeroed();
            utf8_set(&raw mut ud, b'^');
            return prompt_redraw_character(ctx, offset, pwidth, width, gc, &raw const ud);
        }
        1
    }
}

/// Draw the stored completion matches.
/// C `vendor/tmux/prompt.c:340`: `static void prompt_draw_complete(struct prompt *pr, struct screen_write_ctx *ctx, u_int ax, u_int aw, u_int cx, u_int py, const struct grid_cell *base)`
unsafe fn prompt_draw_complete(
    pr: *mut prompt,
    ctx: *mut screen_write_ctx,
    ax: u32,
    aw: u32,
    cx: u32,
    py: u32,
    base: *const grid_cell,
) {
    unsafe {
        if (*pr).complete_display.is_null() {
            return;
        }
        if (*pr).index != utf8_strlen((*pr).buffer) {
            return;
        }
        if cx < ax || cx - ax >= aw {
            return;
        }
        let avail = aw - (cx - ax);

        let mut gc: grid_cell = zeroed();
        memcpy__(&raw mut gc, base);
        gc.attr |= grid_attr::GRID_ATTR_UNDERSCORE;
        screen_write_cursormove(ctx, cx as i32, py as i32, 0);

        let mut width = 0;
        let ud = utf8_fromcstr((*pr).complete_display);
        let mut i = 0usize;
        while (*ud.add(i)).size != 0 {
            if width + (*ud.add(i)).width as u32 > avail {
                break;
            }
            utf8_copy(&raw mut gc.data, ud.add(i));
            screen_write_cell(ctx, &raw const gc);
            width += (*ud.add(i)).width as u32;
            i += 1;
        }
        free_(ud);
    }
}

/// Expand prompt string using the current input.
/// C `vendor/tmux/prompt.c:373`: `static char *prompt_expand(struct prompt *pr)`
unsafe fn prompt_expand(pr: *mut prompt) -> *mut u8 {
    unsafe {
        let ft = if cmd_find_valid_state(&raw const (*pr).state) {
            format_create_from_state(null_mut(), null_mut(), &raw mut (*pr).state)
        } else {
            format_create_defaults(null_mut(), null_mut(), null_mut(), null_mut(), null_mut())
        };
        let tmp = utf8_tocstr((*pr).buffer);
        format_add!(ft, "prompt_input", "{}", _s(tmp));
        free_(tmp);

        format_add!(ft, "prompt_flags", "{}", prompt_flags_to_string((*pr).flags));
        format_add!(ft, "prompt_type", "{}", prompt_type_string((*pr).type_ as u32));
        let prompt = format_expand_time(ft, (*pr).string);
        format_add!(ft, "message", "{}", _s(prompt));
        if (*pr).flags.intersects(prompt_flags::PROMPT_COMMANDMODE) {
            format_add!(ft, "command_prompt", "1");
        } else {
            format_add!(ft, "command_prompt", "0");
        }
        let expanded = format_expand_time(ft, (*pr).message_format);
        free_(prompt);
        format_free(ft);
        expanded
    }
}

/// Work out the width used by the prompt string.
/// C `vendor/tmux/prompt.c:402`: `static u_int prompt_width(struct prompt *pr, u_int aw)`
unsafe fn prompt_width(pr: *mut prompt, aw: u32) -> u32 {
    unsafe {
        let expanded = prompt_expand(pr);
        let mut start = format_width(cstr_to_str(expanded));
        if start > aw {
            start = aw;
        }
        free_(expanded);
        start
    }
}

/// Choose a completion from a mouse position.
/// C `vendor/tmux/prompt.c:417`: `static enum prompt_key_result prompt_mouse_complete(struct prompt *pr, u_int x, u_int cx, u_int ax, u_int aw, int *redraw)`
unsafe fn prompt_mouse_complete(
    pr: *mut prompt,
    x: u32,
    cx: u32,
    ax: u32,
    aw: u32,
    redraw: *mut i32,
) -> prompt_key_result {
    unsafe {
        if (*pr).complete_display.is_null() || (*pr).complete_size == 0 {
            return prompt_key_result::PROMPT_KEY_NOT_HANDLED;
        }
        if (*pr).index != utf8_strlen((*pr).buffer) {
            return prompt_key_result::PROMPT_KEY_NOT_HANDLED;
        }
        if cx < ax || cx - ax >= aw || x < cx {
            return prompt_key_result::PROMPT_KEY_NOT_HANDLED;
        }

        let avail = aw - (cx - ax);
        let clicked = x - cx;
        let mut width = utf8_cstrwidth((*pr).complete_display);
        if width > avail {
            width = avail;
        }
        if clicked >= width {
            return prompt_key_result::PROMPT_KEY_NOT_HANDLED;
        }

        let mut end = 0u32;
        for i in 0..(*pr).complete_size {
            let start = end + 1;
            end = start + utf8_cstrwidth(*(*pr).complete_list.add(i as usize));
            if clicked < start || clicked >= end {
                continue;
            }

            let replace = format_nul!("{} ", _s(*(*pr).complete_list.add(i as usize)));
            if prompt_replace_complete(pr, replace) != 0 {
                prompt_clear_complete(pr);
                if !redraw.is_null() {
                    *redraw = 1;
                }
            }
            free_(replace);
            return prompt_key_result::PROMPT_KEY_HANDLED;
        }
        prompt_key_result::PROMPT_KEY_HANDLED
    }
}

/// Draw prompt.
/// C `vendor/tmux/prompt.c:459`: `void prompt_draw(struct prompt *pr, struct prompt_draw_data *pd)`
pub unsafe fn prompt_draw(pr: *mut prompt, pd: *mut prompt_draw_data) {
    unsafe {
        let ctx = (*pd).ctx;
        let s = (*ctx).s;
        let ax = (*pd).area_x;
        let py = (*pd).prompt_line;
        let aw = (*pd).area_width;
        let cx = (*pd).cursor_x;
        let mut gc: grid_cell = zeroed();

        // Choose the cursor colour and style for this prompt.
        if (*pr).flags.intersects(prompt_flags::PROMPT_COMMANDMODE) {
            memcpy__(&raw mut gc, &raw const (*pr).command_style);
            (*s).default_cstyle = (*pr).command_cstyle;
            (*s).default_mode = (*pr).command_cmode;
            (*s).default_ccolour = (*pr).command_ccolour;
        } else {
            memcpy__(&raw mut gc, &raw const (*pr).style);
            (*s).default_cstyle = (*pr).cstyle;
            (*s).default_mode = (*pr).cmode;
            (*s).default_ccolour = (*pr).ccolour;
        }

        let expanded = prompt_expand(pr);
        let mut start = format_width(cstr_to_str(expanded));
        if start > aw {
            start = aw;
        }
        *cx = ax + start;

        screen_write_cursormove(ctx, ax as i32, py as i32, 0);
        format_draw(ctx, &raw const gc, aw, cstr_to_str(expanded), null_mut(), 0);
        screen_write_cursormove(ctx, (ax + start) as i32, py as i32, 0);
        free_(expanded);

        let left = aw - start;
        if left == 0 {
            return;
        }

        let pcursor = utf8_strwidth((*pr).buffer, (*pr).index as isize);
        let mut pwidth = utf8_strwidth((*pr).buffer, -1);
        if (*pr).flags.intersects(prompt_flags::PROMPT_QUOTENEXT) {
            pwidth += 1;
        }
        let offset;
        if pcursor >= left {
            // The cursor would be outside the screen so start drawing with it
            // on the right.
            offset = (pcursor - left) + 1;
            pwidth = left;
        } else {
            offset = 0;
        }
        if pwidth > left {
            pwidth = left;
        }
        *cx = ax + start + pcursor - offset;

        let mut width = 0u32;
        let mut i = 0usize;
        while (*(*pr).buffer.add(i)).size != 0 {
            if prompt_redraw_quote(pr, pcursor, ctx, offset, pwidth, &raw mut width, &raw mut gc)
                == 0
            {
                break;
            }
            if prompt_redraw_character(
                ctx,
                offset,
                pwidth,
                &raw mut width,
                &raw mut gc,
                (*pr).buffer.add(i),
            ) == 0
            {
                break;
            }
            i += 1;
        }
        prompt_redraw_quote(pr, pcursor, ctx, offset, pwidth, &raw mut width, &raw mut gc);

        prompt_draw_complete(pr, ctx, ax, aw, *cx, py, &raw const gc);
    }
}

/// Move cursor in prompt from a mouse position.
/// C `vendor/tmux/prompt.c:531`: `enum prompt_key_result prompt_mouse(struct prompt *pr, u_int x, u_int ax, u_int aw, int *redraw)`
pub unsafe fn prompt_mouse(
    pr: *mut prompt,
    x: u32,
    ax: u32,
    aw: u32,
    redraw: *mut i32,
) -> prompt_key_result {
    unsafe {
        if x < ax || x >= ax + aw {
            return prompt_key_result::PROMPT_KEY_NOT_HANDLED;
        }

        let start = prompt_width(pr, aw);
        let left = aw - start;
        if left == 0 {
            return prompt_key_result::PROMPT_KEY_HANDLED;
        }

        let pcursor = utf8_strwidth((*pr).buffer, (*pr).index as isize);
        let mut pwidth = utf8_strwidth((*pr).buffer, -1);
        if (*pr).flags.intersects(prompt_flags::PROMPT_QUOTENEXT) {
            pwidth += 1;
        }
        let offset = if pcursor >= left {
            (pcursor - left) + 1
        } else {
            0
        };

        let cx = ax + start + pcursor - offset;
        let result = prompt_mouse_complete(pr, x, cx, ax, aw, redraw);
        if result != prompt_key_result::PROMPT_KEY_NOT_HANDLED {
            return result;
        }

        let mut target = if x <= ax + start {
            offset
        } else {
            offset + x - (ax + start)
        };
        if target > pwidth {
            target = pwidth;
        }

        let mut width = 0u32;
        let mut idx = 0usize;
        while (*(*pr).buffer.add(idx)).size != 0 {
            if width >= target {
                break;
            }
            width += (*(*pr).buffer.add(idx)).width as u32;
            idx += 1;
        }
        if idx == (*pr).index {
            return prompt_key_result::PROMPT_KEY_HANDLED;
        }

        (*pr).index = idx;
        prompt_clear_complete(pr);
        if !redraw.is_null() {
            *redraw = 1;
        }

        prompt_key_result::PROMPT_KEY_HANDLED
    }
}

/// Is this a separator?
/// C `vendor/tmux/prompt.c:588`: `static int prompt_in_list(const char *ws, const struct utf8_data *ud)`
unsafe fn prompt_in_list(ws: *const u8, ud: *const utf8_data) -> i32 {
    unsafe {
        if (*ud).size != 1 || (*ud).width != 1 {
            return 0;
        }
        !libc::strchr(ws, (*ud).data[0] as i32).is_null() as i32
    }
}

/// Is this a space?
/// C `vendor/tmux/prompt.c:597`: `static int prompt_space(const struct utf8_data *ud)`
unsafe fn prompt_space(ud: *const utf8_data) -> i32 {
    unsafe {
        if (*ud).size != 1 || (*ud).width != 1 {
            return 0;
        }
        ((*ud).data[0] == b' ') as i32
    }
}

/// Is this a keypad key?
/// C `vendor/tmux/prompt.c:606`: `static key_code prompt_keypad_key(key_code key)`
fn prompt_keypad_key(key: key_code) -> key_code {
    if key & KEYC_MASK_MODIFIERS != 0 {
        return key;
    }
    match key {
        code::KEYC_KP_SLASH => b'/' as key_code,
        code::KEYC_KP_STAR => b'*' as key_code,
        code::KEYC_KP_MINUS => b'-' as key_code,
        code::KEYC_KP_SEVEN => b'7' as key_code,
        code::KEYC_KP_EIGHT => b'8' as key_code,
        code::KEYC_KP_NINE => b'9' as key_code,
        code::KEYC_KP_PLUS => b'+' as key_code,
        code::KEYC_KP_FOUR => b'4' as key_code,
        code::KEYC_KP_FIVE => b'5' as key_code,
        code::KEYC_KP_SIX => b'6' as key_code,
        code::KEYC_KP_ONE => b'1' as key_code,
        code::KEYC_KP_TWO => b'2' as key_code,
        code::KEYC_KP_THREE => b'3' as key_code,
        code::KEYC_KP_ENTER => b'\r' as key_code,
        code::KEYC_KP_ZERO => b'0' as key_code,
        code::KEYC_KP_PERIOD => b'.' as key_code,
        _ => key,
    }
}

/// Translate key from vi to emacs. Return 0 to drop key, 1 to process the key
/// as an emacs key; return 2 to append to the buffer. Set `*redraw` if the
/// translation changed something the host needs to redraw (such as switching
/// between insert and command mode).
/// C `vendor/tmux/prompt.c:655`: `static int prompt_translate_key(struct prompt *pr, key_code key, key_code *new_key, int *redraw)`
unsafe fn prompt_translate_key(
    pr: *mut prompt,
    key: key_code,
    new_key: *mut key_code,
    redraw: *mut i32,
) -> i32 {
    unsafe {
        if !(*pr).flags.intersects(prompt_flags::PROMPT_COMMANDMODE) {
            match key {
                code::A_CTRL
                | code::C_CTRL
                | code::E_CTRL
                | code::G_CTRL
                | code::H_CTRL
                | code::TAB
                | code::K_CTRL
                | code::N_CTRL
                | code::P_CTRL
                | code::T_CTRL
                | code::U_CTRL
                | code::V_CTRL
                | code::W_CTRL
                | code::Y_CTRL
                | code::LF
                | code::CR
                | code::LEFT_CTRL
                | code::RIGHT_CTRL
                | code::KEYC_BSPACE
                | code::KEYC_DC
                | code::KEYC_DOWN
                | code::KEYC_END
                | code::KEYC_HOME
                | code::KEYC_LEFT
                | code::KEYC_RIGHT
                | code::KEYC_UP => {
                    *new_key = key;
                    return 1;
                }
                code::ESC | code::LBRACKET_CTRL => {
                    (*pr).flags |= prompt_flags::PROMPT_COMMANDMODE;
                    if (*pr).index != 0 {
                        (*pr).index -= 1;
                    }
                    *redraw = 1;
                    return 0;
                }
                _ => (),
            }
            *new_key = key;
            return 2;
        }

        match key {
            code::KEYC_BSPACE => {
                *new_key = keyc::KEYC_LEFT as u64;
                return 1;
            }
            code::A_UPPER | code::I_UPPER | code::C_UPPER | code::S | code::A => {
                (*pr).flags &= !prompt_flags::PROMPT_COMMANDMODE;
                *redraw = 1;
                // switch mode and fall through
            }
            code::S_UPPER => {
                (*pr).flags &= !prompt_flags::PROMPT_COMMANDMODE;
                *redraw = 1;
                *new_key = b'u' as u64 | KEYC_CTRL;
                return 1;
            }
            code::I => {
                (*pr).flags &= !prompt_flags::PROMPT_COMMANDMODE;
                *redraw = 1;
                return 0;
            }
            code::ESC | code::LBRACKET_CTRL => {
                return 0;
            }
            _ => (),
        }

        match key {
            code::A_UPPER | code::DOLLAR => {
                *new_key = keyc::KEYC_END as u64;
                1
            }
            code::I_UPPER | code::ZERO | code::CARET => {
                *new_key = keyc::KEYC_HOME as u64;
                1
            }
            code::C_UPPER | code::D_UPPER => {
                *new_key = b'k' as u64 | KEYC_CTRL;
                1
            }
            code::KEYC_BSPACE | code::X_UPPER => {
                *new_key = keyc::KEYC_BSPACE as u64;
                1
            }
            code::B => {
                *new_key = b'b' as u64 | KEYC_META;
                1
            }
            code::B_UPPER => {
                *new_key = b'B' as u64 | KEYC_VI;
                1
            }
            code::D => {
                *new_key = b'u' as u64 | KEYC_CTRL;
                1
            }
            code::E => {
                *new_key = b'e' as u64 | KEYC_VI;
                1
            }
            code::E_UPPER => {
                *new_key = b'E' as u64 | KEYC_VI;
                1
            }
            code::W => {
                *new_key = b'w' as u64 | KEYC_VI;
                1
            }
            code::W_UPPER => {
                *new_key = b'W' as u64 | KEYC_VI;
                1
            }
            code::P => {
                *new_key = b'y' as u64 | KEYC_CTRL;
                1
            }
            code::Q => {
                *new_key = b'c' as u64 | KEYC_CTRL;
                1
            }
            code::S | code::KEYC_DC | code::X => {
                *new_key = keyc::KEYC_DC as u64;
                1
            }
            code::KEYC_DOWN | code::J => {
                *new_key = keyc::KEYC_DOWN as u64;
                1
            }
            code::KEYC_LEFT | code::H => {
                *new_key = keyc::KEYC_LEFT as u64;
                1
            }
            code::A | code::KEYC_RIGHT | code::L => {
                *new_key = keyc::KEYC_RIGHT as u64;
                1
            }
            code::KEYC_UP | code::K => {
                *new_key = keyc::KEYC_UP as u64;
                1
            }
            code::H_CTRL | code::C_CTRL | code::LF | code::CR => 1,
            _ => 0,
        }
    }
}

/// Paste into prompt.
/// C `vendor/tmux/prompt.c:804`: `static int prompt_paste(struct prompt *pr)`
unsafe fn prompt_paste(pr: *mut prompt) -> i32 {
    unsafe {
        let mut bufsize: usize = 0;
        let n: usize;
        let ud: *mut utf8_data;

        let size = utf8_strlen((*pr).buffer);
        if !(*pr).copied.is_null() {
            ud = (*pr).copied;
            n = utf8_strlen((*pr).copied);
        } else {
            let pb = paste_get_top(null_mut());
            if pb.is_null() {
                return 0;
            }
            let bufdata: *const u8 = paste_buffer_data(pb, &raw mut bufsize).cast();
            let mut udp = xreallocarray_::<utf8_data>(null_mut(), bufsize + 1).as_ptr();
            ud = udp;
            let mut i: u32 = 0;
            while i as usize != bufsize {
                let mut more = utf8_open(udp, *bufdata.add(i as usize));
                if more == utf8_state::UTF8_MORE {
                    while {
                        i += 1;
                        i as usize != bufsize && more == utf8_state::UTF8_MORE
                    } {
                        more = utf8_append(udp, *bufdata.add(i as usize));
                    }
                    if more == utf8_state::UTF8_DONE {
                        udp = udp.add(1);
                        continue;
                    }
                    i -= (*udp).have as u32;
                }
                if *bufdata.add(i as usize) <= 31 || *bufdata.add(i as usize) >= 127 {
                    break;
                }
                utf8_set(udp, *bufdata.add(i as usize));
                udp = udp.add(1);
                i += 1;
            }
            (*udp).size = 0;
            n = udp.offset_from_unsigned(ud);
        }
        if n != 0 {
            (*pr).buffer = xreallocarray_::<utf8_data>((*pr).buffer, size + n + 1).as_ptr();
            if (*pr).index == size {
                libc::memcpy(
                    (*pr).buffer.add((*pr).index).cast(),
                    ud.cast(),
                    n * size_of::<utf8_data>(),
                );
                (*pr).index += n;
                (*(*pr).buffer.add((*pr).index)).size = 0;
            } else {
                libc::memmove(
                    (*pr).buffer.add((*pr).index + n).cast(),
                    (*pr).buffer.add((*pr).index).cast(),
                    (size + 1 - (*pr).index) * size_of::<utf8_data>(),
                );
                libc::memcpy(
                    (*pr).buffer.add((*pr).index).cast(),
                    ud.cast(),
                    n * size_of::<utf8_data>(),
                );
                (*pr).index += n;
            }
        }
        if ud != (*pr).copied {
            free_(ud);
        }
        1
    }
}

/// Finish completion.
/// C `vendor/tmux/prompt.c:867`: `static int prompt_replace_complete(struct prompt *pr, const char *s)`
pub(crate) unsafe fn prompt_replace_complete(pr: *mut prompt, mut s: *const u8) -> i32 {
    unsafe {
        let mut word: [u8; 64] = [0; 64];
        let mut allocated: *mut u8 = null_mut();

        // Work out where the cursor currently is.
        let idx = (*pr).index.saturating_sub(1);
        let mut size = utf8_strlen((*pr).buffer);

        // Find the word we are in.
        let mut first = (*pr).buffer.add(idx);
        while first.addr() > (*pr).buffer.addr() && prompt_space(first) == 0 {
            first = first.sub(1);
        }
        while (*first).size != 0 && prompt_space(first) != 0 {
            first = first.add(1);
        }
        let mut last = (*pr).buffer.add(idx);
        while (*last).size != 0 && prompt_space(last) == 0 {
            last = last.add(1);
        }
        while last.addr() > (*pr).buffer.addr() && prompt_space(last) != 0 {
            last = last.sub(1);
        }
        if (*last).size != 0 {
            last = last.add(1);
        }
        if last < first {
            return 0;
        }
        if s.is_null() {
            let mut used = 0usize;
            let mut ud = first;
            while ud < last {
                if used + (*ud).size as usize >= word.len() {
                    break;
                }
                libc::memcpy(
                    (&raw mut word).cast::<u8>().add(used).cast(),
                    (&raw const (*ud).data).cast(),
                    (*ud).size as usize,
                );
                used += (*ud).size as usize;
                ud = ud.add(1);
            }
            if ud != last {
                return 0;
            }
            word[used] = b'\0';

            // Try to complete it.
            allocated = prompt_complete(
                pr,
                (&raw const word).cast(),
                first.offset_from_unsigned((*pr).buffer) as u32,
            );
            if allocated.is_null() {
                return 0;
            }
            s = allocated;
        }
        let slen = libc::strlen(s);

        // Trim out word.
        let n: usize = size - last.offset_from_unsigned((*pr).buffer) + 1; /* with \0 */
        libc::memmove(first.cast(), last.cast(), n * size_of::<utf8_data>());
        size -= last.offset_from_unsigned(first);

        // Insert the new word.
        size += slen;
        let off: usize = first.offset_from_unsigned((*pr).buffer);
        (*pr).buffer = xreallocarray_::<utf8_data>((*pr).buffer, size + 1).as_ptr();
        first = (*pr).buffer.add(off);
        libc::memmove(first.add(slen).cast(), first.cast(), n * size_of::<utf8_data>());
        for i in 0..slen {
            utf8_set(first.add(i), *s.add(i));
        }
        (*pr).index = first.offset_from_unsigned((*pr).buffer) + slen;

        free_(allocated);
        1
    }
}

/// Prompt forward to the next beginning of a word.
/// C `vendor/tmux/prompt.c:937`: `static void prompt_forward_word(struct prompt *pr, size_t size, int vi, const char *separators)`
unsafe fn prompt_forward_word(pr: *mut prompt, size: usize, vi: i32, separators: *const u8) {
    unsafe {
        let mut idx = (*pr).index;

        // In emacs mode, skip until the first non-whitespace character.
        if vi == 0 {
            while idx != size && prompt_space((*pr).buffer.add(idx)) != 0 {
                idx += 1;
            }
        }

        // Can't move forward if we're already at the end.
        if idx == size {
            (*pr).index = idx;
            return;
        }

        // Determine the current character class (separators or not).
        let word_is_separators = (prompt_in_list(separators, (*pr).buffer.add(idx)) != 0
            && prompt_space((*pr).buffer.add(idx)) == 0) as i32;

        // Skip ahead until the first space or opposite character class.
        loop {
            idx += 1;
            if prompt_space((*pr).buffer.add(idx)) != 0 {
                // In vi mode, go to the start of the next word.
                if vi != 0 {
                    while idx != size && prompt_space((*pr).buffer.add(idx)) != 0 {
                        idx += 1;
                    }
                }
                break;
            }
            if !(idx != size
                && word_is_separators == prompt_in_list(separators, (*pr).buffer.add(idx)))
            {
                break;
            }
        }

        (*pr).index = idx;
    }
}

/// Prompt forward to the next end of a word.
/// C `vendor/tmux/prompt.c:979`: `static void prompt_end_word(struct prompt *pr, size_t size, const char *separators)`
unsafe fn prompt_end_word(pr: *mut prompt, size: usize, separators: *const u8) {
    unsafe {
        let mut idx = (*pr).index;

        // Can't move forward if we're already at the end.
        if idx == size {
            return;
        }

        // Find the next word.
        loop {
            idx += 1;
            if idx == size {
                (*pr).index = idx;
                return;
            }
            if prompt_space((*pr).buffer.add(idx)) == 0 {
                break;
            }
        }

        // Determine the character class (separators or not).
        let word_is_separators = prompt_in_list(separators, (*pr).buffer.add(idx));

        // Skip ahead until the next space or opposite character class.
        loop {
            idx += 1;
            if idx == size {
                break;
            }
            if !(prompt_space((*pr).buffer.add(idx)) == 0
                && word_is_separators == prompt_in_list(separators, (*pr).buffer.add(idx)))
            {
                break;
            }
        }

        // Back up to the previous character to stop at the end of the word.
        (*pr).index = idx - 1;
    }
}

/// Prompt backward to the previous beginning of a word.
/// C `vendor/tmux/prompt.c:1015`: `static void prompt_backward_word(struct prompt *pr, const char *separators)`
unsafe fn prompt_backward_word(pr: *mut prompt, separators: *const u8) {
    unsafe {
        let mut idx = (*pr).index;

        // Find non-whitespace.
        while idx != 0 {
            idx -= 1;
            if prompt_space((*pr).buffer.add(idx)) == 0 {
                break;
            }
        }
        let word_is_separators = prompt_in_list(separators, (*pr).buffer.add(idx));

        // Find the character before the beginning of the word.
        while idx != 0 {
            idx -= 1;
            if prompt_space((*pr).buffer.add(idx)) != 0
                || word_is_separators != prompt_in_list(separators, (*pr).buffer.add(idx))
            {
                // Go back to the word.
                idx += 1;
                break;
            }
        }
        (*pr).index = idx;
    }
}

/// Fire input callback when done.
/// C `vendor/tmux/prompt.c:1045`: `static enum prompt_key_result prompt_done(struct prompt *pr, const char *s, int *redraw)`
unsafe fn prompt_done(pr: *mut prompt, s: *const u8, redraw: *mut i32) -> prompt_key_result {
    unsafe {
        if prompt_fire_callback(pr, s, prompt_key_result::PROMPT_KEY_CLOSE, redraw) != 0 {
            return prompt_key_result::PROMPT_KEY_CLOSE;
        }
        prompt_key_result::PROMPT_KEY_HANDLED
    }
}

/// Check for a movement key.
/// C `vendor/tmux/prompt.c:1054`: `static enum prompt_key_result prompt_check_move(struct prompt *pr, key_code key)`
unsafe fn prompt_check_move(pr: *mut prompt, key: key_code) -> prompt_key_result {
    unsafe {
        if !(*pr).flags.intersects(prompt_flags::PROMPT_INCREMENTAL) {
            return prompt_key_result::PROMPT_KEY_NOT_HANDLED;
        }
        match key {
            code::KEYC_UP | code::KEYC_DOWN | code::KEYC_PPAGE | code::KEYC_NPAGE => (),
            code::KEYC_LEFT | code::KEYC_RIGHT => {
                if (*pr).flags.intersects(prompt_flags::PROMPT_EDITARROWS) {
                    return prompt_key_result::PROMPT_KEY_NOT_HANDLED;
                }
            }
            _ => return prompt_key_result::PROMPT_KEY_NOT_HANDLED,
        }
        let s = utf8_tocstr((*pr).buffer);
        if prompt_fire_callback(pr, s, prompt_key_result::PROMPT_KEY_MOVE, null_mut()) != 0 {
            free_(s);
            return prompt_key_result::PROMPT_KEY_CLOSE;
        }
        free_(s);
        prompt_key_result::PROMPT_KEY_MOVE
    }
}

/// Handle keys in prompt.
/// C `vendor/tmux/prompt.c:1085`: `enum prompt_key_result prompt_key(struct prompt *pr, key_code key, int *redraw)`
#[expect(clippy::too_many_lines)]
pub unsafe fn prompt_key(
    pr: *mut prompt,
    mut key: key_code,
    redraw: *mut i32,
) -> prompt_key_result {
    unsafe {
        let mut s;
        let mut prefix = b'=';
        let histstr: *const u8;
        let mut idx: usize;
        let mut tmp: utf8_data = zeroed();
        let mut result = prompt_key_result::PROMPT_KEY_HANDLED;
        let word_is_separators: i32;

        (*pr).closed = 0;

        // Drop any inline completion matches; the Tab handler rebuilds them if
        // completion is still applicable.
        prompt_clear_complete(pr);

        if (*pr).flags.intersects(prompt_flags::PROMPT_KEY) {
            let ks = key_string_lookup_key(key, 0);
            if prompt_fire_callback(pr, ks, prompt_key_result::PROMPT_KEY_CLOSE, null_mut()) == 0 {
                (*pr).closed = 1;
            }
            return prompt_key_result::PROMPT_KEY_CLOSE;
        }
        let size: usize = utf8_strlen((*pr).buffer);

        key &= !KEYC_MASK_FLAGS;
        key = prompt_keypad_key(key);

        'changed: {
            'append_key: {
                'process_key: {
                    if (*pr).flags.intersects(prompt_flags::PROMPT_NUMERIC) {
                        if key >= b'0' as u64 && key <= b'9' as u64 {
                            break 'append_key;
                        }
                        s = utf8_tocstr((*pr).buffer);
                        if prompt_fire_callback(
                            pr,
                            s,
                            prompt_key_result::PROMPT_KEY_CLOSE,
                            null_mut(),
                        ) == 0
                        {
                            (*pr).closed = 1;
                        }
                        free_(s);
                        return prompt_key_result::PROMPT_KEY_NOT_HANDLED;
                    }

                    if (*pr)
                        .flags
                        .intersects(prompt_flags::PROMPT_SINGLE | prompt_flags::PROMPT_QUOTENEXT)
                    {
                        if key & KEYC_MASK_KEY == keyc::KEYC_BSPACE as u64 {
                            key = 0x7f;
                        } else if key & KEYC_MASK_KEY > 0x7f {
                            if !KEYC_IS_UNICODE(key) {
                                return prompt_key_result::PROMPT_KEY_HANDLED;
                            }
                            key &= KEYC_MASK_KEY;
                        } else {
                            key &= if key & KEYC_CTRL != 0 {
                                0x1f
                            } else {
                                KEYC_MASK_KEY
                            };
                        }
                        (*pr).flags &= !prompt_flags::PROMPT_QUOTENEXT;
                        break 'append_key;
                    }

                    if (*pr).keys == modekey::MODEKEY_VI as i32 {
                        match prompt_translate_key(pr, key, &raw mut key, redraw) {
                            1 => break 'process_key,
                            2 => break 'append_key,
                            _ => return prompt_key_result::PROMPT_KEY_HANDLED,
                        }
                    }
                } // process_key:

                result = prompt_check_move(pr, key);
                if result != prompt_key_result::PROMPT_KEY_NOT_HANDLED {
                    return result;
                }
                result = prompt_key_result::PROMPT_KEY_HANDLED;

                match key {
                    code::KEYC_LEFT | code::B_CTRL => {
                        if (*pr).index > 0 {
                            (*pr).index -= 1;
                        }
                    }
                    code::KEYC_RIGHT | code::F_CTRL => {
                        if (*pr).index < size {
                            (*pr).index += 1;
                        }
                    }
                    code::KEYC_HOME | code::A_CTRL => {
                        if (*pr).index != 0 {
                            (*pr).index = 0;
                        }
                    }
                    code::KEYC_END | code::E_CTRL => {
                        if (*pr).index != size {
                            (*pr).index = size;
                        }
                    }
                    code::TAB => {
                        if prompt_replace_complete(pr, null()) != 0 {
                            break 'changed;
                        }
                    }
                    code::KEYC_BSPACE | code::H_CTRL => {
                        if (*pr).flags.intersects(prompt_flags::PROMPT_BSPACE_EXIT) && size == 0 {
                            return prompt_done(pr, null(), redraw);
                        }
                        if (*pr).index != 0 {
                            if (*pr).index == size {
                                (*pr).index -= 1;
                                (*(*pr).buffer.add((*pr).index)).size = 0;
                            } else {
                                libc::memmove(
                                    (*pr).buffer.add((*pr).index - 1).cast(),
                                    (*pr).buffer.add((*pr).index).cast(),
                                    (size + 1 - (*pr).index) * size_of::<utf8_data>(),
                                );
                                (*pr).index -= 1;
                            }
                            break 'changed;
                        }
                    }
                    code::KEYC_DC | code::D_CTRL => {
                        if (*pr).index != size {
                            libc::memmove(
                                (*pr).buffer.add((*pr).index).cast(),
                                (*pr).buffer.add((*pr).index + 1).cast(),
                                (size + 1 - (*pr).index) * size_of::<utf8_data>(),
                            );
                            break 'changed;
                        }
                    }
                    code::U_CTRL => {
                        (*(*pr).buffer).size = 0;
                        (*pr).index = 0;
                        break 'changed;
                    }
                    code::K_CTRL => {
                        if (*pr).index < size {
                            (*(*pr).buffer.add((*pr).index)).size = 0;
                            break 'changed;
                        }
                    }
                    code::W_CTRL => {
                        // Find non-whitespace.
                        idx = (*pr).index;
                        while idx != 0 {
                            idx -= 1;
                            if prompt_space((*pr).buffer.add(idx)) == 0 {
                                break;
                            }
                        }
                        word_is_separators =
                            prompt_in_list((*pr).word_separators, (*pr).buffer.add(idx));

                        // Find the character before the beginning of the word.
                        while idx != 0 {
                            idx -= 1;
                            if prompt_space((*pr).buffer.add(idx)) != 0
                                || word_is_separators
                                    != prompt_in_list(
                                        (*pr).word_separators,
                                        (*pr).buffer.add(idx),
                                    )
                            {
                                // Go back to the word.
                                idx += 1;
                                break;
                            }
                        }

                        free_((*pr).copied);
                        (*pr).copied = xcalloc_::<utf8_data>(((*pr).index - idx) + 1).as_ptr();
                        libc::memcpy(
                            (*pr).copied.cast(),
                            (*pr).buffer.add(idx).cast(),
                            ((*pr).index - idx) * size_of::<utf8_data>(),
                        );

                        libc::memmove(
                            (*pr).buffer.add(idx).cast(),
                            (*pr).buffer.add((*pr).index).cast(),
                            (size + 1 - (*pr).index) * size_of::<utf8_data>(),
                        );
                        libc::memset(
                            (*pr).buffer.add(size - ((*pr).index - idx)).cast(),
                            b'\0' as i32,
                            ((*pr).index - idx) * size_of::<utf8_data>(),
                        );
                        (*pr).index = idx;

                        break 'changed;
                    }
                    code::RIGHT_CTRL | code::F_META => {
                        prompt_forward_word(pr, size, 0, (*pr).word_separators);
                        break 'changed;
                    }
                    code::E_UPPER_VI => {
                        prompt_end_word(pr, size, c!(""));
                        break 'changed;
                    }
                    code::E_VI => {
                        prompt_end_word(pr, size, (*pr).word_separators);
                        break 'changed;
                    }
                    code::W_UPPER_VI => {
                        prompt_forward_word(pr, size, 1, c!(""));
                        break 'changed;
                    }
                    code::W_VI => {
                        prompt_forward_word(pr, size, 1, (*pr).word_separators);
                        break 'changed;
                    }
                    code::B_VI => {
                        prompt_backward_word(pr, c!(""));
                        break 'changed;
                    }
                    code::LEFT_CTRL | code::B_META => {
                        prompt_backward_word(pr, (*pr).word_separators);
                        break 'changed;
                    }
                    code::KEYC_UP | code::P_CTRL => {
                        histstr = prompt_up_history(
                            (&raw mut (*pr).hindex).cast(),
                            (*pr).type_ as u32,
                        );
                        if !histstr.is_null() {
                            free_((*pr).buffer);
                            (*pr).buffer = utf8_fromcstr(histstr);
                            (*pr).index = utf8_strlen((*pr).buffer);
                            break 'changed;
                        }
                    }
                    code::KEYC_DOWN | code::N_CTRL => {
                        histstr = prompt_down_history(
                            (&raw mut (*pr).hindex).cast(),
                            (*pr).type_ as u32,
                        );
                        if !histstr.is_null() {
                            free_((*pr).buffer);
                            (*pr).buffer = utf8_fromcstr(histstr);
                            (*pr).index = utf8_strlen((*pr).buffer);
                            break 'changed;
                        }
                    }
                    code::Y_CTRL => {
                        if prompt_paste(pr) != 0 {
                            break 'changed;
                        }
                    }
                    code::T_CTRL => {
                        idx = (*pr).index;
                        if idx < size {
                            idx += 1;
                        }
                        if idx >= 2 {
                            utf8_copy(&raw mut tmp, (*pr).buffer.add(idx - 2));
                            utf8_copy((*pr).buffer.add(idx - 2), (*pr).buffer.add(idx - 1));
                            utf8_copy((*pr).buffer.add(idx - 1), &raw const tmp);
                            (*pr).index = idx;
                            break 'changed;
                        }
                    }
                    code::CR | code::LF => {
                        s = utf8_tocstr((*pr).buffer);
                        if *s != b'\0' {
                            prompt_add_history(s, (*pr).type_ as u32);
                        }
                        result = prompt_done(pr, s, redraw);
                        free_(s);
                        return result;
                    }
                    code::ESC | code::LBRACKET_CTRL | code::C_CTRL | code::G_CTRL => {
                        return prompt_done(pr, null(), redraw);
                    }
                    code::R_CTRL => {
                        if !(*pr).flags.intersects(prompt_flags::PROMPT_INCREMENTAL) {
                            // break out of the switch
                        } else {
                            if (*(*pr).buffer).size == 0 {
                                prefix = b'=';
                                free_((*pr).buffer);
                                (*pr).buffer = utf8_fromcstr((*pr).last);
                                (*pr).index = utf8_strlen((*pr).buffer);
                            } else {
                                prefix = b'-';
                            }
                            break 'changed;
                        }
                    }
                    code::S_CTRL => {
                        if !(*pr).flags.intersects(prompt_flags::PROMPT_INCREMENTAL) {
                            // break out of the switch
                        } else {
                            if (*(*pr).buffer).size == 0 {
                                prefix = b'=';
                                free_((*pr).buffer);
                                (*pr).buffer = utf8_fromcstr((*pr).last);
                                (*pr).index = utf8_strlen((*pr).buffer);
                            } else {
                                prefix = b'+';
                            }
                            break 'changed;
                        }
                    }
                    code::V_CTRL => {
                        (*pr).flags |= prompt_flags::PROMPT_QUOTENEXT;
                    }
                    _ => break 'append_key,
                }

                *redraw = 1;
                return prompt_key_result::PROMPT_KEY_HANDLED;
            } // append_key:

            if key <= 0x7f {
                utf8_set(&raw mut tmp, key as u8);
                if key <= 0x1f || key == 0x7f {
                    tmp.width = 2;
                }
            } else if KEYC_IS_UNICODE(key) {
                tmp = utf8_to_data(key as u32);
            } else {
                return prompt_key_result::PROMPT_KEY_HANDLED;
            }

            (*pr).buffer = xreallocarray_((*pr).buffer, size + 2).as_ptr();

            if (*pr).index == size {
                utf8_copy((*pr).buffer.add((*pr).index), &raw const tmp);
                (*pr).index += 1;
                (*(*pr).buffer.add((*pr).index)).size = 0;
            } else {
                libc::memmove(
                    (*pr).buffer.add((*pr).index + 1).cast(),
                    (*pr).buffer.add((*pr).index).cast(),
                    (size + 1 - (*pr).index) * size_of::<utf8_data>(),
                );
                utf8_copy((*pr).buffer.add((*pr).index), &raw const tmp);
                (*pr).index += 1;
            }

            if (*pr).flags.intersects(prompt_flags::PROMPT_SINGLE) {
                if utf8_strlen((*pr).buffer) != 1 {
                    (*pr).closed = 1;
                    result = prompt_key_result::PROMPT_KEY_CLOSE;
                } else {
                    s = utf8_tocstr((*pr).buffer);
                    result = prompt_done(pr, s, redraw);
                    free_(s);
                }
            }
        } // changed:

        *redraw = 1;
        if (*pr).flags.intersects(prompt_flags::PROMPT_INCREMENTAL) {
            s = utf8_tocstr((*pr).buffer);
            let cp = format_nul!("{}{}", prefix as char, _s(s));
            prompt_fire_callback(pr, cp, prompt_key_result::PROMPT_KEY_HANDLED, null_mut());
            free_(cp);
            free_(s);
        }
        result
    }
}

/// Add to completion list.
/// C `vendor/tmux/prompt.c:1413`: `static void prompt_complete_add(char ***list, u_int *size, const char *s)`
unsafe fn prompt_complete_add(list: *mut *mut *mut u8, size: *mut u32, s: *const u8) {
    unsafe {
        for i in 0..*size {
            if libc::strcmp(*(*list).add(i as usize), s) == 0 {
                return;
            }
        }
        *list = xreallocarray_::<*mut u8>(*list, (*size) as usize + 1).as_ptr();
        *(*list).add(*size as usize) = xstrdup(s).as_ptr();
        *size += 1;
    }
}

/// Build completion list.
/// C `vendor/tmux/prompt.c:1427`: `static char **prompt_complete_commands(u_int *size, const char *s)`
pub(crate) unsafe fn prompt_complete_commands(size: *mut u32, s: *const u8) -> *mut *mut u8 {
    unsafe {
        let mut list: *mut *mut u8 = null_mut();
        let slen = libc::strlen(s);
        let s_str = cstr_to_str(s);

        *size = 0;
        for cmdent in CMD_TABLE {
            if cmdent.name.starts_with(s_str) {
                let name = CString::new(cmdent.name).unwrap();
                prompt_complete_add(&raw mut list, size, name.as_ptr().cast());
            }
        }
        let o = options_get_only(GLOBAL_OPTIONS, "command-alias");
        if !o.is_null() {
            let mut a = options_array_first(o);
            while !a.is_null() {
                'next: {
                    let value = (*options_array_item_value(a)).string;
                    let cp = libc::strchr(value, b'=' as i32);
                    if cp.is_null() {
                        break 'next;
                    }
                    let valuelen = cp.offset_from_unsigned(value);
                    if slen > valuelen || libc::strncmp(value, s, slen) != 0 {
                        break 'next;
                    }

                    let tmp = format_nul!("{:.*}", valuelen, _s(value));
                    prompt_complete_add(&raw mut list, size, tmp);
                    free_(tmp);
                } // next:
                a = options_array_next(a);
            }
        }
        list
    }
}

/// Find longest prefix.
/// C `vendor/tmux/prompt.c:1465`: `static char *prompt_complete_prefix(char **list, u_int size)`
unsafe fn prompt_complete_prefix(list: *mut *mut u8, size: u32) -> *mut u8 {
    unsafe {
        if list.is_null() || size == 0 {
            return null_mut();
        }
        let out = xstrdup(*list).as_ptr();
        for i in 1..size {
            let item = *list.add(i as usize);
            let mut j = 0usize;
            while *out.add(j) != b'\0' && *item.add(j) != b'\0' {
                if *out.add(j) != *item.add(j) {
                    break;
                }
                j += 1;
            }
            *out.add(j) = b'\0';
        }
        out
    }
}

/// Sort the completion list by `strcmp`. C passes the two-pointer comparator to
/// `qsort`; Rust sorts the slice with the same ordering in one call.
/// C `vendor/tmux/prompt.c:1486`: `static int prompt_complete_sort(const void *a, const void *b)`
unsafe fn prompt_complete_sort(list: *mut *mut u8, size: u32) {
    unsafe {
        let slice = std::slice::from_raw_parts_mut(list, size as usize);
        slice.sort_unstable_by(|a, b| libc::strcmp(*a, *b).cmp(&0));
    }
}

/// Free the stored inline completion matches.
/// C `vendor/tmux/prompt.c:1495`: `static void prompt_clear_complete(struct prompt *pr)`
unsafe fn prompt_clear_complete(pr: *mut prompt) {
    unsafe {
        for i in 0..(*pr).complete_size {
            free_(*(*pr).complete_list.add(i as usize));
        }
        free_((*pr).complete_list);
        (*pr).complete_list = null_mut();
        (*pr).complete_size = 0;

        free_((*pr).complete_display);
        (*pr).complete_display = null_mut();
    }
}

/// Store the match list for inline display and build the dim suffix string: a
/// leading space then the matches separated by spaces.
/// C `vendor/tmux/prompt.c:1514`: `static void prompt_store_complete(struct prompt *pr, char **list, u_int size)`
unsafe fn prompt_store_complete(pr: *mut prompt, list: *mut *mut u8, size: u32) {
    unsafe {
        prompt_clear_complete(pr);
        (*pr).complete_list = list;
        (*pr).complete_size = size;

        let mut display = xstrdup_(c"").as_ptr();
        for i in 0..size {
            let cp = format_nul!("{} {}", _s(display), _s(*list.add(i as usize)));
            free_(display);
            display = cp;
        }
        (*pr).complete_display = display;
    }
}

/// Complete word. Returns the text to insert when a unique match or a longer
/// common prefix is available; otherwise stores the match list for inline
/// display (and returns NULL) or returns NULL if there is nothing to do.
/// C `vendor/tmux/prompt.c:1538`: `static char *prompt_complete(struct prompt *pr, const char *word, u_int offset)`
unsafe fn prompt_complete(pr: *mut prompt, word: *const u8, offset: u32) -> *mut u8 {
    unsafe {
        let mut size = 0u32;

        if (*pr).type_ != prompt_type::PROMPT_TYPE_COMMAND || offset != 0 || *word == b'\0' {
            return null_mut();
        }

        let list = prompt_complete_commands(&raw mut size, word);
        if size == 0 {
            free_(list);
            return null_mut();
        }
        prompt_complete_sort(list, size);
        for i in 0..size {
            log_debug!("complete {i}: {}", _s(*list.add(i as usize)));
        }

        let mut out = if size == 1 {
            format_nul!("{} ", _s(*list))
        } else {
            prompt_complete_prefix(list, size)
        };
        if !out.is_null() && libc::strcmp(word, out) == 0 {
            free_(out);
            out = null_mut();
        }

        if !out.is_null() || size <= 1 {
            // Inserting (or nothing to show): drop the list.
            for i in 0..size {
                free_(*list.add(i as usize));
            }
            free_(list);
            return out;
        }

        // Multiple matches but nothing to insert: keep them for redraw.
        prompt_store_complete(pr, list, size);
        null_mut()
    }
}

/// Return the type of the prompt as an enum.
/// C `vendor/tmux/prompt.c:1581`: `enum prompt_type prompt_type(const char *type)`
pub unsafe fn prompt_type(type_: *const u8) -> prompt_type {
    unsafe {
        for i in 0..PROMPT_NTYPES {
            if libc::streq_(type_, prompt_type_string(i)) {
                return prompt_type::try_from(i).unwrap();
            }
        }
        prompt_type::PROMPT_TYPE_INVALID
    }
}

/// Get prompt type as a string.
/// C `vendor/tmux/prompt.c:1594`: `const char *prompt_type_string(enum prompt_type type)`
pub fn prompt_type_string(type_: u32) -> &'static str {
    match type_ {
        0 => "command",
        1 => "search",
        _ => "invalid",
    }
}

mod code {
    use super::*;

    pub const A: u64 = 'a' as u64;
    pub const B: u64 = 'b' as u64;
    pub const D: u64 = 'd' as u64;
    pub const E: u64 = 'e' as u64;
    pub const H: u64 = 'h' as u64;
    pub const I: u64 = 'i' as u64;
    pub const J: u64 = 'j' as u64;
    pub const K: u64 = 'k' as u64;
    pub const L: u64 = 'l' as u64;
    pub const P: u64 = 'p' as u64;
    pub const Q: u64 = 'q' as u64;
    pub const S: u64 = 's' as u64;
    pub const W: u64 = 'w' as u64;
    pub const X: u64 = 'x' as u64;

    pub const DOLLAR: u64 = '$' as u64;
    pub const ZERO: u64 = '0' as u64;
    pub const CARET: u64 = '^' as u64;

    pub const A_UPPER: u64 = 'A' as u64;
    pub const B_UPPER: u64 = 'B' as u64;
    pub const C_UPPER: u64 = 'C' as u64;
    pub const D_UPPER: u64 = 'D' as u64;
    pub const E_UPPER: u64 = 'E' as u64;
    pub const I_UPPER: u64 = 'I' as u64;
    pub const S_UPPER: u64 = 'S' as u64;
    pub const W_UPPER: u64 = 'W' as u64;
    pub const X_UPPER: u64 = 'X' as u64;

    pub const TAB: u64 = b'\x09' as u64;
    pub const KEYC_HOME: u64 = keyc::KEYC_HOME as u64;
    pub const KEYC_END: u64 = keyc::KEYC_END as u64;
    pub const KEYC_UP: u64 = keyc::KEYC_UP as u64;
    pub const KEYC_DOWN: u64 = keyc::KEYC_DOWN as u64;
    pub const KEYC_LEFT: u64 = keyc::KEYC_LEFT as u64;
    pub const KEYC_RIGHT: u64 = keyc::KEYC_RIGHT as u64;
    pub const KEYC_BSPACE: u64 = keyc::KEYC_BSPACE as u64;
    pub const KEYC_DC: u64 = keyc::KEYC_DC as u64;
    pub const KEYC_PPAGE: u64 = keyc::KEYC_PPAGE as u64;
    pub const KEYC_NPAGE: u64 = keyc::KEYC_NPAGE as u64;

    pub const KEYC_KP_SLASH: u64 = keyc::KEYC_KP_SLASH as u64;
    pub const KEYC_KP_STAR: u64 = keyc::KEYC_KP_STAR as u64;
    pub const KEYC_KP_MINUS: u64 = keyc::KEYC_KP_MINUS as u64;
    pub const KEYC_KP_SEVEN: u64 = keyc::KEYC_KP_SEVEN as u64;
    pub const KEYC_KP_EIGHT: u64 = keyc::KEYC_KP_EIGHT as u64;
    pub const KEYC_KP_NINE: u64 = keyc::KEYC_KP_NINE as u64;
    pub const KEYC_KP_PLUS: u64 = keyc::KEYC_KP_PLUS as u64;
    pub const KEYC_KP_FOUR: u64 = keyc::KEYC_KP_FOUR as u64;
    pub const KEYC_KP_FIVE: u64 = keyc::KEYC_KP_FIVE as u64;
    pub const KEYC_KP_SIX: u64 = keyc::KEYC_KP_SIX as u64;
    pub const KEYC_KP_ONE: u64 = keyc::KEYC_KP_ONE as u64;
    pub const KEYC_KP_TWO: u64 = keyc::KEYC_KP_TWO as u64;
    pub const KEYC_KP_THREE: u64 = keyc::KEYC_KP_THREE as u64;
    pub const KEYC_KP_ENTER: u64 = keyc::KEYC_KP_ENTER as u64;
    pub const KEYC_KP_ZERO: u64 = keyc::KEYC_KP_ZERO as u64;
    pub const KEYC_KP_PERIOD: u64 = keyc::KEYC_KP_PERIOD as u64;

    pub const A_CTRL: u64 = 'a' as u64 | KEYC_CTRL;
    pub const B_CTRL: u64 = 'b' as u64 | KEYC_CTRL;
    pub const C_CTRL: u64 = 'c' as u64 | KEYC_CTRL;
    pub const D_CTRL: u64 = 'd' as u64 | KEYC_CTRL;
    pub const E_CTRL: u64 = 'e' as u64 | KEYC_CTRL;
    pub const F_CTRL: u64 = 'f' as u64 | KEYC_CTRL;
    pub const G_CTRL: u64 = 'g' as u64 | KEYC_CTRL;
    pub const H_CTRL: u64 = 'h' as u64 | KEYC_CTRL;
    pub const K_CTRL: u64 = 'k' as u64 | KEYC_CTRL;
    pub const N_CTRL: u64 = 'n' as u64 | KEYC_CTRL;
    pub const P_CTRL: u64 = 'p' as u64 | KEYC_CTRL;
    pub const R_CTRL: u64 = 'r' as u64 | KEYC_CTRL;
    pub const S_CTRL: u64 = 's' as u64 | KEYC_CTRL;
    pub const T_CTRL: u64 = 't' as u64 | KEYC_CTRL;
    pub const U_CTRL: u64 = 'u' as u64 | KEYC_CTRL;
    pub const V_CTRL: u64 = 'v' as u64 | KEYC_CTRL;
    pub const W_CTRL: u64 = 'w' as u64 | KEYC_CTRL;
    pub const Y_CTRL: u64 = 'y' as u64 | KEYC_CTRL;
    pub const LBRACKET_CTRL: u64 = '[' as u64 | KEYC_CTRL;

    pub const LEFT_CTRL: u64 = keyc::KEYC_LEFT as u64 | KEYC_CTRL;
    pub const RIGHT_CTRL: u64 = keyc::KEYC_RIGHT as u64 | KEYC_CTRL;

    pub const B_META: u64 = 'b' as u64 | KEYC_META;
    pub const F_META: u64 = 'f' as u64 | KEYC_META;

    pub const E_UPPER_VI: u64 = 'E' as u64 | KEYC_VI;
    pub const E_VI: u64 = 'e' as u64 | KEYC_VI;
    pub const W_UPPER_VI: u64 = 'W' as u64 | KEYC_VI;
    pub const W_VI: u64 = 'w' as u64 | KEYC_VI;
    pub const B_VI: u64 = 'b' as u64 | KEYC_VI;

    pub const CR: u64 = '\r' as u64;
    pub const LF: u64 = '\n' as u64;
    pub const ESC: u64 = '\x1b' as u64;
}
