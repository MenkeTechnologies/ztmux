// Copyright (c) 2007 Nicholas Marriott <nicholas.marriott@gmail.com>
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

use crate::*;
use crate::options_::*;

/// Status timer callback.
/// C `vendor/tmux/status.c:38`: `static void status_timer_callback(__unused int fd, __unused short events, void *arg)`
unsafe extern "C-unwind" fn status_timer_callback(_fd: i32, _events: i16, c: NonNull<client>) {
    unsafe {
        let c = c.as_ptr();
        let s: *mut session = (*c).session;

        evtimer_del(&raw mut (*c).status.timer);

        if s.is_null() {
            return;
        }

        if (*c).message_string.is_none() && (*c).prompt.is_null() {
            (*c).flags |= client_flag::REDRAWSTATUS;
        }

        let mut tv: timeval = zeroed();
        timerclear(&raw mut tv);
        tv.tv_sec = options_get_number_((*s).options, "status-interval");

        if tv.tv_sec != 0 {
            evtimer_add(&raw mut (*c).status.timer, &raw const tv);
        }
        log_debug!("client {:p}, status interval {}", c, tv.tv_sec);
    }
}

/// Start status timer for client.
/// C `vendor/tmux/status.c:62`: `void status_timer_start(struct client *c)`
pub unsafe fn status_timer_start(c: NonNull<client>) {
    unsafe {
        let s: *mut session = (*c.as_ptr()).session;

        if event_initialized(&raw mut (*c.as_ptr()).status.timer) != 0 {
            evtimer_del(&raw mut (*c.as_ptr()).status.timer);
        } else {
            evtimer_set(
                &raw mut (*c.as_ptr()).status.timer,
                status_timer_callback,
                c,
            );
        }

        if !s.is_null() && options_get_number_((*s).options, "status") != 0 {
            status_timer_callback(-1, 0, c);
        }
    }
}

/// Start status timer for all clients.
/// C `vendor/tmux/status.c:77`: `void status_timer_start_all(void)`
pub unsafe fn status_timer_start_all() {
    unsafe {
        for c in tailq_foreach(&raw mut CLIENTS) {
            status_timer_start(c);
        }
    }
}

/// Update status cache.
/// C `vendor/tmux/status.c:87`: `void status_update_cache(struct session *s)`
pub unsafe fn status_update_cache(s: *mut session) {
    unsafe {
        (*s).statuslines = options_get_number_((*s).options, "status") as u32;
        if (*s).statuslines == 0 {
            (*s).statusat = -1;
        } else if options_get_number_((*s).options, "status-position") == 0 {
            (*s).statusat = 0;
        } else {
            (*s).statusat = 1;
        }
    }
}

/// Get screen line of status line. -1 means off.
/// C `vendor/tmux/status.c:100`: `int status_at_line(struct client *c)`
pub unsafe fn status_at_line(c: *mut client) -> i32 {
    unsafe {
        let s: *mut session = (*c).session;

        if (*c)
            .flags
            .intersects(client_flag::STATUSOFF | client_flag::CONTROL)
        {
            return -1;
        }
        if (*s).statusat != 1 {
            return (*s).statusat;
        }
        (*c).tty.sy as i32 - status_line_size(c) as i32
    }
}

/// Get size of status line for client's session. 0 means off.
/// C `vendor/tmux/status.c:113`: `u_int status_line_size(struct client *c)`
pub unsafe fn status_line_size(c: *mut client) -> u32 {
    unsafe {
        let s: *mut session = (*c).session;

        if (*c)
            .flags
            .intersects(client_flag::STATUSOFF | client_flag::CONTROL)
        {
            return 0;
        }
        if s.is_null() {
            return options_get_number_(GLOBAL_S_OPTIONS, "status") as u32;
        }
        (*s).statuslines
    }
}

/// Get the prompt line number for client's session. 1 means at the bottom.
/// C `vendor/tmux/status.c:126`: `u_int status_prompt_line_at(struct client *c)`
pub(crate) unsafe fn status_prompt_line_at(c: *mut client) -> u32 {
    unsafe {
        let s = (*c).session;

        if (*c)
            .flags
            .intersects(client_flag::STATUSOFF | client_flag::CONTROL)
        {
            return 1;
        }
        options_get_number_((*s).options, "message-line") as u32
    }
}

