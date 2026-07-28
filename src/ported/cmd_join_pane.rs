// Copyright (c) 2011 George Nachman <tmux@georgester.com>
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
use crate::compat::queue::{tailq_insert_after, tailq_insert_before, tailq_remove};
use crate::*;
use crate::options_::options_set_parent;

pub static CMD_JOIN_PANE_ENTRY: cmd_entry = cmd_entry {
    name: "join-pane",
    alias: Some("joinp"),

    args: args_parse::new("bdfhvp:l:s:t:", 0, 0, None),
    usage: "[-bdfhv] [-l size] [-s src-pane] [-t dst-pane]",

    source: cmd_entry_flag::new(
        b's',
        cmd_find_type::CMD_FIND_PANE,
        cmd_find_flags::CMD_FIND_DEFAULT_MARKED,
    ),
    target: cmd_entry_flag::new(b't', cmd_find_type::CMD_FIND_PANE, cmd_find_flags::empty()),

    flags: cmd_flag::empty(),
    exec: cmd_join_pane_exec,
};

pub static CMD_MOVE_PANE_ENTRY: cmd_entry = cmd_entry {
    name: "move-pane",
    alias: Some("movep"),

    args: args_parse::new("bdfhMvl:L::P:R::s:t:U::X:Y:z:", 0, 0, None),
    usage: "[-bdfhMv] [-D lines] [-l size] [-L columns] [-P position] \
            [-R columns] [-s src-pane] [-t dst-pane] [-U lines] \
            [-X x-position] [-Y y-position] [-z z-index]",

    source: cmd_entry_flag::new(
        b's',
        cmd_find_type::CMD_FIND_PANE,
        cmd_find_flags::CMD_FIND_DEFAULT_MARKED,
    ),
    target: cmd_entry_flag::new(b't', cmd_find_type::CMD_FIND_PANE, cmd_find_flags::empty()),

    flags: cmd_flag::empty(),
    exec: cmd_join_pane_exec,
};

