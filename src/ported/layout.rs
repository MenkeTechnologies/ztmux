// Copyright (c) 2009 Nicholas Marriott <nicholas.marriott@gmail.com>
// Copyright (c) 2016 Stephen Kent <smkent@smkent.net>
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

/// C `vendor/tmux/layout.c:61`: `struct layout_cell *layout_create_cell(struct layout_cell *lcparent)`
pub unsafe fn layout_create_cell(lcparent: *mut layout_cell) -> *mut layout_cell {
    unsafe {
        let lc = Box::leak(Box::new(layout_cell {
            type_: layout_type::LAYOUT_WINDOWPANE,
            flags: 0,
            parent: lcparent,
            sx: u32::MAX,
            sy: u32::MAX,
            xoff: u32::MAX,
            yoff: u32::MAX,
            wp: null_mut(),
            cells: tailq_head {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            },
            entry: tailq_entry::default(),
        }));
        tailq_init(&raw mut lc.cells);

        lc
    }
}

/// C `vendor/tmux/layout.c:91`: `void layout_free_cell(struct layout_cell *lc, int only_nodes)`
///
/// When `only_nodes` is set, `LAYOUT_WINDOWPANE` (leaf) cells are left intact so
/// their panes keep the existing layout cell — the main-pane layouts rely on
/// this to reuse each pane's current size.
pub unsafe fn layout_free_cell(lc: *mut layout_cell, only_nodes: c_int) {
    unsafe {
        if lc.is_null() || (only_nodes != 0 && (*lc).type_ == layout_type::LAYOUT_WINDOWPANE) {
            return;
        }

        match (*lc).type_ {
            layout_type::LAYOUT_LEFTRIGHT | layout_type::LAYOUT_TOPBOTTOM => {
                let mut lcchild = tailq_first(&raw mut (*lc).cells);
                while !lcchild.is_null() {
                    let lcnext = tailq_next(lcchild);
                    if only_nodes == 0 || (*lcchild).type_ != layout_type::LAYOUT_WINDOWPANE {
                        tailq_remove(&raw mut (*lc).cells, lcchild);
                        layout_free_cell(lcchild, only_nodes);
                    }
                    lcchild = lcnext;
                }
            }
            layout_type::LAYOUT_WINDOWPANE => {
                if !(*lc).wp.is_null() {
                    (*(*(*lc).wp).layout_cell).parent = null_mut();
                    (*(*lc).wp).layout_cell = null_mut();
                }
            }
        }

        free_(lc);
    }
}

/// C `vendor/tmux/layout.c:263`: `int layout_cell_is_tiled(struct layout_cell *lc)`
///
/// A cell is tiled when it is a leaf (window pane) and not floating.
pub unsafe fn layout_cell_is_tiled(lc: *mut layout_cell) -> c_int {
    unsafe {
        let is_leaf = (*lc).type_ == layout_type::LAYOUT_WINDOWPANE;
        let is_floating = (*lc).flags & LAYOUT_CELL_FLOATING != 0;
        (is_leaf && !is_floating) as c_int
    }
}

/// C `vendor/tmux/layout.c:124`: `void layout_print_cell(struct layout_cell *lc, const char *hdr, u_int n)`
pub unsafe fn layout_print_cell(lc: *mut layout_cell, hdr: *const u8, n: u32) {
    unsafe {
        let type_str = match (*lc).type_ {
            layout_type::LAYOUT_LEFTRIGHT => c"LEFTRIGHT",
            layout_type::LAYOUT_TOPBOTTOM => c"TOPBOTTOM",
            layout_type::LAYOUT_WINDOWPANE => c"WINDOWPANE",
        };

        log_debug!(
            "{}:{}{:p} type {} [parent {:p}] wp={:p} [{},{} {}x{}]",
            _s(hdr),
            if n == 0 { "" } else { " " },
            lc as *mut c_void,
            type_str.to_string_lossy(),
            (*lc).parent as *mut c_void,
            (*lc).wp as *mut c_void,
            (*lc).xoff,
            (*lc).yoff,
            (*lc).sx,
            (*lc).sy,
        );

        match (*lc).type_ {
            layout_type::LAYOUT_LEFTRIGHT | layout_type::LAYOUT_TOPBOTTOM => {
                for lcchild in tailq_foreach(&raw mut (*lc).cells) {
                    layout_print_cell(lcchild.as_ptr(), hdr, n + 1);
                }
            }
            layout_type::LAYOUT_WINDOWPANE => (),
        }
    }
}

/// C `vendor/tmux/layout.c:162`: `struct layout_cell *layout_search_by_border(struct layout_cell *lc, u_int x, u_int y)`
pub unsafe fn layout_search_by_border(lc: *mut layout_cell, x: u32, y: u32) -> *mut layout_cell {
    unsafe {
        let mut last: *mut layout_cell = null_mut();

        for lcchild in tailq_foreach(&raw mut (*lc).cells) {
            let lcchild = lcchild.as_ptr();

            if x >= (*lcchild).xoff
                && x < (*lcchild).xoff + (*lcchild).sx
                && y >= (*lcchild).yoff
                && y < (*lcchild).yoff + (*lcchild).sy
            {
                // Inside the cell - recurse
                return layout_search_by_border(lcchild, x, y);
            }

            if last.is_null() {
                last = lcchild;
                continue;
            }

            match (*lc).type_ {
                layout_type::LAYOUT_LEFTRIGHT => {
                    if x < (*lcchild).xoff && x >= (*last).xoff + (*last).sx {
                        return last;
                    }
                }
                layout_type::LAYOUT_TOPBOTTOM => {
                    if y < (*lcchild).yoff && y >= (*last).yoff + (*last).sy {
                        return last;
                    }
                }
                layout_type::LAYOUT_WINDOWPANE => (),
            }

            last = lcchild;
        }

        null_mut()
    }
}

/// C `vendor/tmux/layout.c:203`: `void layout_set_size(struct layout_cell *lc, u_int sx, u_int sy, int xoff, int yoff)`
pub unsafe fn layout_set_size(lc: *mut layout_cell, sx: u32, sy: u32, xoff: u32, yoff: u32) {
    unsafe {
        (*lc).sx = sx;
        (*lc).sy = sy;
        (*lc).xoff = xoff;
        (*lc).yoff = yoff;
    }
}

/// C `vendor/tmux/layout.c:214`: `void layout_make_leaf(struct layout_cell *lc, struct window_pane *wp)`
pub unsafe fn layout_make_leaf(lc: *mut layout_cell, wp: *mut window_pane) {
    unsafe {
        (*lc).type_ = layout_type::LAYOUT_WINDOWPANE;
        tailq_init(&raw mut (*lc).cells);
        (*wp).layout_cell = lc;
        (*lc).wp = wp;
    }
}

/// C `vendor/tmux/layout.c:226`: `void layout_make_node(struct layout_cell *lc, enum layout_type type)`
pub unsafe fn layout_make_node(lc: *mut layout_cell, type_: layout_type) {
    unsafe {
        if type_ == layout_type::LAYOUT_WINDOWPANE {
            fatalx("bad layout type");
        }
        (*lc).type_ = type_;
        tailq_init(&raw mut (*lc).cells);

        if !(*lc).wp.is_null() {
            (*(*lc).wp).layout_cell = null_mut();
        }
        (*lc).wp = null_mut();
    }
}

/// Fix cell offsets for a child cell.
/// C `vendor/tmux/layout.c:328`: `static void layout_fix_offsets1(struct layout_cell *lc)`
unsafe fn layout_fix_offsets1(lc: *mut layout_cell) {
    unsafe {
        if (*lc).type_ == layout_type::LAYOUT_LEFTRIGHT {
            let mut xoff = (*lc).xoff;
            for lcchild in tailq_foreach(&raw mut (*lc).cells) {
                let lcchild = lcchild.as_ptr();
                (*lcchild).xoff = xoff;
                (*lcchild).yoff = (*lc).yoff;
                if (*lcchild).type_ != layout_type::LAYOUT_WINDOWPANE {
                    layout_fix_offsets1(lcchild);
                }
                xoff += (*lcchild).sx + 1;
            }
        } else {
            let mut yoff = (*lc).yoff;
            for lcchild in tailq_foreach(&raw mut (*lc).cells) {
                let lcchild = lcchild.as_ptr();
                (*lcchild).xoff = (*lc).xoff;
                (*lcchild).yoff = yoff;
                if (*lcchild).type_ != layout_type::LAYOUT_WINDOWPANE {
                    layout_fix_offsets1(lcchild);
                }
                yoff += (*lcchild).sy + 1;
            }
        }
    }
}

/// Update cell offsets based on their sizes.
/// C `vendor/tmux/layout.c:362`: `void layout_fix_offsets(struct window *w)`
pub unsafe fn layout_fix_offsets(w: *mut window) {
    unsafe {
        let lc = (*w).layout_root;
        (*lc).xoff = 0;
        (*lc).yoff = 0;
        layout_fix_offsets1(lc);
    }
}

/// Is this a top cell?
/// C `vendor/tmux/layout.c:378`: `static int layout_cell_is_top(struct window *w, struct layout_cell *lc)`
unsafe fn layout_cell_is_top(w: *mut window, mut lc: *mut layout_cell) -> c_int {
    unsafe {
        while lc != (*w).layout_root {
            let next = (*lc).parent;
            if (*next).type_ == layout_type::LAYOUT_TOPBOTTOM
                && lc != tailq_first(&raw mut (*next).cells)
            {
                return 0;
            }
            lc = next;
        }
        1
    }
}