/// Get window at window list position.
/// C `vendor/tmux/status.c:142`: `struct style_range *status_get_range(struct client *c, u_int x, u_int y)`
pub unsafe fn status_get_range(c: *mut client, x: u32, y: u32) -> *mut style_range {
    unsafe {
        let sl = &raw mut (*c).status;

        if y >= (*sl).entries.len() as u32 {
            return null_mut();
        }
        for sr in tailq_foreach(&raw mut (*sl).entries[y as usize].ranges).map(NonNull::as_ptr) {
            if x >= (*sr).start && x < (*sr).end {
                return sr;
            }
        }
        null_mut()
    }
}

/// Free all ranges.
unsafe fn status_free_ranges(srs: *mut style_ranges) {
    unsafe {
        for sr in tailq_foreach(srs).map(NonNull::as_ptr) {
            tailq_remove(srs, sr);
            free_(sr);
        }
    }
}

/// Save old status line.
/// C `vendor/tmux/status.c:153`: `static void status_push_screen(struct client *c)`
unsafe fn status_push_screen(c: *mut client) {
    unsafe {
        let sl = &raw mut (*c).status;

        if (*sl).active == &raw mut (*sl).screen {
            (*sl).active = Box::leak(Box::new(zeroed())) as *mut screen;
            screen_init((*sl).active, (*c).tty.sx, status_line_size(c), 0);
        }
        (*sl).references += 1;
    }
}

/// Restore old status line.
/// C `vendor/tmux/status.c:166`: `static void status_pop_screen(struct client *c)`
unsafe fn status_pop_screen(c: *mut client) {
    unsafe {
        let sl = &raw mut (*c).status;

        (*sl).references -= 1;
        if (*sl).references == 0 {
            screen_free((*sl).active);
            free_((*sl).active);
            (*sl).active = &raw mut (*sl).screen;
        }
    }
}

/// Initialize status line.
/// C `vendor/tmux/status.c:179`: `void status_init(struct client *c)`
pub unsafe fn status_init(c: *mut client) {
    unsafe {
        let sl = &raw mut (*c).status;

        for i in 0..(*sl).entries.len() {
            tailq_init(&raw mut (*sl).entries[i].ranges);
        }

        screen_init(&raw mut (*sl).screen, (*c).tty.sx, 1, 0);
        (*sl).active = &raw mut (*sl).screen;
    }
}

/// Free status line.
/// C `vendor/tmux/status.c:193`: `void status_free(struct client *c)`
pub unsafe fn status_free(c: *mut client) {
    unsafe {
        let sl = &raw mut (*c).status;

        for i in 0..(*sl).entries.len() {
            status_free_ranges(&raw mut (*sl).entries[i].ranges);
            (*sl).entries[i].expanded = None;
        }

        if event_initialized(&raw mut (*sl).timer) != 0 {
            evtimer_del(&raw mut (*sl).timer);
        }

        if (*sl).active != &raw mut (*sl).screen {
            screen_free((*sl).active);
            free_((*sl).active);
        }
        screen_free(&raw mut (*sl).screen);
    }
}

