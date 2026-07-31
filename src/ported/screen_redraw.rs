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

//! Port of `vendor/tmux/screen-redraw.c`: draw the visible area of a window to
//! a client.
//!
//! A scene is built for the client and cached (in `struct client`). When the
//! offset or size of the visible part of the window changes, the scene is
//! invalidated. It is also invalidated when the generation number is
//! increased; this is done at various points, such as when a pane is moved or
//! resized. The scene only includes the part of the client used for the
//! window: panes, pane status lines, borders and any area outside the window.
//! The client status line and overlay are not included.
//!
//! A scene is made from spans. A span is a horizontal run of cells on one
//! visible line that can be drawn in the same way. Each span has a type, for
//! example: pane content or pane border cells or pane status line. A span also
//! includes enough additional data to draw it, but does not include items such
//! as the style and content — those are determined when it is drawn.
//!
//! Scenes are built in two stages:
//!
//! 1) [`redraw_build_cells`] fills a temporary grid of [`redraw_build_cell`]
//!    objects, one per visible cell. It marks pane contents, borders, pane
//!    status lines and any cells outside the window. Border cells may belong to
//!    multiple panes, so they may be marked multiple times, with each pane
//!    adding its own state.
//!
//! 2) [`redraw_make_scene`] takes the grid of [`redraw_build_cell`] objects and
//!    converts them into spans by joining adjacent cells that have the same
//!    type and data. These spans together make up the scene
//!    ([`redraw_scene`]).
//!
//! Once generated, a scene is redrawn by looping over some or all of the spans
//! (in [`redraw_draw`]), working out the style and content, and writing to the
//! client terminal. Until it is invalidated, the scene may be redrawn multiple
//! times without being rebuilt.
//!
//! ztmux divergences from the C, all structural rather than behavioural:
//!
//! * ztmux has no per-pane prompt line, so `redraw_draw_pane_prompt` has no
//!   counterpart here.
//! * The build grid is a `Vec` owned by the build context rather than the C's
//!   reused file-static buffer. Scenes are cached, so a build is rare.

use crate::*;
use crate::options_::*;

/// C `vendor/tmux/screen-redraw.c:101`: `#define REDRAW_START_ISOLATE`
const START_ISOLATE: *const u8 = c"\xe2\x81\xa6".as_ptr().cast();
/// C `vendor/tmux/screen-redraw.c:102`: `#define REDRAW_END_ISOLATE`
const END_ISOLATE: *const u8 = c"\xe2\x81\xa9".as_ptr().cast();

/// C `vendor/tmux/screen-redraw.c:236`: `static const char *redraw_flags_to_string(int flags)`
fn redraw_flags_to_string(flags: i32) -> String {
    let mut s = String::new();
    if flags & REDRAW_STATUS != 0 {
        s.push_str("status ");
    }
    if flags & REDRAW_PANE != 0 {
        s.push_str("pane ");
    }
    if flags & REDRAW_PANE_BORDER != 0 {
        s.push_str("border ");
    }
    if flags & REDRAW_PANE_STATUS != 0 {
        s.push_str("pane-status ");
    }
    if flags & REDRAW_PANE_SCROLLBAR != 0 {
        s.push_str("scrollbar ");
    }
    if flags & REDRAW_OVERLAY != 0 {
        s.push_str("overlay ");
    }
    if flags == REDRAW_ALL {
        s.push_str("all ");
    }
    let _ = s.pop();
    s
}

/// Get window offset and expand size to cover any part outside the window.
/// C `vendor/tmux/screen-redraw.c:264`: `static void redraw_get_window_offset(struct client *c, u_int *ox, u_int *oy, u_int *sx, u_int *sy)`
unsafe fn redraw_get_window_offset(
    c: *mut client,
    ox: *mut u32,
    oy: *mut u32,
    sx: *mut u32,
    sy: *mut u32,
) {
    unsafe {
        tty_window_offset(&raw mut (*c).tty, ox, oy, sx, sy);

        let tty_sx = (*c).tty.sx;
        let tty_sy = (*c).tty.sy.saturating_sub(status_line_size(c));
        if *sx < tty_sx {
            *sx = tty_sx;
        }
        if *sy < tty_sy {
            *sy = tty_sy;
        }
    }
}

/// Initialize the context for building scene.
/// C `vendor/tmux/screen-redraw.c:282`: `static void redraw_set_context(struct client *c, struct redraw_build_ctx *bctx)`
unsafe fn redraw_set_context(c: *mut client) -> redraw_build_ctx {
    unsafe {
        let s = (*c).session;
        let w = (*(*s).curw).window;

        let mut bctx = redraw_build_ctx {
            c,
            w,
            ox: 0,
            oy: 0,
            sx: 0,
            sy: 0,
            ind: 0,
            cells: Vec::new(),
        };
        redraw_get_window_offset(
            c,
            &raw mut bctx.ox,
            &raw mut bctx.oy,
            &raw mut bctx.sx,
            &raw mut bctx.sy,
        );

        bctx.ind = options_get_number_((*w).options, "pane-border-indicators") as i32;
        bctx
    }
}

/// Return a cell.
/// C `vendor/tmux/screen-redraw.c:296`: `static struct redraw_build_cell *redraw_get_build_cell(struct redraw_build_ctx *bctx, u_int x, u_int y)`
unsafe fn redraw_get_build_cell(
    bctx: *mut redraw_build_ctx,
    x: u32,
    y: u32,
) -> *mut redraw_build_cell {
    unsafe {
        let idx = (y as usize) * ((*bctx).sx as usize) + (x as usize);
        (*bctx).cells.as_mut_ptr().add(idx)
    }
}

/// Reset cell to either empty or outside the window.
/// C `vendor/tmux/screen-redraw.c:303`: `static void redraw_reset_cell(struct redraw_build_ctx *bctx, u_int x, u_int y)`
unsafe fn redraw_reset_cell(bctx: *mut redraw_build_ctx, x: u32, y: u32) {
    unsafe {
        let bc = redraw_get_build_cell(bctx, x, y);
        let w = (*bctx).w;

        *bc = redraw_build_cell::default();
        if (*bctx).ox + x <= (*w).sx && (*bctx).oy + y <= (*w).sy {
            (*bc).data.type_ = redraw_span_type::REDRAW_SPAN_EMPTY;
        } else {
            (*bc).data.type_ = redraw_span_type::REDRAW_SPAN_OUTSIDE;
        }
    }
}

/// Convert window position to scene position. Return 0 if outside the scene.
/// C `vendor/tmux/screen-redraw.c:316`: `static int redraw_window_to_scene(struct redraw_build_ctx *bctx, int wx, int wy, u_int *x, u_int *y)`
unsafe fn redraw_window_to_scene(
    bctx: *mut redraw_build_ctx,
    wx: i32,
    wy: i32,
    x: *mut u32,
    y: *mut u32,
) -> i32 {
    unsafe {
        if wx < 0 || wy < 0 {
            return 0;
        }
        if wx as u32 > (*(*bctx).w).sx || wy as u32 > (*(*bctx).w).sy {
            return 0;
        }
        if wx < (*bctx).ox as i32 || wy < (*bctx).oy as i32 {
            return 0;
        }

        let sx = wx - (*bctx).ox as i32;
        let sy = wy - (*bctx).oy as i32;

        if sx as u32 >= (*bctx).sx || sy as u32 >= (*bctx).sy {
            return 0;
        }
        *x = sx as u32;
        *y = sy as u32;
        1
    }
}

/// Convert pane position to scene position. Return 0 if outside the scene. A
/// floating pane is clipped to the window edge.
/// C `vendor/tmux/screen-redraw.c:343`: `static int redraw_pane_to_scene(struct redraw_build_ctx *bctx, struct window_pane *wp, int px, int py, u_int *x, u_int *y)`
unsafe fn redraw_pane_to_scene(
    bctx: *mut redraw_build_ctx,
    wp: *mut window_pane,
    px: i32,
    py: i32,
    x: *mut u32,
    y: *mut u32,
) -> i32 {
    unsafe {
        let (wx, wy) = ((*wp).xoff + px, (*wp).yoff + py);

        if window_pane_is_floating(wp) != 0 {
            let left = (*wp).xoff - 1;
            let right = (*wp).xoff + (*wp).sx as i32;
            let top = (*wp).yoff - 1;
            let bottom = (*wp).yoff + (*wp).sy as i32;

            if left < 0 && wx < 0 {
                return 0;
            }
            if right > (*(*bctx).w).sx as i32 && wx >= (*(*bctx).w).sx as i32 {
                return 0;
            }
            if top < 0 && wy < 0 {
                return 0;
            }
            if bottom > (*(*bctx).w).sy as i32 && wy >= (*(*bctx).w).sy as i32 {
                return 0;
            }
        }
        redraw_window_to_scene(bctx, wx, wy, x, y)
    }
}

/// Convert redraw border mask to a border cell type.
/// C `vendor/tmux/screen-redraw.c:369`: `static int redraw_get_cell_type(int mask)`
fn redraw_get_cell_type(mask: i32) -> cell_type {
    const L: i32 = REDRAW_BORDER_L;
    const R: i32 = REDRAW_BORDER_R;
    const U: i32 = REDRAW_BORDER_U;
    const D: i32 = REDRAW_BORDER_D;

    match mask {
        m if m == L | R | U | D => cell_type::CELL_JOIN,
        m if m == L | R | U => cell_type::CELL_BOTTOMJOIN,
        m if m == L | R | D => cell_type::CELL_TOPJOIN,
        m if m == L | R || m == L || m == R => cell_type::CELL_LEFTRIGHT,
        m if m == L | U | D => cell_type::CELL_RIGHTJOIN,
        m if m == L | U => cell_type::CELL_BOTTOMRIGHT,
        m if m == L | D => cell_type::CELL_TOPRIGHT,
        m if m == R | U | D => cell_type::CELL_LEFTJOIN,
        m if m == R | U => cell_type::CELL_BOTTOMLEFT,
        m if m == R | D => cell_type::CELL_TOPLEFT,
        m if m == U | D || m == U || m == D => cell_type::CELL_TOPBOTTOM,
        _ => cell_type::CELL_OUTSIDE,
    }
}

