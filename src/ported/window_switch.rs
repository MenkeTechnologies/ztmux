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

//! Port of `vendor/tmux/window-switch.c`: the `switch-mode` picker.
//!
//! A flat, fuzzy-filtered list of sessions or windows with an always-open
//! incremental prompt on the bottom row. The prompt is a `struct prompt` the
//! mode owns outright — every keystroke goes to `prompt_key` first, and only
//! the keys the prompt does not consume move the selection.

use crate::*;
use crate::fuzzy::fuzzy_match;

const WINDOW_SWITCH_DEFAULT_COMMAND: &str = "switch-client -Zt '%%'";

const WINDOW_SWITCH_DEFAULT_FORMAT: &str = concat!(
    "#{?window_format,",
    "#{window_name} ",
    "#[dim]#{session_name}:#{window_index}#{window_flags}#[default] ",
    "#[dim]#{pane_current_command}#[default] ",
    "#[dim]#{?#{!=:#{pane_title},#{host_short}},#{pane_title},}#[default]",
    ",",
    "#{session_name} ",
    "#[dim]#{session_windows} windows#[default] ",
    "#{?session_attached,attached,#[dim]detached#[default]} ",
    "#[dim]#{window_name}#[default]",
    "}"
);

/// C `vendor/tmux/window-switch.c:53`: `const struct window_mode window_switch_mode`
pub static WINDOW_SWITCH_MODE: window_mode = window_mode {
    name: "switch-mode",
    default_format: Some(WINDOW_SWITCH_DEFAULT_FORMAT),

    init: window_switch_init,
    free: window_switch_free,
    resize: window_switch_resize,
    key: Some(window_switch_key),
    update: None,
    key_table: None,
    command: None,
    formats: None,
    get_screen: None,
};

/// C `vendor/tmux/window-switch.c:63`: `enum window_switch_type`
#[repr(u32)]
#[derive(Copy, Clone, Default, Eq, PartialEq)]
enum window_switch_type {
    #[default]
    WINDOW_SWITCH_TYPE_SESSION,
    WINDOW_SWITCH_TYPE_WINDOW,
}

/// C `vendor/tmux/window-switch.c:68`: `struct window_switch_itemdata`
struct window_switch_itemdata {
    type_: window_switch_type,
    session: i32,
    winlink: i32,

    tag: u64,
    text: *mut u8,
    matched: Option<BitStr>,

    score: u32,
    order: u32,
}

/// C `vendor/tmux/window-switch.c:81`: `struct window_switch_modedata`
struct window_switch_modedata {
    /// C `vendor/tmux/window-switch.c:82`: set by `window_switch_init` and
    /// never read — every use goes through `wme->wp` instead. Kept so the
    /// struct matches the C.
    #[expect(dead_code)]
    wp: *mut window_pane,
    screen: screen,
    zoomed: i32,

    format: *mut u8,
    command: *mut u8,

    type_: window_switch_type,
    filter: *mut u8,
    prompt: *mut prompt,
    prompt_cx: u32,

    /// C keeps `item_list`/`item_size` and a separate `matches` array of
    /// borrowed pointers into it. Here the items are owned by `item_list` and
    /// `matches` holds their indices, which is the same two-level structure
    /// without a second set of raw pointers to keep valid.
    item_list: Vec<window_switch_itemdata>,
    matches: Vec<u32>,

    current: u32,
    offset: u32,
}

/// C `vendor/tmux/window-switch.c:113`: `static struct window_switch_itemdata *window_switch_add_item(struct window_switch_modedata *data)`
fn window_switch_add_item(data: &mut window_switch_modedata) -> &mut window_switch_itemdata {
    data.item_list.push(window_switch_itemdata {
        type_: window_switch_type::WINDOW_SWITCH_TYPE_SESSION,
        session: 0,
        winlink: -1,
        tag: 0,
        text: null_mut(),
        matched: None,
        score: 0,
        order: 0,
    });
    data.item_list.last_mut().unwrap()
}

/// C `vendor/tmux/window-switch.c:124`: `static void window_switch_add_session(struct window_switch_modedata *data, struct session *s, u_int *order)`
unsafe fn window_switch_add_session(
    data: &mut window_switch_modedata,
    s: *mut session,
    order: &mut u32,
) {
    unsafe {
        let ft = format_create(null_mut(), null_mut(), FORMAT_NONE, format_flags::empty());
        format_defaults(ft, null_mut(), NonNull::new(s), None, None);

        let format = data.format;
        let item = window_switch_add_item(data);
        item.type_ = window_switch_type::WINDOW_SWITCH_TYPE_SESSION;
        item.session = (*s).id as i32;
        item.winlink = -1;
        item.tag = s as u64;
        item.order = *order;
        *order += 1;
        item.text = format_expand(ft, format);

        format_free(ft);
    }
}