/// Draw status line for client.
/// C `vendor/tmux/status.c:215`: `int status_redraw(struct client *c)`
pub unsafe fn status_redraw(c: *mut client) -> i32 {
    unsafe {
        let sl = &raw mut (*c).status;
        // status_line_entry *sle;
        let s = (*c).session;
        let mut ctx: screen_write_ctx = zeroed();
        let mut gc: grid_cell = zeroed();

        // u_int lines, i, n;

        let width = (*c).tty.sx;

        let mut force = false;
        let mut changed = false;

        // int flags, force = 0, changed = 0, fg, bg;

        // struct options_entry *o;
        // union options_value *ov;
        // struct format_tree *ft;
        // char *expanded;

        log_debug!("status_redraw enter");

        // Shouldn't get here if not the active screen.
        if (*sl).active != &raw mut (*sl).screen {
            fatalx("not the active screen");
        }

        // No status line?
        let lines = status_line_size(c);
        if (*c).tty.sy == 0 || lines == 0 {
            return 1;
        }

        // Create format tree.
        let mut flags = format_flags::FORMAT_STATUS;
        if (*c).flags.intersects(client_flag::STATUSFORCE) {
            flags |= format_flags::FORMAT_FORCE;
        }
        let ft = format_create(c, null_mut(), FORMAT_NONE, flags);
        format_defaults(ft, c, None, None, None);

        // Set up default colour.
        style_apply(&raw mut gc, (*s).options, c!("status-style"), ft);
        let fg = options_get_number_((*s).options, "status-fg") as i32;
        if !COLOUR_DEFAULT(fg) {
            gc.fg = fg;
        }
        let bg = options_get_number_((*s).options, "status-bg") as i32;
        if !COLOUR_DEFAULT(bg) {
            gc.bg = bg;
        }
        if !grid_cells_equal(&raw const gc, &raw const (*sl).style) {
            force = true;
            memcpy__(&raw mut (*sl).style, &raw mut gc);
        }

        // Resize the target screen.
        if screen_size_x(&raw mut (*sl).screen) != width
            || screen_size_y(&raw mut (*sl).screen) != lines
        {
            screen_resize(&raw mut (*sl).screen, width, lines, 0);
            changed = true;
            force = true;
        }
        screen_write_start(&raw mut ctx, &raw mut (*sl).screen);

        // Write the status lines.
        let o = options_get(&mut *(*s).options, "status-format");
        if o.is_null() {
            for _ in 0..(width * lines) {
                screen_write_putc(&raw mut ctx, &raw mut gc, b' ');
            }
        } else {
            for i in 0..lines {
                screen_write_cursormove(&raw mut ctx, 0, i as i32, 0);

                let ov = options_array_get(o, i);
                if ov.is_null() {
                    for _ in 0..width {
                        screen_write_putc(&raw mut ctx, &raw mut gc, b' ');
                    }
                    continue;
                }
                let sle = &raw mut (*sl).entries[i as usize];

                let expanded = format_expand_time(ft, (*ov).string);
                if !force
                    && (*sle).expanded.is_some()
                    && libc::strcmp(expanded, (*sle).expanded_ptr()) == 0
                {
                    free_(expanded);
                    continue;
                }
                changed = true;

                for _ in 0..width {
                    screen_write_putc(&raw mut ctx, &raw mut gc, b' ');
                }
                screen_write_cursormove(&raw mut ctx, 0, i as i32, 0);

                status_free_ranges(&raw mut (*sle).ranges);
                format_draw(
                    &raw mut ctx,
                    &raw mut gc,
                    width,
                    cstr_to_str(expanded),
                    &raw mut (*sle).ranges,
                    0,
                );

                // Adopt the owned expansion; assigning drops the old one.
                (*sle).expanded = Some(std::ffi::CString::from_raw(expanded.cast()));
            }
        }
        screen_write_stop(&raw mut ctx);

        // Free the format tree.
        format_free(ft);

        // Return if the status line has changed.
        // log_debug("%s exit: force=%d, changed=%d", __func__, force, changed);
        (force || changed) as i32
    }
}

macro_rules! status_message_set {
   ($c:expr, $delay:expr, $ignore_styles:expr, $ignore_keys:expr, $no_freeze:expr, $fmt:literal $(, $args:expr)* $(,)?) => {
        crate::status::status_message_set_($c, $delay, $ignore_styles, $ignore_keys, $no_freeze, format_args!($fmt $(, $args)*))
    };
}
pub(crate) use status_message_set;

