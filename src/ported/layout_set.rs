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
use crate::*;
use crate::options_::options_get_number_;
use crate::options_::options_get_string_;

struct layout_sets_entry {
    name: SyncCharPtr,
    arrange: Option<unsafe fn(*mut window)>,
}
impl layout_sets_entry {
    const fn new(name: &'static CStr, arrange: unsafe fn(*mut window)) -> Self {
        Self {
            name: SyncCharPtr::new(name),
            arrange: Some(arrange),
        }
    }
}

const LAYOUT_SETS_LEN: usize = 7;
static LAYOUT_SETS: [layout_sets_entry; LAYOUT_SETS_LEN] = [
    layout_sets_entry::new(c"even-horizontal", layout_set_even_h),
    layout_sets_entry::new(c"even-vertical", layout_set_even_v),
    layout_sets_entry::new(c"main-horizontal", layout_set_main_h),
    layout_sets_entry::new(c"main-horizontal-mirrored", layout_set_main_h_mirrored),
    layout_sets_entry::new(c"main-vertical", layout_set_main_v),
    layout_sets_entry::new(c"main-vertical-mirrored", layout_set_main_v_mirrored),
    layout_sets_entry::new(c"tiled", layout_set_tiled),
];

/// C `vendor/tmux/layout-set.c:126`: `static struct window_pane *layout_set_first_tiled(struct window *w)`
///
/// The first pane that still owns a tiled (leaf, non-floating) layout cell —
/// used as the main pane after `layout_free(w, 1)` keeps the leaf cells intact.
unsafe fn layout_set_first_tiled(w: *mut window) -> *mut window_pane {
    unsafe {
        for wp in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr) {
            if !(*wp).layout_cell.is_null() && layout_cell_is_tiled((*wp).layout_cell) != 0 {
                return wp;
            }
        }
        null_mut()
    }
}

/// C `vendor/tmux/layout-set.c:53`: `int layout_set_lookup(const char *name)`
pub unsafe fn layout_set_lookup(name: *const u8) -> i32 {
    unsafe {
        let mut matched: i32 = -1;

        for (i, ls) in LAYOUT_SETS.iter().enumerate() {
            if libc::strcmp(ls.name.as_ptr(), name) == 0 {
                return i as i32;
            }
        }

        for (i, ls) in LAYOUT_SETS.iter().enumerate() {
            if libc::strncmp(ls.name.as_ptr(), name, strlen(name)) == 0 {
                if matched != -1 {
                    // ambiguous
                    return -1;
                }
                matched = i as i32;
            }
        }

        matched
    }
}

/// C `vendor/tmux/layout-set.c:74`: `u_int layout_set_select(struct window *w, u_int layout)`
pub unsafe fn layout_set_select(w: *mut window, mut layout: u32) -> u32 {
    unsafe {
        if layout > LAYOUT_SETS_LEN as u32 - 1 {
            layout = LAYOUT_SETS_LEN as u32 - 1;
        }

        if let Some(arrange) = LAYOUT_SETS[layout as usize].arrange {
            arrange(w);
        }

        (*w).lastlayout = layout as i32;
        layout
    }
}

/// C `vendor/tmux/layout-set.c:87`: `u_int layout_set_next(struct window *w)`
pub unsafe fn layout_set_next(w: *mut window) -> u32 {
    unsafe {
        let mut layout: u32;

        if (*w).lastlayout == -1 {
            layout = 0;
        } else {
            layout = ((*w).lastlayout + 1) as u32;
            if layout > LAYOUT_SETS_LEN as u32 - 1 {
                layout = 0;
            }
        }

        if let Some(arrange) = LAYOUT_SETS[layout as usize].arrange {
            arrange(w);
        }
        (*w).lastlayout = layout as i32;
        layout
    }
}