/// C `vendor/tmux/window-switch.c:145`: `static void window_switch_add_window(struct window_switch_modedata *data, struct winlink *wl, u_int *order)`
unsafe fn window_switch_add_window(
    data: &mut window_switch_modedata,
    wl: *mut winlink,
    order: &mut u32,
) {
    unsafe {
        let ft = format_create(null_mut(), null_mut(), FORMAT_NONE, format_flags::empty());
        format_defaults(
            ft,
            null_mut(),
            NonNull::new((*wl).session),
            NonNull::new(wl),
            None,
        );

        let format = data.format;
        let item = window_switch_add_item(data);
        item.type_ = window_switch_type::WINDOW_SWITCH_TYPE_WINDOW;
        item.session = (*(*wl).session).id as i32;
        item.winlink = (*wl).idx;
        item.tag = wl as u64;
        item.order = *order;
        *order += 1;
        item.text = format_expand(ft, format);

        format_free(ft);
    }
}

/// Order matches by score (descending), then by the order they were built in.
/// C `vendor/tmux/window-switch.c:166`: `static int window_switch_compare(const void *a0, const void *b0)`
fn window_switch_compare(
    a: &window_switch_itemdata,
    b: &window_switch_itemdata,
) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    // C returns -1/1 from the two `>`/`<` pairs; the same three-way decision,
    // with the score reversed so the highest score sorts first.
    match a.score.cmp(&b.score) {
        Ordering::Greater => Ordering::Less,
        Ordering::Less => Ordering::Greater,
        Ordering::Equal => a.order.cmp(&b.order),
    }
}

/// C `vendor/tmux/window-switch.c:183`: `static void window_switch_build(struct window_switch_modedata *data)`
unsafe fn window_switch_build(data: &mut window_switch_modedata) {
    unsafe {
        let sx = screen_size_x(&raw mut data.screen);
        let mut order = 0u32;

        let sort_crit = sort_criteria {
            order: sort_order::SORT_NAME,
            reversed: false,
            order_seq: null_mut(),
        };

        for item in data.item_list.drain(..) {
            free_(item.text);
        }

        match data.type_ {
            window_switch_type::WINDOW_SWITCH_TYPE_SESSION => {
                for s in sort_get_sessions(sort_crit) {
                    window_switch_add_session(data, s, &mut order);
                }
            }
            window_switch_type::WINDOW_SWITCH_TYPE_WINDOW => {
                for wl in sort_get_winlinks(sort_crit) {
                    window_switch_add_window(data, wl, &mut order);
                }
            }
        }

        let f = cstr_to_str(data.filter);
        let mut m: Vec<u32> = Vec::new();
        for i in 0..data.item_list.len() {
            let item = &mut data.item_list[i];
            if f.is_empty() {
                item.matched = None;
                m.push(i as u32);
                continue;
            }

            let text = std::slice::from_raw_parts(item.text, libc::strlen(item.text));
            match fuzzy_match(f.as_bytes(), text, sx) {
                None => {
                    item.matched = None;
                    continue;
                }
                Some((bits, score)) => {
                    item.matched = Some(bits);
                    item.score = score;
                    m.push(i as u32);
                }
            }
        }
        m.sort_by(|&a, &b| {
            window_switch_compare(&data.item_list[a as usize], &data.item_list[b as usize])
        });

        data.matches = m;
    }
}

/// C `vendor/tmux/window-switch.c:237`: `static u_int window_switch_visible(struct window_switch_modedata *data)`
unsafe fn window_switch_visible(data: &mut window_switch_modedata) -> u32 {
    unsafe {
        let sy = screen_size_y(&raw mut data.screen);

        if sy <= 1 {
            return 0;
        }
        sy - 1
    }
}

/// C `vendor/tmux/window-switch.c:247`: `static void window_switch_set_current(struct window_switch_modedata *data, u_int current)`
unsafe fn window_switch_set_current(data: &mut window_switch_modedata, mut current: u32) {
    unsafe {
        let visible = window_switch_visible(data);
        let size = data.matches.len() as u32;

        if size == 0 {
            data.current = 0;
            data.offset = 0;
            return;
        }

        if current > size - 1 {
            current = size - 1;
        }
        data.current = current;

        if data.current < data.offset {
            data.offset = data.current;
        } else if visible != 0 && data.current >= data.offset + visible {
            data.offset = data.current - visible + 1;
        }
    }
}