/// Set a status line message.
/// C `vendor/tmux/status.c:340`: `void status_message_set(struct client *c, int delay, int ignore_styles, int ignore_keys, int no_freeze, const char *fmt, ...)`
pub unsafe fn status_message_set_(
    c: *mut client,
    mut delay: i32,
    ignore_styles: i32,
    ignore_keys: bool,
    no_freeze: i32,
    args: std::fmt::Arguments,
) {
    unsafe {
        let mut tv: timeval = zeroed();
        let s = args.to_string();

        // log_debug("%s: %s", __func__, s);

        if c.is_null() {
            server_add_message!("message: {}", s);
            return;
        }

        status_message_clear(NonNull::new_unchecked(c));
        // ztmux: the floating message overlay doesn't take over the status row,
        // so the powerline status bar stays visible - skip the screen push.
        if !crate::extensions::ratatui_ui::enabled() {
            status_push_screen(c);
        }
        let cs = crate::cstring_truncating(s);
        server_add_message!(
            "{} message: {}",
            _s((*c).name),
            _s(cs.as_ptr().cast::<u8>())
        );
        (*c).message_string = Some(cs);

        // With delay -1, the display-time option is used; zero means wait for
        // key press; more than zero is the actual delay time in milliseconds.
        if delay == -1 {
            delay = options_get_number_((*(*c).session).options, "display-time") as i32;
        }
        if delay > 0 {
            tv.tv_sec = (delay / 1000) as libc::time_t;
            tv.tv_usec = (delay as libc::suseconds_t % 1000) * (1000 as libc::suseconds_t);

            if event_initialized(&raw mut (*c).message_timer) != 0 {
                evtimer_del(&raw mut (*c).message_timer);
            }
            evtimer_set(
                &raw mut (*c).message_timer,
                status_message_callback,
                NonNull::new_unchecked(c),
            );

            evtimer_add(&raw mut (*c).message_timer, &raw mut tv);
        }

        if delay != 0 {
            (*c).message_ignore_keys = ignore_keys as i32;
        }
        (*c).message_ignore_styles = ignore_styles;

        // ztmux: float the message as a ratatui overlay instead of freezing the
        // whole window and painting over the status row. The panes and status
        // bar keep redrawing underneath; the display-time timer (or the next
        // key) tears the overlay down via status_message_clear.
        if crate::extensions::ratatui_ui::enabled() {
            (*c).tty.flags |= tty_flags::TTY_NOCURSOR;
            (*c).flags |= client_flag::REDRAWSTATUS;
            crate::extensions::ratatui_ui::set_message_overlay(c);
            return;
        }

        if no_freeze == 0 {
            (*c).tty.flags |= tty_flags::TTY_FREEZE;
        }
        (*c).tty.flags |= tty_flags::TTY_NOCURSOR;
        (*c).flags |= client_flag::REDRAWSTATUS;
    }
}

/// Clear status line message.
/// C `vendor/tmux/status.c:393`: `void status_message_clear(struct client *c)`
pub unsafe fn status_message_clear(c: NonNull<client>) {
    unsafe {
        let c = c.as_ptr();
        if (*c).message_string.is_none() {
            return;
        }

        // ztmux: tear down the floating message overlay if we put one up.
        if crate::extensions::ratatui_ui::enabled() {
            crate::extensions::ratatui_ui::clear_message_overlay(c);
        }

        (*c).message_string = None;
        (*c).message_string = None;

        if (*c).prompt.is_null() {
            (*c).tty.flags &= !(tty_flags::TTY_NOCURSOR | tty_flags::TTY_FREEZE);
        }
        (*c).flags |= CLIENT_ALLREDRAWFLAGS; /* was frozen and may have changed */

        // ztmux: no status screen was pushed for the overlay message (see set).
        if !crate::extensions::ratatui_ui::enabled() {
            status_pop_screen(c);
        }
    }
}

/// Clear status line message after timer expires.
/// C `vendor/tmux/status.c:453`: `static void status_message_callback(__unused int fd, __unused short event, void *data)`
unsafe extern "C-unwind" fn status_message_callback(_fd: i32, _event: i16, data: NonNull<client>) {
    unsafe {
        status_message_clear(data);
    }
}

