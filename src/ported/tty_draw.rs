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

//! Port of `vendor/tmux/tty-draw.c`: write one line of a screen to a terminal.
//!
//! The line is walked as a state machine that groups adjacent cells into runs:
//! a run of visually identical cells is buffered and written once, and a run of
//! cells that are visually empty (same background, no attributes, blank or
//! cleared) is erased with a single escape sequence rather than written out as
//! spaces. That grouping is what keeps output small when a mostly-blank line is
//! repainted.
//!
//! Callers are responsible for clipping to the visible part of the line: every
//! one of them splits the range with `tty_check_overlay_range` first, so this
//! function never consults the overlay itself.
//!
//! This used to take `defaults`/`palette` where the C passes a `struct
//! tty_style_ctx`. It takes the struct now -- the split parameters had no room
//! for the `dim` field, which is why `dim=` in a style was unimplementable.

use crate::*;

/// Current state when drawing line.
/// C `vendor/tmux/tty-draw.c:26`: `enum tty_draw_line_state`
#[derive(Copy, Clone, Eq, PartialEq)]
enum tty_draw_line_state {
    TTY_DRAW_LINE_FIRST,
    TTY_DRAW_LINE_FLUSH,
    TTY_DRAW_LINE_NEW1,
    TTY_DRAW_LINE_NEW2,
    TTY_DRAW_LINE_EMPTY,
    TTY_DRAW_LINE_SAME,
    TTY_DRAW_LINE_DONE,
}

/// Clear part of the line.
/// C `vendor/tmux/tty-draw.c:46`: `static void tty_draw_line_clear(struct tty *tty, u_int px, u_int py, u_int nx, const struct grid_cell *defaults, u_int bg, int wrapped)`
unsafe fn tty_draw_line_clear(
    tty: *mut tty,
    px: u32,
    py: u32,
    nx: u32,
    defaults: *const grid_cell,
    bg: u32,
    wrapped: i32,
) {
    unsafe {
        let c = (*tty).client;

        // Nothing to clear.
        if nx == 0 {
            return;
        }

        // If genuine BCE is available, can try escape sequences.
        if (*c).overlay_check.is_none()
            && wrapped == 0
            && nx >= 10
            && !tty_fake_bce(tty, defaults, bg)
        {
            // Off the end of the line, use EL if available.
            if px + nx >= (*tty).sx && tty_term_has((*tty).term, tty_code_code::TTYC_EL) {
                tty_cursor(tty, px, py);
                tty_putcode(tty, tty_code_code::TTYC_EL);
                return;
            }

            // At the start of the line. Use EL1.
            if px == 0 && tty_term_has((*tty).term, tty_code_code::TTYC_EL1) {
                tty_cursor(tty, px + nx - 1, py);
                tty_putcode(tty, tty_code_code::TTYC_EL1);
                return;
            }

            // Section of line. Use ECH if possible.
            if tty_term_has((*tty).term, tty_code_code::TTYC_ECH) {
                tty_cursor(tty, px, py);
                tty_putcode_i(tty, tty_code_code::TTYC_ECH, nx as i32);
                return;
            }
        }

        // Couldn't use an escape sequence, use spaces.
        if px != 0 || wrapped == 0 {
            tty_cursor(tty, px, py);
        }
        if nx == 1 {
            tty_putc(tty, b' ');
        } else if nx == 2 {
            tty_putn(tty, c"  ".as_ptr().cast(), 2, 2);
        } else {
            tty_repeat_space(tty, nx);
        }
    }
}

/// Is this cell empty?
/// C `vendor/tmux/tty-draw.c:93`: `static u_int tty_draw_line_get_empty(const struct grid_cell *gc, const struct grid_cell *last, u_int nx)`
unsafe fn tty_draw_line_get_empty(
    gc: *const grid_cell,
    last: *const grid_cell,
    nx: u32,
) -> u32 {
    unsafe {
        let mut empty = 0;

        if (*gc).data.width as u32 > nx {
            empty = nx;
        } else if (*gc).flags.intersects(grid_flag::PADDING) {
            empty = 1;
        } else if (*gc).flags.intersects(grid_flag::SELECTED) {
            empty = 0;
        } else if (*gc).bg == (*last).bg && (*gc).attr.is_empty() && (*gc).link == 0 {
            if (*gc).flags.intersects(grid_flag::CLEARED) {
                empty = 1;
            } else if (*gc).flags.intersects(grid_flag::TAB) {
                empty = (*gc).data.width as u32;
            } else if (*gc).data.size == 1 && (*gc).data.data[0] == b' ' {
                empty = 1;
            }
        }
        empty
    }
}