/// Return if there are two panes for the border colour indicator.
/// C `vendor/tmux/screen-redraw.c:404`: `static int redraw_check_two_pane_colours(struct window *w, enum layout_type *type)`
unsafe fn redraw_check_two_pane_colours(w: *mut window, type_: *mut layout_type) -> i32 {
    unsafe {
        let mut count = 0;

        for wp in tailq_foreach::<_, discr_entry>(&raw mut (*w).panes).map(NonNull::as_ptr) {
            if window_pane_is_floating(wp) != 0 || (*wp).layout_cell.is_null() {
                continue;
            }
            count += 1;
            if count > 2 || (*(*wp).layout_cell).parent.is_null() {
                return 0;
            }
            *type_ = (*(*(*wp).layout_cell).parent).type_;
        }
        i32::from(count == 2)
    }
}

/// Mark the inside of a pane.
/// C `vendor/tmux/screen-redraw.c:423`: `static void redraw_mark_pane_inside(struct redraw_build_ctx *bctx, struct window_pane *wp)`
unsafe fn redraw_mark_pane_inside(bctx: *mut redraw_build_ctx, wp: *mut window_pane) {
    unsafe {
        let (mut x, mut y) = (0u32, 0u32);

        for py in 0..(*wp).sy {
            for px in 0..(*wp).sx {
                if redraw_pane_to_scene(
                    bctx,
                    wp,
                    px as i32,
                    py as i32,
                    &raw mut x,
                    &raw mut y,
                ) == 0
                {
                    continue;
                }
                let bc = redraw_get_build_cell(bctx, x, y);
                *bc = redraw_build_cell::default();
                (*bc).data.type_ = redraw_span_type::REDRAW_SPAN_PANE;
                (*bc).data.p_wp = wp;
                (*bc).data.p_px = px;
                (*bc).data.p_py = py;
            }
        }
    }
}

/// Mark scrollbar data.
/// C `vendor/tmux/screen-redraw.c:444`: `static void redraw_mark_pane_scrollbar(struct redraw_build_ctx *bctx, struct window_pane *wp, int sb_w, int sb_left, int overlay)`
unsafe fn redraw_mark_pane_scrollbar(
    bctx: *mut redraw_build_ctx,
    wp: *mut window_pane,
    sb_w: i32,
    sb_left: i32,
    overlay: i32,
) {
    unsafe {
        let (mut x, mut y) = (0u32, 0u32);
        let (sx, ex);

        if sb_w == 0 {
            return;
        }

        if overlay != 0 && sb_left != 0 {
            sx = (*wp).xoff;
            ex = sx + sb_w - 1;
        } else if overlay != 0 {
            ex = (*wp).xoff + (*wp).sx as i32 - 1;
            sx = ex - sb_w + 1;
        } else if sb_left != 0 {
            sx = (*wp).xoff - sb_w;
            ex = (*wp).xoff - 1;
        } else {
            sx = (*wp).xoff + (*wp).sx as i32;
            ex = sx + sb_w - 1;
        }

        for sy in 0..(*wp).sy {
            let wy = (*wp).yoff + sy as i32;
            for wx in sx..=ex {
                if redraw_window_to_scene(bctx, wx, wy, &raw mut x, &raw mut y) == 0 {
                    continue;
                }
                let bc = redraw_get_build_cell(bctx, x, y);
                *bc = redraw_build_cell::default();
                (*bc).data.type_ = redraw_span_type::REDRAW_SPAN_SCROLLBAR;
                (*bc).data.sb_wp = wp;
                (*bc).data.sb_y = sy;
                (*bc).data.sb_height = (*wp).sy;
                if sb_left != 0 {
                    (*bc).data.sb_flags |= REDRAW_SCROLLBAR_LEFT;
                } else {
                    (*bc).data.sb_flags |= REDRAW_SCROLLBAR_RIGHT;
                }
                if overlay != 0 {
                    (*bc).data.sb_flags |= REDRAW_SCROLLBAR_OVERLAY;
                }
            }
        }
    }
}

/// Return if span data belongs to pane, that is: is the cell adjacent to this
/// pane?
/// C `vendor/tmux/screen-redraw.c:494`: `static int redraw_data_has_pane(struct redraw_span_data *data, struct window_pane *wp)`
unsafe fn redraw_data_has_pane(data: *const redraw_span_data, wp: *mut window_pane) -> i32 {
    unsafe {
        i32::from(
            (*data).b_top_wp == wp
                || (*data).b_bottom_wp == wp
                || (*data).b_left_wp == wp
                || (*data).b_right_wp == wp,
        )
    }
}

/// Mark one border cell. If a non-border cell is marked as a border, replace
/// it. If it is already a border and this is not a floating pane, merge the
/// border mask and pane ownership.
/// C `vendor/tmux/screen-redraw.c:513`: `static void redraw_mark_border_cell(struct redraw_build_ctx *bctx, int wx, int wy, struct window_pane *wp, int top_owner, int bottom_owner, int mask, enum pane_lines pane_lines, int floating)`
#[expect(clippy::too_many_arguments)]
unsafe fn redraw_mark_border_cell(
    bctx: *mut redraw_build_ctx,
    wx: i32,
    wy: i32,
    wp: *mut window_pane,
    top_owner: i32,
    bottom_owner: i32,
    mut mask: i32,
    pane_lines: pane_lines,
    floating: i32,
) {
    unsafe {
        let (mut x, mut y) = (0u32, 0u32);
        let mut reset = 0;

        if redraw_window_to_scene(bctx, wx, wy, &raw mut x, &raw mut y) == 0 {
            return;
        }
        let bc = redraw_get_build_cell(bctx, x, y);

        // If this is a tiled pane, only empty and border cells may be marked.
        // Border cells are merged and empty cells are updated.
        //
        // Floating panes may mark any existing cell type. All cells are reset
        // except borders that already belong to this pane, they need to be
        // merged.
        if floating == 0 {
            if (*bc).data.type_ == redraw_span_type::REDRAW_SPAN_EMPTY {
                reset = 1;
            } else if (*bc).data.type_ != redraw_span_type::REDRAW_SPAN_BORDER {
                return;
            }
        } else if (*bc).data.type_ != redraw_span_type::REDRAW_SPAN_BORDER
            || redraw_data_has_pane(&raw const (*bc).data, wp) == 0
        {
            reset = 1;
        }
        if reset != 0 {
            *bc = redraw_build_cell::default();
            (*bc).data.type_ = redraw_span_type::REDRAW_SPAN_BORDER;
        }

        if top_owner != 0 {
            (*bc).data.b_top_wp = wp;
            (*bc).data.b_top_lines = pane_lines;
        }
        if bottom_owner != 0 {
            (*bc).data.b_bottom_wp = wp;
            (*bc).data.b_bottom_lines = pane_lines;
        }

        if mask & (REDRAW_BORDER_U | REDRAW_BORDER_D) != 0 {
            if wx < (*wp).xoff {
                (*bc).data.b_right_wp = wp;
                (*bc).data.b_right_lines = pane_lines;
            } else if wx >= (*wp).xoff + (*wp).sx as i32 {
                (*bc).data.b_left_wp = wp;
                (*bc).data.b_left_lines = pane_lines;
            }
        }

        mask |= (*bc).data.b_cell_mask;
        (*bc).data.b_cell_mask = mask;
        (*bc).data.b_cell_type = redraw_get_cell_type(mask);
    }
}

/// Mark border cells for a pane status line, keeping the border cell type for
/// drawing.
/// C `vendor/tmux/screen-redraw.c:578`: `static void redraw_mark_border_status(struct redraw_build_ctx *bctx, struct window_pane *wp, int left, int right, int top, int bottom)`
unsafe fn redraw_mark_border_status(
    bctx: *mut redraw_build_ctx,
    wp: *mut window_pane,
    _left: i32,
    right: i32,
    top: i32,
    bottom: i32,
) {
    unsafe {
        let (mut x, mut y) = (0u32, 0u32);

        let pane_status = window_pane_get_pane_status(wp);
        if pane_status == pane_status::PANE_STATUS_OFF {
            return;
        }
        let wy = if pane_status == pane_status::PANE_STATUS_TOP {
            top
        } else {
            bottom
        };

        let sx = (*wp).xoff + 2;
        let ex = right - 1;
        if sx > ex {
            return;
        }

        let mut off = 0;
        for wx in sx..=ex {
            if redraw_window_to_scene(bctx, wx, wy, &raw mut x, &raw mut y) != 0 {
                let bc = redraw_get_build_cell(bctx, x, y);
                if (*bc).data.type_ == redraw_span_type::REDRAW_SPAN_BORDER {
                    let cell_type = (*bc).data.b_cell_type;

                    (*bc).data.type_ = redraw_span_type::REDRAW_SPAN_STATUS;
                    (*bc).data.st_wp = wp;
                    (*bc).data.st_offset = off;
                    (*bc).data.st_cell_type = cell_type;
                }
            }
            off += 1;
        }
    }
}