/// C `vendor/tmux/layout-set.c:106`: `u_int layout_set_previous(struct window *w)`
pub unsafe fn layout_set_previous(w: *mut window) -> u32 {
    unsafe {
        let mut layout: u32;

        if (*w).lastlayout == -1 {
            layout = (LAYOUT_SETS_LEN - 1) as u32;
        } else {
            layout = (*w).lastlayout as u32;
            if layout == 0 {
                layout = (LAYOUT_SETS_LEN - 1) as u32;
            } else {
                layout -= 1;
            }
        }

        if let Some(arrange) = LAYOUT_SETS[layout as usize].arrange {
            arrange(w);
        }
        (*w).lastlayout = layout as i32;
        layout
    }
}

/// C `vendor/tmux/layout-set.c:139`: `static void layout_set_even(struct window *w, enum layout_type type)`
pub unsafe fn layout_set_even(w: *mut window, type_: layout_type) {
    let __func__ = c!("layout_set_even");
    unsafe {
        let mut sx: u32;
        let mut sy: u32;

        layout_print_cell((*w).layout_root, __func__, 1);

        // Get number of panes.
        let n = window_count_panes(w);
        if n <= 1 {
            return;
        }

        // Free the old root and construct a new.
        layout_free(w, 0);
        let lc = layout_create_cell(null_mut());
        (*w).layout_root = lc;
        if type_ == layout_type::LAYOUT_LEFTRIGHT {
            sx = (n * (PANE_MINIMUM + 1)) - 1;
            if sx < (*w).sx {
                sx = (*w).sx;
            }
            sy = (*w).sy;
        } else {
            sy = (n * (PANE_MINIMUM + 1)) - 1;
            if sy < (*w).sy {
                sy = (*w).sy;
            }
            sx = (*w).sx;
        }
        layout_set_size(lc, sx, sy, 0, 0);
        layout_make_node(lc, type_);

        // Build new leaf cells.
        for wp in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr) {
            let lcnew = layout_create_cell(lc);
            layout_make_leaf(lcnew, wp);
            (*lcnew).sx = (*w).sx;
            (*lcnew).sy = (*w).sy;
            tailq_insert_tail(&raw mut (*lc).cells, lcnew);
        }

        // Spread out cells.
        layout_spread_cell(w, lc);

        // Fix cell offsets.
        layout_fix_offsets(w);
        layout_fix_panes(w, null_mut());

        layout_print_cell((*w).layout_root, __func__, 1);

        window_resize(w, (*lc).sx, (*lc).sy, -1, -1);
        notify_window(c"window-layout-changed", w);
        server_redraw_window(w);
    }
}

/// C `vendor/tmux/layout-set.c:191`: `static void layout_set_even_h(struct window *w)`
unsafe fn layout_set_even_h(w: *mut window) {
    unsafe {
        layout_set_even(w, layout_type::LAYOUT_LEFTRIGHT);
    }
}

/// C `vendor/tmux/layout-set.c:197`: `static void layout_set_even_v(struct window *w)`
unsafe fn layout_set_even_v(w: *mut window) {
    unsafe {
        layout_set_even(w, layout_type::LAYOUT_TOPBOTTOM);
    }
}