/// C `vendor/tmux/window-switch.c:268`: `static void window_switch_draw_screen(struct window_mode_entry *wme)`
unsafe fn window_switch_draw_screen(wme: NonNull<window_mode_entry>) {
    unsafe {
        let wme = wme.as_ptr();
        let wp = (*wme).wp;
        let data = &mut *((*wme).data.cast::<window_switch_modedata>());
        let oo = (*wp).options;
        let mut ctx: screen_write_ctx = zeroed();
        let s = &raw mut data.screen;
        let sx = screen_size_x(s);
        let sy = screen_size_y(s);
        let mut mgc: grid_cell = zeroed();
        let mut sgc: grid_cell = zeroed();
        let mut gc: grid_cell = zeroed();
        let dgc = &raw const GRID_DEFAULT_CELL;

        screen_write_start(&raw mut ctx, s);
        screen_write_clearscreen(&raw mut ctx, 8);

        if sy <= 1 {
            screen_write_stop(&raw mut ctx);
            return;
        }

        style_apply(&raw mut mgc, oo, c!("switch-mode-match-style"), null_mut());
        style_apply(&raw mut sgc, oo, c!("mode-style"), null_mut());

        let visible = window_switch_visible(data);
        for i in 0..visible {
            let idx = data.offset + i;
            if idx >= data.matches.len() as u32 {
                break;
            }
            let item = &data.item_list[data.matches[idx as usize] as usize];

            screen_write_cursormove(&raw mut ctx, 0, i as i32, 0);
            if idx == data.current {
                screen_write_clearendofline(&raw mut ctx, sgc.bg as u32);
                format_draw(
                    &raw mut ctx,
                    &raw const sgc,
                    sx,
                    cstr_to_str(item.text),
                    null_mut(),
                    0,
                );
            } else {
                format_draw(&raw mut ctx, dgc, sx, cstr_to_str(item.text), null_mut(), 0);
            }

            let Some(matched) = item.matched.as_ref() else {
                continue;
            };
            for j in 0..sx {
                if !matched.bit_test(j) {
                    continue;
                }
                grid_get_cell((*s).grid, j, i, &raw mut gc);
                gc.attr = mgc.attr;
                gc.fg = mgc.fg;
                gc.bg = mgc.bg;
                screen_write_cursormove(&raw mut ctx, j as i32, i as i32, 0);
                screen_write_cell(&raw mut ctx, &raw const gc);
            }
        }

        if !data.prompt.is_null() {
            let mut pdd: prompt_draw_data = zeroed();
            pdd.ctx = &raw mut ctx;
            pdd.cursor_x = &raw mut data.prompt_cx;
            pdd.area_x = 0;
            pdd.area_width = sx;
            pdd.prompt_line = sy - 1;
            (*s).mode |= mode_flag::MODE_CURSOR;
            prompt_draw(data.prompt, &raw mut pdd);
            screen_write_cursormove(&raw mut ctx, data.prompt_cx as i32, sy as i32 - 1, 0);
        }
        screen_write_stop(&raw mut ctx);
    }
}