/// Mark existing border cells where indicator arrows will be drawn.
/// C `vendor/tmux/screen-redraw.c:616`: `static void redraw_mark_border_arrows(struct redraw_build_ctx *bctx, struct window_pane *wp, int left, int right, int top, int bottom)`
unsafe fn redraw_mark_border_arrows(
    bctx: *mut redraw_build_ctx,
    wp: *mut window_pane,
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
) {
    unsafe {
        let (mut x, mut y) = (0u32, 0u32);

        if (*bctx).ind != pane_border_indicator::PANE_BORDER_ARROWS as i32
            && (*bctx).ind != pane_border_indicator::PANE_BORDER_BOTH as i32
        {
            return;
        }

        let mut mark = |wx: i32, wy: i32| {
            if redraw_window_to_scene(bctx, wx, wy, &raw mut x, &raw mut y) != 0 {
                let bc = redraw_get_build_cell(bctx, x, y);
                if (*bc).data.type_ == redraw_span_type::REDRAW_SPAN_BORDER {
                    (*bc).data.b_flags |= REDRAW_BORDER_IS_ARROW;
                }
            }
        };

        let wx = (*wp).xoff + 1;
        if wx >= left && wx <= right {
            mark(wx, top);
            mark(wx, bottom);
        }

        let wy = (*wp).yoff + 1;
        if wy >= top && wy <= bottom {
            mark(left, wy);
            mark(right, wy);
        }
    }
}

/// Mark pane borders.
/// C `vendor/tmux/screen-redraw.c:657`: `static void redraw_mark_pane_borders(struct redraw_build_ctx *bctx, struct window_pane *wp, int sb_w, int sb_left)`
unsafe fn redraw_mark_pane_borders(
    bctx: *mut redraw_build_ctx,
    wp: *mut window_pane,
    sb_w: i32,
    sb_left: i32,
) {
    unsafe {
        let pane_lines = window_pane_get_pane_lines(wp);
        let floating = window_pane_is_floating(wp);

        if floating != 0 && pane_lines == pane_lines::PANE_LINES_NONE {
            return;
        }
        let pane_status = window_pane_get_pane_status(wp);

        let mut left = (*wp).xoff - 1;
        let mut right = (*wp).xoff + (*wp).sx as i32;
        if sb_w != 0 {
            if sb_left != 0 {
                left -= sb_w;
            } else {
                right += sb_w;
            }
        }
        let mut top = (*wp).yoff - 1;
        let mut bottom = (*wp).yoff + (*wp).sy as i32;

        let mark_left = left >= 0;
        let mark_right = right <= (*(*bctx).w).sx as i32;
        let mut mark_top = top >= 0;
        let mut mark_bottom = bottom <= (*(*bctx).w).sy as i32;

        if floating != 0 {
            if left < 0 {
                left = 0;
            }
            if right > (*(*bctx).w).sx as i32 {
                right = (*(*bctx).w).sx as i32 - 1;
            }
            if top < 0 {
                top = 0;
            }
            if bottom > (*(*bctx).w).sy as i32 {
                bottom = (*(*bctx).w).sy as i32 - 1;
            }
        } else if pane_status == pane_status::PANE_STATUS_TOP {
            mark_bottom = false;
        } else if pane_status == pane_status::PANE_STATUS_BOTTOM {
            mark_top = false;
        }

        if mark_top {
            for wx in left..=right {
                let mut mask = 0;
                if wx > left {
                    mask |= REDRAW_BORDER_L;
                }
                if wx < right {
                    mask |= REDRAW_BORDER_R;
                }
                redraw_mark_border_cell(bctx, wx, top, wp, 0, 1, mask, pane_lines, floating);
            }
        }
        if mark_bottom {
            for wx in left..=right {
                let mut mask = 0;
                if wx > left {
                    mask |= REDRAW_BORDER_L;
                }
                if wx < right {
                    mask |= REDRAW_BORDER_R;
                }
                redraw_mark_border_cell(bctx, wx, bottom, wp, 1, 0, mask, pane_lines, floating);
            }
        }
        if mark_left {
            for wy in top..=bottom {
                let mut mask = 0;
                if wy > top {
                    mask |= REDRAW_BORDER_U;
                }
                if wy < bottom {
                    mask |= REDRAW_BORDER_D;
                }
                redraw_mark_border_cell(bctx, left, wy, wp, 0, 0, mask, pane_lines, floating);
            }
        }
        if mark_right {
            for wy in top..=bottom {
                let mut mask = 0;
                if wy > top {
                    mask |= REDRAW_BORDER_U;
                }
                if wy < bottom {
                    mask |= REDRAW_BORDER_D;
                }
                redraw_mark_border_cell(bctx, right, wy, wp, 0, 0, mask, pane_lines, floating);
            }
        }

        redraw_mark_border_status(bctx, wp, left, right, top, bottom);
        redraw_mark_border_arrows(bctx, wp, left, right, top, bottom);
    }
}

/// Mark an entire pane in the build grid. Floating panes overwrite anything
/// already below them.
/// C `vendor/tmux/screen-redraw.c:759`: `static void redraw_mark_pane(struct redraw_build_ctx *bctx, struct window_pane *wp)`
unsafe fn redraw_mark_pane(bctx: *mut redraw_build_ctx, wp: *mut window_pane) {
    unsafe {
        let (mut sb_w, mut sb_left, mut overlay) = (0i32, 0i32, 0i32);

        if !window_pane_visible(wp) {
            return;
        }

        if window_pane_scrollbar_visible(wp) != 0 {
            overlay = window_pane_scrollbar_overlay(wp);
            if overlay != 0 {
                sb_w = (*wp).scrollbar_style.width + (*wp).scrollbar_style.pad;
                if sb_w > (*wp).sx as i32 {
                    sb_w = (*wp).scrollbar_style.width;
                    if sb_w > (*wp).sx as i32 {
                        sb_w = (*wp).sx as i32;
                    }
                }
            } else {
                sb_w = (*wp).scrollbar_style.width + (*wp).scrollbar_style.pad;
            }
        }
        if sb_w != 0 && (*(*bctx).w).sb_pos == PANE_SCROLLBARS_LEFT {
            sb_left = 1;
        }

        redraw_mark_pane_inside(bctx, wp);
        redraw_mark_pane_borders(bctx, wp, if overlay != 0 { 0 } else { sb_w }, sb_left);
        redraw_mark_pane_scrollbar(bctx, wp, sb_w, sb_left, overlay);
    }
}

/// Choose the pane that will provide the border style for two-pane layouts.
/// C `vendor/tmux/screen-redraw.c:787`: `static void redraw_mark_two_pane_colours(struct redraw_build_ctx *bctx)`
unsafe fn redraw_mark_two_pane_colours(bctx: *mut redraw_build_ctx) {
    unsafe {
        let mut type_ = layout_type::LAYOUT_WINDOWPANE;

        if (*bctx).ind != pane_border_indicator::PANE_BORDER_COLOUR as i32
            && (*bctx).ind != pane_border_indicator::PANE_BORDER_BOTH as i32
        {
            return;
        }
        if redraw_check_two_pane_colours((*bctx).w, &raw mut type_) == 0 {
            return;
        }

        for y in 0..(*bctx).sy {
            for x in 0..(*bctx).sx {
                let bc = redraw_get_build_cell(bctx, x, y);
                if (*bc).data.type_ != redraw_span_type::REDRAW_SPAN_BORDER {
                    continue;
                }
                let sd = &raw mut (*bc).data;

                let wx = (*bctx).ox + x;
                let wy = (*bctx).oy + y;

                if type_ == layout_type::LAYOUT_LEFTRIGHT
                    && !(*sd).b_left_wp.is_null()
                    && !(*sd).b_right_wp.is_null()
                {
                    (*sd).b_style_wp = if wy <= (*(*bctx).w).sy / 2 {
                        (*sd).b_left_wp
                    } else {
                        (*sd).b_right_wp
                    };
                } else if type_ == layout_type::LAYOUT_TOPBOTTOM
                    && !(*sd).b_top_wp.is_null()
                    && !(*sd).b_bottom_wp.is_null()
                {
                    (*sd).b_style_wp = if wx <= (*(*bctx).w).sx / 2 {
                        (*sd).b_top_wp
                    } else {
                        (*sd).b_bottom_wp
                    };
                }
            }
        }
    }
}

/// Return true if two adjacent build cells can be joined into one span.
/// C `vendor/tmux/screen-redraw.c:827`: `static int redraw_compare_data(struct redraw_build_cell *a, struct redraw_build_cell *b)`
unsafe fn redraw_compare_data(a: *const redraw_build_cell, b: *const redraw_build_cell) -> i32 {
    unsafe {
        let (ad, bd) = (&raw const (*a).data, &raw const (*b).data);

        if (*ad).type_ != (*bd).type_ {
            return 0;
        }

        match (*ad).type_ {
            redraw_span_type::REDRAW_SPAN_PANE => i32::from(
                (*ad).p_wp == (*bd).p_wp
                    && (*ad).p_py == (*bd).p_py
                    && (*ad).p_px + 1 == (*bd).p_px,
            ),
            redraw_span_type::REDRAW_SPAN_BORDER => {
                if (*ad).b_top_wp != (*bd).b_top_wp
                    || (*ad).b_bottom_wp != (*bd).b_bottom_wp
                    || (*ad).b_left_wp != (*bd).b_left_wp
                    || (*ad).b_right_wp != (*bd).b_right_wp
                    || (*ad).b_style_wp != (*bd).b_style_wp
                    || (*ad).b_top_lines != (*bd).b_top_lines
                    || (*ad).b_bottom_lines != (*bd).b_bottom_lines
                    || (*ad).b_left_lines != (*bd).b_left_lines
                    || (*ad).b_right_lines != (*bd).b_right_lines
                    || (*ad).b_cell_type != (*bd).b_cell_type
                    || (*ad).b_cell_mask != (*bd).b_cell_mask
                    || (*ad).b_flags != (*bd).b_flags
                {
                    return 0;
                }
                i32::from((*ad).b_flags & REDRAW_BORDER_IS_ARROW == 0)
            }
            redraw_span_type::REDRAW_SPAN_STATUS => i32::from(
                (*ad).st_wp == (*bd).st_wp
                    && (*ad).st_offset + 1 == (*bd).st_offset
                    && (*ad).st_cell_type == (*bd).st_cell_type,
            ),
            redraw_span_type::REDRAW_SPAN_SCROLLBAR => i32::from(
                (*ad).sb_wp == (*bd).sb_wp
                    && (*ad).sb_y == (*bd).sb_y
                    && (*ad).sb_height == (*bd).sb_height
                    && (*ad).sb_flags == (*bd).sb_flags,
            ),
            redraw_span_type::REDRAW_SPAN_OUTSIDE | redraw_span_type::REDRAW_SPAN_EMPTY => 1,
        }
    }
}