/// Place a floating pane at a named position, or restack it in the z-index.
/// C `vendor/tmux/cmd-join-pane.c:68`: `static enum cmd_retval cmd_join_pane_place(struct cmdq_item *item, struct winlink *wl, struct window_pane *wp, const char *position)`
unsafe fn cmd_join_pane_place(
    item: *mut cmdq_item,
    wl: *mut winlink,
    wp: *mut window_pane,
    position: &str,
) -> cmd_retval {
    unsafe {
        let w = (*wl).window;
        let lc = (*wp).layout_cell;
        let (wx, wy) = ((*w).sx as c_int, (*w).sy as c_int);
        let (px, py) = ((*lc).sx as c_int, (*lc).sy as c_int);
        let (mut xoff, mut yoff) = ((*lc).xoff as c_int, (*lc).yoff as c_int);
        let border =
            c_int::from(window_pane_get_pane_lines(wp) != pane_lines::PANE_LINES_NONE);
        let zi = &raw mut (*w).z_index;

        match position {
            "top-left" => {
                xoff = border;
                yoff = border;
            }
            "top-centre" | "top-center" => {
                xoff = (wx - px) / 2;
                yoff = border;
            }
            "top-right" => {
                xoff = wx - px - border;
                yoff = border;
            }
            "centre-left" | "center-left" => {
                xoff = border;
                yoff = (wy - py) / 2;
            }
            "centre" | "center" => {
                xoff = (wx - px) / 2;
                yoff = (wy - py) / 2;
            }
            "centre-right" | "center-right" => {
                xoff = wx - px - border;
                yoff = (wy - py) / 2;
            }
            "bottom-left" => {
                xoff = border;
                yoff = wy - py - border;
            }
            "bottom-centre" | "bottom-center" => {
                xoff = (wx - px) / 2;
                yoff = wy - py - border;
            }
            "bottom-right" => {
                xoff = wx - px - border;
                yoff = wy - py - border;
            }
            "top-left-centre" | "top-left-center" => {
                xoff = wx / 4 - px / 2;
                yoff = wy / 4 - py / 2;
            }
            "top-right-centre" | "top-right-center" => {
                xoff = (3 * wx) / 4 - px / 2;
                yoff = wy / 4 - py / 2;
            }
            "bottom-left-centre" | "bottom-left-center" => {
                xoff = wx / 4 - px / 2;
                yoff = (3 * wy) / 4 - py / 2;
            }
            "bottom-right-centre" | "bottom-right-center" => {
                xoff = (3 * wx) / 4 - px / 2;
                yoff = (3 * wy) / 4 - py / 2;
            }
            // The remaining positions restack rather than move. Floating panes
            // are held at the head of the z-index list, so "back" means just
            // before the first tiled pane.
            "front" => {
                tailq_remove::<_, discr_zentry>(zi, wp);
                tailq_insert_head::<_, discr_zentry>(zi, wp);
            }
            "back" => {
                tailq_remove::<_, discr_zentry>(zi, wp);
                match first_tiled_pane(w) {
                    Some(owp) => tailq_insert_before::<_, discr_zentry>(owp, wp),
                    None => tailq_insert_tail::<_, discr_zentry>(zi, wp),
                }
            }
            "forward" => {
                let owp = tailq_prev::<_, window_pane, discr_zentry>(wp);
                if !owp.is_null() {
                    tailq_remove::<_, discr_zentry>(zi, wp);
                    tailq_insert_before::<_, discr_zentry>(owp, wp);
                }
            }
            "backward" => {
                let owp = tailq_next::<_, window_pane, discr_zentry>(wp);
                if !owp.is_null() && window_pane_is_floating(owp) != 0 {
                    tailq_remove::<_, discr_zentry>(zi, wp);
                    tailq_insert_after::<_, discr_zentry>(zi, owp, wp);
                }
            }
            "forward-loop" => {
                let owp = tailq_prev::<_, window_pane, discr_zentry>(wp);
                tailq_remove::<_, discr_zentry>(zi, wp);
                if !owp.is_null() {
                    tailq_insert_before::<_, discr_zentry>(owp, wp);
                } else {
                    match first_tiled_pane(w) {
                        Some(owp) => tailq_insert_before::<_, discr_zentry>(owp, wp),
                        None => tailq_insert_tail::<_, discr_zentry>(zi, wp),
                    }
                }
            }
            "backward-loop" => {
                let owp = tailq_next::<_, window_pane, discr_zentry>(wp);
                tailq_remove::<_, discr_zentry>(zi, wp);
                if !owp.is_null() && window_pane_is_floating(owp) != 0 {
                    tailq_insert_after::<_, discr_zentry>(zi, owp, wp);
                } else {
                    tailq_insert_head::<_, discr_zentry>(zi, wp);
                }
            }
            _ => {
                cmdq_error!(item, "unknown position: {}", position);
                return cmd_retval::CMD_RETURN_ERROR;
            }
        }

        if xoff != (*lc).xoff as c_int || yoff != (*lc).yoff as c_int {
            (*lc).xoff = xoff as u32;
            (*lc).yoff = yoff as u32;
            layout_fix_panes(w, null_mut());
        }
        notify_window(c"window-layout-changed", w);
        server_redraw_window(w);

        cmd_retval::CMD_RETURN_NORMAL
    }
}

/// First non-floating pane in the z-index list, i.e. the point floating panes
/// are stacked in front of.
unsafe fn first_tiled_pane(w: *mut window) -> Option<*mut window_pane> {
    unsafe {
        tailq_foreach::<_, discr_zentry>(&raw mut (*w).z_index)
            .map(NonNull::as_ptr)
            .find(|&owp| window_pane_is_floating(owp) == 0)
    }
}