/// C `vendor/tmux/layout-set.c:203`: `static void layout_set_main_h(struct window *w)`
pub unsafe fn layout_set_main_h(w: *mut window) {
    let __func__ = c!("layout_set_main_h");
    unsafe {
        // struct window_pane *wp;
        // struct layout_cell *lc, *lcmain, *lcother, *lcchild;
        // u_int n, mainh, otherh, sx, sy;
        // char *cause;
        // const char *s;
        let mut cause = null_mut();

        layout_print_cell((*w).layout_root, __func__, 1);

        // Get number of panes.
        let mut n = window_count_panes(w);
        if n <= 1 {
            return;
        }
        n -= 1; /* take off main pane */

        // Find available height - take off one line for the border.
        let sy = (*w).sy - 1;

        // Get the main pane height.
        let mut s = options_get_string_((*w).options, "main-pane-height");
        let mut mainh = args_string_percentage(s, 0, sy as i64, sy as i64, &raw mut cause) as u32;
        if !cause.is_null() {
            mainh = 24;
            free_(cause);
        }

        let mut otherh: u32;
        // Work out the other pane height.
        if mainh + PANE_MINIMUM >= sy {
            if sy <= PANE_MINIMUM + PANE_MINIMUM {
                mainh = PANE_MINIMUM;
            } else {
                mainh = sy - PANE_MINIMUM;
            }
            otherh = PANE_MINIMUM;
        } else {
            s = options_get_string_((*w).options, "other-pane-height");
            otherh = args_string_percentage(s, 0, sy as i64, sy as i64, &raw mut cause) as u32;
            if !cause.is_null() || otherh == 0 {
                otherh = sy - mainh;
                free_(cause);
            } else if otherh > sy || sy - otherh < mainh {
                otherh = sy - mainh;
            } else {
                mainh = sy - otherh;
            }
        }

        // Work out what width is needed.
        let mut sx = (n * (PANE_MINIMUM + 1)) - 1;
        if sx < (*w).sx {
            sx = (*w).sx;
        }

        // Free old tree and create a new root. Keep the leaf cells (only_nodes)
        // so each pane retains its current layout cell to reuse below.
        layout_free(w, 1);
        let lc = layout_create_cell(null_mut());
        (*w).layout_root = lc;
        layout_set_size(lc, sx, mainh + otherh + 1, 0, 0);
        layout_make_node(lc, layout_type::LAYOUT_TOPBOTTOM);

        // Use the first tiled pane as the main pane, reusing its cell.
        let wpmain = layout_set_first_tiled(w);
        let lcmain = (*wpmain).layout_cell;
        (*lcmain).parent = lc;
        layout_set_size(lcmain, sx, mainh, 0, 0);
        tailq_insert_tail(&raw mut (*lc).cells, lcmain);

        if n == 1 {
            // Reuse the single other pane's cell as-is (no resize): it keeps
            // its previous dimensions.
            let mut wp = tailq_next::<_, _, discr_entry>(wpmain);
            while !wp.is_null() && layout_cell_is_tiled((*wp).layout_cell) == 0 {
                wp = tailq_next::<_, _, discr_entry>(wp);
            }
            tailq_insert_tail(&raw mut (*lc).cells, (*wp).layout_cell);
            (*(*wp).layout_cell).parent = lc;
        } else {
            let lcother = layout_create_cell(lc);
            layout_set_size(lcother, sx, otherh, 0, 0);
            layout_make_node(lcother, layout_type::LAYOUT_LEFTRIGHT);
            tailq_insert_tail(&raw mut (*lc).cells, lcother);

            // Add the remaining panes as children, reusing their cells.
            for wp in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr) {
                if wp == wpmain {
                    continue;
                }
                let lcchild = (*wp).layout_cell;
                tailq_insert_tail(&raw mut (*lcother).cells, lcchild);
                (*lcchild).parent = lcother;
                if layout_cell_is_tiled(lcchild) != 0 {
                    layout_set_size(lcchild, PANE_MINIMUM, otherh, 0, 0);
                }
            }
            layout_spread_cell(w, lcother);
        }

        // Fix cell offsets.
        layout_fix_offsets(w);
        layout_fix_panes(w, null_mut());

        layout_print_cell((*w).layout_root, __func__, 1);

        window_resize(w, (*lc).sx, (*lc).sy, -1, -1);
        notify_window(c"window-layout-changed", w);
        server_redraw_window(w);
    }
}

