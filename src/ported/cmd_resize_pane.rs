// Copyright (c) 2009 Nicholas Marriott <nicholas.marriott@gmail.com>
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
use crate::compat::queue::tailq_empty;
use crate::*;
use crate::options_::*;

pub static CMD_RESIZE_PANE_ENTRY: cmd_entry = cmd_entry {
    name: "resize-pane",
    alias: Some("resizep"),

    args: args_parse::new("D::L::MR::Tt:U::x:y:Z", 0, 1, None),
    usage: "[-MTZ] [-D lines] [-L columns] [-R columns] [-U lines] \
            [-x width] [-y height] [-t target-pane]",

    target: cmd_entry_flag::new(b't', cmd_find_type::CMD_FIND_PANE, cmd_find_flags::empty()),

    flags: cmd_flag::CMD_AFTERHOOK,
    exec: cmd_resize_pane_exec,
    source: cmd_entry_flag::zeroed(),
};

/// C `vendor/tmux/cmd-resize-pane.c:55`: `static enum cmd_retval cmd_resize_pane_exec(struct cmd *self, struct cmdq_item *item)`
unsafe fn cmd_resize_pane_exec(self_: *mut cmd, item: *mut cmdq_item) -> cmd_retval {
    unsafe {
        let args = cmd_get_args(self_);
        let target = cmdq_get_target(item);
        let wp = (*target).wp;
        let wl = (*target).wl;
        let w = (*wl).window;
        let lc = (*wp).layout_cell;
        let mut cause: *mut u8 = null_mut();
        let mut adjust;
        let x: i32;
        let mut y: i32;
        let gd = (*wp).base.grid;

        if args_has(args, 'T') {
            if !tailq_empty(&raw mut (*wp).modes) {
                return cmd_retval::CMD_RETURN_NORMAL;
            }
            adjust = screen_size_y(&raw mut (*wp).base) - 1 - (*wp).base.cy;
            if adjust > (*gd).hsize {
                adjust = (*gd).hsize;
            }
            grid_remove_history(gd, adjust);
            (*wp).base.cy += adjust;
            (*wp).flags |= window_pane_flags::PANE_REDRAW;
            return cmd_retval::CMD_RETURN_NORMAL;
        }

        if args_has(args, 'M') {
            return cmd_resize_pane_mouse_update(item);
        }

        if args_has(args, 'Z') {
            if (*w).flags.intersects(window_flag::ZOOMED) {
                window_unzoom(w, 1);
            } else {
                window_zoom(wp);
            }
            server_redraw_window(w);
            return cmd_retval::CMD_RETURN_NORMAL;
        }
        server_unzoom_window(w);

        if args_has(args, 'x') {
            x = args_percentage(
                args,
                b'x',
                0,
                PANE_MAXIMUM as i64,
                (*w).sx as i64,
                &raw mut cause,
            ) as i32;
            if !cause.is_null() {
                cmdq_error!(item, "width {}", _s(cause));
                free_(cause);
                return cmd_retval::CMD_RETURN_ERROR;
            }
            if window_pane_is_floating(wp) != 0 {
                if layout_resize_floating_pane_to(
                    wp,
                    layout_type::LAYOUT_LEFTRIGHT,
                    x as u32,
                    &raw mut cause,
                ) != 0
                {
                    cmdq_error!(item, "size {}", _s(cause));
                    free_(cause);
                    return cmd_retval::CMD_RETURN_ERROR;
                }
            } else {
                layout_resize_pane_to(wp, layout_type::LAYOUT_LEFTRIGHT, x as u32);
            }
        }
        if args_has(args, 'y') {
            y = args_percentage(
                args,
                b'y',
                0,
                PANE_MAXIMUM as i64,
                (*w).sy as i64,
                &raw mut cause,
            ) as i32;
            if !cause.is_null() {
                cmdq_error!(item, "height {}", _s(cause));
                free_(cause);
                return cmd_retval::CMD_RETURN_ERROR;
            }

            let status: i32 = options_get_number___(&*(*w).options, "pane-border-status");
            match pane_status::try_from(status) {
                Ok(pane_status::PANE_STATUS_TOP) => {
                    if y != i32::MAX && (*wp).yoff == 1 {
                        y += 1;
                    }
                }
                Ok(pane_status::PANE_STATUS_BOTTOM) => {
                    if y != i32::MAX && (*wp).yoff + (*wp).sy == (*w).sy - 1 {
                        y += 1;
                    }
                }
                Ok(pane_status::PANE_STATUS_OFF) | Err(_) => (),
            }
            if window_pane_is_floating(wp) != 0 {
                if layout_resize_floating_pane_to(
                    wp,
                    layout_type::LAYOUT_TOPBOTTOM,
                    y as u32,
                    &raw mut cause,
                ) != 0
                {
                    cmdq_error!(item, "size {}", _s(cause));
                    free_(cause);
                    return cmd_retval::CMD_RETURN_ERROR;
                }
            } else {
                layout_resize_pane_to(wp, layout_type::LAYOUT_TOPBOTTOM, y as u32);
            }
        }

        // `opposite` is deliberately not reset between flags, matching the C.
        let mut opposite = 0;
        for flag in ['U', 'D', 'L', 'R'] {
            if !args_has(args, flag) {
                continue;
            }

            // `-U`/`-D`/`-L`/`-R` take an optional argument; without one the
            // adjustment comes from the trailing operand, defaulting to 1.
            let mut argval = args_get(args, flag as u8);
            if argval.is_null() {
                argval = if args_count(args) == 0 {
                    c"1".as_ptr().cast()
                } else {
                    args_string(args, 0)
                };
            }
            let adjust = match strtonum(argval, i32::MIN, i32::MAX) {
                Ok(n) => n,
                Err(errstr) => {
                    cmdq_error!(item, "adjustment {}", _s(errstr.as_ptr()));
                    return cmd_retval::CMD_RETURN_ERROR;
                }
            };

            let type_ = if flag == 'L' || flag == 'R' {
                layout_type::LAYOUT_LEFTRIGHT
            } else {
                layout_type::LAYOUT_TOPBOTTOM
            };

            if window_pane_is_floating(wp) != 0 {
                if flag == 'L' || flag == 'U' {
                    opposite = 1;
                }
                if layout_resize_floating_pane(wp, type_, adjust, opposite, &raw mut cause) != 0 {
                    cmdq_error!(item, "adjustment {}", _s(cause));
                    free_(cause);
                    return cmd_retval::CMD_RETURN_ERROR;
                }
            } else {
                let adjust = if flag == 'L' || flag == 'U' {
                    -adjust
                } else {
                    adjust
                };
                layout_resize_pane(wp, type_, adjust, 1);
            }
        }

        if !(*lc).parent.is_null() {
            layout_fix_offsets(w);
        }
        layout_fix_panes(w, null_mut());
        notify_window(c"window-layout-changed", w);
        server_redraw_window(w);
    }

    cmd_retval::CMD_RETURN_NORMAL
}