/// C `vendor/tmux/window-switch.c:335`: `static struct screen *window_switch_init(struct window_mode_entry *wme, struct cmd_find_state *fs, struct args *args)`
unsafe fn window_switch_init(
    wme: NonNull<window_mode_entry>,
    fs: *mut cmd_find_state,
    args: *mut args,
) -> *mut screen {
    unsafe {
        let wp = (*wme.as_ptr()).wp;

        let mut data = Box::new(window_switch_modedata {
            wp,
            screen: zeroed(),
            zoomed: 0,
            format: null_mut(),
            command: null_mut(),
            type_: window_switch_type::WINDOW_SWITCH_TYPE_SESSION,
            filter: null_mut(),
            prompt: null_mut(),
            prompt_cx: 0,
            item_list: Vec::new(),
            matches: Vec::new(),
            current: 0,
            offset: 0,
        });

        if args_has(args, 'w') {
            data.type_ = window_switch_type::WINDOW_SWITCH_TYPE_WINDOW;
        } else {
            data.type_ = window_switch_type::WINDOW_SWITCH_TYPE_SESSION;
        }

        data.filter = xstrdup_(c"").as_ptr();
        if args.is_null() || !args_has(args, 'F') {
            data.format = xstrdup__(WINDOW_SWITCH_DEFAULT_FORMAT);
        } else {
            data.format = xstrdup(args_get(args, b'F')).as_ptr();
        }
        if args.is_null() || args_count(args) == 0 {
            data.command = xstrdup__(WINDOW_SWITCH_DEFAULT_COMMAND);
        } else {
            data.command = xstrdup(args_string(args, 0)).as_ptr();
        }

        let mut pd: prompt_create_data = zeroed();
        prompt_set_options(&raw mut pd, (*fs).s);
        pd.fs = fs;
        pd.prompt = c!("(search) ");
        pd.input = c!("");
        pd.type_ = prompt_type::PROMPT_TYPE_SEARCH;
        pd.flags = prompt_flags::PROMPT_INCREMENTAL
            | prompt_flags::PROMPT_NOFORMAT
            | prompt_flags::PROMPT_ISMODE
            | prompt_flags::PROMPT_EDITARROWS;
        pd.inputcb = Some(window_switch_prompt_callback);
        pd.data = (&raw mut *data).cast();
        data.prompt = prompt_create(&raw const pd);
        prompt_update(data.prompt, c!("(search) "), data.filter);

        if !args_has(args, 'Z') {
            data.zoomed = -1;
        } else {
            data.zoomed =
                i32::from((*(*wp).window).flags.intersects(window_flag::ZOOMED));
            if data.zoomed == 0 && window_zoom(wp) == 0 {
                server_redraw_window((*wp).window);
            }
        }

        screen_init(
            &raw mut data.screen,
            screen_size_x(&raw mut (*wp).base),
            screen_size_y(&raw mut (*wp).base),
            0,
        );

        window_switch_build(&mut data);
        prompt_incremental_start(data.prompt);

        let data = Box::into_raw(data);
        (*wme.as_ptr()).data = data.cast();
        window_switch_draw_screen(wme);

        &raw mut (*data).screen
    }
}

/// C `vendor/tmux/window-switch.c:393`: `static void window_switch_free(struct window_mode_entry *wme)`
unsafe fn window_switch_free(wme: NonNull<window_mode_entry>) {
    unsafe {
        let wme = wme.as_ptr();
        if (*wme).data.is_null() {
            return;
        }
        let mut data = Box::from_raw((*wme).data.cast::<window_switch_modedata>());
        (*wme).data = null_mut();

        if data.zoomed == 0 {
            server_unzoom_window((*(*wme).wp).window);
        }

        for item in data.item_list.drain(..) {
            free_(item.text);
        }

        free_(data.filter);
        prompt_free(data.prompt);
        free_(data.format);
        free_(data.command);
        screen_free(&raw mut data.screen);
    }
}

/// C `vendor/tmux/window-switch.c:416`: `static void window_switch_resize(struct window_mode_entry *wme, u_int sx, u_int sy)`
unsafe fn window_switch_resize(wme: NonNull<window_mode_entry>, sx: u32, sy: u32) {
    unsafe {
        let data = &mut *((*wme.as_ptr()).data.cast::<window_switch_modedata>());

        screen_resize(&raw mut data.screen, sx, sy, 0);
        window_switch_build(data);
        let current = data.current;
        window_switch_set_current(data, current);
        window_switch_draw_screen(wme);
    }
}

/// C `vendor/tmux/window-switch.c:428`: `static int window_switch_run_command(struct window_switch_modedata *data, struct client *c)`
unsafe fn window_switch_run_command(data: &mut window_switch_modedata, c: *mut client) -> i32 {
    unsafe {
        let mut fs: cmd_find_state = zeroed();
        let mut target: *mut u8 = null_mut();
        let mut error: *mut u8 = null_mut();

        if data.matches.is_empty() {
            return 0;
        }
        let item = &data.item_list[data.matches[data.current as usize] as usize];

        cmd_find_clear_state(&raw mut fs, cmd_find_flags::empty());
        match item.type_ {
            window_switch_type::WINDOW_SWITCH_TYPE_SESSION => {
                if let Some(s) = session_find_by_id(item.session as u32) {
                    target = format_nul!("={}:", (*s.as_ptr()).name);
                    cmd_find_from_session(&raw mut fs, s.as_ptr(), cmd_find_flags::empty());
                }
            }
            window_switch_type::WINDOW_SWITCH_TYPE_WINDOW => {
                if let Some(s) = session_find_by_id(item.session as u32) {
                    let wl = winlink_find_by_index(&raw mut (*s.as_ptr()).windows, item.winlink);
                    if !wl.is_null() {
                        target = format_nul!("={}:{}.", (*s.as_ptr()).name, (*wl).idx);
                        cmd_find_from_winlink(&raw mut fs, wl, cmd_find_flags::empty());
                    }
                }
            }
        }
        if target.is_null() {
            return 0;
        }

        let command = cmd_template_replace(data.command, Some(cstr_to_str(target)), 1);
        if !command.is_null() && *command != b'\0' {
            let state = cmdq_new_state(&raw const fs, null(), cmdq_state_flags::empty());
            let status = cmd_parse_and_append(cstr_to_str(command), None, c, state, &raw mut error);
            if status == cmd_parse_status::CMD_PARSE_ERROR {
                if !c.is_null() {
                    *error = (*error).to_ascii_uppercase();
                    status_message_set!(c, -1, 1, false, 0, "{}", _s(error));
                }
                free_(error);
            }
            cmdq_free_state(state);
        }
        free_(command);
        free_(target);
        1
    }
}