/// Draw client message on status line of present else on last line.
/// C `vendor/tmux/status.c:462`: `int status_message_redraw(struct client *c)`
pub unsafe fn status_message_redraw(c: *mut client) -> i32 {
    unsafe {
        // ztmux: the message is drawn as a floating overlay (registered by
        // status_message_set), not on the status line, so the powerline status
        // bar stays visible. Nothing to draw here.
        if crate::extensions::ratatui_ui::enabled() {
            return 0;
        }

        let sl = &raw mut (*c).status;
        let mut ctx: screen_write_ctx = zeroed();
        let s = (*c).session;
        // size_t len;
        // u_int lines, offset, messageline;
        let mut gc: grid_cell = zeroed();
        // struct format_tree *ft;

        if (*c).tty.sx == 0 || (*c).tty.sy == 0 {
            return 0;
        }
        let mut old_screen = (*(*sl).active).clone();

        let mut lines = status_line_size(c);
        if lines <= 1 {
            lines = 1;
        }
        screen_init((*sl).active, (*c).tty.sx, lines, 0);

        let mut messageline = status_prompt_line_at(c);
        if messageline > lines - 1 {
            messageline = lines - 1;
        }

        let ft = format_create_defaults(null_mut(), c, null_mut(), null_mut(), null_mut());
        memcpy__(&raw mut gc, &raw const GRID_DEFAULT_CELL);

        // The message is placed into the format tree rather than drawn
        // directly, so `message-format` decides how it is wrapped and styled.
        // When styles are to be ignored, `#` is doubled first so format_draw
        // treats the content as literal text rather than as directives.
        if (*c).message_ignore_styles != 0 {
            let msg = status_message_escape((*c).message_string_ptr());
            format_add!(ft, "message", "{}", _s(msg));
            free_(msg);
        } else {
            format_add!(ft, "message", "{}", _s((*c).message_string_ptr()));
        }
        format_add!(ft, "command_prompt", "{}", 0);

        let msgfmt = options_get_string_((*s).options, "message-format");
        let expanded = format_expand_time(ft, msgfmt);
        format_free(ft);

        screen_write_start(&raw mut ctx, (*sl).active);
        screen_write_fast_copy(
            &raw mut ctx,
            &raw mut (*sl).screen,
            0,
            0,
            (*c).tty.sx,
            lines,
        );
        screen_write_cursormove(&raw mut ctx, 0, messageline as i32, 0);
        for _ in 0..(*c).tty.sx {
            screen_write_putc(&raw mut ctx, &raw const gc, b' ');
        }
        screen_write_cursormove(&raw mut ctx, 0, messageline as i32, 0);
        format_draw(
            &raw mut ctx,
            &raw const gc,
            (*c).tty.sx,
            cstr_to_str(expanded),
            null_mut(),
            0,
        );
        screen_write_stop(&raw mut ctx);

        free_(expanded);

        if grid_compare((*(*sl).active).grid, old_screen.grid) == 0 {
            screen_free(&raw mut old_screen);
            return 0;
        }
        screen_free(&raw mut old_screen);
        1
    }
}

/// Double every `#` so `format_draw` renders the string literally instead of
/// reading `#[...]` in it as style directives — used for messages that asked
/// for styles to be ignored.
/// C `vendor/tmux/status.c:429`: `static char *status_message_escape(const char *s)`
pub unsafe fn status_message_escape(s: *const u8) -> *mut u8 {
    unsafe {
        let len = libc::strlen(s);
        let bytes = std::slice::from_raw_parts(s, len);
        let n = bytes.iter().filter(|&&b| b == b'#').count();

        let out: *mut u8 = xmalloc(len + n + 1).as_ptr().cast();
        let mut p = out;
        for &b in bytes {
            if b == b'#' {
                *p = b'#';
                p = p.add(1);
            }
            *p = b;
            p = p.add(1);
        }
        *p = b'\0';
        out
    }
}