/// C `vendor/tmux/cmd-resize-pane.c:192`: `static enum cmd_retval cmd_resize_pane_mouse_update(__unused struct cmd *self, struct cmdq_item *item)`
///
/// Arm the drag handler: floating panes resize or move by their own border,
/// tiled panes resize the layout cells the border belongs to.
unsafe fn cmd_resize_pane_mouse_update(item: *mut cmdq_item) -> cmd_retval {
    unsafe {
        let target = cmdq_get_target(item);
        let event = cmdq_get_event(item);
        let wl = (*target).wl;
        let w = (*wl).window;
        let c = cmdq_get_client(item);
        let mut s = (*target).s;

        if !(*event).m.valid {
            return cmd_retval::CMD_RETURN_NORMAL;
        }
        let Some(wp) = cmd_mouse_pane(&raw mut (*event).m, &raw mut s, null_mut()) else {
            return cmd_retval::CMD_RETURN_NORMAL;
        };
        let wp = wp.as_ptr();
        if c.is_null() || (*c).session != s {
            return cmd_retval::CMD_RETURN_NORMAL;
        }

        if window_pane_is_floating(wp) == 0 {
            (*c).tty.mouse_drag_update = Some(cmd_resize_pane_mouse_resize_tiled);
            cmd_resize_pane_mouse_resize_tiled(c, &raw mut (*event).m);
            return cmd_retval::CMD_RETURN_NORMAL;
        }

        window_redraw_active_switch(w, wp);
        window_set_active_pane(w, wp, 1);

        (*c).tty.mouse_drag_update = Some(cmd_resize_pane_mouse_resize_move_floating);
        cmd_resize_pane_mouse_resize_move_floating(c, &raw mut (*event).m);
        cmd_retval::CMD_RETURN_NORMAL
    }
}