/// Build the temporary cells for a redraw scene.
/// C `vendor/tmux/screen-redraw.c:880`: `static void redraw_build_cells(struct redraw_build_ctx *bctx)`
unsafe fn redraw_build_cells(bctx: *mut redraw_build_ctx) {
    unsafe {
        let w = (*bctx).w;

        let ncells = (*bctx).sx as usize * (*bctx).sy as usize;
        (*bctx).cells.clear();
        (*bctx).cells.resize(ncells, redraw_build_cell::default());

        for y in 0..(*bctx).sy {
            for x in 0..(*bctx).sx {
                redraw_reset_cell(bctx, x, y);
            }
        }

        for wp in
            tailq_foreach_reverse::<_, discr_zentry>(&raw mut (*w).z_index).map(NonNull::as_ptr)
        {
            redraw_mark_pane(bctx, wp);
        }
        redraw_mark_two_pane_colours(bctx);
    }
}

/// Build and return a redraw scene for a client. The caller owns the scene and
/// must free it with `redraw_free_scene`.
/// C `vendor/tmux/screen-redraw.c:917`: `static struct redraw_scene *redraw_make_scene(struct client *c)`
unsafe fn redraw_make_scene(c: *mut client) -> *mut redraw_scene {
    unsafe {
        let s = (*c).session;
        let w = (*(*s).curw).window;

        if (*c).flags.intersects(client_flag::SUSPENDED) {
            return null_mut();
        }

        let mut bctx = redraw_set_context(c);

        log_debug!(
            "{}: building @{} scene ({}x{} {},{}; generation {})",
            _s((*c).name),
            (*w).id,
            bctx.sx,
            bctx.sy,
            bctx.ox,
            bctx.oy,
            (*w).redraw_scene_generation,
        );

        redraw_build_cells(&raw mut bctx);

        let mut lines = Vec::with_capacity(bctx.sy as usize);
        for y in 0..bctx.sy {
            let mut line = redraw_line::default();

            let mut x = 0;
            while x < bctx.sx {
                let x0 = x;
                let mut last = redraw_get_build_cell(&raw mut bctx, x, y);
                x += 1;

                while x < bctx.sx {
                    let bc = redraw_get_build_cell(&raw mut bctx, x, y);
                    if redraw_compare_data(last, bc) == 0 {
                        break;
                    }
                    last = bc;
                    x += 1;
                }
                let bc = redraw_get_build_cell(&raw mut bctx, x0, y);
                let type_ = (*bc).data.type_;

                line.spans[type_ as usize].push(redraw_span {
                    x: x0,
                    width: x - x0,
                    data: (*bc).data,
                });
            }
            lines.push(line);
        }

        log_debug!("{}: finished building @{} scene", _s((*c).name), (*w).id);
        Box::into_raw(Box::new(redraw_scene {
            c,
            w,
            lines,
            generation: (*w).redraw_scene_generation,
            sx: bctx.sx,
            sy: bctx.sy,
            ox: bctx.ox,
            oy: bctx.oy,
        }))
    }
}

/// Free a scene.
/// C `vendor/tmux/screen-redraw.c:982`: `void redraw_free_scene(struct redraw_scene *scene)`
pub unsafe fn redraw_free_scene(scene: *mut redraw_scene) {
    unsafe {
        if !scene.is_null() {
            drop(Box::from_raw(scene));
        }
    }
}

/// Mark a window's cached redraw scenes as out of date.
/// C `vendor/tmux/screen-redraw.c:1006`: `void redraw_invalidate_scene(struct window *w)`
pub unsafe fn redraw_invalidate_scene(w: *mut window) {
    unsafe {
        (*w).redraw_scene_generation += 1;
    }
}

/// Mark all cached redraw scenes as out of date.
/// C `vendor/tmux/screen-redraw.c:1013`: `void redraw_invalidate_all_scenes(void)`
pub unsafe fn redraw_invalidate_all_scenes() {
    unsafe {
        for w in rb_foreach(&raw mut WINDOWS).map(NonNull::as_ptr) {
            redraw_invalidate_scene(w);
        }
    }
}

/// Get the cached redraw scene, rebuilding it if needed.
/// C `vendor/tmux/screen-redraw.c:1024`: `static struct redraw_scene *redraw_get_scene(struct client *c)`
unsafe fn redraw_get_scene(c: *mut client) -> *mut redraw_scene {
    unsafe {
        let mut scene = (*c).redraw_scene;
        let w = (*(*(*c).session).curw).window;
        let (mut ox, mut oy, mut sx, mut sy) = (0u32, 0u32, 0u32, 0u32);

        redraw_get_window_offset(
            c,
            &raw mut ox,
            &raw mut oy,
            &raw mut sx,
            &raw mut sy,
        );
        let reason = if scene.is_null() {
            "missing"
        } else if (*scene).w != w {
            "window changed"
        } else if (*scene).generation != (*w).redraw_scene_generation {
            "generation changed"
        } else if (*scene).ox != ox || (*scene).oy != oy {
            "offset changed"
        } else if (*scene).sx != sx || (*scene).sy != sy {
            "size changed"
        } else {
            ""
        };
        if !reason.is_empty() {
            log_debug!(
                "{}: @{} scene invalid: {}",
                _s((*c).name),
                (*w).id,
                reason,
            );
            redraw_free_scene(scene);
            scene = redraw_make_scene(c);
            (*c).redraw_scene = scene;
        }
        scene
    }
}

/// Draw a pane span.
/// C `vendor/tmux/screen-redraw.c:1053`: `static void redraw_draw_pane_span(struct redraw_draw_ctx *dctx, struct redraw_span *span, u_int x, u_int y, u_int n)`
unsafe fn redraw_draw_pane_span(
    dctx: *mut redraw_draw_ctx,
    span: *const redraw_span,
    x: u32,
    y: u32,
    n: u32,
) {
    unsafe {
        let scene = (*dctx).scene;
        let c = (*scene).c;
        let tty = &raw mut (*c).tty;
        let wp = (*span).data.p_wp;
        let s = (*wp).screen;
        let mut defaults: grid_cell = zeroed();

        tty_default_colours(&raw mut defaults, wp);

        let px = (*span).data.p_px + (x - (*span).x);
        let py = (*span).data.p_py;
        tty_draw_line(
            tty,
            s,
            px,
            py,
            n,
            x,
            y,
            &raw const defaults,
            &raw const (*wp).palette,
        );
    }
}

/// Get default border style for spans without a pane.
/// C `vendor/tmux/screen-redraw.c:1077`: `static void redraw_get_default_border_style(struct redraw_draw_ctx *dctx, struct grid_cell *gc, enum pane_lines *pane_lines)`
unsafe fn redraw_get_default_border_style(
    dctx: *mut redraw_draw_ctx,
    gc: *mut grid_cell,
    pane_lines: *mut pane_lines,
) {
    unsafe {
        let scene = (*dctx).scene;
        let c = (*scene).c;
        let s = (*c).session;
        let oo = (*(*scene).w).options;
        let dgc = &raw mut (*dctx).default_gc;

        if (*dctx).flags & REDRAW_DEFAULT_SET == 0 {
            let ft = format_create_defaults(null_mut(), c, s, (*s).curw, null_mut());
            memcpy__(dgc, &raw const GRID_DEFAULT_CELL);
            style_add(dgc, oo, c!("pane-border-style"), ft);
            format_free(ft);
            (*dctx).pane_lines = options_get_number_(oo, "pane-border-lines")
                .try_into()
                .map_or(crate::pane_lines::PANE_LINES_SINGLE, |v: i32| {
                    crate::pane_lines::try_from(v).unwrap_or(crate::pane_lines::PANE_LINES_SINGLE)
                });
            (*dctx).flags |= REDRAW_DEFAULT_SET;
        }
        memcpy__(gc, dgc);
        *pane_lines = (*dctx).pane_lines;
    }
}

/// For this border span, pick the pane whose border style should colour it.
/// Prefer an explicitly assigned style owner, then the active adjacent pane,
/// then any adjacent pane.
/// C `vendor/tmux/screen-redraw.c:1106`: `static struct window_pane *redraw_get_pane_for_border_style(struct redraw_draw_ctx *dctx, struct redraw_span *span)`
unsafe fn redraw_get_pane_for_border_style(
    dctx: *mut redraw_draw_ctx,
    span: *const redraw_span,
) -> *mut window_pane {
    unsafe {
        let active = (*dctx).active;

        if (*span).data.type_ != redraw_span_type::REDRAW_SPAN_BORDER {
            return null_mut();
        }

        if !(*span).data.b_style_wp.is_null() {
            return (*span).data.b_style_wp;
        }
        if !active.is_null() && redraw_data_has_pane(&raw const (*span).data, active) != 0 {
            return active;
        }

        for wp in [
            (*span).data.b_top_wp,
            (*span).data.b_bottom_wp,
            (*span).data.b_left_wp,
            (*span).data.b_right_wp,
        ] {
            if !wp.is_null() {
                return wp;
            }
        }
        null_mut()
    }
}