/// C `vendor/tmux/layout-set.c:300`: `static void layout_set_main_h_mirrored(struct window *w)`
pub unsafe fn layout_set_main_h_mirrored(w: *mut window) {
    let __func__ = c!("layout_set_main_h_mirrored");
    unsafe {
        let mut otherh: u32;
        let mut cause: *mut u8 = null_mut();

        layout_print_cell((*w).layout_root, __func__, 1);

        // Get number of panes.
        let mut n = window_count_panes(w);
        if n <= 1 {
            return;
        }
        n -= 1; // take off main pane

        // Find available height - take off one line for the border.
        let sy = (*w).sy - 1;

        // Get the main pane height.
        let s = options_get_string_((*w).options, "main-pane-height");
        let mut mainh = args_string_percentage(s, 0, sy as i64, sy as i64, &raw mut cause) as u32;
        if !cause.is_null() {
            mainh = 24;
            free_(cause);
        }

        // Work out the other pane height.
        if mainh + PANE_MINIMUM >= sy {
            if sy <= PANE_MINIMUM + PANE_MINIMUM {
                mainh = PANE_MINIMUM;
            } else {
                mainh = sy - PANE_MINIMUM;
            }
            otherh = PANE_MINIMUM;
        } else {
            let s = options_get_string_((*w).options, "other-pane-height");
            otherh = args_string_percentage(s, 0, sy as i64, sy as i64, &raw mut cause) as u32;
            if !cause.is_null() || otherh == 0 {
                otherh = sy - mainh;
                free_(cause);
            } else if otherh > sy || sy - otherh < mainh {
                otherh = sy - mainh;
            } else {
                mainh = sy - otherh;
            }
        }

        // Work out what width is needed.
        let mut sx = (n * (PANE_MINIMUM + 1)) - 1;
        if sx < (*w).sx {
            sx = (*w).sx;
        }

        // Free old tree and create a new root. Keep the leaf cells (only_nodes)
        // so each pane retains its current layout cell to reuse below.
        layout_free(w, 1);
        let lc = layout_create_cell(null_mut());
        (*w).layout_root = lc;
        layout_set_size(lc, sx, mainh + otherh + 1, 0, 0);
        layout_make_node(lc, layout_type::LAYOUT_TOPBOTTOM);

        // Use the first tiled pane as the main pane, reusing its cell.
        let wpmain = layout_set_first_tiled(w);
        let lcmain = (*wpmain).layout_cell;
        (*lcmain).parent = lc;
        layout_set_size(lcmain, sx, mainh, 0, 0);
        tailq_insert_tail(&raw mut (*lc).cells, lcmain);

        // Mirrored: the other panes go at the head so they sit above the main.
        if n == 1 {
            let mut wp = tailq_next::<_, _, discr_entry>(wpmain);
            while !wp.is_null() && layout_cell_is_tiled((*wp).layout_cell) == 0 {
                wp = tailq_next::<_, _, discr_entry>(wp);
            }
            tailq_insert_head(&raw mut (*lc).cells, (*wp).layout_cell);
            (*(*wp).layout_cell).parent = lc;
        } else {
            let lcother = layout_create_cell(lc);
            layout_set_size(lcother, sx, otherh, 0, 0);
            layout_make_node(lcother, layout_type::LAYOUT_LEFTRIGHT);
            tailq_insert_head(&raw mut (*lc).cells, lcother);

            // Add the remaining panes as children, reusing their cells.
            for wp in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr) {
                if wp == wpmain {
                    continue;
                }
                let lcchild = (*wp).layout_cell;
                tailq_insert_tail(&raw mut (*lcother).cells, lcchild);
                (*lcchild).parent = lcother;
                if layout_cell_is_tiled(lcchild) != 0 {
                    layout_set_size(lcchild, PANE_MINIMUM, otherh, 0, 0);
                }
            }
            layout_spread_cell(w, lcother);
        }

        // Fix cell offsets.
        layout_fix_offsets(w);
        layout_fix_panes(w, null_mut());

        layout_print_cell((*w).layout_root, __func__, 1);

        window_resize(w, (*lc).sx, (*lc).sy, -1, -1);
        notify_window(c"window-layout-changed", w);
        server_redraw_window(w);
    }
}