/// Draw a line from screen to tty.
/// C `vendor/tmux/tty-draw.c:117`: `void tty_draw_line(struct tty *tty, struct screen *s, u_int px, u_int py, u_int nx, u_int atx, u_int aty, const struct tty_style_ctx *style_ctx)`
#[expect(clippy::too_many_arguments)]
pub unsafe fn tty_draw_line(
    tty: *mut tty,
    s: *mut screen,
    mut px: u32,
    py: u32,
    mut nx: u32,
    mut atx: u32,
    aty: u32,
    mut style_ctx: *const tty_style_ctx,
) {
    unsafe {
        // C tty-draw.c:130-136: a NULL context means "no pane" -- the default
        // cell, no palette, no dim -- but it still carries THIS screen's
        // hyperlink store, because the ids in its cells index that store.
        let default_style_ctx = tty_style_ctx {
            defaults: &raw const GRID_DEFAULT_CELL,
            palette: null(),
            dim: 0,
            hyperlinks: (*s).hyperlinks,
        };
        if style_ctx.is_null() {
            style_ctx = &raw const default_style_ctx;
        }
        let defaults = (*style_ctx).defaults;

        let gd = (*s).grid;
        let mut gc: grid_cell = zeroed();
        let mut ngc: grid_cell = zeroed();
        let mut last: grid_cell = zeroed();
        let mut gcp: *const grid_cell;
        const SIZEOF_BUF: usize = 1000;
        let mut buf = [0u8; SIZEOF_BUF];

        // py is the line in the screen to draw. px is the start x and nx is
        // the width to draw. atx,aty is the line on the terminal to draw it.

        // There is no point in drawing more than the end of the terminal.
        if atx >= (*tty).sx {
            return;
        }
        if atx + nx >= (*tty).sx {
            nx = (*tty).sx - atx;
        }
        if nx == 0 {
            return;
        }

        // Clamp the width to cellsize - note this is not cellused, because
        // there may be empty background cells after it (from BCE).
        let cellsize = (*grid_get_line(gd, (*gd).hsize + py)).cellsize;
        let ex = if screen_size_x(s) > cellsize {
            cellsize
        } else {
            screen_size_x(s)
        };

        // Turn off cursor while redrawing and reset region and margins.
        let flags = (*tty).flags & tty_flags::TTY_NOCURSOR;
        (*tty).flags |= tty_flags::TTY_NOCURSOR;
        tty_update_mode(tty, (*tty).mode, s);
        tty_region_off(tty);
        tty_margin_off(tty);

        // Start with the default cell as the last cell.
        memcpy__(&raw mut last, &raw const GRID_DEFAULT_CELL);
        last.bg = (*defaults).bg;
        tty_default_attributes(tty, 8, style_ctx);

        'out: {
            // If there is padding at the start, we must have truncated a wide
            // character. Clear it.
            let mut cx = 0;
            let mut i = px;
            while i < px + nx {
                grid_view_get_cell(gd, i, py, &raw mut gc);
                if !gc.flags.intersects(grid_flag::PADDING) {
                    break;
                }
                cx += 1;
                i += 1;
            }
            if cx != 0 {
                // Find the previous cell for the background colour.
                let mut i = px + 1;
                while i > 0 {
                    grid_view_get_cell(gd, i - 1, py, &raw mut gc);
                    if !gc.flags.intersects(grid_flag::PADDING) {
                        break;
                    }
                    i -= 1;
                }
                let bg = if i == 0 {
                    (*defaults).bg
                } else {
                    let mut bg = gc.bg;
                    if gc.flags.intersects(grid_flag::SELECTED) {
                        memcpy__(&raw mut ngc, &raw const gc);
                        if screen_select_cell(s, &raw mut ngc, &raw const gc) {
                            bg = ngc.bg;
                        }
                    }
                    bg
                };
                tty_attributes(tty, &raw const last, style_ctx);
                tty_draw_line_clear(tty, atx, aty, cx, defaults, bg as u32, 0);
                if cx == ex {
                    break 'out;
                }
                atx += cx;
                px += cx;
                nx -= cx;
            }

            // Did the previous line wrap on to this one?
            let mut wrapped = 0;
            if py != 0 && atx == 0 && (*tty).cx >= (*tty).sx && nx == (*tty).sx {
                let gl = grid_get_line(gd, (*gd).hsize + py - 1);
                if (*gl).flags.intersects(grid_line_flag::WRAPPED) {
                    wrapped = 1;
                }
            }
            // Loop over each character in the range.
            let mut last_i = 0;
            let mut i = 0;
            let mut len = 0usize;
            let mut width = 0;
            let mut current_state = tty_draw_line_state::TTY_DRAW_LINE_FIRST;
            loop {
                let empty;
                let next_state;

                // Work out the next state.
                if i == nx {
                    // If this is the last cell, we are done. But we need to go
                    // through the loop again to flush anything in the buffer.
                    empty = 0;
                    next_state = tty_draw_line_state::TTY_DRAW_LINE_DONE;
                    gcp = &raw const GRID_DEFAULT_CELL;
                } else {
                    if i > nx {
                        fatalx_!("position {i} > width {nx}");
                    }

                    if px >= ex || i >= ex - px {
                        // Outside the area being drawn.
                        empty = nx - i;
                        gcp = &raw const GRID_DEFAULT_CELL;
                    } else {
                        // Get the current cell.
                        grid_view_get_cell(gd, px + i, py, &raw mut gc);

                        // Work out empty cells.
                        empty = tty_draw_line_get_empty(&raw const gc, &raw const last, nx - i);
                        if empty != 0 {
                            gcp = &raw const gc;
                        } else {
                            // Update for codeset if needed.
                            gcp = tty_check_codeset(tty, &raw const gc);

                            // And for selection.
                            if (*gcp).flags.intersects(grid_flag::SELECTED) {
                                memcpy__(&raw mut ngc, gcp);
                                if screen_select_cell(s, &raw mut ngc, gcp) {
                                    gcp = &raw const ngc;
                                }
                            }
                        }
                    }

                    // Work out the next state.
                    next_state = if empty != 0 {
                        tty_draw_line_state::TTY_DRAW_LINE_EMPTY
                    } else if current_state == tty_draw_line_state::TTY_DRAW_LINE_FIRST {
                        tty_draw_line_state::TTY_DRAW_LINE_SAME
                    } else if grid_cells_look_equal(gcp, &raw const last) != 0 {
                        if (*gcp).data.size as usize > SIZEOF_BUF - len {
                            tty_draw_line_state::TTY_DRAW_LINE_FLUSH
                        } else {
                            tty_draw_line_state::TTY_DRAW_LINE_SAME
                        }
                    } else if current_state == tty_draw_line_state::TTY_DRAW_LINE_NEW1 {
                        tty_draw_line_state::TTY_DRAW_LINE_NEW2
                    } else {
                        tty_draw_line_state::TTY_DRAW_LINE_NEW1
                    };
                }

                // If the state has changed, flush any collected data.
                if next_state != current_state {
                    if current_state == tty_draw_line_state::TTY_DRAW_LINE_EMPTY {
                        tty_attributes(tty, &raw const last, style_ctx);
                        tty_draw_line_clear(
                            tty,
                            atx + last_i,
                            aty,
                            i - last_i,
                            defaults,
                            last.bg as u32,
                            wrapped,
                        );
                        wrapped = 0;
                    } else if next_state != tty_draw_line_state::TTY_DRAW_LINE_SAME && len != 0 {
                        tty_attributes(tty, &raw const last, style_ctx);
                        if atx + i - width != 0 || wrapped == 0 {
                            tty_cursor(tty, atx + i - width, aty);
                        }
                        if !last.attr.intersects(grid_attr::GRID_ATTR_CHARSET) {
                            tty_putn(tty, (&raw const buf).cast(), len, width);
                        } else {
                            for b in &buf[..len] {
                                tty_putc(tty, *b);
                            }
                        }
                        len = 0;
                        width = 0;
                        wrapped = 0;
                    }
                    last_i = i;
                }

                // Append the cell if it is not empty and not padding.
                if next_state != tty_draw_line_state::TTY_DRAW_LINE_EMPTY {
                    libc::memcpy(
                        (&raw mut buf).cast::<u8>().add(len).cast(),
                        (&raw const (*gcp).data.data).cast(),
                        (*gcp).data.size as usize,
                    );
                    len += (*gcp).data.size as usize;
                    width += (*gcp).data.width as u32;
                }

                // If this is the last cell, we are done.
                if next_state == tty_draw_line_state::TTY_DRAW_LINE_DONE {
                    break;
                }

                // Otherwise move to the next.
                current_state = next_state;
                memcpy__(&raw mut last, gcp);
                if empty != 0 {
                    i += empty;
                } else {
                    i += (*gcp).data.width as u32;
                }
            }
        }

        (*tty).flags = ((*tty).flags & !tty_flags::TTY_NOCURSOR) | flags;
        tty_update_mode(tty, (*tty).mode, s);
    }
}