/// Move a floating pane to an absolute (`-X`/`-Y`) or relative (`-U`/`-D`/`-L`/
/// `-R`) offset.
/// C `vendor/tmux/cmd-join-pane.c:195`: `static enum cmd_retval cmd_join_pane_move(struct cmdq_item *item, struct args *args, struct winlink *wl, struct window_pane *wp)`
unsafe fn cmd_join_pane_move(
    item: *mut cmdq_item,
    args: *mut args,
    wl: *mut winlink,
    wp: *mut window_pane,
) -> cmd_retval {
    unsafe {
        let w = (*wl).window;
        let lc = (*wp).layout_cell;
        let mut cause: *mut u8 = null_mut();
        let mut xoff = (*lc).xoff as c_int;
        let mut yoff = (*lc).yoff as c_int;
        let lines = window_pane_get_pane_lines(wp);

        // -X/-Y are given in pane coordinates; the border occupies the row and
        // column before the pane, so shift past it when one is drawn.
        if args_has(args, 'X') {
            xoff = args_percentage_and_expand(
                args,
                b'X',
                -((*w).sx as i64),
                (*w).sx as i64,
                (*w).sx as i64,
                item,
                &raw mut cause,
            ) as c_int;
            if !cause.is_null() {
                cmdq_error!(item, "position {}", _s(cause));
                free_(cause);
                return cmd_retval::CMD_RETURN_ERROR;
            }
            if lines != pane_lines::PANE_LINES_NONE {
                xoff += 1;
            }
        }
        if args_has(args, 'Y') {
            yoff = args_percentage_and_expand(
                args,
                b'Y',
                -((*w).sy as i64),
                (*w).sy as i64,
                (*w).sy as i64,
                item,
                &raw mut cause,
            ) as c_int;
            if !cause.is_null() {
                cmdq_error!(item, "position {}", _s(cause));
                free_(cause);
                return cmd_retval::CMD_RETURN_ERROR;
            }
            if lines != pane_lines::PANE_LINES_NONE {
                yoff += 1;
            }
        }

        for flag in ['U', 'D', 'L', 'R'] {
            if !args_has(args, flag) {
                continue;
            }

            let mut argval = args_get(args, flag as u8);
            if argval.is_null() {
                argval = c"1".as_ptr().cast();
            }
            let adjust = match strtonum(argval, i32::MIN, i32::MAX) {
                Ok(n) => n,
                Err(errstr) => {
                    cmdq_error!(item, "offset {}", _s(errstr.as_ptr()));
                    return cmd_retval::CMD_RETURN_ERROR;
                }
            };

            match flag {
                'U' => yoff -= adjust,
                'D' => yoff += adjust,
                'L' => xoff -= adjust,
                _ => xoff += adjust,
            }
        }

        if xoff != (*lc).xoff as c_int || yoff != (*lc).yoff as c_int {
            (*lc).xoff = xoff as u32;
            (*lc).yoff = yoff as u32;
            layout_fix_panes(w, null_mut());
            notify_window(c"window-layout-changed", w);
            server_redraw_window(w);
        }

        cmd_retval::CMD_RETURN_NORMAL
    }
}

/// C `vendor/tmux/cmd-join-pane.c:266`: `static enum cmd_retval cmd_join_pane_mouse_update(struct cmdq_item *item)`
unsafe fn cmd_join_pane_mouse_update(item: *mut cmdq_item) -> cmd_retval {
    unsafe {
        let target = cmdq_get_target(item);
        let event = cmdq_get_event(item);
        let c = cmdq_get_client(item);
        let mut s = (*target).s;
        let mut wl: *mut winlink = null_mut();

        if !(*event).m.valid {
            return cmd_retval::CMD_RETURN_NORMAL;
        }
        let Some(wp) = cmd_mouse_pane(&raw mut (*event).m, &raw mut s, &raw mut wl) else {
            return cmd_retval::CMD_RETURN_NORMAL;
        };
        let wp = wp.as_ptr();
        if c.is_null() || (*c).session != s {
            return cmd_retval::CMD_RETURN_NORMAL;
        }
        if window_pane_is_floating(wp) == 0 {
            return cmd_retval::CMD_RETURN_NORMAL;
        }

        let w = (*wl).window;
        window_redraw_active_switch(w, wp);
        window_set_active_pane(w, wp, 1);

        (*c).tty.mouse_drag_update = Some(cmd_join_pane_mouse_move);
        cmd_join_pane_mouse_move(c, &raw mut (*event).m);
        cmd_retval::CMD_RETURN_NORMAL
    }
}