/// Is this a bottom cell?
/// C `vendor/tmux/layout.c:396`: `static int layout_cell_is_bottom(struct window *w, struct layout_cell *lc)`
unsafe fn layout_cell_is_bottom(w: *mut window, mut lc: *mut layout_cell) -> c_int {
    unsafe {
        while lc != (*w).layout_root {
            let next = (*lc).parent;
            if (*next).type_ == layout_type::LAYOUT_TOPBOTTOM
                && lc != tailq_last(&raw mut (*next).cells)
            {
                return 0;
            }
            lc = next;
        }
        1
    }
}

/// Returns 1 if we need to add an extra line for the pane status line. This is
/// the case for the most upper or lower panes only.
unsafe fn layout_add_border(w: *mut window, lc: *mut layout_cell, status: pane_status) -> bool {
    unsafe {
        if status == pane_status::PANE_STATUS_TOP {
            return layout_cell_is_top(w, lc) != 0;
        }
        if status == pane_status::PANE_STATUS_BOTTOM {
            return layout_cell_is_bottom(w, lc) != 0;
        }
        false
    }
}

/// Update pane offsets and sizes based on their cells.
/// C `vendor/tmux/layout.c:436`: `void layout_fix_panes(struct window *w, struct window_pane *skip)`
pub unsafe fn layout_fix_panes(w: *mut window, skip: *mut window_pane) {
    unsafe {
        let status: pane_status =
            pane_status::try_from(options_get_number_((*w).options, "pane-border-status") as i32)
                .unwrap();

        for wp in tailq_foreach::<window_pane, discr_entry>(&raw mut (*w).panes) {
            let wp = wp.as_ptr();
            let lc = (*wp).layout_cell;
            if lc.is_null() || wp == skip {
                continue;
            }

            (*wp).xoff = (*lc).xoff;
            (*wp).yoff = (*lc).yoff;

            // ztmux: reserve a 1-cell ring around the pane for its zellij-style
            // frame, so a program can never draw on the frame (inset == 0 unless
            // `@ztmux-pane-names on`, leaving the parity path untouched).
            let inset = crate::extensions::ratatui_ui::frame_inset();

            if layout_add_border(w, lc, status) {
                if status == pane_status::PANE_STATUS_TOP {
                    (*wp).yoff += 1;
                }
                window_pane_resize(wp, (*lc).sx, (*lc).sy - 1);
            } else if inset != 0 && (*lc).sx > 2 * inset && (*lc).sy > 2 * inset {
                (*wp).xoff += inset;
                (*wp).yoff += inset;
                window_pane_resize(wp, (*lc).sx - 2 * inset, (*lc).sy - 2 * inset);
            } else {
                window_pane_resize(wp, (*lc).sx, (*lc).sy);
            }
        }
    }
}

/// Count the number of available cells in a layout.
/// C `vendor/tmux/layout.c:505`: `u_int layout_count_cells(struct layout_cell *lc)`
pub unsafe fn layout_count_cells(lc: *mut layout_cell) -> u32 {
    unsafe {
        match (*lc).type_ {
            layout_type::LAYOUT_WINDOWPANE => 1,
            layout_type::LAYOUT_LEFTRIGHT | layout_type::LAYOUT_TOPBOTTOM => {
                let mut count = 0;
                for lcchild in tailq_foreach(&raw mut (*lc).cells) {
                    count += layout_count_cells(lcchild.as_ptr());
                }
                count
            }
        }
    }
}

/// Calculate how much size is available to be removed from a cell.
/// C `vendor/tmux/layout.c:525`: `static u_int layout_resize_check(struct window *w, struct layout_cell *lc, enum layout_type type)`
pub unsafe fn layout_resize_check(w: *mut window, lc: *mut layout_cell, type_: layout_type) -> u32 {
    unsafe {
        let mut available: u32;
        let mut minimum: u32;

        let status: pane_status =
            pane_status::try_from(options_get_number_((*w).options, "pane-border-status") as i32)
                .unwrap();

        if (*lc).type_ == layout_type::LAYOUT_WINDOWPANE {
            // Space available in this cell only.
            if type_ == layout_type::LAYOUT_LEFTRIGHT {
                available = (*lc).sx;
                minimum = PANE_MINIMUM;
            } else {
                available = (*lc).sy;
                if layout_add_border(w, lc, status) {
                    minimum = PANE_MINIMUM + 1;
                } else {
                    minimum = PANE_MINIMUM;
                }
            }
            if available > minimum {
                available -= minimum;
            } else {
                available = 0;
            }
        } else if (*lc).type_ == type_ {
            // Same type: total of available space in all child cells.
            available = 0;
            for lcchild in tailq_foreach(&raw mut (*lc).cells) {
                available += layout_resize_check(w, lcchild.as_ptr(), type_);
            }
        } else {
            // Different type: minimum of available space in child cells.
            minimum = u32::MAX;
            for lcchild in tailq_foreach(&raw mut (*lc).cells) {
                available = layout_resize_check(w, lcchild.as_ptr(), type_);
                if available < minimum {
                    minimum = available;
                }
            }
            available = minimum;
        }

        available
    }
}

/// Adjust cell size evenly, including altering its children. This function
/// expects the change to have already been bounded to the space available.
/// C `vendor/tmux/layout.c:579`: `void layout_resize_adjust(struct window *w, struct layout_cell *lc, enum layout_type type, int change)`
pub unsafe fn layout_resize_adjust(
    w: *mut window,
    lc: *mut layout_cell,
    type_: layout_type,
    mut change: i32,
) {
    unsafe {
        // Adjust the cell size
        if type_ == layout_type::LAYOUT_LEFTRIGHT {
            (*lc).sx = ((*lc).sx as i32 + change) as u32;
        } else {
            (*lc).sy = ((*lc).sy as i32 + change) as u32;
        }

        // If this is a leaf cell, that is all that is necessary
        if (*lc).type_ == layout_type::LAYOUT_WINDOWPANE {
            return;
        }

        // Child cell runs in a different direction
        if (*lc).type_ != type_ {
            for lcchild in tailq_foreach(&raw mut (*lc).cells) {
                layout_resize_adjust(w, lcchild.as_ptr(), type_, change);
            }
            return;
        }

        // Child cell runs in the same direction. Adjust each child equally
        // until no further change is possible
        while change != 0 {
            for lcchild in tailq_foreach(&raw mut (*lc).cells) {
                if change == 0 {
                    break;
                }
                if change > 0 {
                    layout_resize_adjust(w, lcchild.as_ptr(), type_, 1);
                    change -= 1;
                    continue;
                }
                if layout_resize_check(w, lcchild.as_ptr(), type_) > 0 {
                    layout_resize_adjust(w, lcchild.as_ptr(), type_, -1);
                    change += 1;
                }
            }
        }
    }
}

/// Destroy a cell and redistribute the space.
/// C `vendor/tmux/layout.c:695`: `void layout_destroy_cell(struct window *w, struct layout_cell *lc, struct layout_cell **lcroot)`
pub unsafe fn layout_destroy_cell(
    w: *mut window,
    lc: *mut layout_cell,
    lcroot: *mut *mut layout_cell,
) {
    unsafe {
        let lcparent = (*lc).parent;

        // If no parent, this is the last pane so window close is imminent and
        // there is no need to resize anything.
        if lcparent.is_null() {
            layout_free_cell(lc, 0);
            *lcroot = std::ptr::null_mut();
            return;
        }

        // Merge the space into the previous or next cell
        let lcother: *mut layout_cell = if lc == tailq_first(&raw mut (*lcparent).cells) {
            tailq_next(lc)
        } else {
            tailq_prev(lc)
        };

        if !lcother.is_null() {
            if (*lcparent).type_ == layout_type::LAYOUT_LEFTRIGHT {
                layout_resize_adjust(w, lcother, (*lcparent).type_, (*lc).sx as i32 + 1);
            } else {
                layout_resize_adjust(w, lcother, (*lcparent).type_, (*lc).sy as i32 + 1);
            }
        }

        // Remove this from the parent's list
        tailq_remove(&mut (*lcparent).cells, lc);
        layout_free_cell(lc, 0);

        // If the parent now has one cell, remove the parent from the tree and
        // replace it by that cell
        let lc = tailq_first(&raw mut (*lcparent).cells);
        if tailq_next(lc).is_null() {
            tailq_remove(&raw mut (*lcparent).cells, lc);

            (*lc).parent = (*lcparent).parent;
            if (*lc).parent.is_null() {
                (*lc).xoff = 0;
                (*lc).yoff = 0;
                *lcroot = lc;
            } else {
                tailq_replace(&mut (*(*lc).parent).cells, lcparent, lc);
            }

            layout_free_cell(lcparent, 0);
        }
    }
}

/// C `vendor/tmux/layout.c:755`: `void layout_init(struct window *w, struct window_pane *wp)`
pub unsafe fn layout_init(w: *mut window, wp: *mut window_pane) {
    unsafe {
        let lc = layout_create_cell(std::ptr::null_mut());
        (*w).layout_root = lc;
        layout_set_size(lc, (*w).sx, (*w).sy, 0, 0);
        layout_make_leaf(lc, wp);
        layout_fix_panes(w, std::ptr::null_mut());
    }
}