/// C `vendor/tmux/cmd-resize-pane.c:230`: `static void cmd_resize_pane_mouse_resize_move_floating(struct client *c, struct mouse_event *m)`
///
/// Resizes or moves the pane by dragging. Resize a floating pane by dragging
/// the borders or corners. Grabbing an edge only resizes that axis (special
/// case). Moves the pane if dragging the top border. Since characters are
/// generally rectangular, to make it easier to grab the corner, the character
/// next to the corner is also considered the corner.
///
/// ztmux has no pane scrollbars, so the C's scrollbar-reserve adjustment of
/// `left`/`right` (cmd-resize-pane.c:252-258) has no counterpart here.
unsafe fn cmd_resize_pane_mouse_resize_move_floating(c: *mut client, m: *mut mouse_event) {
    unsafe {
        let mut wl: *mut winlink = null_mut();
        let mut resizes = 0;

        let Some(wp) = cmd_mouse_pane(m, null_mut(), &raw mut wl) else {
            (*c).tty.mouse_drag_update = None;
            return;
        };
        let wp = wp.as_ptr();
        let w = (*wl).window;
        let lc = (*wp).layout_cell;
        let sx = (*wp).sx as c_int;
        let sy = (*wp).sy as c_int;
        let left = (*wp).xoff as c_int - 1;
        let right = (*wp).xoff as c_int + sx;
        let top = (*wp).yoff as c_int - 1;
        let bottom = (*wp).yoff as c_int + sy;

        let (x, y) = cmd_mouse_position((*m).x as c_int, (*m).y as c_int, m);
        let (lx, ly) = cmd_mouse_position((*m).lx as c_int, (*m).ly as c_int, m);

        let clamp = |v: c_int| v.max(PANE_MINIMUM as c_int);

        if (lx == left || lx == left + 1) && ly == top {
            // Top left corner.
            let new_sx = clamp((*lc).sx as c_int + (lx - x));
            let new_sy = clamp((*lc).sy as c_int + (ly - y));
            // The mouse sits on the border at xoff - 1, hence the +1.
            layout_set_size(lc, new_sx as u32, new_sy as u32, (x + 1) as u32, (y + 1) as u32);
            resizes += 1;
        } else if (lx == right + 1 || lx == right) && ly == top {
            // Top right corner.
            let new_sx = clamp(x - (*lc).xoff as c_int);
            let new_sy = clamp((*lc).sy as c_int + (ly - y));
            layout_set_size(lc, new_sx as u32, new_sy as u32, (*lc).xoff, (y + 1) as u32);
            resizes += 1;
        } else if (lx == left || lx == left + 1) && ly == bottom {
            // Bottom left corner.
            let new_sx = clamp((*lc).sx as c_int + (lx - x));
            let new_sy = y - (*lc).yoff as c_int;
            if new_sy < PANE_MINIMUM as c_int {
                return;
            }
            layout_set_size(lc, new_sx as u32, new_sy as u32, (x + 1) as u32, (*lc).yoff);
            resizes += 1;
        } else if (lx == right + 1 || lx == right) && ly == bottom {
            // Bottom right corner.
            let new_sx = clamp(x - (*lc).xoff as c_int);
            let new_sy = clamp(y - (*lc).yoff as c_int);
            layout_set_size(lc, new_sx as u32, new_sy as u32, (*lc).xoff, (*lc).yoff);
            resizes += 1;
        } else if lx == right {
            // Right border.
            let new_sx = x - (*lc).xoff as c_int;
            if new_sx < PANE_MINIMUM as c_int {
                return;
            }
            layout_set_size(lc, new_sx as u32, (*lc).sy, (*lc).xoff, (*lc).yoff);
            resizes += 1;
        } else if lx == left {
            // Left border.
            let new_sx = (*lc).sx as c_int + (lx - x);
            if new_sx < PANE_MINIMUM as c_int {
                return;
            }
            layout_set_size(lc, new_sx as u32, (*lc).sy, (x + 1) as u32, (*lc).yoff);
            resizes += 1;
        } else if ly == bottom {
            // Bottom border.
            let new_sy = y - (*lc).yoff as c_int;
            if new_sy < PANE_MINIMUM as c_int {
                return;
            }
            layout_set_size(lc, (*lc).sx, new_sy as u32, (*lc).xoff, (*lc).yoff);
            resizes += 1;
        } else if ly == top {
            // Top border (move instead of resize).
            let new_xoff = (*lc).xoff as c_int + (x - lx);
            layout_set_size(lc, (*lc).sx, (*lc).sy, new_xoff as u32, (y + 1) as u32);
            resizes += 1;
        }

        if resizes != 0 {
            layout_fix_panes(w, null_mut());
            server_redraw_window(w);
            server_redraw_window_borders(w);
        }
    }
}