/// Draw arrow indicator if this border span is an arrow cell.
/// C `vendor/tmux/screen-redraw.c:1130`: `static void redraw_draw_border_arrow(struct redraw_draw_ctx *dctx, struct redraw_span *span, struct grid_cell *gc)`
unsafe fn redraw_draw_border_arrow(
    dctx: *mut redraw_draw_ctx,
    span: *const redraw_span,
    gc: *mut grid_cell,
) {
    unsafe {
        let active = (*dctx).active;

        if (*span).data.type_ != redraw_span_type::REDRAW_SPAN_BORDER || active.is_null() {
            return;
        }
        if (*span).data.b_flags & REDRAW_BORDER_IS_ARROW == 0 {
            return;
        }

        let ch = if (*span).data.b_left_wp == active {
            b','
        } else if (*span).data.b_right_wp == active {
            b'+'
        } else if (*span).data.b_top_wp == active {
            b'-'
        } else if (*span).data.b_bottom_wp == active {
            b'.'
        } else {
            return;
        };

        utf8_set(&raw mut (*gc).data, ch);
        (*gc).attr |= grid_attr::GRID_ATTR_CHARSET;
    }
}

/// Draw a border span.
/// C `vendor/tmux/screen-redraw.c:1159`: `static void redraw_draw_border_span(struct redraw_draw_ctx *dctx, struct redraw_span *span, u_int x, u_int y, u_int n)`
unsafe fn redraw_draw_border_span(
    dctx: *mut redraw_draw_ctx,
    span: *const redraw_span,
    x: u32,
    y: u32,
    n: u32,
) {
    unsafe {
        let scene = (*dctx).scene;
        let c = (*scene).c;
        let tty = &raw mut (*c).tty;
        let w = (*scene).w;
        let mut gc: grid_cell = zeroed();
        let mut pane_lines = crate::pane_lines::PANE_LINES_SINGLE;

        let (wp, cell_type) = if (*span).data.type_ != redraw_span_type::REDRAW_SPAN_BORDER {
            (null_mut(), cell_type::CELL_OUTSIDE)
        } else {
            (
                redraw_get_pane_for_border_style(dctx, span),
                (*span).data.b_cell_type,
            )
        };

        if wp.is_null() {
            redraw_get_default_border_style(dctx, &raw mut gc, &raw mut pane_lines);
            window_get_border_cell(w, null_mut(), pane_lines, cell_type, &raw mut gc);
        } else {
            window_pane_get_border_style(wp, c, &raw mut gc);
            window_pane_get_border_cell(wp, cell_type, &raw mut gc);
        }

        if (*span).data.type_ == redraw_span_type::REDRAW_SPAN_BORDER
            && !(*dctx).marked.is_null()
            && redraw_data_has_pane(&raw const (*span).data, (*dctx).marked) != 0
        {
            gc.attr ^= grid_attr::GRID_ATTR_REVERSE;
        }
        redraw_draw_border_arrow(dctx, span, &raw mut gc);

        // ztmux: in the framed (inset) mode our ratatui pane frames ARE the
        // borders, so blank tmux's own border cells — otherwise the leftover
        // border column between two panes' frames shows as a stray dotted line.
        if crate::extensions::ratatui_ui::frame_inset() != 0 {
            memcpy__(&raw mut gc, &raw const GRID_DEFAULT_CELL);
        }

        let isolates = cell_type == cell_type::CELL_TOPBOTTOM
            && (*dctx).flags & REDRAW_ISOLATES != 0;
        tty_cursor(tty, x, y);
        if isolates {
            tty_puts(tty, END_ISOLATE);
        }
        for _ in 0..n {
            tty_cell(tty, &raw mut gc, &GRID_DEFAULT_CELL, null_mut(), null_mut());
        }
        if isolates {
            tty_puts(tty, START_ISOLATE);
        }
    }
}

/// Draw a pane status span.
/// C `vendor/tmux/screen-redraw.c:1206`: `static void redraw_draw_status_span(struct redraw_draw_ctx *dctx, struct redraw_span *span, u_int x, u_int y, u_int n)`
unsafe fn redraw_draw_status_span(
    dctx: *mut redraw_draw_ctx,
    span: *const redraw_span,
    x: u32,
    y: u32,
    mut n: u32,
) {
    unsafe {
        let scene = (*dctx).scene;
        let c = (*scene).c;
        let tty = &raw mut (*c).tty;
        let wp = (*span).data.st_wp;
        let s = &raw mut (*wp).status_screen;
        let sx = screen_size_x(s);

        let px = (*span).data.st_offset + (x - (*span).x);
        if px < sx {
            if n > sx - px {
                n = sx - px;
            }
            tty_draw_line(tty, s, px, 0, n, x, y, &GRID_DEFAULT_CELL, null_mut());
        }
    }
}

/// Draw a scrollbar span.
/// C `vendor/tmux/screen-redraw.c:1229`: `static void redraw_draw_scrollbar_span(struct redraw_draw_ctx *dctx, struct redraw_span *span, u_int x, u_int y, u_int n)`
unsafe fn redraw_draw_scrollbar_span(
    dctx: *mut redraw_draw_ctx,
    span: *const redraw_span,
    x: u32,
    y: u32,
    n: u32,
) {
    unsafe {
        let scene = (*dctx).scene;
        let wp = (*span).data.sb_wp;
        let s = (*wp).screen;
        let tty = &raw mut (*(*scene).c).tty;
        let sb_style = &raw const (*wp).scrollbar_style;
        let sb_h = (*span).data.sb_height;
        let sb_y = (*span).data.sb_y;

        let total_height;
        let pct_view;
        let mut slider_h;
        let mut slider_y;

        if window_pane_mode(wp) == WINDOW_PANE_NO_MODE {
            total_height = screen_size_y(s) + screen_hsize(s);
            if total_height == 0 {
                return;
            }
            pct_view = f64::from(sb_h) / f64::from(total_height);
            slider_h = (f64::from(sb_h) * pct_view) as u32;
            slider_y = sb_h - slider_h;
        } else {
            if tailq_first(&raw mut (*wp).modes).is_null() {
                return;
            }
            let (mut cm_y, mut cm_size) = (0u32, 0u32);
            if window_copy_get_current_offset(wp, &raw mut cm_y, &raw mut cm_size) == 0 {
                return;
            }
            total_height = cm_size + sb_h;
            if total_height == 0 {
                return;
            }
            pct_view = f64::from(sb_h) / f64::from(total_height);
            slider_h = (f64::from(sb_h) * pct_view) as u32;
            slider_y = (f64::from(sb_h + 1) * (f64::from(cm_y) / f64::from(total_height))) as u32;
        }

        if slider_h < 1 {
            slider_h = 1;
        }
        if slider_y >= sb_h {
            slider_y = sb_h - 1;
        }

        (*wp).sb_slider_y = slider_y;
        (*wp).sb_slider_h = slider_h;

        let gc = (*sb_style).gc;
        let mut slgc = gc;
        slgc.fg = gc.bg;
        slgc.bg = gc.fg;
        let mut pad_gc: grid_cell = zeroed();
        tty_default_colours(&raw mut pad_gc, wp);

        let sb_w = (*sb_style).width as u32;
        let sb_pad = (*sb_style).pad as u32;
        let off = x - (*span).x;

        tty_cursor(tty, x, y);
        for i in 0..n {
            if (*span).data.sb_flags & REDRAW_SCROLLBAR_LEFT != 0 {
                if off + i >= sb_w && off + i < sb_w + sb_pad {
                    tty_cell(tty, &raw const pad_gc, &GRID_DEFAULT_CELL, null_mut(), null_mut());
                    continue;
                }
            } else if off + i < sb_pad {
                tty_cell(tty, &raw const pad_gc, &GRID_DEFAULT_CELL, null_mut(), null_mut());
                continue;
            }

            let gcp = if sb_y >= slider_y && sb_y < slider_y + slider_h {
                &raw const slgc
            } else {
                &raw const gc
            };
            tty_cell(tty, gcp, &GRID_DEFAULT_CELL, null_mut(), null_mut());
        }
    }
}

/// Draw a span.
/// C `vendor/tmux/screen-redraw.c:1315`: `static void redraw_draw_span(struct redraw_draw_ctx *dctx, struct redraw_span *span, u_int y)`
unsafe fn redraw_draw_span(dctx: *mut redraw_draw_ctx, span: *const redraw_span, y: u32) {
    unsafe {
        let scene = (*dctx).scene;
        let type_ = (*span).data.type_;
        let c = (*scene).c;
        let tty = &raw mut (*c).tty;

        if type_ == redraw_span_type::REDRAW_SPAN_STATUS
            && !(*(*span).data.st_wp)
                .flags
                .intersects(window_pane_flags::PANE_NEWSTATUS)
        {
            return;
        }

        let r = tty_check_overlay_range(tty, (*span).x, y, (*span).width);
        for i in 0..(*r).used {
            let rr = (*r).ranges.add(i as usize);
            if (*rr).nx == 0 {
                continue;
            }
            let (x, n) = ((*rr).px, (*rr).nx);

            match type_ {
                redraw_span_type::REDRAW_SPAN_PANE => redraw_draw_pane_span(dctx, span, x, y, n),
                redraw_span_type::REDRAW_SPAN_BORDER
                | redraw_span_type::REDRAW_SPAN_EMPTY
                | redraw_span_type::REDRAW_SPAN_OUTSIDE => {
                    redraw_draw_border_span(dctx, span, x, y, n);
                }
                redraw_span_type::REDRAW_SPAN_STATUS => {
                    redraw_draw_status_span(dctx, span, x, y, n);
                }
                redraw_span_type::REDRAW_SPAN_SCROLLBAR => {
                    redraw_draw_scrollbar_span(dctx, span, x, y, n);
                }
            }
        }
    }
}