/// C `vendor/tmux/layout-set.c:397`: `static void layout_set_main_v(struct window *w)`
pub unsafe fn layout_set_main_v(w: *mut window) {
    let __func__ = c!("layout_set_main_v");
    let mut cause = null_mut();

    unsafe {
        layout_print_cell((*w).layout_root, __func__, 1);

        // Get number of panes.
        let mut n = window_count_panes(w);
        if n <= 1 {
            return;
        }
        n -= 1; // take off main pane

        // Find available width - take off one line for the border.
        let sx = (*w).sx - 1;

        // Get the main pane width.
        let s = options_get_string_((*w).options, "main-pane-width");
        let mut mainw: u32 =
            args_string_percentage(s, 0, sx as i64, sx as i64, &raw mut cause) as u32;
        // C: default only when the option failed to parse (cause != NULL). The
        // check was inverted, so a valid main-pane-width was overwritten with 80.
        if !cause.is_null() {
            mainw = 80;
            free_(cause);
        }

        // Work out the other pane width.
        let mut otherw;
        if mainw + PANE_MINIMUM >= sx {
            if sx <= PANE_MINIMUM + PANE_MINIMUM {
                mainw = PANE_MINIMUM;
            } else {
                mainw = sx - PANE_MINIMUM;
            }
            otherw = PANE_MINIMUM;
        } else {
            let s = options_get_string_((*w).options, "other-pane-width");
            otherw = args_string_percentage(s, 0, sx as i64, sx as i64, &raw mut cause) as u32;
            if !cause.is_null() || otherw == 0 {
                otherw = sx - mainw;
                free_(cause);
            } else if otherw > sx || sx - otherw < mainw {
                otherw = sx - mainw;
            } else {
                mainw = sx - otherw;
            }
        }

        // Work out what height is needed.
        let mut sy = (n * (PANE_MINIMUM + 1)) - 1;
        if sy < (*w).sy {
            sy = (*w).sy;
        }

        // Free old tree and create a new root. Keep the leaf cells (only_nodes)
        // so each pane retains its current layout cell to reuse below.
        layout_free(w, 1);
        let lc = layout_create_cell(null_mut());
        (*w).layout_root = lc;
        layout_set_size(lc, mainw + otherw + 1, sy, 0, 0);
        layout_make_node(lc, layout_type::LAYOUT_LEFTRIGHT);

        // Use the first tiled pane as the main pane, reusing its cell.
        let wpmain = layout_set_first_tiled(w);
        let lcmain = (*wpmain).layout_cell;
        (*lcmain).parent = lc;
        layout_set_size(lcmain, mainw, sy, 0, 0);
        tailq_insert_tail(&raw mut (*lc).cells, lcmain);

        if n == 1 {
            // Reuse the single other pane's cell as-is (no resize): it keeps
            // its previous dimensions.
            let mut wp = tailq_next::<_, _, discr_entry>(wpmain);
            while !wp.is_null() && layout_cell_is_tiled((*wp).layout_cell) == 0 {
                wp = tailq_next::<_, _, discr_entry>(wp);
            }
            tailq_insert_tail(&raw mut (*lc).cells, (*wp).layout_cell);
            (*(*wp).layout_cell).parent = lc;
        } else {
            let lcother = layout_create_cell(lc);
            layout_set_size(lcother, otherw, sy, 0, 0);
            layout_make_node(lcother, layout_type::LAYOUT_TOPBOTTOM);
            tailq_insert_tail(&raw mut (*lc).cells, lcother);

            // Add the remaining panes as children, reusing their cells.
            for wp in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr) {
                if wp == wpmain {
                    continue;
                }
                let lcchild = (*wp).layout_cell;
                tailq_insert_tail(&raw mut (*lcother).cells, lcchild);
                (*lcchild).parent = lcother;
                if layout_cell_is_tiled(lcchild) != 0 {
                    layout_set_size(lcchild, otherw, PANE_MINIMUM, 0, 0);
                }
            }
            layout_spread_cell(w, lcother);
        }

        // Fix cell offsets.
        layout_fix_offsets(w);
        layout_fix_panes(w, null_mut());

        layout_print_cell((*w).layout_root, __func__, 1);

        window_resize(w, (*lc).sx, (*lc).sy, -1, -1);
        notify_window(c"window-layout-changed", w);
        server_redraw_window(w);
    }
}