/// Calculate prompt/message area geometry from the style's width and align
/// directives: x offset and available width within the status line.
/// C `vendor/tmux/status.c:413`: `static void status_message_area(struct client *c, u_int *area_x, u_int *area_w)`
unsafe fn status_message_area(c: *mut client, area_x: *mut u32, area_w: *mut u32) {
    unsafe {
        let s = (*c).session;
        let w: u32;

        // Get width from message-style's width directive.
        let sy = options_string_to_style((*s).options, "message-style", null_mut());
        if !sy.is_null() && (*sy).width >= 0 {
            if (*sy).width_percentage != 0 {
                w = ((*c).tty.sx * (*sy).width as u32) / 100;
            } else {
                w = (*sy).width as u32;
            }
        } else {
            w = (*c).tty.sx;
        }
        let w = if w == 0 || w > (*c).tty.sx {
            (*c).tty.sx
        } else {
            w
        };

        // Get horizontal position from message-style's align directive.
        if !sy.is_null() {
            *area_x = match (*sy).align {
                style_align::STYLE_ALIGN_CENTRE | style_align::STYLE_ALIGN_ABSOLUTE_CENTRE => {
                    ((*c).tty.sx - w) / 2
                }
                style_align::STYLE_ALIGN_RIGHT => (*c).tty.sx - w,
                _ => 0,
            };
        } else {
            *area_x = 0;
        }

        *area_w = w;
    }
}

/// The client-level indirection `status_prompt_set` puts between the prompt
/// object and its caller: the prompt only knows a `void *`, so this carries the
/// client through to callbacks that still want one.
/// C `vendor/tmux/status.c:527`: `struct status_prompt_data`
#[repr(C)]
struct status_prompt_data {
    c: *mut client,
    inputcb: status_prompt_input_cb,
    freecb: prompt_free_cb,
    data: *mut c_void,
}

/// C `vendor/tmux/status.c:535`: `static enum prompt_result status_prompt_input_callback(void *data, const char *s, enum prompt_key_result key)`
unsafe fn status_prompt_input_callback(
    data: NonNull<c_void>,
    s: *const u8,
    key: prompt_key_result,
) -> prompt_result {
    unsafe {
        let spd: *mut status_prompt_data = data.as_ptr().cast();
        let c = (*spd).c;
        let inputcb = (*spd).inputcb;
        let arg = (*spd).data;

        if let (Some(inputcb), Some(arg)) = (inputcb, NonNull::new(arg)) {
            return inputcb(c, arg, s, key);
        }
        prompt_result::PROMPT_CLOSE
    }
}

/// C `vendor/tmux/status.c:549`: `static void status_prompt_free_callback(void *data)`
unsafe fn status_prompt_free_callback(data: NonNull<c_void>) {
    unsafe {
        let spd: *mut status_prompt_data = data.as_ptr().cast();
        let freecb = (*spd).freecb;
        let arg = (*spd).data;

        if let (Some(freecb), Some(arg)) = (freecb, NonNull::new(arg)) {
            freecb(arg);
        }
        free_(spd);
    }
}

/// Accept prompt immediately.
/// C `vendor/tmux/status.c:562`: `static enum cmd_retval status_prompt_accept(__unused struct cmdq_item *item, void *data)`
unsafe fn status_prompt_accept(_item: *mut cmdq_item, data: *mut c_void) -> cmd_retval {
    unsafe {
        let c: *mut client = data.cast();
        if !(*c).prompt.is_null() {
            status_prompt_key(c, b'y' as key_code, null_mut());
        }
        cmd_retval::CMD_RETURN_NORMAL
    }
}