/// Draw pane lines.
/// C `vendor/tmux/screen-redraw.c:1349`: `static void redraw_draw_pane_lines(struct redraw_draw_ctx *dctx, struct window_pane *wp, int flags)`
unsafe fn redraw_draw_pane_lines(dctx: *mut redraw_draw_ctx, wp: *mut window_pane, flags: i32) {
    unsafe {
        let scene = (*dctx).scene;

        let mut top = (*wp).yoff - (*scene).oy as i32;
        if top < 0 {
            top = 0;
        }
        let mut bottom = (*wp).yoff + (*wp).sy as i32 - (*scene).oy as i32;
        if bottom < 0 {
            bottom = 0;
        }
        if bottom > (*scene).sy as i32 {
            bottom = (*scene).sy as i32;
        }

        for y in top..bottom {
            let cy = if (*dctx).flags & REDRAW_STATUS_TOP != 0 {
                (*dctx).status_lines + y as u32
            } else {
                y as u32
            };
            if flags & REDRAW_PANE != 0 {
                let lines = &(*scene).lines;
                let spans =
                    &lines[y as usize].spans[redraw_span_type::REDRAW_SPAN_PANE as usize];
                for i in 0..spans.len() {
                    let span = spans.as_ptr().add(i);
                    if (*span).data.p_wp == wp {
                        redraw_draw_span(dctx, span, cy);
                    }
                }
            }
            if flags & REDRAW_PANE_SCROLLBAR != 0 {
                let lines = &(*scene).lines;
                let spans =
                    &lines[y as usize].spans[redraw_span_type::REDRAW_SPAN_SCROLLBAR as usize];
                for i in 0..spans.len() {
                    let span = spans.as_ptr().add(i);
                    if (*span).data.sb_wp == wp {
                        redraw_draw_span(dctx, span, cy);
                    }
                }
            }
        }
    }
}

/// Draw lines.
/// C `vendor/tmux/screen-redraw.c:1393`: `static void redraw_draw_lines(struct redraw_draw_ctx *dctx, int flags)`
unsafe fn redraw_draw_lines(dctx: *mut redraw_draw_ctx, flags: i32) {
    unsafe {
        let scene = (*dctx).scene;

        for y in 0..(*scene).sy {
            let lines = &(*scene).lines;
            let line = &lines[y as usize];
            let cy = if (*dctx).flags & REDRAW_STATUS_TOP != 0 {
                (*dctx).status_lines + y
            } else {
                y
            };
            for type_ in 0..REDRAW_SPAN_TYPES {
                if flags != REDRAW_ALL {
                    let want = match type_ {
                        t if t == redraw_span_type::REDRAW_SPAN_PANE as usize => REDRAW_PANE,
                        t if t == redraw_span_type::REDRAW_SPAN_OUTSIDE as usize => REDRAW_OUTSIDE,
                        t if t == redraw_span_type::REDRAW_SPAN_EMPTY as usize => REDRAW_EMPTY,
                        t if t == redraw_span_type::REDRAW_SPAN_BORDER as usize => {
                            REDRAW_PANE_BORDER
                        }
                        t if t == redraw_span_type::REDRAW_SPAN_STATUS as usize => {
                            REDRAW_PANE_STATUS
                        }
                        t if t == redraw_span_type::REDRAW_SPAN_SCROLLBAR as usize => {
                            REDRAW_PANE_SCROLLBAR
                        }
                        _ => 0,
                    };
                    if want == 0 || flags & want == 0 {
                        continue;
                    }
                }
                let spans = &line.spans[type_];
                for i in 0..spans.len() {
                    redraw_draw_span(dctx, spans.as_ptr().add(i), cy);
                }
            }
        }
    }
}

/// Get line for pane status line.
/// C `vendor/tmux/screen-redraw.c:1444`: `static int redraw_pane_status_line(struct redraw_draw_ctx *dctx, struct window_pane *wp, u_int *line)`
unsafe fn redraw_pane_status_line(
    dctx: *mut redraw_draw_ctx,
    wp: *mut window_pane,
    line: *mut u32,
) -> i32 {
    unsafe {
        let scene = (*dctx).scene;

        let pane_status = window_pane_get_pane_status(wp);
        if pane_status == pane_status::PANE_STATUS_OFF {
            return 0;
        }

        let wy = if pane_status == pane_status::PANE_STATUS_TOP {
            (*wp).yoff - 1
        } else {
            (*wp).yoff + (*wp).sy as i32
        };
        if wy < 0 || wy < (*scene).oy as i32 {
            return 0;
        }
        if wy as u32 >= (*scene).oy + (*scene).sy {
            return 0;
        }
        *line = wy as u32 - (*scene).oy;
        1
    }
}

/// Get available width for pane status line, plus the index of the pane's
/// first status span on that line.
/// C `vendor/tmux/screen-redraw.c:1466`: `static u_int redraw_pane_status_width(struct redraw_draw_ctx *dctx, struct window_pane *wp, struct redraw_span **first)`
unsafe fn redraw_pane_status_width(
    dctx: *mut redraw_draw_ctx,
    wp: *mut window_pane,
    first: *mut usize,
) -> u32 {
    unsafe {
        let scene = (*dctx).scene;
        let mut y = 0;
        let mut width = 0;

        if redraw_pane_status_line(dctx, wp, &raw mut y) == 0 {
            return 0;
        }

        *first = usize::MAX;
        let lines = &(*scene).lines;
        let spans = &lines[y as usize].spans[redraw_span_type::REDRAW_SPAN_STATUS as usize];
        for (i, span) in spans.iter().enumerate() {
            if span.data.st_wp == wp {
                if *first == usize::MAX {
                    *first = i;
                }
                let end = span.data.st_offset + span.width;
                if end > width {
                    width = end;
                }
            }
        }
        width
    }
}

/// Set up draw context.
/// C `vendor/tmux/screen-redraw.c:1490`: `static void redraw_set_draw_context(struct redraw_draw_ctx *dctx, struct redraw_scene *scene)`
unsafe fn redraw_set_draw_context(scene: *mut redraw_scene) -> redraw_draw_ctx {
    unsafe {
        let c = (*scene).c;
        let s = (*c).session;
        let oo = (*s).options;
        let tty = &raw mut (*c).tty;

        let mut dctx = redraw_draw_ctx {
            scene,
            active: null_mut(),
            marked: null_mut(),
            status_lines: 0,
            pane_lines: crate::pane_lines::PANE_LINES_SINGLE,
            default_gc: zeroed(),
            flags: 0,
        };

        if server_is_marked(s, (*s).curw, MARKED_PANE.wp) {
            dctx.marked = MARKED_PANE.wp;
        }
        dctx.active = server_client_get_pane(c);

        let mut lines = status_line_size(c);
        if (*c).message_string.is_some() || (*c).prompt_string.is_some() {
            lines = if lines == 0 { 1 } else { lines };
        }
        if lines != 0 && options_get_number_(oo, "status-position") == 0 {
            dctx.flags |= REDRAW_STATUS_TOP;
        }
        dctx.status_lines = lines;

        if (*c).flags.intersects(client_flag::UTF8)
            && tty_term_has((*tty).term, tty_code_code::TTYC_BIDI)
        {
            dctx.flags |= REDRAW_ISOLATES;
        }
        dctx
    }
}