/// C `vendor/tmux/layout-set.c:494`: `static void layout_set_main_v_mirrored(struct window *w)`
pub unsafe fn layout_set_main_v_mirrored(w: *mut window) {
    let __func__ = c!("layout_set_main_v_mirrored");
    unsafe {
        let mut cause: *mut u8 = null_mut();

        layout_print_cell((*w).layout_root, __func__, 1);

        // Get number of panes.
        let mut n = window_count_panes(w);
        if n <= 1 {
            return;
        }
        n -= 1; // take off main pane

        // Find available width - take off one line for the border.
        let sx = (*w).sx - 1;

        // Get the main pane width.
        let s = options_get_string_((*w).options, "main-pane-width");
        let mut mainw = args_string_percentage(s, 0, sx as i64, sx as i64, &raw mut cause) as u32;
        if !cause.is_null() {
            mainw = 80;
            free_(cause);
        }

        // Work out the other pane width.
        let mut otherw: u32;
        if mainw + PANE_MINIMUM >= sx {
            if sx <= PANE_MINIMUM + PANE_MINIMUM {
                mainw = PANE_MINIMUM;
            } else {
                mainw = sx - PANE_MINIMUM;
            }
            otherw = PANE_MINIMUM;
        } else {
            let s = options_get_string_((*w).options, "other-pane-width");
            otherw = args_string_percentage(s, 0, sx as i64, sx as i64, &raw mut cause) as u32;
            if !cause.is_null() || otherw == 0 {
                otherw = sx - mainw;
                free_(cause);
            } else if otherw > sx || sx - otherw < mainw {
                otherw = sx - mainw;
            } else {
                mainw = sx - otherw;
            }
        }

        // Work out what height is needed.
        let mut sy = (n * (PANE_MINIMUM + 1)) - 1;
        if sy < (*w).sy {
            sy = (*w).sy;
        }

        // Free old tree and create a new root. Keep the leaf cells (only_nodes)
        // so each pane retains its current layout cell to reuse below.
        layout_free(w, 1);
        let lc = layout_create_cell(null_mut());
        (*w).layout_root = lc;
        layout_set_size(lc, mainw + otherw + 1, sy, 0, 0);
        layout_make_node(lc, layout_type::LAYOUT_LEFTRIGHT);

        // Use the first tiled pane as the main pane, reusing its cell.
        let wpmain = layout_set_first_tiled(w);
        let lcmain = (*wpmain).layout_cell;
        (*lcmain).parent = lc;
        layout_set_size(lcmain, mainw, sy, 0, 0);
        tailq_insert_tail(&raw mut (*lc).cells, lcmain);

        // Mirrored: the other panes go at the head so they sit left of the main.
        if n == 1 {
            let mut wp = tailq_next::<_, _, discr_entry>(wpmain);
            while !wp.is_null() && layout_cell_is_tiled((*wp).layout_cell) == 0 {
                wp = tailq_next::<_, _, discr_entry>(wp);
            }
            tailq_insert_head(&raw mut (*lc).cells, (*wp).layout_cell);
            (*(*wp).layout_cell).parent = lc;
        } else {
            let lcother = layout_create_cell(lc);
            layout_make_node(lcother, layout_type::LAYOUT_TOPBOTTOM);
            layout_set_size(lcother, otherw, sy, 0, 0);
            tailq_insert_head(&raw mut (*lc).cells, lcother);

            // Add the remaining panes as children, reusing their cells.
            for wp in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr) {
                if wp == wpmain {
                    continue;
                }
                let lcchild = (*wp).layout_cell;
                tailq_insert_tail(&raw mut (*lcother).cells, lcchild);
                (*lcchild).parent = lcother;
                if layout_cell_is_tiled(lcchild) != 0 {
                    layout_set_size(lcchild, otherw, PANE_MINIMUM, 0, 0);
                }
            }
            layout_spread_cell(w, lcother);
        }

        // Fix cell offsets.
        layout_fix_offsets(w);
        layout_fix_panes(w, null_mut());

        layout_print_cell((*w).layout_root, __func__, 1);

        window_resize(w, (*lc).sx, (*lc).sy, -1, -1);
        notify_window(c"window-layout-changed", w);
        server_redraw_window(w);
    }
}