/// Enable status line prompt.
/// C `vendor/tmux/status.c:573`: `void status_prompt_set(struct client *c, struct cmd_find_state *fs, const char *msg, const char *input, status_prompt_input_cb inputcb, prompt_free_cb freecb, void *data, int flags, enum prompt_type prompt_type)`
pub unsafe fn status_prompt_set<T>(
    c: *mut client,
    fs: *mut cmd_find_state,
    msg: *const u8,
    input: *const u8,
    inputcb: unsafe fn(*mut client, NonNull<T>, *const u8, prompt_key_result) -> prompt_result,
    freecb: unsafe fn(NonNull<T>),
    data: *mut T,
    flags: prompt_flags,
    prompt_type: prompt_type,
) {
    unsafe {
        server_client_clear_overlay(c);

        status_message_clear(NonNull::new_unchecked(c));
        status_prompt_clear(c);
        // ztmux: the floating overlay prompt doesn't take over the status row,
        // so the powerline status bar stays visible - skip the screen push.
        if !crate::extensions::ratatui_ui::enabled() {
            status_push_screen(c);
        }

        let spd: *mut status_prompt_data = xcalloc_::<status_prompt_data>(1).as_ptr();
        (*spd).c = c;
        (*spd).inputcb = Some(std::mem::transmute::<
            unsafe fn(*mut client, NonNull<T>, *const u8, prompt_key_result) -> prompt_result,
            unsafe fn(*mut client, NonNull<c_void>, *const u8, prompt_key_result) -> prompt_result,
        >(inputcb));
        (*spd).freecb = Some(std::mem::transmute::<
            unsafe fn(NonNull<T>),
            unsafe fn(NonNull<c_void>),
        >(freecb));
        (*spd).data = data.cast();

        let mut pd: prompt_create_data = zeroed();
        prompt_set_options(&raw mut pd, (*c).session);
        pd.fs = fs;
        pd.prompt = msg;
        pd.input = input;
        pd.type_ = prompt_type;
        pd.flags = flags;
        pd.inputcb = Some(status_prompt_input_callback);
        pd.freecb = Some(status_prompt_free_callback);
        pd.data = spd.cast();
        (*c).prompt = prompt_create(&raw const pd);

        if !flags.intersects(prompt_flags::PROMPT_INCREMENTAL)
            && !flags.intersects(prompt_flags::PROMPT_NOFREEZE)
        {
            // ztmux: never freeze the whole window for the overlay prompt - the
            // panes and status bar must keep redrawing (freezing also ghosts the
            // shrinking completion box). We draw our own block cursor, so keep
            // NOCURSOR to hide the pane's hardware cursor.
            if crate::extensions::ratatui_ui::enabled() {
                (*c).tty.flags |= tty_flags::TTY_NOCURSOR;
            } else {
                (*c).tty.flags |= tty_flags::TTY_FREEZE;
            }
        }
        (*c).flags |= client_flag::REDRAWSTATUS;

        // ztmux: float the prompt as a ratatui overlay instead of the status row.
        if crate::extensions::ratatui_ui::enabled() {
            crate::extensions::ratatui_ui::set_prompt_overlay(c);
        }

        prompt_incremental_start((*c).prompt);

        if flags.intersects(prompt_flags::PROMPT_SINGLE)
            && flags.intersects(prompt_flags::PROMPT_ACCEPT)
        {
            cmdq_append(c, cmdq_get_callback!(status_prompt_accept, c.cast()).as_ptr());
        }
    }
}

/// Remove status line prompt.
/// C `vendor/tmux/status.c:616`: `void status_prompt_clear(struct client *c)`
pub unsafe fn status_prompt_clear(c: *mut client) {
    unsafe {
        if (*c).prompt.is_null() {
            return;
        }

        // ztmux: tear down the floating prompt overlay if we put one up.
        if crate::extensions::ratatui_ui::enabled() {
            crate::extensions::ratatui_ui::clear_prompt_overlay(c);
        }

        prompt_free((*c).prompt);
        (*c).prompt = null_mut();

        (*c).tty.flags &= !(tty_flags::TTY_NOCURSOR | tty_flags::TTY_FREEZE);
        (*c).flags |= CLIENT_ALLREDRAWFLAGS; /* was frozen and may have changed */

        // ztmux: no status screen was pushed for the overlay prompt (see set).
        if !crate::extensions::ratatui_ui::enabled() {
            status_pop_screen(c);
        }
    }
}

/// Update status line prompt with a new prompt string.
/// C `vendor/tmux/status.c:632`: `void status_prompt_update(struct client *c, const char *msg, const char *input)`
pub unsafe fn status_prompt_update(c: *mut client, msg: *const u8, input: *const u8) {
    unsafe {
        if (*c).prompt.is_null() {
            return;
        }
        prompt_update((*c).prompt, msg, input);
        (*c).flags |= client_flag::REDRAWSTATUS;
    }
}

/// Get the screen line on which the prompt is drawn.
/// C `vendor/tmux/status.c:642`: `static u_int status_prompt_screen_line(struct client *c)`
unsafe fn status_prompt_screen_line(c: *mut client) -> u32 {
    unsafe {
        let tty = &raw mut (*c).tty;

        if options_get_number_((*(*c).session).options, "status-position") == 0 {
            return status_prompt_line_at(c);
        }
        let n = status_line_size(c) - status_prompt_line_at(c);
        if n <= (*tty).sy {
            return (*tty).sy - n;
        }
        (*tty).sy - 1
    }
}