/// C `vendor/tmux/layout.c:767`: `void layout_free(struct window *w, int only_nodes)`
pub unsafe fn layout_free(w: *mut window, only_nodes: c_int) {
    unsafe {
        layout_free_cell((*w).layout_root, only_nodes);
    }
}

/// Resize the entire layout after window resize.
/// C `vendor/tmux/layout.c:774`: `void layout_resize(struct window *w, u_int sx, u_int sy)`
pub unsafe fn layout_resize(w: *mut window, sx: c_uint, sy: c_uint) {
    unsafe {
        let lc = (*w).layout_root;

        // Adjust horizontally. Do not attempt to reduce the layout lower than
        // the minimum (more than the amount returned by layout_resize_check).
        //
        // This can mean that the window size is smaller than the total layout
        // size: redrawing this is handled at a higher level, but it does leave
        // a problem with growing the window size here: if the current size is
        // < the minimum, growing proportionately by adding to each pane is
        // wrong as it would keep the layout size larger than the window size.
        // Instead, spread the difference between the minimum and the new size
        // out proportionately - this should leave the layout fitting the new
        // window size.
        let mut xchange = sx as c_int - (*lc).sx as c_int;
        let xlimit = layout_resize_check(w, lc, layout_type::LAYOUT_LEFTRIGHT) as i32;
        if xchange < 0 && xchange < -xlimit {
            xchange = -xlimit;
        }
        if xlimit == 0 {
            if sx <= (*lc).sx {
                // lc->sx is minimum possible
                xchange = 0;
            } else {
                xchange = sx as c_int - (*lc).sx as c_int;
            }
        }
        if xchange != 0 {
            layout_resize_adjust(w, lc, layout_type::LAYOUT_LEFTRIGHT, xchange);
        }

        // Adjust vertically in a similar fashion.
        let mut ychange = sy as c_int - (*lc).sy as c_int;
        let ylimit = layout_resize_check(w, lc, layout_type::LAYOUT_TOPBOTTOM) as i32;
        if ychange < 0 && ychange < -ylimit {
            ychange = -ylimit;
        }
        if ylimit == 0 {
            if sy <= (*lc).sy {
                // lc->sy is minimum possible
                ychange = 0;
            } else {
                ychange = sy as c_int - (*lc).sy as c_int;
            }
        }
        if ychange != 0 {
            layout_resize_adjust(w, lc, layout_type::LAYOUT_TOPBOTTOM, ychange);
        }

        // Fix cell offsets.
        layout_fix_offsets(w);
        layout_fix_panes(w, std::ptr::null_mut());
    }
}

/// Resize a pane to an absolute size.
/// C `vendor/tmux/layout.c:828`: `void layout_resize_pane_to(struct window_pane *wp, enum layout_type type, u_int new_size)`
pub unsafe fn layout_resize_pane_to(wp: *mut window_pane, type_: layout_type, new_size: u32) {
    unsafe {
        let mut lc = (*wp).layout_cell;
        let mut lcparent;

        // Find next parent of the same type
        lcparent = (*lc).parent;
        while !lcparent.is_null() && (*lcparent).type_ != type_ {
            lc = lcparent;
            lcparent = (*lc).parent;
        }
        if lcparent.is_null() {
            return;
        }

        // Work out the size adjustment
        let size = if type_ == layout_type::LAYOUT_LEFTRIGHT {
            (*lc).sx
        } else {
            (*lc).sy
        };

        let change = if lc == tailq_last(&raw mut (*lcparent).cells) {
            size as i32 - new_size as i32
        } else {
            new_size as i32 - size as i32
        };

        // Resize the pane
        layout_resize_pane(wp, type_, change, 1);
    }
}

/// C `vendor/tmux/layout.c:932`: `void layout_resize_layout(struct window *w, struct layout_cell *lc, enum layout_type type, int change, int opposite)`
pub unsafe fn layout_resize_layout(
    w: *mut window,
    lc: *mut layout_cell,
    type_: layout_type,
    change: c_int,
    opposite: c_int,
) {
    unsafe {
        let mut needed = change;
        let mut size;

        // Grow or shrink the cell
        while needed != 0 {
            if change > 0 {
                size = layout_resize_pane_grow(w, lc, type_, needed, opposite);
                needed -= size;
            } else {
                size = layout_resize_pane_shrink(w, lc, type_, needed);
                needed += size;
            }

            if size == 0 {
                // no more change possible
                break;
            }
        }

        // Fix cell offsets
        layout_fix_offsets(w);
        layout_fix_panes(w, null_mut());
        notify_window(c"window-layout-changed", w);
    }
}

/// C `vendor/tmux/layout.c:961`: `void layout_resize_pane(struct window_pane *wp, enum layout_type type, int change, int opposite)`
pub unsafe fn layout_resize_pane(
    wp: *mut window_pane,
    type_: layout_type,
    change: c_int,
    opposite: c_int,
) {
    unsafe {
        let mut lc = (*wp).layout_cell;
        let mut lcparent;

        // Find next parent of the same type
        lcparent = (*lc).parent;
        while !lcparent.is_null() && (*lcparent).type_ != type_ {
            lc = lcparent;
            lcparent = (*lc).parent;
        }
        if lcparent.is_null() {
            return;
        }

        // If this is the last cell, move back one
        if lc == tailq_last(&raw mut (*lcparent).cells) {
            lc = tailq_prev(lc);
        }

        layout_resize_layout((*wp).window, lc, type_, change, opposite);
    }
}

/// Helper function to grow pane.
/// C `vendor/tmux/layout.c:987`: `static int layout_resize_pane_grow(struct window *w, struct layout_cell *lc, enum layout_type type, int needed, int opposite)`
pub unsafe fn layout_resize_pane_grow(
    w: *mut window,
    lc: *mut layout_cell,
    type_: layout_type,
    needed: c_int,
    opposite: c_int,
) -> c_int {
    unsafe {
        let mut size: u32 = 0;

        // Growing. Always add to the current cell
        let lcadd = lc;

        // Look towards the tail for a suitable cell for reduction
        let mut lcremove = tailq_next(lc);
        while !lcremove.is_null() {
            size = layout_resize_check(w, lcremove, type_);
            if size > 0 {
                break;
            }
            lcremove = tailq_next(lcremove);
        }

        // If none found, look towards the head
        if opposite != 0 && lcremove.is_null() {
            lcremove = tailq_prev(lc);
            while !lcremove.is_null() {
                size = layout_resize_check(w, lcremove, type_);
                if size > 0 {
                    break;
                }
                lcremove = tailq_prev(lcremove);
            }
        }
        if lcremove.is_null() {
            return 0;
        }

        // Change the cells
        if size > needed as u32 {
            size = needed as u32;
        }
        layout_resize_adjust(w, lcadd, type_, size as c_int);
        layout_resize_adjust(w, lcremove, type_, -(size as c_int));
        size as c_int
    }
}

/// Helper function to shrink pane.
/// C `vendor/tmux/layout.c:1028`: `static int layout_resize_pane_shrink(struct window *w, struct layout_cell *lc, enum layout_type type, int needed)`
pub unsafe fn layout_resize_pane_shrink(
    w: *mut window,
    lc: *mut layout_cell,
    type_: layout_type,
    needed: c_int,
) -> c_int {
    unsafe {
        let mut size: u32;

        // Shrinking. Find cell to remove from by walking towards head
        let mut lcremove = lc;
        loop {
            size = layout_resize_check(w, lcremove, type_);
            if size != 0 {
                break;
            }
            lcremove = tailq_prev(lcremove);
            if lcremove.is_null() {
                break;
            }
        }
        if lcremove.is_null() {
            return 0;
        }

        // And add onto the next cell (from the original cell)
        let lcadd = tailq_next(lc);
        if lcadd.is_null() {
            return 0;
        }

        // Change the cells
        if size > (-needed) as u32 {
            size = (-needed) as u32;
        }
        layout_resize_adjust(w, lcadd, type_, size as c_int);
        layout_resize_adjust(w, lcremove, type_, -(size as c_int));
        size as c_int
    }
}

/// Assign window pane to newly split cell.
/// C `vendor/tmux/layout.c:1060`: `void layout_assign_pane(struct layout_cell *lc, struct window_pane *wp, int do_not_resize)`
pub unsafe fn layout_assign_pane(lc: *mut layout_cell, wp: *mut window_pane, do_not_resize: c_int) {
    unsafe {
        layout_make_leaf(lc, wp);
        if do_not_resize != 0 {
            layout_fix_panes((*wp).window, wp);
        } else {
            layout_fix_panes((*wp).window, null_mut());
        }
    }
}

/// Calculate the new pane size for resized parent.
/// C `vendor/tmux/layout.c:1072`: `static u_int layout_new_pane_size(struct window *w, u_int previous, struct layout_cell *lc, enum layout_type type, u_int size, u_int count_left, u_int size_left)`
pub unsafe fn layout_new_pane_size(
    w: *mut window,
    previous: u32,
    lc: *mut layout_cell,
    type_: layout_type,
    size: u32,
    count_left: u32,
    size_left: u32,
) -> u32 {
    unsafe {
        // If this is the last cell, it can take all of the remaining size.
        if count_left == 1 {
            return size_left;
        }

        // How much is available in this parent?
        let available: u32 = layout_resize_check(w, lc, type_);

        // Work out the minimum size of this cell and the new size
        // proportionate to the previous size.
        let mut min: u32 = (PANE_MINIMUM + 1) * (count_left - 1);
        let mut new_size: u32 = if type_ == layout_type::LAYOUT_LEFTRIGHT {
            if (*lc).sx - available > min {
                min = (*lc).sx - available;
            }
            ((*lc).sx * size) / previous
        } else {
            if (*lc).sy - available > min {
                min = (*lc).sy - available;
            }
            ((*lc).sy * size) / previous
        };

        // Check against the maximum and minimum size.
        let max: u32 = size_left - min;
        if new_size > max {
            new_size = max;
        }
        if new_size < PANE_MINIMUM {
            new_size = PANE_MINIMUM;
        }
        new_size
    }
}