/// Draw scene to client.
/// C `vendor/tmux/screen-redraw.c:1580`: `static void redraw_draw(struct client *c, struct window_pane *wp, int flags)`
unsafe fn redraw_draw(c: *mut client, wp: *mut window_pane, mut flags: i32) {
    unsafe {
        let s = (*c).session;
        let w = (*(*s).curw).window;
        let tty = &raw mut (*c).tty;

        if (*c).flags.intersects(client_flag::SUSPENDED) {
            return;
        }

        if flags & REDRAW_STATUS != 0 {
            let redraw = if (*c).message_string.is_some() {
                status_message_redraw(c)
            } else if (*c).prompt_string.is_some() {
                status_prompt_redraw(c)
            } else {
                status_redraw(c)
            };
            // ztmux keeps upstream's older CLIENT_REDRAWSTATUSALWAYS, which
            // forces the status line out even when its contents are unchanged.
            if redraw == 0
                && flags != REDRAW_ALL
                && !(*c).flags.intersects(client_flag::REDRAWSTATUSALWAYS)
            {
                flags &= !REDRAW_STATUS;
                if flags == 0 {
                    return;
                }
            }
        }

        log_debug!(
            "{}: starting @{} redraw ({})",
            _s((*c).name),
            (*w).id,
            redraw_flags_to_string(flags),
        );

        let scene = redraw_get_scene(c);
        if scene.is_null() {
            return;
        }
        let mut draw_ctx = redraw_set_draw_context(scene);
        let dctx = &raw mut draw_ctx;

        if flags & (REDRAW_PANE_BORDER | REDRAW_PANE_STATUS) != 0 {
            for loop_ in
                tailq_foreach::<_, discr_entry>(&raw mut (*(*scene).w).panes).map(NonNull::as_ptr)
            {
                (*loop_).border_gc_set = 0;
                (*loop_).active_border_gc_set = 0;
            }
        }

        if flags & REDRAW_PANE_STATUS != 0 {
            let mut redraw = 0;
            for loop_ in
                tailq_foreach::<_, discr_entry>(&raw mut (*(*scene).w).panes).map(NonNull::as_ptr)
            {
                if flags == REDRAW_ALL {
                    (*loop_).flags |= window_pane_flags::PANE_NEWSTATUS;
                } else {
                    (*loop_).flags &= !window_pane_flags::PANE_NEWSTATUS;
                }

                let mut first = usize::MAX;
                let width = redraw_pane_status_width(dctx, loop_, &raw mut first);
                if width == 0 {
                    continue;
                }

                let mut line = 0;
                if redraw_pane_status_line(dctx, loop_, &raw mut line) == 0 {
                    continue;
                }
                // Move the status spans out for the call: it draws into the
                // pane's own status screen and cannot touch the scene, but the
                // borrow checker cannot see that through the raw pointer.
                let lines = &mut (*scene).lines;
                let slot =
                    &mut lines[line as usize].spans[redraw_span_type::REDRAW_SPAN_STATUS as usize];
                let spans = std::mem::take(slot);
                let changed = window_make_pane_status(loop_, c, width, &spans, first);
                let lines = &mut (*scene).lines;
                lines[line as usize].spans[redraw_span_type::REDRAW_SPAN_STATUS as usize] = spans;
                if changed != 0 {
                    (*loop_).flags |= window_pane_flags::PANE_NEWSTATUS;
                    redraw = 1;
                }
            }
            if redraw == 0 && flags != REDRAW_ALL {
                flags &= !REDRAW_PANE_STATUS;
                if flags == 0 {
                    return;
                }
            }
        }

        if flags & REDRAW_PANE != 0 {
            if !wp.is_null() {
                if (*wp).base.mode.intersects(mode_flag::MODE_SYNC) {
                    screen_write_stop_sync(wp);
                }
                screen_write_clear_dirty(wp);
            } else {
                for loop_ in tailq_foreach::<_, discr_entry>(&raw mut (*(*scene).w).panes)
                    .map(NonNull::as_ptr)
                {
                    if !window_pane_visible(loop_) {
                        continue;
                    }
                    if (*loop_).base.mode.intersects(mode_flag::MODE_SYNC) {
                        screen_write_stop_sync(loop_);
                    }
                    screen_write_clear_dirty(loop_);
                }
            }
        }
        tty_sync_start(tty);
        tty_update_mode(tty, (*tty).mode, null_mut());

        if !wp.is_null() {
            redraw_draw_pane_lines(dctx, wp, flags);
        } else {
            redraw_draw_lines(dctx, flags);
        }

        // ztmux: the ratatui pane frames and info bar sit on top of the scene.
        // Both are no-ops unless the ratatui UI is enabled.
        if flags & (REDRAW_PANE | REDRAW_PANE_BORDER) != 0 {
            let mut rctx = MaybeUninit::<screen_redraw_ctx>::uninit();
            screen_redraw_set_context(c, rctx.as_mut_ptr());
            for loop_ in
                tailq_foreach::<_, discr_entry>(&raw mut (*(*scene).w).panes).map(NonNull::as_ptr)
            {
                if window_pane_visible(loop_) {
                    crate::extensions::ratatui_ui::draw_pane_frame(rctx.as_mut_ptr(), loop_);
                }
            }
            crate::extensions::ratatui_ui::draw_info_bar(rctx.as_mut_ptr());
        }

        if flags & REDRAW_STATUS != 0 {
            let mut lines = (*dctx).status_lines;
            if (*c).message_string.is_some() || (*c).prompt_string.is_some() {
                lines = if lines == 0 { 1 } else { lines };
            }
            let y = if (*dctx).flags & REDRAW_STATUS_TOP != 0 {
                0
            } else {
                (*c).tty.sy - lines
            };
            let sl = (*c).status.active;
            for i in 0..lines {
                tty_draw_line(
                    tty,
                    sl,
                    0,
                    i,
                    u32::MAX,
                    0,
                    y + i,
                    &GRID_DEFAULT_CELL,
                    null_mut(),
                );
            }
        }
        if let Some(overlay_draw) = (*c).overlay_draw
            && flags & REDRAW_OVERLAY != 0
        {
            let mut rctx = MaybeUninit::<screen_redraw_ctx>::uninit();
            screen_redraw_set_context(c, rctx.as_mut_ptr());
            overlay_draw(c, (*c).overlay_data, rctx.as_mut_ptr());
        }

        tty_reset(tty);
        tty_sync_end(tty);

        #[cfg(feature = "sixel")]
        {
            if !wp.is_null() {
                crate::tty_::tty_draw_images(c, wp, (*wp).screen);
            } else {
                for loop_ in tailq_foreach::<_, discr_entry>(&raw mut (*(*scene).w).panes)
                    .map(NonNull::as_ptr)
                {
                    crate::tty_::tty_draw_images(c, loop_, (*loop_).screen);
                }
            }
        }

        log_debug!("{}: finished @{} redraw", _s((*c).name), (*(*scene).w).id);
    }
}

/// Get border cell type beneath status cell at offset x in pane status line.
/// C `vendor/tmux/screen-redraw.c:1717`: `int redraw_get_status_border_cell_type(struct redraw_span **spanp, u_int x)`
///
/// `spans`/`cursor` replace the C's span cursor: the Rust scene stores spans in
/// a `Vec`, so the walk is by index and `usize::MAX` stands in for `NULL`.
pub unsafe fn redraw_get_status_border_cell_type(
    spans: &[redraw_span],
    cursor: &mut usize,
    x: u32,
) -> cell_type {
    let Some(first) = spans.get(*cursor) else {
        return cell_type::CELL_LEFTRIGHT;
    };
    if first.data.type_ != redraw_span_type::REDRAW_SPAN_STATUS {
        return cell_type::CELL_LEFTRIGHT;
    }
    let wp = first.data.st_wp;

    for (i, span) in spans.iter().enumerate().skip(*cursor) {
        if span.data.type_ != redraw_span_type::REDRAW_SPAN_STATUS || span.data.st_wp != wp {
            continue;
        }
        let start = span.data.st_offset;
        let end = start + span.width;
        if x >= start && x < end {
            *cursor = i;
            return span.data.st_cell_type;
        }
        if start > x {
            *cursor = i;
            return cell_type::CELL_LEFTRIGHT;
        }
    }
    *cursor = usize::MAX;
    cell_type::CELL_LEFTRIGHT
}

/// Draw screen.
/// C `vendor/tmux/screen-redraw.c:1745`: `void redraw_screen(struct client *c)`
pub unsafe fn redraw_screen(c: *mut client) {
    unsafe {
        let mut flags = 0;

        if (*c).flags.intersects(client_flag::REDRAWWINDOW) {
            redraw_draw(c, null_mut(), REDRAW_ALL);
        } else {
            if (*c).flags.intersects(client_flag::REDRAWBORDERS) {
                flags |= REDRAW_PANE_BORDER | REDRAW_PANE_STATUS;
            }
            if (*c)
                .flags
                .intersects(client_flag::REDRAWSTATUS | client_flag::REDRAWSTATUSALWAYS)
            {
                flags |= REDRAW_STATUS | REDRAW_PANE_STATUS;
            }
            // ztmux keeps an overlay redraw tied to any redraw of this client:
            // popups and menus are drawn over the scene, which the scene cache
            // knows nothing about, so a border-only pass must repaint them.
            if (*c).flags.intersects(client_flag::REDRAWOVERLAY) || (*c).overlay_draw.is_some() {
                flags |= REDRAW_OVERLAY;
            }
            if flags != 0 {
                redraw_draw(c, null_mut(), flags);
            }
        }
    }
}

/// Draw a single pane.
/// C `vendor/tmux/screen-redraw.c:1763`: `void redraw_pane(struct client *c, struct window_pane *wp)`
pub unsafe fn redraw_pane(c: *mut client, wp: *mut window_pane) {
    unsafe {
        redraw_draw(c, wp, REDRAW_PANE | REDRAW_PANE_SCROLLBAR);
    }
}

/// Draw a pane's scrollbar.
/// C `vendor/tmux/screen-redraw.c:1780`: `void redraw_pane_scrollbar(struct client *c, struct window_pane *wp)`
pub unsafe fn redraw_pane_scrollbar(c: *mut client, wp: *mut window_pane) {
    unsafe {
        redraw_draw(c, wp, REDRAW_PANE_SCROLLBAR);
    }
}