/// C `vendor/tmux/cmd-resize-pane.c:355`: `static void cmd_resize_pane_mouse_resize_tiled(struct client *c, struct mouse_event *m)`
unsafe fn cmd_resize_pane_mouse_resize_tiled(c: *mut client, m: *mut mouse_event) {
    unsafe {
        let mut y: u32;
        let mut ly: u32;

        const OFFSETS: [[c_int; 2]; 5] = [[0, 0], [0, 1], [1, 0], [0, -1], [-1, 0]];
        let mut ncells: u32 = 0;
        let mut cells: [*mut layout_cell; OFFSETS.len()] = zeroed();
        let mut resizes: u32 = 0;

        let wl: *mut winlink = transmute_ptr(cmd_mouse_window(m, null_mut()));
        if wl.is_null() {
            (*c).tty.mouse_drag_update = None;
            return;
        }
        let w: *mut window = (*wl).window;

        y = (*m).y + (*m).oy;
        let x: u32 = (*m).x + (*m).ox;
        if (*m).statusat == 0 && y >= (*m).statuslines {
            y -= (*m).statuslines;
        } else if (*m).statusat > 0 && y >= (*m).statusat as u32 {
            y = ((*m).statusat - 1) as u32;
        }
        ly = (*m).ly + (*m).oy;
        let lx: u32 = (*m).lx + (*m).ox;
        if (*m).statusat == 0 && ly >= (*m).statuslines {
            ly -= (*m).statuslines;
        } else if (*m).statusat > 0 && ly >= (*m).statusat as u32 {
            ly = ((*m).statusat - 1) as u32;
        }

        for offset in OFFSETS {
            let mut lc = layout_search_by_border(
                (*w).layout_root,
                (lx as i32 + offset[0]).max(0) as u32,
                (ly as i32 + offset[1]).max(0) as u32,
            );
            if lc.is_null() {
                continue;
            }

            for j in 0..ncells {
                if cells[j as usize] == lc {
                    lc = null_mut();
                    break;
                }
            }
            if lc.is_null() {
                continue;
            }

            cells[ncells as usize] = lc;
            ncells += 1;
        }
        if ncells == 0 {
            return;
        }

        for i in 0..ncells {
            let type_ = (*(*cells[i as usize]).parent).type_;
            if y != ly && type_ == layout_type::LAYOUT_TOPBOTTOM {
                layout_resize_layout(w, cells[i as usize], type_, y as i32 - ly as i32, 0);
                resizes += 1;
            } else if x != lx && type_ == layout_type::LAYOUT_LEFTRIGHT {
                layout_resize_layout(w, cells[i as usize], type_, x as i32 - lx as i32, 0);
                resizes += 1;
            }
        }
        if resizes != 0 {
            server_redraw_window(w);
        }
    }
}