/// Check if the cell and all its children can be resized to a specific size.
/// C `vendor/tmux/layout.c:1110`: `static int layout_set_size_check(struct window *w, struct layout_cell *lc, enum layout_type type, int size)`
pub unsafe fn layout_set_size_check(
    w: *mut window,
    lc: *mut layout_cell,
    type_: layout_type,
    size: c_int,
) -> bool {
    unsafe {
        let mut new_size: u32;
        let mut available: u32;
        let previous: u32;
        let mut idx: u32;

        // Cells with no children must just be bigger than minimum
        if (*lc).type_ == layout_type::LAYOUT_WINDOWPANE {
            return size >= PANE_MINIMUM as i32;
        }
        available = size as u32;

        // Count number of children
        let count: u32 = tailq_foreach(&raw mut (*lc).cells).count() as u32;

        // Check new size will work for each child
        if (*lc).type_ == type_ {
            if available < (count * 2) - 1 {
                return false;
            }

            if type_ == layout_type::LAYOUT_LEFTRIGHT {
                previous = (*lc).sx;
            } else {
                previous = (*lc).sy;
            }

            idx = 0;
            for lcchild in tailq_foreach(&raw mut (*lc).cells).map(NonNull::as_ptr) {
                new_size = layout_new_pane_size(
                    w,
                    previous,
                    lcchild,
                    type_,
                    size as u32,
                    count - idx,
                    available,
                );
                if idx == count - 1 {
                    if new_size > available {
                        return false;
                    }
                    available -= new_size;
                } else {
                    if new_size + 1 > available {
                        return false;
                    }
                    available -= new_size + 1;
                }
                if !layout_set_size_check(w, lcchild, type_, new_size as i32) {
                    return false;
                }
                idx += 1;
            }
        } else {
            for lcchild in tailq_foreach(&raw mut (*lc).cells).map(NonNull::as_ptr) {
                if (*lcchild).type_ == layout_type::LAYOUT_WINDOWPANE {
                    continue;
                }
                if !layout_set_size_check(w, lcchild, type_, size) {
                    return false;
                }
            }
        }

        true
    }
}

// unsafe extern "C" { pub fn layout_resize_child_cells(w: *mut window, lc: *mut layout_cell); }
/// Resize all child cells to fit within the current cell.
/// C `vendor/tmux/layout.c:1167`: `static void layout_resize_child_cells(struct window *w, struct layout_cell *lc)`
pub unsafe fn layout_resize_child_cells(w: *mut window, lc: *mut layout_cell) {
    unsafe {
        if (*lc).type_ == layout_type::LAYOUT_WINDOWPANE {
            return;
        }

        // What is the current size used?
        let mut count: u32 = 0;
        let mut previous: u32 = 0;
        for lcchild in tailq_foreach(&raw mut (*lc).cells).map(NonNull::as_ptr) {
            count += 1;
            if (*lc).type_ == layout_type::LAYOUT_LEFTRIGHT {
                previous += (*lcchild).sx;
            } else if (*lc).type_ == layout_type::LAYOUT_TOPBOTTOM {
                previous += (*lcchild).sy;
            }
        }
        previous += count - 1;

        // And how much is available?
        let mut available: u32 = 0;
        if (*lc).type_ == layout_type::LAYOUT_LEFTRIGHT {
            available = (*lc).sx;
        } else if (*lc).type_ == layout_type::LAYOUT_TOPBOTTOM {
            available = (*lc).sy;
        }

        // Resize children into the new size.
        for (idx, lcchild) in tailq_foreach(&raw mut (*lc).cells)
            .map(NonNull::as_ptr)
            .enumerate()
        {
            if (*lc).type_ == layout_type::LAYOUT_TOPBOTTOM {
                (*lcchild).sx = (*lc).sx;
                (*lcchild).xoff = (*lc).xoff;
            } else {
                (*lcchild).sx = layout_new_pane_size(
                    w,
                    previous,
                    lcchild,
                    (*lc).type_,
                    (*lc).sx,
                    count - idx as u32,
                    available,
                );
                // C layout.c `available -= (lcchild->sx + 1)` on u_int: underflow
                // wraps (harmless — a transient over-subscription during a
                // full-size `-f` split is corrected by the follow-up resize).
                // Checked `-=` would panic, so wrap like C.
                available = available.wrapping_sub((*lcchild).sx + 1);
            }
            if (*lc).type_ == layout_type::LAYOUT_LEFTRIGHT {
                (*lcchild).sy = (*lc).sy;
            } else {
                (*lcchild).sy = layout_new_pane_size(
                    w,
                    previous,
                    lcchild,
                    (*lc).type_,
                    (*lc).sy,
                    count - idx as u32,
                    available,
                );
                // C layout.c `available -= (lcchild->sy + 1)` on u_int: wraps on
                // underflow (see the sx case above).
                available = available.wrapping_sub((*lcchild).sy + 1);
            }
            layout_resize_child_cells(w, lcchild);
        }
    }
}

/// C `vendor/tmux/layout.c:1229`: `struct layout_cell *layout_replace_with_node(struct window *w, struct layout_cell *lc, enum layout_type type)`
///
/// Wrap `lc` in a new parent node of `type`, replacing it in the tree.
// floating-pane API, consumed from the next phase (F2)
pub unsafe fn layout_replace_with_node(
    w: *mut window,
    lc: *mut layout_cell,
    type_: layout_type,
) -> *mut layout_cell {
    unsafe {
        let lcparent = layout_create_cell((*lc).parent);
        layout_make_node(lcparent, type_);
        layout_set_size(lcparent, (*lc).sx, (*lc).sy, (*lc).xoff, (*lc).yoff);
        if (*lc).parent.is_null() {
            (*w).layout_root = lcparent;
        } else {
            tailq_replace(&raw mut (*(*lc).parent).cells, lc, lcparent);
        }

        // Insert the old cell.
        (*lc).parent = lcparent;
        tailq_insert_head(&raw mut (*lcparent).cells, lc);

        lcparent
    }
}

/// C `vendor/tmux/layout.c:1456`: `struct layout_cell *layout_floating_pane(struct window *w, struct window_pane *wp, u_int sx, u_int sy, int ox, int oy)`
///
/// Create a floating cell (marked `LAYOUT_CELL_FLOATING`) beside `wp`'s cell.
// floating-pane API, consumed from the next phase (F2)
pub unsafe fn layout_floating_pane(
    w: *mut window,
    wp: *mut window_pane,
    sx: u32,
    sy: u32,
    ox: c_int,
    oy: c_int,
) -> *mut layout_cell {
    unsafe {
        let lc = if wp.is_null() {
            (*w).layout_root
        } else {
            (*wp).layout_cell
        };
        let mut lcparent = (*lc).parent;

        if lcparent.is_null() {
            // Adding a pane to a root that isn't a node. Create and insert a
            // new root node.
            lcparent = layout_replace_with_node(w, lc, layout_type::LAYOUT_TOPBOTTOM);
        }

        let lcnew = layout_create_cell(lcparent);
        tailq_insert_after(&raw mut (*lcparent).cells, lc, lcnew);
        (*lcnew).flags |= LAYOUT_CELL_FLOATING;
        layout_set_size(lcnew, sx, sy, ox as u32, oy as u32);

        lcnew
    }
}

