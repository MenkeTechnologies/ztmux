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

//! Port of `vendor/tmux/window-border.c`: border glyph selection, border
//! styles and the pane status line.

use crate::*;
use crate::options_::*;

/// C `vendor/tmux/window-border.c:28`: `void window_get_border_cell(struct window *w, struct window_pane *wp, enum pane_lines pane_lines, int cell_type, struct grid_cell *gc)`
pub unsafe fn window_get_border_cell(
    w: *mut window,
    wp: *mut window_pane,
    pane_lines: pane_lines,
    cell_type: cell_type,
    gc: *mut grid_cell,
) {
    unsafe {
        let mut idx: u32 = 0;

        if cell_type == cell_type::CELL_OUTSIDE && !(*w).fill_character.is_null() {
            utf8_copy(&mut (*gc).data, (*w).fill_character);
            return;
        }

        match pane_lines {
            pane_lines::PANE_LINES_NUMBER => {
                if cell_type == cell_type::CELL_OUTSIDE {
                    (*gc).attr |= grid_attr::GRID_ATTR_CHARSET;
                    utf8_set(
                        &mut (*gc).data,
                        CELL_BORDERS[cell_type::CELL_OUTSIDE as usize],
                    );
                    return;
                }
                (*gc).attr &= !grid_attr::GRID_ATTR_CHARSET;
                if !wp.is_null() && window_pane_index(wp, &raw mut idx) == 0 {
                    utf8_set(&mut (*gc).data, b'0' + ((idx % 10) as u8));
                } else {
                    utf8_set(&mut (*gc).data, b'*');
                }
            }
            pane_lines::PANE_LINES_DOUBLE => {
                (*gc).attr &= !grid_attr::GRID_ATTR_CHARSET;
                utf8_copy(&mut (*gc).data, tty_acs_double_borders(cell_type));
            }
            pane_lines::PANE_LINES_HEAVY => {
                (*gc).attr &= !grid_attr::GRID_ATTR_CHARSET;
                utf8_copy(&mut (*gc).data, tty_acs_heavy_borders(cell_type));
            }
            pane_lines::PANE_LINES_SIMPLE => {
                (*gc).attr &= !grid_attr::GRID_ATTR_CHARSET;
                utf8_set(&mut (*gc).data, SIMPLE_BORDERS[cell_type as usize]);
            }
            pane_lines::PANE_LINES_NONE | pane_lines::PANE_LINES_SPACES => {
                (*gc).attr &= !grid_attr::GRID_ATTR_CHARSET;
                utf8_set(&mut (*gc).data, b' ');
            }
            _ => {
                (*gc).attr |= grid_attr::GRID_ATTR_CHARSET;
                utf8_set(&mut (*gc).data, CELL_BORDERS[cell_type as usize]);
            }
        }
    }
}

/// C `vendor/tmux/window-border.c:77`: `void window_pane_get_border_cell(struct window_pane *wp, int cell_type, struct grid_cell *gc)`
pub unsafe fn window_pane_get_border_cell(
    wp: *mut window_pane,
    cell_type: cell_type,
    gc: *mut grid_cell,
) {
    unsafe {
        let pane_lines = window_pane_get_pane_lines(wp);
        window_get_border_cell((*wp).window, wp, pane_lines, cell_type, gc);
    }
}

/// C `vendor/tmux/window-border.c:87`: `void window_pane_get_border_style(struct window_pane *wp, struct client *c, struct grid_cell *gc)`
pub unsafe fn window_pane_get_border_style(
    wp: *mut window_pane,
    c: *mut client,
    gc: *mut grid_cell,
) {
    unsafe {
        let s = (*c).session;

        let active = wp == server_client_get_pane(c);
        let (flag, saved, option) = if active {
            (
                &raw mut (*wp).active_border_gc_set,
                &raw mut (*wp).active_border_gc,
                c!("pane-active-border-style"),
            )
        } else {
            (
                &raw mut (*wp).border_gc_set,
                &raw mut (*wp).border_gc,
                c!("pane-border-style"),
            )
        };

        if *flag == 0 {
            let ft = format_create_defaults(null_mut(), c, s, (*s).curw, wp);
            style_apply(saved, (*wp).options, option, ft);
            format_free(ft);
            // ztmux: robust sync-state border colour (never overwritten by
            // output); no-op unless the ratatui UI is on and the pane has a
            // sync state.
            crate::extensions::ratatui_ui::apply_sync_border(saved, wp);
            *flag = 1;
        }
        memcpy__(gc, saved);
    }
}

/// C `vendor/tmux/window-border.c:117`: `int window_make_pane_status(struct window_pane *wp, struct client *c, u_int width, struct redraw_span *span)`
///
/// `spans`/`first` stand in for the C's `struct redraw_span *` cursor: the Rust
/// scene keeps spans in a `Vec`, so the walk is by index into that slice.
pub unsafe fn window_make_pane_status(
    wp: *mut window_pane,
    c: *mut client,
    width: u32,
    spans: &[redraw_span],
    first: usize,
) -> i32 {
    unsafe {
        let mut gc: grid_cell = zeroed();
        let mut ctx: MaybeUninit<screen_write_ctx> = MaybeUninit::uninit();

        let pane_status = window_pane_get_pane_status(wp);
        if pane_status == pane_status::PANE_STATUS_OFF || width == 0 {
            return 0;
        }

        let ft = format_create(
            c,
            null_mut(),
            (FORMAT_PANE | (*wp).id) as i32,
            format_flags::FORMAT_STATUS,
        );
        format_defaults(
            ft,
            c,
            NonNull::new((*c).session),
            NonNull::new((*(*c).session).curw),
            NonNull::new(wp),
        );

        let fmt = options_get_string_((*wp).options, "pane-border-format");
        let expanded = format_expand_time(ft, fmt);

        let mut old = (*wp).status_screen.clone();
        screen_init(&raw mut (*wp).status_screen, width, 1, 0);
        (*wp).status_screen.mode = mode_flag::empty();
        screen_write_start(ctx.as_mut_ptr(), &raw mut (*wp).status_screen);

        window_pane_get_border_style(wp, c, &raw mut gc);
        let pane_lines = window_pane_get_pane_lines(wp);
        let mut cursor = first;
        for i in 0..width {
            let cell_type = redraw_get_status_border_cell_type(spans, &mut cursor, i);
            window_get_border_cell((*wp).window, wp, pane_lines, cell_type, &raw mut gc);
            screen_write_cell(ctx.as_mut_ptr(), &raw const gc);
        }
        gc.attr &= !grid_attr::GRID_ATTR_CHARSET;

        screen_write_cursormove(ctx.as_mut_ptr(), 0, 0, 0);
        format_draw(
            ctx.as_mut_ptr(),
            &raw mut gc,
            width,
            cstr_to_str(expanded),
            null_mut(),
            0,
        );

        screen_write_stop(ctx.as_mut_ptr());
        format_free(ft);
        free_(expanded);

        if grid_compare((*wp).status_screen.grid, old.grid) == 0 {
            screen_free(&raw mut old);
            return 0;
        }
        screen_free(&raw mut old);
        1
    }
}