/// Draw client prompt on status line of present else on last line.
/// C `vendor/tmux/status.c:657`: `int status_prompt_redraw(struct client *c)`
pub unsafe fn status_prompt_redraw(c: *mut client) -> i32 {
    unsafe {
        // ztmux: the ratatui prompt is drawn as a floating overlay (registered
        // by status_prompt_set), not on the status line, so the powerline status
        // bar stays visible. Nothing to draw here.
        if crate::extensions::ratatui_ui::enabled() {
            return 0;
        }

        let sl = &raw mut (*c).status;
        let mut ctx: screen_write_ctx = zeroed();
        let mut ax: u32 = 0;
        let mut aw: u32 = 0;

        if (*c).tty.sx == 0 || (*c).tty.sy == 0 {
            return 0;
        }
        let mut old_screen = (*(*sl).active).clone();

        let mut lines = status_line_size(c);
        if lines <= 1 {
            lines = 1;
        }
        screen_init((*sl).active, (*c).tty.sx, lines, 0);

        let mut promptline = status_prompt_line_at(c);
        if promptline > lines - 1 {
            promptline = lines - 1;
        }

        status_message_area(c, &raw mut ax, &raw mut aw);

        screen_write_start(&raw mut ctx, (*sl).active);
        screen_write_fast_copy(
            &raw mut ctx,
            &raw mut (*sl).screen,
            0,
            0,
            (*c).tty.sx,
            lines,
        );

        let mut pdd: prompt_draw_data = zeroed();
        pdd.ctx = &raw mut ctx;
        pdd.area_x = ax;
        pdd.area_width = aw;
        pdd.prompt_line = promptline;
        pdd.cursor_x = &raw mut (*sl).prompt_cx;
        prompt_draw((*c).prompt, &raw mut pdd);

        screen_write_stop(&raw mut ctx);

        if grid_compare((*(*sl).active).grid, old_screen.grid) == 0 {
            screen_free(&raw mut old_screen);
            return 0;
        }
        screen_free(&raw mut old_screen);
        1
    }
}

/// Work out the tty cursor position for the prompt.
/// C `vendor/tmux/status.c:702`: `void status_prompt_cursor(struct client *c, u_int *cx, u_int *cy)`
pub unsafe fn status_prompt_cursor(c: *mut client, cx: *mut u32, cy: *mut u32) {
    unsafe {
        *cy = status_prompt_screen_line(c);
        *cx = (*c).status.prompt_cx;
    }
}

/// Handle keys in prompt.
/// C `vendor/tmux/status.c:710`: `enum prompt_key_result status_prompt_key(struct client *c, key_code key, struct mouse_event *m)`
pub unsafe fn status_prompt_key(
    c: *mut client,
    key: key_code,
    m: *mut mouse_event,
) -> prompt_key_result {
    unsafe {
        let mut ax: u32 = 0;
        let mut aw: u32 = 0;
        let mut redraw: i32 = 0;

        let result = if KEYC_IS_MOUSE(key) {
            if m.is_null()
                || MOUSE_BUTTONS((*m).b) != MOUSE_BUTTON_1
                || MOUSE_DRAG((*m).b)
                || MOUSE_RELEASE((*m).b)
                || (*m).y != status_prompt_screen_line(c)
            {
                return prompt_key_result::PROMPT_KEY_NOT_HANDLED;
            }
            status_message_area(c, &raw mut ax, &raw mut aw);
            prompt_mouse((*c).prompt, (*m).x, ax, aw, &raw mut redraw)
        } else {
            prompt_key((*c).prompt, key, &raw mut redraw)
        };
        if redraw != 0 && !(*c).prompt.is_null() {
            (*c).flags |= client_flag::REDRAWSTATUS;
        }
        if !(*c).prompt.is_null() && prompt_closed((*c).prompt) != 0 {
            status_prompt_clear(c);
        }
        result
    }
}