/// C `vendor/tmux/layout.c:1671`: `int layout_floating_args_parse(struct cmdq_item *item, struct args *args, enum pane_lines lines, struct window *w, u_int *sxp, u_int *syp, int *oxp, int *oyp, char **cause)`
// floating-pane API, consumed from the next phase (F2)
pub unsafe fn layout_floating_args_parse(
    item: *mut cmdq_item,
    args: *mut args,
    lines: pane_lines,
    w: *mut window,
    sxp: *mut u32,
    syp: *mut u32,
    oxp: *mut c_int,
    oyp: *mut c_int,
    cause: *mut *mut u8,
) -> c_int {
    unsafe {
        let mut error: *mut u8 = null_mut();

        let mut sx: c_int = if *sxp == u32::MAX {
            ((*w).sx / 2) as c_int
        } else {
            *sxp as c_int
        };
        let mut sy: c_int = if *syp == u32::MAX {
            ((*w).sy / 4) as c_int
        } else {
            *syp as c_int
        };
        let mut ox: c_int = if *oxp == c_int::MAX { c_int::MAX } else { *oxp };
        let mut oy: c_int = if *oyp == c_int::MAX { c_int::MAX } else { *oyp };

        if args_has(args, 'x') {
            sx = args_percentage_and_expand(
                args, b'x', 0, PANE_MAXIMUM as i64, (*w).sx as i64, item, &raw mut error,
            ) as c_int;
            if !error.is_null() {
                *cause = format_nul!("position {}", _s(error));
                free_(error);
                return -1;
            }
            if lines != pane_lines::PANE_LINES_NONE {
                sx -= 2;
            }
        }
        if args_has(args, 'y') {
            sy = args_percentage_and_expand(
                args, b'y', 0, PANE_MAXIMUM as i64, (*w).sy as i64, item, &raw mut error,
            ) as c_int;
            if !error.is_null() {
                *cause = format_nul!("position {}", _s(error));
                free_(error);
                return -1;
            }
            if lines != pane_lines::PANE_LINES_NONE {
                sy -= 2;
            }
        }
        if args_has(args, 'X') {
            ox = args_percentage_and_expand(
                args, b'X', (-sx) as i64, (*w).sx as i64, (*w).sx as i64, item, &raw mut error,
            ) as c_int;
            if !error.is_null() {
                *cause = format_nul!("position {}", _s(error));
                free_(error);
                return -1;
            }
        }
        if args_has(args, 'Y') {
            oy = args_percentage_and_expand(
                args, b'Y', (-sy) as i64, (*w).sy as i64, (*w).sy as i64, item, &raw mut error,
            ) as c_int;
            if !error.is_null() {
                *cause = format_nul!("position {}", _s(error));
                free_(error);
                return -1;
            }
        }

        if ox == c_int::MAX {
            if (*w).last_new_pane_x == 0 {
                ox = 4;
            } else {
                ox = (*w).last_new_pane_x as c_int + 4;
                if (*w).last_new_pane_x > (*w).sx {
                    ox = 4;
                }
            }
            (*w).last_new_pane_x = ox as u32;
        } else if lines != pane_lines::PANE_LINES_NONE {
            ox += 1;
        }
        if oy == c_int::MAX {
            if (*w).last_new_pane_y == 0 {
                oy = 2;
            } else {
                oy = (*w).last_new_pane_y as c_int + 2;
                if (*w).last_new_pane_y > (*w).sy {
                    oy = 2;
                }
            }
            (*w).last_new_pane_y = oy as u32;
        } else if lines != pane_lines::PANE_LINES_NONE {
            oy += 1;
        }

        if sx < PANE_MINIMUM as c_int || sx > PANE_MAXIMUM as c_int {
            *cause = xstrdup(c!("invalid width")).as_ptr();
            return -1;
        }
        if sy < PANE_MINIMUM as c_int || sy > PANE_MAXIMUM as c_int {
            *cause = xstrdup(c!("invalid height")).as_ptr();
            return -1;
        }

        *sxp = sx as u32;
        *syp = sy as u32;
        *oxp = ox;
        *oyp = oy;
        0
    }
}

/// C `vendor/tmux/layout.c:1654`: `struct layout_cell *layout_get_floating_cell(struct cmdq_item *item, struct args *args, enum pane_lines lines, struct window *w, struct window_pane *wp, char **cause)`
// floating-pane API, consumed from the next phase (F2)
pub unsafe fn layout_get_floating_cell(
    item: *mut cmdq_item,
    args: *mut args,
    lines: pane_lines,
    w: *mut window,
    wp: *mut window_pane,
    cause: *mut *mut u8,
) -> *mut layout_cell {
    unsafe {
        let mut sx: u32 = u32::MAX;
        let mut sy: u32 = u32::MAX;
        let mut ox: c_int = c_int::MAX;
        let mut oy: c_int = c_int::MAX;

        if layout_floating_args_parse(
            item, args, lines, w, &raw mut sx, &raw mut sy, &raw mut ox, &raw mut oy, cause,
        ) != 0
        {
            return null_mut();
        }

        layout_floating_pane(w, wp, sx, sy, ox, oy)
    }
}

/// Split a pane into two. size is a hint, or -1 for default half/half
/// split. This must be followed by `layout_assign_pane` before much else happens!
/// C `vendor/tmux/layout.c:1323`: `struct layout_cell *layout_split_pane(struct window_pane *wp, enum layout_type type, int size, int flags)`
pub unsafe fn layout_split_pane(
    wp: *mut window_pane,
    type_: layout_type,
    size: i32,
    flags: spawn_flags,
) -> *mut layout_cell {
    unsafe {
        let minimum: u32;
        let mut resize_first: u32 = 0;
        let full_size = flags.intersects(SPAWN_FULLSIZE);

        // If full_size is specified, add a new cell at the top of the window
        // layout. Otherwise, split the cell for the current pane.
        let lc: *mut layout_cell = if full_size {
            (*(*wp).window).layout_root
        } else {
            (*wp).layout_cell
        };
        let status = pane_status::try_from(options_get_number_(
            (*(*wp).window).options,
            "pane-border-status",
        ) as i32)
        .unwrap();

        // Copy the old cell size
        let sx = (*lc).sx;
        let sy = (*lc).sy;
        let xoff = (*lc).xoff;
        let yoff = (*lc).yoff;

        // Check there is enough space for the two new panes
        match type_ {
            layout_type::LAYOUT_LEFTRIGHT => {
                if sx < PANE_MINIMUM * 2 + 1 {
                    return null_mut();
                }
            }
            layout_type::LAYOUT_TOPBOTTOM => {
                if layout_add_border((*wp).window, lc, status) {
                    minimum = PANE_MINIMUM * 2 + 2;
                } else {
                    minimum = PANE_MINIMUM * 2 + 1;
                }
                if sy < minimum {
                    return null_mut();
                }
            }
            _ => fatalx("bad layout type"),
        }

        // Calculate new cell sizes. size is the target size or -1 for middle
        // split, size1 is the size of the top/left and size2 the bottom/right.
        let saved_size = if type_ == layout_type::LAYOUT_LEFTRIGHT {
            sx
        } else {
            sy
        };

        let mut size2 = if size < 0 {
            saved_size.div_ceil(2) - 1
        } else if flags.intersects(SPAWN_BEFORE) {
            saved_size - size as u32 - 1
        } else {
            size as u32
        };

        if size2 < PANE_MINIMUM {
            size2 = PANE_MINIMUM;
        } else if size2 > saved_size - 2 {
            size2 = saved_size - 2;
        }
        let size1 = saved_size - 1 - size2;

        // Which size are we using?
        let new_size = if flags.intersects(SPAWN_BEFORE) {
            size2
        } else {
            size1
        };

        // Confirm there is enough space for full size pane.
        if full_size && !layout_set_size_check((*wp).window, lc, type_, new_size as i32) {
            return null_mut();
        }

        let lcparent: *mut layout_cell;
        let lcnew: *mut layout_cell;

        if !(*lc).parent.is_null() && (*(*lc).parent).type_ == type_ {
            // If the parent exists and is of the same type as the split,
            // create a new cell and insert it after this one.
            lcparent = (*lc).parent;
            lcnew = layout_create_cell(lcparent);
            if flags.intersects(SPAWN_BEFORE) {
                tailq_insert_before(lc, lcnew);
            } else {
                tailq_insert_after(&raw mut (*lcparent).cells, lc, lcnew);
            }
        } else if full_size && (*lc).parent.is_null() && (*lc).type_ == type_ {
            // If the new full size pane is the same type as the root
            // split, insert the new pane under the existing root cell
            // instead of creating a new root cell. The existing layout
            // must be resized before inserting the new cell.
            if (*lc).type_ == layout_type::LAYOUT_LEFTRIGHT {
                (*lc).sx = new_size;
                layout_resize_child_cells((*wp).window, lc);
                (*lc).sx = saved_size;
            } else if (*lc).type_ == layout_type::LAYOUT_TOPBOTTOM {
                (*lc).sy = new_size;
                layout_resize_child_cells((*wp).window, lc);
                (*lc).sy = saved_size;
            }
            resize_first = 1;

            // Create the new cell.
            lcnew = layout_create_cell(lc);
            let size = saved_size - 1 - new_size;
            if (*lc).type_ == layout_type::LAYOUT_LEFTRIGHT {
                layout_set_size(lcnew, size, sy, 0, 0);
            } else if (*lc).type_ == layout_type::LAYOUT_TOPBOTTOM {
                layout_set_size(lcnew, sx, size, 0, 0);
            }
            if flags.intersects(SPAWN_BEFORE) {
                tailq_insert_head(&raw mut (*lc).cells, lcnew);
            } else {
                tailq_insert_tail(&raw mut (*lc).cells, lcnew);
            }
        } else {
            // Otherwise create a new parent and insert it.

            // Create and insert the replacement parent.
            lcparent = layout_create_cell((*lc).parent);
            layout_make_node(lcparent, type_);
            layout_set_size(lcparent, sx, sy, xoff, yoff);
            if (*lc).parent.is_null() {
                (*(*wp).window).layout_root = lcparent;
            } else {
                tailq_replace(&raw mut (*(*lc).parent).cells, lc, lcparent);
            }

            // Insert the old cell.
            (*lc).parent = lcparent;
            tailq_insert_head(&raw mut (*lcparent).cells, lc);

            // Create the new child cell.
            lcnew = layout_create_cell(lcparent);
            if flags.intersects(SPAWN_BEFORE) {
                tailq_insert_head(&raw mut (*lcparent).cells, lcnew);
            } else {
                tailq_insert_tail(&raw mut (*lcparent).cells, lcnew);
            }
        }

        let (lc1, lc2) = if flags.intersects(SPAWN_BEFORE) {
            (lcnew, lc)
        } else {
            (lc, lcnew)
        };

        // Set new cell sizes. size1 is the size of the top/left and size2 the
        // bottom/right.
        if resize_first == 0 && type_ == layout_type::LAYOUT_LEFTRIGHT {
            layout_set_size(lc1, size1, sy, xoff, yoff);
            layout_set_size(lc2, size2, sy, xoff + (*lc1).sx + 1, yoff);
        } else if resize_first == 0 && type_ == layout_type::LAYOUT_TOPBOTTOM {
            layout_set_size(lc1, sx, size1, xoff, yoff);
            layout_set_size(lc2, sx, size2, xoff, yoff + (*lc1).sy + 1);
        }

        if full_size {
            if resize_first == 0 {
                layout_resize_child_cells((*wp).window, lc);
            }
            layout_fix_offsets((*wp).window);
        } else {
            layout_make_leaf(lc, wp);
        }

        lcnew
    }
}