/// C `vendor/tmux/window-switch.c:485`: `static enum prompt_result window_switch_prompt_callback(void *arg, const char *s, enum prompt_key_result key)`
unsafe fn window_switch_prompt_callback(
    arg: NonNull<c_void>,
    mut s: *const u8,
    key: prompt_key_result,
) -> prompt_result {
    unsafe {
        let data = &mut *(arg.as_ptr().cast::<window_switch_modedata>());

        if key != prompt_key_result::PROMPT_KEY_HANDLED {
            return prompt_result::PROMPT_CONTINUE;
        }

        if s.is_null() {
            s = c!("");
        } else if *s != b'\0' {
            // The incremental prompt prefixes the buffer with `=`, `+` or `-`.
            s = s.add(1);
        }

        free_(data.filter);
        data.filter = xstrdup(s).as_ptr();
        window_switch_build(data);
        data.current = 0;
        data.offset = 0;

        prompt_result::PROMPT_CONTINUE
    }
}

/// C `vendor/tmux/window-switch.c:508`: `static void window_switch_key(struct window_mode_entry *wme, struct client *c, __unused struct session *s, __unused struct winlink *wl, key_code key, struct mouse_event *m)`
unsafe fn window_switch_key(
    wme: NonNull<window_mode_entry>,
    c: *mut client,
    _s: *mut session,
    _wl: *mut winlink,
    mut key: key_code,
    m: *mut mouse_event,
) {
    unsafe {
        let wp = (*wme.as_ptr()).wp;
        let data = &mut *((*wme.as_ptr()).data.cast::<window_switch_modedata>());
        let mut current = data.current;
        let mut x: u32 = 0;
        let mut y: u32 = 0;
        let mut size = data.matches.len() as u32;
        let mut redraw: i32 = 0;

        'moved: {
            if KEYC_IS_MOUSE(key) {
                if m.is_null() || cmd_mouse_at(wp, m, &raw mut x, &raw mut y, 0) != 0 {
                    return;
                }
                if !data.prompt.is_null()
                    && screen_size_y(&raw mut data.screen) != 0
                    && y == screen_size_y(&raw mut data.screen) - 1
                    && MOUSE_BUTTONS((*m).b) == MOUSE_BUTTON_1
                    && !MOUSE_DRAG((*m).b)
                    && !MOUSE_RELEASE((*m).b)
                {
                    let result = prompt_mouse(
                        data.prompt,
                        x,
                        0,
                        screen_size_x(&raw mut data.screen),
                        &raw mut redraw,
                    );
                    if redraw != 0 || result == prompt_key_result::PROMPT_KEY_HANDLED {
                        window_switch_draw_screen(wme);
                        (*wp).flags |= window_pane_flags::PANE_REDRAW;
                    }
                    return;
                }
                match key {
                    code::KEYC_WHEELUP_PANE => {
                        if size != 0 && current != 0 {
                            window_switch_set_current(data, current - 1);
                        }
                        break 'moved;
                    }
                    code::KEYC_WHEELDOWN_PANE => {
                        if size != 0 && current != size - 1 {
                            window_switch_set_current(data, current + 1);
                        }
                        break 'moved;
                    }
                    code::KEYC_MOUSEDOWN1_PANE | code::KEYC_DOUBLECLICK1_PANE => {
                        if y >= window_switch_visible(data) || data.offset + y >= size {
                            return;
                        }
                        let target = data.offset + y;
                        window_switch_set_current(data, target);
                        if key == code::KEYC_DOUBLECLICK1_PANE {
                            if window_switch_run_command(data, c) != 0 {
                                window_pane_reset_mode(wp);
                            }
                            return;
                        }
                        break 'moved;
                    }
                    _ => return,
                }
            }

            match key {
                code::P_CTRL | code::K_CTRL => key = keyc::KEYC_UP as key_code,
                code::N_CTRL | code::J_CTRL => key = keyc::KEYC_DOWN as key_code,
                _ => (),
            }

            match key {
                code::CR => {
                    if window_switch_run_command(data, c) != 0 {
                        window_pane_reset_mode(wp);
                    }
                    return;
                }
                code::ESC | code::LBRACKET_CTRL | code::C_CTRL | code::G_CTRL => {
                    window_pane_reset_mode(wp);
                    return;
                }
                _ => (),
            }

            if !data.prompt.is_null() {
                let result = prompt_key(data.prompt, key, &raw mut redraw);
                if redraw != 0 {
                    window_switch_draw_screen(wme);
                    (*wp).flags |= window_pane_flags::PANE_REDRAW;
                }
                if result == prompt_key_result::PROMPT_KEY_HANDLED
                    || result == prompt_key_result::PROMPT_KEY_NOT_HANDLED
                {
                    return;
                }
                current = data.current;
                size = data.matches.len() as u32;
            }

            match key {
                code::KEYC_UP => {
                    if size == 0 {
                        break 'moved;
                    }
                    if current == 0 {
                        window_switch_set_current(data, size - 1);
                    } else {
                        window_switch_set_current(data, current - 1);
                    }
                }
                code::KEYC_DOWN => {
                    if size == 0 {
                        break 'moved;
                    }
                    if current == size - 1 {
                        window_switch_set_current(data, 0);
                    } else {
                        window_switch_set_current(data, current + 1);
                    }
                }
                code::KEYC_PPAGE => {
                    let visible = window_switch_visible(data);
                    if current >= visible {
                        window_switch_set_current(data, current - visible);
                    } else {
                        window_switch_set_current(data, 0);
                    }
                }
                code::KEYC_NPAGE => {
                    let visible = window_switch_visible(data);
                    window_switch_set_current(data, current + visible);
                }
                code::KEYC_HOME => window_switch_set_current(data, 0),
                code::KEYC_END if size > 0 => {
                    window_switch_set_current(data, size - 1);
                }
                _ => (),
            }
        }
        // moved:
        window_switch_draw_screen(wme);
        (*wp).flags |= window_pane_flags::PANE_REDRAW;
    }
}