/// Set up redraw context.
///
/// ztmux extension point: overlays (`popup`, `menu`, `display-panes`) and the
/// ratatui UI draw through `screen_redraw_ctx`, which upstream dropped when the
/// scene builder landed. The context is derived from the same client state the
/// scene uses, so both agree on offsets and status-line placement.
pub unsafe fn screen_redraw_set_context(c: *mut client, ctx: *mut screen_redraw_ctx) {
    unsafe {
        let s = (*c).session;
        let oo = (*s).options;
        let w = (*(*s).curw).window;
        let wo = (*w).options;

        memset0(ctx);
        (*ctx).c = c;

        let mut lines = status_line_size(c);
        if (*c).message_string.is_some() || (*c).prompt_string.is_some() {
            lines = if lines == 0 { 1 } else { lines };
        }
        if lines != 0 && options_get_number_(oo, "status-position") == 0 {
            (*ctx).statustop = 1;
        }
        (*ctx).statuslines = lines;

        (*ctx).pane_status = (options_get_number_(wo, "pane-border-status") as i32)
            .try_into()
            .unwrap_or(pane_status::PANE_STATUS_OFF);
        (*ctx).pane_lines = (options_get_number_(wo, "pane-border-lines") as i32)
            .try_into()
            .unwrap_or(crate::pane_lines::PANE_LINES_SINGLE);

        redraw_get_window_offset(
            c,
            &raw mut (*ctx).ox,
            &raw mut (*ctx).oy,
            &raw mut (*ctx).sx,
            &raw mut (*ctx).sy,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A scene-building context over a `w` x `h` window with no scroll offset,
    // plus the window and pane it describes. `window` and `window_pane` are
    // plain C-style structs with no Drop impl, so zeroed instances with the few
    // geometry fields set are enough to drive the marking functions, which read
    // nothing else.
    unsafe fn ctx(w: u32, h: u32) -> (redraw_build_ctx, Box<window>, Box<window_pane>) {
        unsafe {
            let mut win: Box<window> = Box::new(zeroed());
            win.sx = w;
            win.sy = h;
            win.sb_pos = PANE_SCROLLBARS_RIGHT;
            let wp: Box<window_pane> = Box::new(zeroed());
            let bctx = redraw_build_ctx {
                c: null_mut(),
                w: &raw mut *win,
                ox: 0,
                oy: 0,
                sx: w,
                sy: h,
                ind: 0,
                cells: vec![redraw_build_cell::default(); (w * h) as usize],
            };
            (bctx, win, wp)
        }
    }

    // Every scene column that ended up marked as a scrollbar on line `y`.
    unsafe fn scrollbar_columns(bctx: *mut redraw_build_ctx, y: u32) -> Vec<u32> {
        unsafe {
            (0..(*bctx).sx)
                .filter(|&x| {
                    (*redraw_get_build_cell(bctx, x, y)).data.type_
                        == redraw_span_type::REDRAW_SPAN_SCROLLBAR
                })
                .collect()
        }
    }

    // A reserved bar on the right sits in the columns immediately PAST the
    // pane, which is where layout_fix_panes left room for it
    // (screen-redraw.c:465). Its span data records the pane, the line within
    // the bar and the bar's full height.
    #[test]
    fn reserved_scrollbar_marks_the_columns_past_the_pane() {
        unsafe {
            let (mut bctx, _win, mut wp) = ctx(20, 4);
            wp.xoff = 2;
            wp.yoff = 1;
            wp.sx = 10;
            wp.sy = 3;

            redraw_mark_pane_scrollbar(&raw mut bctx, &raw mut *wp, 2, 0, 0);

            // Line 0 is above the pane, so nothing is marked there.
            assert_eq!(scrollbar_columns(&raw mut bctx, 0), Vec::<u32>::new());
            for (i, y) in (1..4).enumerate() {
                assert_eq!(scrollbar_columns(&raw mut bctx, y), vec![12, 13], "line {y}");
                let d = (*redraw_get_build_cell(&raw mut bctx, 12, y)).data;
                assert_eq!(d.sb_wp, &raw mut *wp);
                assert_eq!(d.sb_y, i as u32);
                assert_eq!(d.sb_height, 3);
                assert_eq!(d.sb_flags, REDRAW_SCROLLBAR_RIGHT);
            }
        }
    }

    // On the left the reserved bar sits in the columns immediately BEFORE the
    // pane (screen-redraw.c:462), and the flag flips.
    #[test]
    fn reserved_scrollbar_on_the_left_marks_the_columns_before_the_pane() {
        unsafe {
            let (mut bctx, _win, mut wp) = ctx(20, 3);
            wp.xoff = 5;
            wp.yoff = 0;
            wp.sx = 10;
            wp.sy = 3;

            redraw_mark_pane_scrollbar(&raw mut bctx, &raw mut *wp, 2, 1, 0);

            assert_eq!(scrollbar_columns(&raw mut bctx, 0), vec![3, 4]);
            let d = (*redraw_get_build_cell(&raw mut bctx, 3, 0)).data;
            assert_eq!(d.sb_flags, REDRAW_SCROLLBAR_LEFT);
        }
    }

    // An overlay bar has no column of its own, so it is drawn over the pane's
    // last (or first) columns instead (screen-redraw.c:456).
    #[test]
    fn overlay_scrollbar_marks_columns_inside_the_pane() {
        unsafe {
            let (mut bctx, _win, mut wp) = ctx(20, 2);
            wp.xoff = 4;
            wp.yoff = 0;
            wp.sx = 8; // pane covers 4..=11
            wp.sy = 2;

            redraw_mark_pane_scrollbar(&raw mut bctx, &raw mut *wp, 3, 0, 1);
            assert_eq!(scrollbar_columns(&raw mut bctx, 0), vec![9, 10, 11]);
            assert_eq!(
                (*redraw_get_build_cell(&raw mut bctx, 9, 0)).data.sb_flags,
                REDRAW_SCROLLBAR_RIGHT | REDRAW_SCROLLBAR_OVERLAY
            );

            let (mut bctx, _win, _) = ctx(20, 2);
            redraw_mark_pane_scrollbar(&raw mut bctx, &raw mut *wp, 3, 1, 1);
            assert_eq!(scrollbar_columns(&raw mut bctx, 0), vec![4, 5, 6]);
            assert_eq!(
                (*redraw_get_build_cell(&raw mut bctx, 4, 0)).data.sb_flags,
                REDRAW_SCROLLBAR_LEFT | REDRAW_SCROLLBAR_OVERLAY
            );
        }
    }

    // A zero-width bar marks nothing at all (screen-redraw.c:453) — that is how
    // a pane with no visible scrollbar is expressed.
    #[test]
    fn zero_width_scrollbar_marks_nothing() {
        unsafe {
            let (mut bctx, _win, mut wp) = ctx(10, 2);
            wp.sx = 10;
            wp.sy = 2;
            redraw_mark_pane_scrollbar(&raw mut bctx, &raw mut *wp, 0, 0, 0);
            assert_eq!(scrollbar_columns(&raw mut bctx, 0), Vec::<u32>::new());
        }
    }

    // Columns of a scrollbar join into one span only when they belong to the
    // same pane, the same line of the bar, the same bar height and the same
    // flags (screen-redraw.c:870). Unlike a pane span, the x position inside
    // the bar is not part of the data, so adjacent columns of one line join.
    #[test]
    fn scrollbar_cells_join_only_within_one_line_of_one_bar() {
        unsafe {
            let mut wp: Box<window_pane> = Box::new(zeroed());
            let mut other: Box<window_pane> = Box::new(zeroed());

            let cell = |wp: *mut window_pane, y: u32, height: u32, flags: i32| {
                let mut bc = redraw_build_cell::default();
                bc.data.type_ = redraw_span_type::REDRAW_SPAN_SCROLLBAR;
                bc.data.sb_wp = wp;
                bc.data.sb_y = y;
                bc.data.sb_height = height;
                bc.data.sb_flags = flags;
                bc
            };

            let a = cell(&raw mut *wp, 2, 5, REDRAW_SCROLLBAR_RIGHT);
            assert_eq!(redraw_compare_data(&raw const a, &raw const a), 1);

            for b in [
                cell(&raw mut *other, 2, 5, REDRAW_SCROLLBAR_RIGHT),
                cell(&raw mut *wp, 3, 5, REDRAW_SCROLLBAR_RIGHT),
                cell(&raw mut *wp, 2, 6, REDRAW_SCROLLBAR_RIGHT),
                cell(&raw mut *wp, 2, 5, REDRAW_SCROLLBAR_LEFT),
            ] {
                assert_eq!(redraw_compare_data(&raw const a, &raw const b), 0);
            }

            // A scrollbar cell never joins a cell of another type.
            let mut pane_cell = redraw_build_cell::default();
            pane_cell.data.type_ = redraw_span_type::REDRAW_SPAN_PANE;
            assert_eq!(redraw_compare_data(&raw const a, &raw const pane_cell), 0);
        }
    }

    // Window-scope options with the shipped defaults, which is all
    // `redraw_mark_pane_borders` reads (`pane-border-lines`,
    // `pane-border-status`).
    unsafe fn window_options() -> *mut options {
        unsafe {
            let o = options_create(null_mut());
            for oe in &OPTIONS_TABLE {
                if oe.scope & OPTIONS_TABLE_WINDOW != 0 {
                    options_default(o, oe);
                }
            }
            o
        }
    }

    // Every scene column marked as a border on line `y`.
    unsafe fn border_columns(bctx: *mut redraw_build_ctx, y: u32) -> Vec<u32> {
        unsafe {
            (0..(*bctx).sx)
                .filter(|&x| {
                    (*redraw_get_build_cell(bctx, x, y)).data.type_
                        == redraw_span_type::REDRAW_SPAN_BORDER
                })
                .collect()
        }
    }

    // A reserved bar pushes the pane's border out past the column it took, so
    // the border is drawn beyond the bar and not between bar and pane
    // (screen-redraw.c:676). Without a bar the border hugs the pane.
    #[test]
    fn reserved_scrollbar_moves_the_border_past_the_bar() {
        unsafe {
            let oo = window_options();

            let check = |sb_w: i32, sb_left: i32| {
                let (mut bctx, mut win, mut wp) = ctx(20, 5);
                win.options = oo;
                wp.window = &raw mut *win;
                wp.options = oo;
                wp.xoff = 5;
                wp.yoff = 1;
                wp.sx = 8; // pane covers columns 5..=12
                wp.sy = 3;
                for y in 0..bctx.sy {
                    for x in 0..bctx.sx {
                        redraw_reset_cell(&raw mut bctx, x, y);
                    }
                }
                redraw_mark_pane_borders(&raw mut bctx, &raw mut *wp, sb_w, sb_left);
                // Line 2 is inside the pane's rows, so only the two side
                // borders are marked on it.
                border_columns(&raw mut bctx, 2)
            };

            // No scrollbar: borders at xoff-1 and xoff+sx.
            assert_eq!(check(0, 0), vec![4, 13]);
            // Two reserved columns on the right: the right border moves out by
            // two, the left one does not move.
            assert_eq!(check(2, 0), vec![4, 15]);
            // Two reserved columns on the left: the left border moves out.
            assert_eq!(check(2, 1), vec![2, 13]);

            options_free(oo);
        }
    }
}