/// Destroy the cell associated with a pane.
/// C `vendor/tmux/layout.c:1485`: `void layout_close_pane(struct window_pane *wp)`
pub unsafe fn layout_close_pane(wp: *mut window_pane) {
    unsafe {
        let w = (*wp).window;

        // Remove the cell
        layout_destroy_cell(w, (*wp).layout_cell, &raw mut (*w).layout_root);

        // Fix pane offsets and sizes
        if !(*w).layout_root.is_null() {
            layout_fix_offsets(w);
            layout_fix_panes(w, null_mut());
        }
        notify_window(c"window-layout-changed", w);
    }
}

/// Spread cells evenly within a parent cell
/// C `vendor/tmux/layout.c:1506`: `int layout_spread_cell(struct window *w, struct layout_cell *parent)`
pub unsafe fn layout_spread_cell(w: *mut window, parent: *mut layout_cell) -> c_int {
    unsafe {
        // Count number of cells
        let number = tailq_foreach(&raw mut (*parent).cells).count() as u32;
        if number <= 1 {
            return 0;
        }

        let status: pane_status = (options_get_number_((*w).options, "pane-border-status") as i32)
            .try_into()
            .unwrap();

        // Calculate available size
        let size = match (*parent).type_ {
            layout_type::LAYOUT_LEFTRIGHT => (*parent).sx,
            layout_type::LAYOUT_TOPBOTTOM => {
                if layout_add_border(w, parent, status) {
                    (*parent).sy - 1
                } else {
                    (*parent).sy
                }
            }
            _ => return 0,
        };

        if size < number - 1 {
            return 0;
        }

        let each = (size - (number - 1)) / number;
        if each == 0 {
            return 0;
        }

        // Remaining space after the evenly-distributed part is handed out one
        // column/row at a time to the LEADING cells (vendor/tmux/layout.c),
        // not dumped on the last cell. For size 80, n 2: each=39, remainder=1,
        // so the first pane becomes 40 and the second 39 — matching tmux.
        //
        // C evaluates this in u_int: `size - n*(each+1)` can momentarily be
        // negative (== UINT_MAX) before the `+ 1` restores a small count, so use
        // wrapping ops to avoid a debug-build overflow panic on those inputs.
        let mut remainder = size
            .wrapping_sub(number.wrapping_mul(each + 1))
            .wrapping_add(1);

        let mut changed = 0;
        for lc in tailq_foreach(&raw mut (*parent).cells).map(NonNull::as_ptr) {
            let change = match (*parent).type_ {
                layout_type::LAYOUT_LEFTRIGHT => {
                    let mut change = each as i32 - (*lc).sx as i32;
                    if remainder > 0 {
                        change += 1;
                        remainder -= 1;
                    }
                    layout_resize_adjust(w, lc, layout_type::LAYOUT_LEFTRIGHT, change);
                    change
                }
                layout_type::LAYOUT_TOPBOTTOM => {
                    let mut this = if layout_add_border(w, lc, status) {
                        each + 1
                    } else {
                        each
                    };
                    if remainder > 0 {
                        this += 1;
                        remainder -= 1;
                    }
                    let change = this as i32 - (*lc).sy as i32;
                    layout_resize_adjust(w, lc, layout_type::LAYOUT_TOPBOTTOM, change);
                    change
                }
                _ => 0,
            };

            if change != 0 {
                changed = 1;
            }
        }

        changed
    }
}