/// C `vendor/tmux/layout-set.c:592`: `static void layout_set_tiled(struct window *w)`
pub unsafe fn layout_set_tiled(w: *mut window) {
    let __func__ = c!("layout_set_tiled");

    unsafe {
        layout_print_cell((*w).layout_root, __func__, 1);

        // Get number of panes.
        let n = window_count_panes(w);
        if n <= 1 {
            return;
        }

        // Get maximum columns from window option.
        let max_columns = options_get_number_((*w).options, "tiled-layout-max-columns") as u32;

        // How many rows and columns are wanted?
        let mut rows = 1;
        let mut columns = 1;
        while rows * columns < n {
            rows += 1;
            if rows * columns < n && (max_columns == 0 || columns < max_columns) {
                columns += 1;
            }
        }

        // What width and height should they be?
        let mut width = ((*w).sx - (columns - 1)) / columns;
        if width < PANE_MINIMUM {
            width = PANE_MINIMUM;
        }
        let mut height = ((*w).sy - (rows - 1)) / rows;
        if height < PANE_MINIMUM {
            height = PANE_MINIMUM;
        }

        // Free old tree and create a new root.
        layout_free(w, 0);
        let lc = layout_create_cell(null_mut());
        (*w).layout_root = lc;
        let mut sx = ((width + 1) * columns) - 1;
        if sx < (*w).sx {
            sx = (*w).sx;
        }
        let mut sy = ((height + 1) * rows) - 1;
        if sy < (*w).sy {
            sy = (*w).sy;
        }
        layout_set_size(lc, sx, sy, 0, 0);
        layout_make_node(lc, layout_type::LAYOUT_TOPBOTTOM);

        // Create a grid of the cells.
        let mut wp = tailq_first(&raw mut (*w).panes);
        for j in 0..rows {
            // If this is the last cell, all done.
            if wp.is_null() {
                break;
            }

            // Create the new row.
            let lcrow = layout_create_cell(lc);
            layout_set_size(lcrow, (*w).sx, height, 0, 0);
            tailq_insert_tail(&raw mut (*lc).cells, lcrow);

            // If only one column, just use the row directly.
            if n - (j * columns) == 1 || columns == 1 {
                layout_make_leaf(lcrow, wp);
                wp = tailq_next::<_, _, discr_entry>(wp);
                continue;
            }

            // Add in the columns.
            layout_make_node(lcrow, layout_type::LAYOUT_LEFTRIGHT);
            let mut i = 0;
            for i_ in 0..columns {
                i = i_;
                // Create and add a pane cell.
                let lcchild = layout_create_cell(lcrow);
                layout_set_size(lcchild, width, height, 0, 0);
                layout_make_leaf(lcchild, wp);
                tailq_insert_tail(&raw mut (*lcrow).cells, lcchild);

                // Move to the next cell.
                wp = tailq_next::<_, _, discr_entry>(wp);
                if wp.is_null() {
                    break;
                }
                i += 1;
            }

            // Adjust the row and columns to fit the full width if necessary.
            if i == columns {
                i -= 1;
            }
            let used = ((i + 1) * (width + 1)) - 1;
            if (*w).sx <= used {
                continue;
            }
            let lcchild = tailq_last(&raw mut (*lcrow).cells);
            layout_resize_adjust(
                w,
                lcchild,
                layout_type::LAYOUT_LEFTRIGHT,
                ((*w).sx - used) as i32,
            );
        }

        // Adjust the last row height to fit if necessary.
        let used = (rows * height) + rows - 1;
        if (*w).sy > used {
            let lcrow = tailq_last(&raw mut (*lc).cells);
            layout_resize_adjust(
                w,
                lcrow,
                layout_type::LAYOUT_TOPBOTTOM,
                ((*w).sy - used) as i32,
            );
        }

        // Fix cell offsets.
        layout_fix_offsets(w);
        layout_fix_panes(w, null_mut());

        layout_print_cell((*w).layout_root, __func__, 1);

        window_resize(w, (*lc).sx, (*lc).sy, -1, -1);
        notify_window(c"window-layout-changed", w);
        server_redraw_window(w);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // LAYOUT_SETS order (vendor/tmux/layout-set.c):
    //   0 even-horizontal          4 main-vertical
    //   1 even-vertical            5 main-vertical-mirrored
    //   2 main-horizontal          6 tiled
    //   3 main-horizontal-mirrored

    #[test]
    fn lookup_exact_names() {
        unsafe {
            assert_eq!(layout_set_lookup(crate::c!("even-horizontal")), 0);
            assert_eq!(layout_set_lookup(crate::c!("even-vertical")), 1);
            assert_eq!(layout_set_lookup(crate::c!("tiled")), 6);
            // An exact match wins even when the name is a prefix of another
            // entry (main-horizontal vs main-horizontal-mirrored).
            assert_eq!(layout_set_lookup(crate::c!("main-horizontal")), 2);
            assert_eq!(layout_set_lookup(crate::c!("main-vertical")), 4);
        }
    }

    #[test]
    fn lookup_unique_prefix() {
        unsafe {
            // "ti" is a unique prefix of tiled only.
            assert_eq!(layout_set_lookup(crate::c!("ti")), 6);
            // "even-h" only matches even-horizontal (even-vertical differs at
            // the sixth character).
            assert_eq!(layout_set_lookup(crate::c!("even-h")), 0);
        }
    }

    #[test]
    fn lookup_ambiguous_and_unknown_return_minus_one() {
        unsafe {
            // "even" matches both even-horizontal and even-vertical.
            assert_eq!(layout_set_lookup(crate::c!("even")), -1);
            // "main-h" matches main-horizontal and main-horizontal-mirrored.
            assert_eq!(layout_set_lookup(crate::c!("main-h")), -1);
            // No layout has this name.
            assert_eq!(layout_set_lookup(crate::c!("does-not-exist")), -1);
        }
    }

    // Every full name resolves to its own index via the exact-match pass
    // (vendor/tmux/layout-set.c:53). This guards the LAYOUT_SETS ordering the
    // rest of the module (select/next/previous) depends on.
    #[test]
    fn lookup_every_name_roundtrips() {
        unsafe {
            for (i, ls) in LAYOUT_SETS.iter().enumerate() {
                assert_eq!(
                    layout_set_lookup(ls.name.as_ptr()),
                    i as i32,
                    "name at index {i} did not resolve to itself"
                );
            }
        }
    }

    // main-vertical vs main-vertical-mirrored: the bare name is an exact hit,
    // "main-v" is ambiguous, and the longer unique prefix reaches the mirror.
    #[test]
    fn lookup_main_vertical_variants() {
        unsafe {
            assert_eq!(layout_set_lookup(crate::c!("main-vertical")), 4);
            assert_eq!(layout_set_lookup(crate::c!("main-vertical-mirrored")), 5);
            // "main-v" prefixes both main-vertical and main-vertical-mirrored.
            assert_eq!(layout_set_lookup(crate::c!("main-v")), -1);
            // First character past "main-vertical" disambiguates to the mirror.
            assert_eq!(layout_set_lookup(crate::c!("main-vertical-m")), 5);
        }
    }

    // The empty string is a prefix of every entry (strncmp with length 0 returns
    // 0), so it is ambiguous across all seven and returns -1.
    #[test]
    fn lookup_empty_string_ambiguous() {
        unsafe {
            assert_eq!(layout_set_lookup(crate::c!("")), -1);
        }
    }

    // "tiled" has no siblings, so each of its proper prefixes resolves uniquely.
    #[test]
    fn lookup_tiled_prefixes_all_resolve() {
        unsafe {
            for p in ["t", "ti", "til", "tile", "tiled"] {
                let mut s = p.to_string();
                s.push('\0');
                assert_eq!(layout_set_lookup(s.as_bytes().as_ptr()), 6, "prefix {p:?}");
            }
        }
    }

    // Matching is case-sensitive (strcmp/strncmp): an upper-case name misses
    // entirely, and "main" alone is ambiguous across all four main-* layouts.
    #[test]
    fn lookup_case_sensitive_and_main_ambiguous() {
        unsafe {
            assert_eq!(layout_set_lookup(crate::c!("MAIN-VERTICAL")), -1);
            assert_eq!(layout_set_lookup(crate::c!("main")), -1);
            assert_eq!(layout_set_lookup(crate::c!("main-horizontal-mirrored")), 3);
        }
    }
}