/// Drag a floating pane by its body: translate it by the mouse delta.
/// C `vendor/tmux/cmd-join-pane.c:294`: `static void cmd_join_pane_mouse_move(struct client *c, struct mouse_event *m)`
unsafe fn cmd_join_pane_mouse_move(c: *mut client, m: *mut mouse_event) {
    unsafe {
        let mut wl: *mut winlink = null_mut();

        let Some(wp) = cmd_mouse_pane(m, null_mut(), &raw mut wl) else {
            (*c).tty.mouse_drag_update = None;
            return;
        };
        let wp = wp.as_ptr();
        let w = (*wl).window;
        let lc = (*wp).layout_cell;

        let (x, y) = cmd_mouse_position((*m).x as c_int, (*m).y as c_int, m);
        let (lx, ly) = cmd_mouse_position((*m).lx as c_int, (*m).ly as c_int, m);

        if x != lx || y != ly {
            (*lc).xoff = ((*lc).xoff as c_int + (x - lx)) as u32;
            (*lc).yoff = ((*lc).yoff as c_int + (y - ly)) as u32;
            layout_fix_panes(w, null_mut());
            server_redraw_window(w);
            server_redraw_window_borders(w);
        }
    }
}

/// Restack a floating pane to an explicit z-index, counting from the front.
/// C `vendor/tmux/cmd-join-pane.c:331`: `static enum cmd_retval cmd_join_pane_zindex(struct cmdq_item *item, struct winlink *wl, struct window_pane *wp, const char *s)`
unsafe fn cmd_join_pane_zindex(
    item: *mut cmdq_item,
    wl: *mut winlink,
    wp: *mut window_pane,
    s: *const u8,
) -> cmd_retval {
    unsafe {
        let w = (*wl).window;
        let zi = &raw mut (*w).z_index;

        let z = match strtonum(s, 0, u32::MAX) {
            Ok(n) => n,
            Err(errstr) => {
                cmdq_error!(item, "z-index {}", _s(errstr.as_ptr()));
                return cmd_retval::CMD_RETURN_ERROR;
            }
        };
        tailq_remove::<_, discr_zentry>(zi, wp);

        let mut n: u32 = 0;
        let mut before: Option<*mut window_pane> = None;
        for owp in tailq_foreach::<_, discr_zentry>(zi).map(NonNull::as_ptr) {
            if window_pane_is_floating(owp) == 0 || n >= z {
                before = Some(owp);
                break;
            }
            n += 1;
        }

        match before {
            Some(owp) => tailq_insert_before::<_, discr_zentry>(owp, wp),
            None => tailq_insert_tail::<_, discr_zentry>(zi, wp),
        }

        notify_window(c"window-layout-changed", w);
        server_redraw_window(w);

        cmd_retval::CMD_RETURN_NORMAL
    }
}