/// Spread out a pane and its parent cells
/// C `vendor/tmux/layout.c:1573`: `void layout_spread_out(struct window_pane *wp)`
pub unsafe fn layout_spread_out(wp: *mut window_pane) {
    unsafe {
        let mut parent = (*wp).layout_cell;
        if parent.is_null() {
            return;
        }
        parent = (*parent).parent;
        if parent.is_null() {
            return;
        }

        let w = (*wp).window;
        while !parent.is_null() {
            if layout_spread_cell(w, parent) != 0 {
                layout_fix_offsets(w);
                layout_fix_panes(w, null_mut());
                break;
            }
            parent = (*parent).parent;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Build a leaf (WINDOWPANE) cell of a given size. Offsets default to 0.
    unsafe fn leaf(sx: u32, sy: u32) -> *mut layout_cell {
        unsafe {
            let lc = layout_create_cell(null_mut());
            (*lc).type_ = layout_type::LAYOUT_WINDOWPANE;
            (*lc).sx = sx;
            (*lc).sy = sy;
            (*lc).xoff = 0;
            (*lc).yoff = 0;
            lc
        }
    }

    // Build a node cell of `type_` with the given children linked in order.
    unsafe fn node(type_: layout_type, children: &[*mut layout_cell]) -> *mut layout_cell {
        unsafe {
            let p = layout_create_cell(null_mut());
            (*p).type_ = type_;
            (*p).sx = 0;
            (*p).sy = 0;
            (*p).xoff = 0;
            (*p).yoff = 0;
            for &c in children {
                (*c).parent = p;
                tailq_insert_tail(&raw mut (*p).cells, c);
            }
            p
        }
    }

    #[test]
    fn create_cell_defaults() {
        unsafe {
            let lc = layout_create_cell(null_mut());
            assert!((*lc).type_ == layout_type::LAYOUT_WINDOWPANE);
            assert!((*lc).parent.is_null());
            assert!((*lc).wp.is_null());
            // Sizes/offsets start as UINT_MAX (vendor/tmux/layout.c:61).
            assert_eq!((*lc).sx, u32::MAX);
            assert_eq!((*lc).sy, u32::MAX);
            assert!(tailq_empty(&raw mut (*lc).cells));
            layout_free_cell(lc, 0);
        }
    }

    #[test]
    fn count_cells_recurses() {
        unsafe {
            // LEFTRIGHT { leaf, TOPBOTTOM { leaf, leaf } } -> 3 leaves.
            let inner = node(layout_type::LAYOUT_TOPBOTTOM, &[leaf(10, 5), leaf(10, 5)]);
            let root = node(layout_type::LAYOUT_LEFTRIGHT, &[leaf(20, 10), inner]);
            assert_eq!(layout_count_cells(root), 3);
            layout_free_cell(root, 0);
        }
    }

    #[test]
    fn resize_adjust_leaf_touches_only_its_axis() {
        unsafe {
            let lc = leaf(40, 24);
            // LEFTRIGHT changes sx; sy is untouched.
            layout_resize_adjust(null_mut(), lc, layout_type::LAYOUT_LEFTRIGHT, 5);
            assert_eq!((*lc).sx, 45);
            assert_eq!((*lc).sy, 24);
            // TOPBOTTOM changes sy; sx is untouched.
            layout_resize_adjust(null_mut(), lc, layout_type::LAYOUT_TOPBOTTOM, -4);
            assert_eq!((*lc).sx, 45);
            assert_eq!((*lc).sy, 20);
            layout_free_cell(lc, 0);
        }
    }

    #[test]
    fn resize_adjust_grows_children_equally() {
        unsafe {
            // Same-direction parent hands a positive change out one column at a
            // time, round-robin (vendor/tmux/layout.c:579). +3 over 2 children
            // -> 2 and 1.
            let a = leaf(40, 24);
            let b = leaf(40, 24);
            let p = node(layout_type::LAYOUT_LEFTRIGHT, &[a, b]);
            (*p).sx = 81;
            layout_resize_adjust(null_mut(), p, layout_type::LAYOUT_LEFTRIGHT, 3);
            assert_eq!((*a).sx, 42);
            assert_eq!((*b).sx, 41);
            layout_free_cell(p, 0);
        }
    }

    #[test]
    fn fix_offsets_lays_cells_left_to_right() {
        unsafe {
            // Two 40-wide cells with a 1-col border between them.
            let a = leaf(40, 24);
            let b = leaf(39, 24);
            let p = node(layout_type::LAYOUT_LEFTRIGHT, &[a, b]);
            (*p).xoff = 0;
            (*p).yoff = 0;
            layout_fix_offsets1(p);
            assert_eq!(((*a).xoff, (*a).yoff), (0, 0));
            // second cell starts after first cell + 1 border column.
            assert_eq!(((*b).xoff, (*b).yoff), (41, 0));
            layout_free_cell(p, 0);
        }
    }

    #[test]
    fn fix_offsets_stacks_cells_top_to_bottom() {
        unsafe {
            let a = leaf(80, 12);
            let b = leaf(80, 11);
            let p = node(layout_type::LAYOUT_TOPBOTTOM, &[a, b]);
            layout_fix_offsets1(p);
            assert_eq!(((*a).xoff, (*a).yoff), (0, 0));
            // second cell starts after first cell + 1 border row.
            assert_eq!(((*b).xoff, (*b).yoff), (0, 13));
            layout_free_cell(p, 0);
        }
    }

    #[test]
    fn search_by_border_finds_the_divider() {
        unsafe {
            // |  a (0..40)  | border@40 |  b (41..81)  |
            let a = leaf(40, 24);
            let b = leaf(40, 24);
            let p = node(layout_type::LAYOUT_LEFTRIGHT, &[a, b]);
            (*p).xoff = 0;
            (*p).yoff = 0;
            layout_fix_offsets1(p);
            // Inside a cell -> not a border hit.
            assert!(layout_search_by_border(p, 10, 5).is_null());
            // On the divider column between a and b -> returns the left cell.
            assert_eq!(layout_search_by_border(p, 40, 5), a);
            layout_free_cell(p, 0);
        }
    }

    // Locks the layout_spread_cell remainder fix (commit b5099243e9): the
    // even-* layouts hand the leftover columns to the LEADING cells one at a
    // time, so an 80-col / 2-pane split is 40|39, not 39|40. This reproduces
    // the LEFTRIGHT branch of layout_spread_cell against the real
    // layout_resize_adjust primitive (the options-dependent border path is
    // exercised by the parity harness, which is 122/122 with this fix).
    fn spread_sizes(size: u32, number: u32) -> Vec<u32> {
        let each = (size - (number - 1)) / number;
        // C computes this in u_int, which wraps; the leading `size - n*(each+1)`
        // can momentarily go negative (== UINT_MAX) before the `+ 1` brings it
        // back to a small non-negative count. Mirror that with wrapping ops.
        let mut remainder = size
            .wrapping_sub(number.wrapping_mul(each + 1))
            .wrapping_add(1);
        (0..number)
            .map(|_| {
                let mut this = each;
                if remainder > 0 {
                    this += 1;
                    remainder -= 1;
                }
                this
            })
            .collect()
    }

    #[test]
    fn spread_remainder_goes_to_leading_cells() {
        // 80 columns, 2 panes -> 40 | 39 (the bug produced 39 | 40).
        assert_eq!(spread_sizes(80, 2), vec![40, 39]);
        // 80 columns, 3 panes -> 26 | 26 | 26 (each=26, no remainder).
        assert_eq!(spread_sizes(80, 3), vec![26, 26, 26]);
        // Every case: sizes sum with (number-1) borders back to the full size,
        // and are non-increasing (leaders never smaller than trailers).
        for size in [10u32, 24, 80, 100, 200] {
            for number in 2u32..=6 {
                if size < number - 1 {
                    continue;
                }
                let sizes = spread_sizes(size, number);
                let sum: u32 = sizes.iter().sum();
                assert_eq!(sum + (number - 1), size, "size {size} n {number}");
                for w in sizes.windows(2) {
                    assert!(w[0] >= w[1], "size {size} n {number}: {sizes:?}");
                }
            }
        }
    }

    #[test]
    fn spread_via_resize_adjust_yields_40_39() {
        unsafe {
            // Drive the actual resize primitive with the spread formula.
            let a = leaf(0, 24);
            let b = leaf(0, 24);
            let p = node(layout_type::LAYOUT_LEFTRIGHT, &[a, b]);
            let size = 80u32;
            let sizes = spread_sizes(size, 2);
            for (lc, &want) in [a, b].iter().zip(sizes.iter()) {
                let change = want as i32 - (**lc).sx as i32;
                layout_resize_adjust(null_mut(), *lc, layout_type::LAYOUT_LEFTRIGHT, change);
            }
            assert_eq!((*a).sx, 40);
            assert_eq!((*b).sx, 39);
            layout_free_cell(p, 0);
        }
    }

    #[test]
    fn set_size_sets_all_four_fields() {
        unsafe {
            // layout_set_size just stores sx/sy/xoff/yoff verbatim
            // (vendor/tmux/layout.c:203).
            let lc = leaf(1, 1);
            layout_set_size(lc, 80, 24, 3, 7);
            assert_eq!((*lc).sx, 80);
            assert_eq!((*lc).sy, 24);
            assert_eq!((*lc).xoff, 3);
            assert_eq!((*lc).yoff, 7);
            layout_free_cell(lc, 0);
        }
    }

    #[test]
    fn make_node_converts_leaf_and_reinits_cells() {
        unsafe {
            // A fresh leaf (no pane attached) becomes a node; the cell list is
            // reset and stays empty (vendor/tmux/layout.c:226).
            let lc = leaf(40, 24);
            assert!((*lc).type_ == layout_type::LAYOUT_WINDOWPANE);
            layout_make_node(lc, layout_type::LAYOUT_LEFTRIGHT);
            assert!((*lc).type_ == layout_type::LAYOUT_LEFTRIGHT);
            assert!((*lc).wp.is_null());
            assert!(tailq_empty(&raw mut (*lc).cells));
            // A node may also be re-typed to the other direction.
            layout_make_node(lc, layout_type::LAYOUT_TOPBOTTOM);
            assert!((*lc).type_ == layout_type::LAYOUT_TOPBOTTOM);
            layout_free_cell(lc, 0);
        }
    }

    #[test]
    fn make_leaf_wires_pane_then_make_node_detaches_it() {
        unsafe {
            // layout_make_leaf cross-links the cell and the pane; a later
            // layout_make_node on the same cell must detach the pane
            // (vendor/tmux/layout.c:214 and :226). We only touch wp.layout_cell,
            // so a placeholder pane whose other fields are uninitialised is safe.
            let lc = layout_create_cell(null_mut());
            // Zeroed, not uninit: window_pane owns Option<CString> fields, whose all-zero
            // form is a valid `None`; garbage is not.
            let wp: *mut window_pane = Box::into_raw(Box::new(zeroed::<window_pane>()));

            layout_make_leaf(lc, wp);
            assert!((*lc).type_ == layout_type::LAYOUT_WINDOWPANE);
            assert_eq!((*lc).wp, wp);
            assert_eq!((*wp).layout_cell, lc);

            layout_make_node(lc, layout_type::LAYOUT_LEFTRIGHT);
            assert!((*lc).type_ == layout_type::LAYOUT_LEFTRIGHT);
            assert!((*lc).wp.is_null());
            assert!((*wp).layout_cell.is_null());
            layout_free_cell(lc, 0);
        }
    }

    #[test]
    fn count_cells_deeper_nested_tree() {
        unsafe {
            // TOPBOTTOM {
            //   LEFTRIGHT { leaf, leaf },
            //   LEFTRIGHT { leaf, TOPBOTTOM { leaf, leaf } }
            // } -> 2 + (1 + 2) = 5 window panes (vendor/tmux/layout.c:505).
            let l1 = node(layout_type::LAYOUT_LEFTRIGHT, &[leaf(10, 5), leaf(10, 5)]);
            let inner = node(layout_type::LAYOUT_TOPBOTTOM, &[leaf(10, 2), leaf(10, 2)]);
            let l2 = node(layout_type::LAYOUT_LEFTRIGHT, &[leaf(10, 5), inner]);
            let root = node(layout_type::LAYOUT_TOPBOTTOM, &[l1, l2]);
            assert_eq!(layout_count_cells(root), 5);
            layout_free_cell(root, 0);
        }
    }

    #[test]
    fn search_by_border_topbottom_finds_divider() {
        unsafe {
            // a occupies rows 0..12, the border sits on row 12, b on rows 13..24.
            let a = leaf(80, 12);
            let b = leaf(80, 11);
            let p = node(layout_type::LAYOUT_TOPBOTTOM, &[a, b]);
            (*p).xoff = 0;
            (*p).yoff = 0;
            (*p).sx = 80;
            (*p).sy = 24;
            layout_fix_offsets1(p);
            // Inside the top cell -> not a border hit.
            assert!(layout_search_by_border(p, 5, 3).is_null());
            // On the divider row between a and b -> returns the upper cell.
            assert_eq!(layout_search_by_border(p, 5, 12), a);
            layout_free_cell(p, 0);
        }
    }

    #[test]
    fn resize_adjust_different_direction_grows_all_children() {
        unsafe {
            // Applying a TOPBOTTOM change to a LEFTRIGHT node pushes the *same*
            // change into the node and into every child, leaving the crossing
            // axis (sx) untouched (vendor/tmux/layout.c:579).
            let a = leaf(40, 10);
            let b = leaf(39, 10);
            let p = node(layout_type::LAYOUT_LEFTRIGHT, &[a, b]);
            (*p).sx = 80;
            (*p).sy = 10;
            layout_resize_adjust(null_mut(), p, layout_type::LAYOUT_TOPBOTTOM, 5);
            assert_eq!((*p).sy, 15);
            assert_eq!((*a).sy, 15);
            assert_eq!((*b).sy, 15);
            assert_eq!((*a).sx, 40);
            assert_eq!((*b).sx, 39);
            layout_free_cell(p, 0);
        }
    }

    #[test]
    fn resize_adjust_same_direction_nested_round_robin() {
        unsafe {
            // LEFTRIGHT { a, LEFTRIGHT { c, d } }. A +2 same-direction change is
            // handed out one column at a time round-robin: a gets 1, the nested
            // node gets 1 which it forwards to its own first child c
            // (vendor/tmux/layout.c:579).
            let a = leaf(10, 24);
            let c = leaf(10, 24);
            let d = leaf(10, 24);
            let q = node(layout_type::LAYOUT_LEFTRIGHT, &[c, d]);
            (*q).sx = 21; // 10 + 1 + 10
            (*q).sy = 24;
            let p = node(layout_type::LAYOUT_LEFTRIGHT, &[a, q]);
            (*p).sx = 32; // 10 + 1 + 21
            (*p).sy = 24;
            layout_resize_adjust(null_mut(), p, layout_type::LAYOUT_LEFTRIGHT, 2);
            assert_eq!((*p).sx, 34);
            assert_eq!((*a).sx, 11);
            assert_eq!((*q).sx, 22);
            assert_eq!((*c).sx, 11);
            assert_eq!((*d).sx, 10);
            layout_free_cell(p, 0);
        }
    }

    // A NUMBER "pane-border-status" entry so layout_resize_check can read the
    // option off a hand-built window without the runtime GLOBAL trees. Value 0 ==
    // PANE_STATUS_OFF, so layout_add_border never fires (vendor/tmux/layout.c:411).
    static PBS_OE: options_table_entry = options_table_entry {
        name: "pane-border-status",
        type_: options_table_type::OPTIONS_TABLE_NUMBER,
        default_num: 0,
        ..options_table_entry::const_default()
    };

    // Build a window whose options tree carries pane-border-status = OFF.
    unsafe fn window_off() -> *mut window {
        unsafe {
            let w = Box::into_raw(Box::new(std::mem::zeroed::<window>()));
            let opts = options_create(null_mut());
            // options_empty adds the entry with value.number == 0 (OFF).
            options_empty(opts, &PBS_OE);
            (*w).options = opts;
            w
        }
    }

    // layout_cell_is_tiled: 1 for a plain leaf, 0 for a floating leaf, 0 for a
    // node (vendor/tmux/layout.c:263).
    #[test]
    fn cell_is_tiled_variants() {
        unsafe {
            let l = leaf(80, 24);
            assert_eq!(layout_cell_is_tiled(l), 1);
            (*l).flags |= LAYOUT_CELL_FLOATING;
            assert_eq!(layout_cell_is_tiled(l), 0);
            layout_free_cell(l, 0);

            let n = node(layout_type::LAYOUT_LEFTRIGHT, &[leaf(10, 5), leaf(10, 5)]);
            assert_eq!(layout_cell_is_tiled(n), 0);
            layout_free_cell(n, 0);
        }
    }

    // layout_resize_check on a leaf returns the space above the pane minimum in
    // the requested direction (vendor/tmux/layout.c:340). With status OFF the
    // minimum is PANE_MINIMUM (1) for both axes.
    #[test]
    fn resize_check_leaf_axes() {
        unsafe {
            let w = window_off();
            let l = leaf(40, 24);
            // LEFTRIGHT looks at sx: 40 - 1 = 39.
            assert_eq!(layout_resize_check(w, l, layout_type::LAYOUT_LEFTRIGHT), 39);
            // TOPBOTTOM looks at sy: 24 - 1 = 23.
            assert_eq!(layout_resize_check(w, l, layout_type::LAYOUT_TOPBOTTOM), 23);
            layout_free_cell(l, 0);
        }
    }

    // A leaf at or below the minimum has no room to give.
    #[test]
    fn resize_check_leaf_at_minimum_is_zero() {
        unsafe {
            let w = window_off();
            let l = leaf(1, 1);
            assert_eq!(layout_resize_check(w, l, layout_type::LAYOUT_LEFTRIGHT), 0);
            assert_eq!(layout_resize_check(w, l, layout_type::LAYOUT_TOPBOTTOM), 0);
            layout_free_cell(l, 0);
        }
    }

    // Same-type parent sums the space of all children; different-type parent
    // takes the minimum (vendor/tmux/layout.c:340).
    #[test]
    fn resize_check_node_sum_vs_min() {
        unsafe {
            let w = window_off();
            let a = leaf(40, 24); // LR room 39
            let b = leaf(30, 24); // LR room 29
            let p = node(layout_type::LAYOUT_LEFTRIGHT, &[a, b]);
            (*p).sx = 71;
            (*p).sy = 24;
            // Same direction (LEFTRIGHT): 39 + 29 = 68.
            assert_eq!(layout_resize_check(w, p, layout_type::LAYOUT_LEFTRIGHT), 68);
            // Cross direction (TOPBOTTOM): min(sy-1, sy-1) = min(23, 23) = 23.
            assert_eq!(layout_resize_check(w, p, layout_type::LAYOUT_TOPBOTTOM), 23);
            layout_free_cell(p, 0);
        }
    }

    // layout_resize_adjust with a negative same-direction change shrinks children
    // round-robin, but only those that still have room per layout_resize_check
    // (vendor/tmux/layout.c:579).
    #[test]
    fn resize_adjust_negative_round_robin() {
        unsafe {
            let w = window_off();
            let a = leaf(40, 24);
            let b = leaf(40, 24);
            let p = node(layout_type::LAYOUT_LEFTRIGHT, &[a, b]);
            (*p).sx = 81;
            (*p).sy = 24;
            layout_resize_adjust(w, p, layout_type::LAYOUT_LEFTRIGHT, -3);
            // node sx drops by 3; -3 handed out a,b,a -> 38 | 39.
            assert_eq!((*p).sx, 78);
            assert_eq!((*a).sx, 38);
            assert_eq!((*b).sx, 39);
            layout_free_cell(p, 0);
        }
    }

    // layout_set_size_check on a leaf just requires size >= PANE_MINIMUM
    // (vendor/tmux/layout.c:1110). No window needed for the leaf branch.
    #[test]
    fn set_size_check_leaf_minimum() {
        unsafe {
            let l = leaf(80, 24);
            assert!(layout_set_size_check(
                null_mut(),
                l,
                layout_type::LAYOUT_LEFTRIGHT,
                PANE_MINIMUM as i32
            ));
            assert!(!layout_set_size_check(
                null_mut(),
                l,
                layout_type::LAYOUT_LEFTRIGHT,
                PANE_MINIMUM as i32 - 1
            ));
            layout_free_cell(l, 0);
        }
    }

    // layout_new_pane_size short-circuits for the last cell, returning all of the
    // remaining size regardless of the window (vendor/tmux/layout.c:1044).
    #[test]
    fn new_pane_size_last_cell_takes_remaining() {
        unsafe {
            let l = leaf(40, 24);
            let got = layout_new_pane_size(
                null_mut(),
                100,
                l,
                layout_type::LAYOUT_LEFTRIGHT,
                50,
                1,  // count_left == 1
                33, // size_left
            );
            assert_eq!(got, 33);
            layout_free_cell(l, 0);
        }
    }

    // layout_search_by_border returns NULL when the point is not on any divider
    // (vendor/tmux/layout.c:162).
    #[test]
    fn search_by_border_outside_returns_null() {
        unsafe {
            let a = leaf(40, 24);
            let b = leaf(40, 24);
            let p = node(layout_type::LAYOUT_LEFTRIGHT, &[a, b]);
            (*p).xoff = 0;
            (*p).yoff = 0;
            layout_fix_offsets1(p);
            // Well past the right edge of both cells.
            assert!(layout_search_by_border(p, 200, 5).is_null());
            layout_free_cell(p, 0);
        }
    }

    // layout_count_cells of a single leaf is 1 (vendor/tmux/layout.c:505).
    #[test]
    fn count_cells_single_leaf() {
        unsafe {
            let l = leaf(80, 24);
            assert_eq!(layout_count_cells(l), 1);
            layout_free_cell(l, 0);
        }
    }

    // layout_fix_offsets1 lays three LEFTRIGHT children left to right, each after
    // the previous plus a one-column border (vendor/tmux/layout.c:203).
    #[test]
    fn fix_offsets_three_leftright_children() {
        unsafe {
            let a = leaf(20, 24);
            let b = leaf(30, 24);
            let c = leaf(28, 24);
            let p = node(layout_type::LAYOUT_LEFTRIGHT, &[a, b, c]);
            (*p).xoff = 0;
            (*p).yoff = 0;
            layout_fix_offsets1(p);
            assert_eq!(((*a).xoff, (*a).yoff), (0, 0));
            assert_eq!(((*b).xoff, (*b).yoff), (21, 0)); // 20 + 1 border
            assert_eq!(((*c).xoff, (*c).yoff), (52, 0)); // 21 + 30 + 1 border
            layout_free_cell(p, 0);
        }
    }

    // A cross-direction change on a TOPBOTTOM node applies the same delta to the
    // node and to every child on the changed (sx) axis (vendor/tmux/layout.c:579).
    #[test]
    fn resize_adjust_topbottom_cross_direction() {
        unsafe {
            let a = leaf(80, 12);
            let b = leaf(80, 11);
            let p = node(layout_type::LAYOUT_TOPBOTTOM, &[a, b]);
            (*p).sx = 80;
            (*p).sy = 24;
            layout_resize_adjust(null_mut(), p, layout_type::LAYOUT_LEFTRIGHT, 4);
            assert_eq!((*p).sx, 84);
            assert_eq!((*a).sx, 84);
            assert_eq!((*b).sx, 84);
            // sy axis untouched.
            assert_eq!((*a).sy, 12);
            assert_eq!((*b).sy, 11);
            layout_free_cell(p, 0);
        }
    }
}