mod code {
    use super::*;

    pub const CR: u64 = '\r' as u64;
    pub const ESC: u64 = '\x1b' as u64;

    pub const C_CTRL: u64 = 'c' as u64 | KEYC_CTRL;
    pub const G_CTRL: u64 = 'g' as u64 | KEYC_CTRL;
    pub const J_CTRL: u64 = 'j' as u64 | KEYC_CTRL;
    pub const K_CTRL: u64 = 'k' as u64 | KEYC_CTRL;
    pub const N_CTRL: u64 = 'n' as u64 | KEYC_CTRL;
    pub const P_CTRL: u64 = 'p' as u64 | KEYC_CTRL;
    pub const LBRACKET_CTRL: u64 = '[' as u64 | KEYC_CTRL;

    pub const KEYC_UP: u64 = keyc::KEYC_UP as u64;
    pub const KEYC_DOWN: u64 = keyc::KEYC_DOWN as u64;
    pub const KEYC_PPAGE: u64 = keyc::KEYC_PPAGE as u64;
    pub const KEYC_NPAGE: u64 = keyc::KEYC_NPAGE as u64;
    pub const KEYC_HOME: u64 = keyc::KEYC_HOME as u64;
    pub const KEYC_END: u64 = keyc::KEYC_END as u64;

    pub const KEYC_WHEELUP_PANE: u64 = keyc::KEYC_WHEELUP_PANE as u64;
    pub const KEYC_WHEELDOWN_PANE: u64 = keyc::KEYC_WHEELDOWN_PANE as u64;
    pub const KEYC_MOUSEDOWN1_PANE: u64 = keyc::KEYC_MOUSEDOWN1_PANE as u64;
    pub const KEYC_DOUBLECLICK1_PANE: u64 = keyc::KEYC_DOUBLECLICK1_PANE as u64;
}