/// C `vendor/tmux/cmd-join-pane.c:367`: `static enum cmd_retval cmd_join_pane_exec(struct cmd *self, struct cmdq_item *item)`
unsafe fn cmd_join_pane_exec(self_: *mut cmd, item: *mut cmdq_item) -> cmd_retval {
    unsafe {
        let args = cmd_get_args(self_);
        let current = cmdq_get_current(item);
        let target = cmdq_get_target(item);
        let source = cmdq_get_source(item);
        let mut cause = null_mut();
        let mut type_: layout_type;

        let mut curval: u32 = 0;

        let dst_s = (*target).s;
        let dst_wl = (*target).wl;
        let dst_wp = (*target).wp;
        let dst_w = (*dst_wl).window;
        let dst_idx = (*dst_wl).idx;
        server_unzoom_window(dst_w);

        // move-pane on a floating pane repositions or restacks it in place;
        // only the join-style forms below fall through to moving the source.
        if std::ptr::eq(cmd_get_entry(self_), &raw const CMD_MOVE_PANE_ENTRY) {
            if args_has(args, 'M') {
                return cmd_join_pane_mouse_update(item);
            }
            if window_pane_is_floating(dst_wp) == 0 {
                cmdq_error!(item, "pane is not floating");
                return cmd_retval::CMD_RETURN_ERROR;
            }
            let position = args_get(args, b'P');
            if !position.is_null() {
                return cmd_join_pane_place(item, dst_wl, dst_wp, &_s(position).to_string());
            }
            let zindex = args_get(args, b'z');
            if !zindex.is_null() {
                return cmd_join_pane_zindex(item, dst_wl, dst_wp, zindex);
            }
            if ['X', 'Y', 'U', 'D', 'L', 'R']
                .iter()
                .any(|&flag| args_has(args, flag))
            {
                return cmd_join_pane_move(item, args, dst_wl, dst_wp);
            }
        }

        let src_wl = (*source).wl;
        let src_wp = (*source).wp;
        let src_w = (*src_wl).window;
        server_unzoom_window(src_w);

        if src_wp == dst_wp {
            cmdq_error!(item, "source and target panes must be different");
            return cmd_retval::CMD_RETURN_ERROR;
        }

        type_ = layout_type::LAYOUT_TOPBOTTOM;
        if args_has(args, 'h') {
            type_ = layout_type::LAYOUT_LEFTRIGHT;
        }

        // If the 'p' flag is dropped then this bit can be moved into 'l'.
        if args_has(args, 'l') || args_has(args, 'p') {
            if args_has(args, 'f') {
                match type_ {
                    layout_type::LAYOUT_TOPBOTTOM => curval = (*dst_w).sy,
                    _ => curval = (*dst_w).sx,
                }
            } else {
                match type_ {
                    layout_type::LAYOUT_TOPBOTTOM => curval = (*dst_wp).sy,
                    _ => curval = (*dst_wp).sx,
                }
            }
        }

        let mut size: i32 = -1;
        if args_has(args, 'l') {
            size = args_percentage_and_expand(
                args,
                b'l',
                0,
                i32::MAX as i64,
                curval as i64,
                item,
                &raw mut cause,
            ) as _;
        } else if args_has(args, 'p') {
            size = args_strtonum_and_expand(args, b'l', 0, 100, item, &raw mut cause) as _;
            if cause.is_null() {
                size = curval as i32 * size / 100;
            }
        }
        if !cause.is_null() {
            cmdq_error!(item, "size {}", _s(cause));
            free_(cause);
            return cmd_retval::CMD_RETURN_ERROR;
        }

        let mut flags: spawn_flags = spawn_flags::empty();
        if args_has(args, 'b') {
            flags |= SPAWN_BEFORE;
        }
        if args_has(args, 'f') {
            flags |= SPAWN_FULLSIZE;
        }

        let lc: *mut layout_cell = layout_split_pane(dst_wp, type_, size, flags);
        if lc.is_null() {
            cmdq_error!(item, "create pane failed: pane too small");
            return cmd_retval::CMD_RETURN_ERROR;
        }

        layout_close_pane(src_wp);

        server_client_remove_pane(src_wp);
        window_lost_pane(src_w, src_wp);
        tailq_remove::<_, discr_entry>(&raw mut (*src_w).panes, src_wp);

        (*src_wp).window = dst_w;
        options_set_parent(&mut *(*src_wp).options, (*dst_w).options);
        (*src_wp).flags |= window_pane_flags::PANE_STYLECHANGED;
        if flags.intersects(SPAWN_BEFORE) {
            tailq_insert_before::<_, discr_entry>(dst_wp, src_wp);
        } else {
            tailq_insert_after::<_, discr_entry>(&raw mut (*dst_w).panes, dst_wp, src_wp);
        }
        layout_assign_pane(lc, src_wp, 0);
        colour_palette_from_option(Some(&mut (*src_wp).palette), (*src_wp).options);

        recalculate_sizes();

        server_redraw_window(src_w);
        server_redraw_window(dst_w);

        if !args_has(args, 'd') {
            window_set_active_pane(dst_w, src_wp, 1);
            session_select(dst_s, dst_idx);
            cmd_find_from_session(current, dst_s, cmd_find_flags::empty());
            server_redraw_session(dst_s);
        } else {
            server_status_session(dst_s);
        }

        if window_count_panes(src_w) == 0 {
            server_kill_window(src_w, 1);
        } else {
            notify_window(c"window-layout-changed", src_w);
        }
        notify_window(c"window-layout-changed", dst_w);

        cmd_retval::CMD_RETURN_NORMAL
    }
}
