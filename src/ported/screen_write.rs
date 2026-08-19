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
use crate::options_::options_get_number_;
use crate::*;

#[repr(i32)]
#[derive(Copy, Clone, Default, Eq, PartialEq)]
pub enum screen_write_citem_type {
    #[default]
    Text,
    Clear,
}

impl_tailq_entry!(screen_write_citem, entry, tailq_entry<screen_write_citem>);
#[repr(C)]
pub struct screen_write_citem {
    x: u32,
    wrapped: bool,

    type_: screen_write_citem_type,
    used: u32,
    bg: u32,

    gc: grid_cell,

    entry: tailq_entry<screen_write_citem>,
}

#[repr(C)]
pub struct screen_write_cline {
    data: *mut u8,
    items: tailq_head<screen_write_citem>,
}

pub static mut SCREEN_WRITE_CITEM_FREELIST: tailq_head<screen_write_citem> =
    TAILQ_HEAD_INITIALIZER!(SCREEN_WRITE_CITEM_FREELIST);

/// C `vendor/tmux/screen-write.c:63`: `static struct screen_write_citem *screen_write_get_citem(void)`
unsafe fn screen_write_get_citem() -> NonNull<screen_write_citem> {
    unsafe {
        if let Some(ci) = NonNull::new(tailq_first(&raw mut SCREEN_WRITE_CITEM_FREELIST)) {
            tailq_remove(&raw mut SCREEN_WRITE_CITEM_FREELIST, ci.as_ptr());
            memset0(ci.as_ptr());
            return ci;
        }
        NonNull::new(xcalloc1::<screen_write_citem>()).unwrap()
    }
}

/// C `vendor/tmux/screen-write.c:77`: `static void screen_write_free_citem(struct screen_write_citem *ci)`
unsafe fn screen_write_free_citem(ci: *mut screen_write_citem) {
    unsafe {
        tailq_insert_tail(&raw mut SCREEN_WRITE_CITEM_FREELIST, ci);
    }
}

/// C `vendor/tmux/screen-write.c:83`: `static void screen_write_offset_timer(__unused int fd, __unused short events, void *data)`
unsafe extern "C-unwind" fn screen_write_offset_timer(_fd: i32, _events: i16, w: NonNull<window>) {
    unsafe {
        tty_update_window_offset(w.as_ptr());
    }
}

/// Set cursor position.
/// C `vendor/tmux/screen-write.c:92`: `static void screen_write_set_cursor(struct screen_write_ctx *ctx, int cx, int cy)`
unsafe fn screen_write_set_cursor(ctx: *mut screen_write_ctx, mut cx: i32, mut cy: i32) {
    unsafe {
        let wp = (*ctx).wp;
        let s = (*ctx).s;
        let tv: timeval = timeval {
            tv_usec: 10000,
            tv_sec: 0,
        };

        if cx != -1 && cx as u32 == (*s).cx && cy != -1 && cy as u32 == (*s).cy {
            return;
        }

        if cx != -1 {
            if cx as u32 > screen_size_x(s) {
                cx = screen_size_x(s) as i32 - 1;
            } // allow last column
            (*s).cx = cx as u32;
        }
        if cy != -1 {
            if cy as u32 > screen_size_y(s) - 1 {
                cy = screen_size_y(s) as i32 - 1;
            }
            (*s).cy = cy as u32;
        }

        if wp.is_null() {
            return;
        }
        let w = (*wp).window;

        if event_initialized(&raw mut (*w).offset_timer) == 0 {
            evtimer_set(
                &raw mut (*w).offset_timer,
                screen_write_offset_timer,
                NonNull::new_unchecked(w),
            );
        }
        if evtimer_pending(&raw mut (*w).offset_timer, null_mut()) == 0 {
            evtimer_add(&raw mut (*w).offset_timer, &raw const tv);
        }
    }
}

/// C `vendor/tmux/screen-write.c`: `static void screen_write_sync_callback(int fd, short events, void *data)`
unsafe extern "C-unwind" fn screen_write_sync_callback(
    _fd: i32,
    _events: i16,
    wp: NonNull<window_pane>,
) {
    unsafe {
        screen_write_stop_sync(wp.as_ptr());
    }
}

/// Enter synchronized-output mode (DEC 2026): set the mode flag and (re)arm the
/// 1-second safety timer that clears it if the application never sends the reset.
/// ztmux writes cells immediately instead of buffering dirty lines during sync,
/// so C's `screen_write_flush_dirty` on stop has nothing to flush — the
/// deferred-render batching is a perf optimisation not ported here, but the
/// mode state apps set and query (`#{synchronized_output_flag}`, DECRQM) is
/// faithful.
/// C `vendor/tmux/screen-write.c`: `void screen_write_start_sync(struct window_pane *wp)`
pub unsafe fn screen_write_start_sync(wp: *mut window_pane) {
    unsafe {
        let tv = timeval { tv_sec: 1, tv_usec: 0 };
        if wp.is_null() {
            return;
        }
        (*wp).base.mode |= mode_flag::MODE_SYNC;
        if event_initialized(&raw mut (*wp).sync_timer) == 0 {
            evtimer_set(
                &raw mut (*wp).sync_timer,
                screen_write_sync_callback,
                NonNull::new_unchecked(wp),
            );
        }
        evtimer_add(&raw mut (*wp).sync_timer, &raw const tv);
        log_debug!("screen_write_start_sync: %{} started sync mode", (*wp).id);
    }
}

/// Leave synchronized-output mode.
/// C `vendor/tmux/screen-write.c`: `void screen_write_stop_sync(struct window_pane *wp)`
pub unsafe fn screen_write_stop_sync(wp: *mut window_pane) {
    unsafe {
        if wp.is_null() || !(*wp).base.mode.intersects(mode_flag::MODE_SYNC) {
            return;
        }
        if event_initialized(&raw mut (*wp).sync_timer) != 0 {
            evtimer_del(&raw mut (*wp).sync_timer);
        }
        (*wp).base.mode &= !mode_flag::MODE_SYNC;

        screen_write_flush_dirty(wp);

        log_debug!("screen_write_stop_sync: %{} stopped sync mode", (*wp).id);
    }
}

/// Do a full redraw.
/// C `vendor/tmux/screen-write.c:125`: `static void screen_write_redraw_cb(const struct tty_ctx *ttyctx)`
unsafe fn screen_write_redraw_cb(ttyctx: *const tty_ctx) {
    unsafe {
        let wp: *mut window_pane = (*ttyctx).arg.cast();

        if !wp.is_null() {
            (*wp).flags |= window_pane_flags::PANE_REDRAW;
        }
    }
}

/// Update context for client.
/// C `vendor/tmux/screen-write.c:135`: `static int screen_write_set_client_cb(struct tty_ctx *ttyctx, struct client *c)`
unsafe fn screen_write_set_client_cb(ttyctx: *mut tty_ctx, c: *mut client) -> i32 {
    unsafe {
        let wp: *mut window_pane = (*ttyctx).arg.cast();

        if (*ttyctx).allow_invisible_panes != 0 {
            if session_has((*c).session, (*wp).window) {
                return 1;
            }
            return 0;
        }

        if (*(*(*c).session).curw).window != (*wp).window {
            return 0;
        }
        if (*wp).layout_cell.is_null() {
            return 0;
        }

        if (*wp)
            .flags
            .intersects(window_pane_flags::PANE_REDRAW | window_pane_flags::PANE_DROP)
        {
            return -1;
        }
        if (*c).flags.intersects(client_flag::REDRAWPANES) {
            // Redraw is already deferred to redraw another pane - redraw
            // this one also when that happens.
            // log_debug("%s: adding %%%u to deferred redraw", __func__, (*wp).id);
            (*wp).flags |=
                window_pane_flags::PANE_REDRAW | window_pane_flags::PANE_REDRAWSCROLLBAR;
            return -1;
        }

        (*ttyctx).bigger = tty_window_offset(
            &raw mut (*c).tty,
            &raw mut (*ttyctx).wox,
            &raw mut (*ttyctx).woy,
            &raw mut (*ttyctx).wsx,
            &raw mut (*ttyctx).wsy,
        );

        (*ttyctx).rxoff = (*wp).xoff as u32;
        (*ttyctx).xoff = (*wp).xoff as u32;

        (*ttyctx).ryoff = (*wp).yoff as u32;
        (*ttyctx).yoff = (*wp).yoff as u32;

        if status_at_line(c) == 0 {
            (*ttyctx).yoff += status_line_size(c);
        }

        1
    }
}

/// Set up context for TTY command.
/// C `vendor/tmux/screen-write.c:262`: `static void screen_write_initctx(struct screen_write_ctx *ctx, struct tty_ctx *ttyctx, int is_sync, int check_obscured)`
/// Should these lines be drawn to the tty now?
/// C `vendor/tmux/screen-write.c:220`: `static int screen_write_should_draw_lines(struct screen_write_ctx *ctx, u_int y, u_int ny)`
///
/// No while the pane already has a full redraw pending, and no during
/// synchronized-output mode — there the lines are recorded and drawn once when
/// the mode ends. An app that repaints inside a sync block (htop and friends)
/// otherwise costs a draw per operation, which shows up as flicker.
unsafe fn screen_write_should_draw_lines(
    ctx: *mut screen_write_ctx,
    mut y: u32,
    mut ny: u32,
) -> bool {
    unsafe {
        let wp = (*ctx).wp;
        let s = (*ctx).s;
        let sy = screen_size_y(s);

        if !wp.is_null()
            && (*wp)
                .flags
                .intersects(window_pane_flags::PANE_REDRAW | window_pane_flags::PANE_DROP)
        {
            return false;
        }
        if (*s).mode.intersects(mode_flag::MODE_SYNC) {
            if !wp.is_null() && y < sy && ny != 0 {
                if ny > sy - y {
                    ny = sy - y;
                }
                // A resize invalidates the map, so redo the whole pane.
                let stale = match &(*wp).sync_dirty {
                    Some(bs) => bs.len() < sy,
                    None => false,
                };
                if (*wp).sync_dirty.is_none() || stale {
                    if stale {
                        y = 0;
                        ny = sy;
                    }
                    (*wp).sync_dirty = Some(Box::new(BitStr::new(sy)));
                }
                if let Some(bs) = &mut (*wp).sync_dirty {
                    bs.bit_nset(y, y + ny - 1);
                }
            }
            return false;
        }
        true
    }
}

/// C `vendor/tmux/screen-write.c:255`: `static int screen_write_should_draw_line(struct screen_write_ctx *ctx, u_int y)`
unsafe fn screen_write_should_draw_line(ctx: *mut screen_write_ctx, y: u32) -> bool {
    unsafe { screen_write_should_draw_lines(ctx, y, 1) }
}

/// Draw the lines touched during synchronized-output mode.
/// C `vendor/tmux/screen-write.c:1252`: `static void screen_write_flush_dirty(struct window_pane *wp)`
pub unsafe fn screen_write_flush_dirty(wp: *mut window_pane) {
    unsafe {
        if wp.is_null() || (*wp).sync_dirty.is_none() {
            return;
        }
        let s = &raw mut (*wp).base;
        let sy = screen_size_y(s);

        let mut ctx: screen_write_ctx = zeroed();
        let mut ttyctx: tty_ctx = zeroed();
        screen_write_start_pane(&raw mut ctx, wp, s);
        screen_write_initctx(&raw mut ctx, &raw mut ttyctx, 1);

        let mut lines = 0;
        for y in 0..sy {
            let dirty = match &(*wp).sync_dirty {
                Some(bs) => y < bs.len() && bs.bit_test(y),
                None => false,
            };
            if dirty {
                screen_write_redraw_line(&raw mut ctx, &raw mut ttyctx, y);
                lines += 1;
            }
        }
        log_debug!(
            "screen_write_flush_dirty: %{} had {} dirty lines",
            (*wp).id,
            lines,
        );

        screen_write_stop(&raw mut ctx);
        screen_write_clear_dirty(wp);
    }
}

/// C `vendor/tmux/screen-write.c`: `void screen_write_clear_dirty(struct window_pane *wp)`
pub unsafe fn screen_write_clear_dirty(wp: *mut window_pane) {
    unsafe {
        if !wp.is_null() {
            (*wp).sync_dirty = None;
        }
    }
}

/// Whether a floating pane overlaps this write context's pane, so the cheap
/// whole-screen escape sequences must not be used.
/// C `vendor/tmux/screen-write.c`: `static int screen_write_pane_is_obscured(struct screen_write_ctx *ctx)`
///
/// The answer is cached on the context: it is asked once per clear operation.
pub unsafe fn screen_write_pane_is_obscured(ctx: *mut screen_write_ctx) -> bool {
    unsafe {
        let base = (*ctx).wp;
        if base.is_null() {
            return false;
        }
        if (*ctx).flags & SCREEN_WRITE_CHECKED_IF_OBSCURED != 0 {
            return (*ctx).flags & SCREEN_WRITE_OBSCURED != 0;
        }
        (*ctx).flags |= SCREEN_WRITE_CHECKED_IF_OBSCURED;

        let w = (*base).window;
        if (*base).xoff < 0
            || (*base).yoff < 0
            || (*base).xoff + (*base).sx as i32 > (*w).sx as i32
            || (*base).yoff + (*base).sy as i32 > (*w).sy as i32
        {
            (*ctx).flags |= SCREEN_WRITE_OBSCURED;
            return true;
        }

        let (bx, by) = ((*base).xoff, (*base).yoff);
        let (bsx, bsy) = ((*base).sx as i32, (*base).sy as i32);

        // Walk toward the head of the z-index: those panes are drawn above.
        let mut wp = base;
        loop {
            wp = tailq_prev::<_, window_pane, discr_zentry>(wp);
            if wp.is_null() {
                return false;
            }
            let (px, py) = ((*wp).xoff, (*wp).yoff);
            let (psx, psy) = ((*wp).sx as i32, (*wp).sy as i32);
            let overlaps_y =
                (py >= by && py <= by + bsy) || (py + psy >= by && py + psy <= by + bsy);
            let overlaps_x =
                (px >= bx && px <= bx + bsx) || (px + psx >= bx && px + psx <= bx + bsx);
            if window_pane_is_floating(wp) != 0 && overlaps_y && overlaps_x {
                (*ctx).flags |= SCREEN_WRITE_OBSCURED;
                return true;
            }
        }
    }
}

/// Whether `gc` is a plain single-width printable cell, so a one-column redraw
/// can be sent as a single cell rather than a whole line.
/// C `vendor/tmux/screen-write.c`: `static int screen_write_cell_is_single(const struct grid_cell *gc)`
unsafe fn screen_write_cell_is_single(gc: *const grid_cell) -> bool {
    unsafe {
        (*gc).data.width == 1
            && (*gc).data.size == 1
            && (*gc).data.data[0] >= 0x20
            && (*gc).data.data[0] != 0x7f
            && !(*gc).flags.intersects(
                grid_flag::CLEARED | grid_flag::PADDING | grid_flag::TAB,
            )
    }
}

/// Redraw all visible cells on one line of the pane, skipping the spans a
/// floating pane covers.
/// C `vendor/tmux/screen-write.c:1201`: `static void screen_write_redraw_line(struct screen_write_ctx *ctx, struct tty_ctx *ttyctx, u_int yy)`
unsafe fn screen_write_redraw_line(
    ctx: *mut screen_write_ctx,
    ttyctx: *mut tty_ctx,
    yy: u32,
) {
    unsafe {
        let wp = (*ctx).wp;
        let s = (*ctx).s;
        let sx = screen_size_x(s);
        let mut gc: grid_cell = zeroed();
        let mut ngc: grid_cell = zeroed();
        let (xoff, yoff) = ((*wp).xoff as c_int, (*wp).yoff as c_int);

        let r = window_visible_ranges(wp, xoff, yoff + yy as c_int, sx, null_mut());
        for i in 0..(*r).used as usize {
            let ri = *(*r).ranges.add(i);
            if ri.nx == 0 {
                continue;
            }

            let cx = (ri.px as c_int - xoff) as u32;
            if cx >= sx {
                continue;
            }
            // `screen-write.c:1221` computes this in u_int; a range wider than
            // the pane wraps there rather than aborting.
            (*ttyctx).num = if cx.wrapping_add(ri.nx) > sx {
                sx.wrapping_sub(cx)
            } else {
                ri.nx
            };
            if (*ttyctx).num == 0 {
                continue;
            }
            (*ttyctx).ocx = cx;
            (*ttyctx).ocy = yy;

            if (*ttyctx).num != 1 {
                tty_write(tty_cmd_redrawline, ttyctx);
                continue;
            }

            grid_view_get_cell((*s).grid, cx, yy, &raw mut gc);
            if !screen_write_cell_is_single(&raw const gc) {
                tty_write(tty_cmd_redrawline, ttyctx);
                continue;
            }
            if !gc.flags.intersects(grid_flag::SELECTED) {
                (*ttyctx).cell = &raw const gc;
            } else {
                screen_select_cell(s, &raw mut ngc, &raw const gc);
                (*ttyctx).cell = &raw const ngc;
            }
            tty_write(tty_cmd_cell, ttyctx);
        }
    }
}

/// Redraw all visible cells in a pane.
/// C `vendor/tmux/screen-write.c:1290`: `static void screen_write_redraw_pane(struct screen_write_ctx *ctx, struct tty_ctx *ttyctx)`
unsafe fn screen_write_redraw_pane(ctx: *mut screen_write_ctx, ttyctx: *mut tty_ctx) {
    unsafe {
        for yy in 0..screen_size_y((*ctx).s) {
            screen_write_redraw_line(ctx, ttyctx, yy);
        }
    }
}

/// Queue a clear of `nx` cells at `px` on the current line.
/// C `vendor/tmux/screen-write.c`: `static void screen_write_collect_insert_clear(struct screen_write_ctx *ctx, u_int px, u_int nx, u_int bg)`
unsafe fn screen_write_collect_insert_clear(
    ctx: *mut screen_write_ctx,
    px: u32,
    nx: u32,
    bg: u32,
) {
    unsafe {
        if nx == 0 {
            return;
        }
        let s = (*ctx).s;
        let cl = (*s).write_list.add((*s).cy as usize);
        let ci = (*ctx).item;
        (*ci).x = px;
        (*ci).used = nx;
        (*ci).type_ = screen_write_citem_type::Clear;
        (*ci).bg = bg;

        let before =
            screen_write_collect_trim(ctx, (*s).cy, (*ci).x, (*ci).used, &raw mut (*ci).wrapped);
        if before.is_null() {
            tailq_insert_tail(&raw mut (*cl).items, ci);
        } else {
            tailq_insert_before(before, ci);
        }
        (*ctx).item = screen_write_get_citem().as_ptr();
    }
}

unsafe fn screen_write_initctx(ctx: *mut screen_write_ctx, ttyctx: *mut tty_ctx, sync: i32) {
    unsafe {
        let s = (*ctx).s;

        memset0(ttyctx);

        (*ttyctx).s = s;
        (*ttyctx).sx = screen_size_x(s);
        (*ttyctx).sy = screen_size_y(s);

        (*ttyctx).ocx = (*s).cx;
        (*ttyctx).ocy = (*s).cy;
        (*ttyctx).orlower = (*s).rlower;
        (*ttyctx).orupper = (*s).rupper;

        // C screen-write.c:282-284: the context's `defaults` pointer aims at the
        // cell stored beside it, and the hyperlink store is this screen's.
        memcpy__(&raw mut (*ttyctx).defaults, &raw const GRID_DEFAULT_CELL);
        (*ttyctx).style_ctx.defaults = &raw const (*ttyctx).defaults;
        (*ttyctx).style_ctx.hyperlinks = (*(*ctx).s).hyperlinks;

        if let Some(init_ctx_cb) = (*ctx).init_ctx_cb {
            init_ctx_cb(ctx, ttyctx);
            if !(*ttyctx).style_ctx.palette.is_null() {
                let palette = (*ttyctx).style_ctx.palette;
                if (*ttyctx).defaults.fg == 8 {
                    (*ttyctx).defaults.fg = (*palette).fg;
                }
                if (*ttyctx).defaults.bg == 8 {
                    (*ttyctx).defaults.bg = (*palette).bg;
                }
            }
        } else {
            (*ttyctx).redraw_cb = Some(screen_write_redraw_cb);
            if !(*ctx).wp.is_null() {
                // The pane's dim rides along with its default colours.
                tty_default_colours(
                    &raw mut (*ttyctx).defaults,
                    (*ctx).wp,
                    &raw mut (*ttyctx).style_ctx.dim,
                );
                (*ttyctx).style_ctx.palette = &raw mut (*(*ctx).wp).palette;
                (*ttyctx).set_client_cb = Some(screen_write_set_client_cb);
                (*ttyctx).arg = (*ctx).wp.cast();
            }
        }

        if (*ctx).flags & SCREEN_WRITE_SYNC == 0 {
            // For the active pane or for an overlay (no pane), we want to
            // only use synchronized updates if requested (commands that
            // move the cursor); for other panes, always use it, since the
            // cursor will have to move.
            if !(*ctx).wp.is_null() {
                // ztmux: a window with a floating pane always syncs. Drawing
                // either the float or a pane clipped around it moves the cursor
                // for every span, so an unsynchronised frame is visibly torn —
                // the float flickered whenever dynamic content sat behind it.
                // Upstream forces sync on non-active panes for the same stated
                // reason ("the cursor will have to move"); this extends it to
                // the case its scene cache makes unnecessary.
                if (*ctx).wp != (*(*(*ctx).wp).window).active
                    || window_has_floating_panes((*(*ctx).wp).window) != 0
                {
                    (*ttyctx).num = 1;
                } else {
                    (*ttyctx).num = sync as u32;
                }
            } else {
                (*ttyctx).num = 0x10 | (sync as u32);
            }
            tty_write(tty_cmd_syncstart, ttyctx);
            (*ctx).flags |= SCREEN_WRITE_SYNC;
        }
    }
}

/// Make write list.
/// C `vendor/tmux/screen-write.c:328`: `void screen_write_make_list(struct screen *s)`
pub unsafe fn screen_write_make_list(s: *mut screen) {
    unsafe {
        (*s).write_list = xcalloc_(screen_size_y(s) as usize).as_ptr();
        for y in 0..screen_size_y(s) {
            tailq_init(&raw mut (*(*s).write_list.add(y as usize)).items);
        }
    }
}

/// Free write list.
/// C `vendor/tmux/screen-write.c:339`: `void screen_write_free_list(struct screen *s)`
pub unsafe fn screen_write_free_list(s: *mut screen) {
    unsafe {
        for y in 0..screen_size_y(s) {
            free_((*(*s).write_list.add(y as usize)).data);
        }
        free_((*s).write_list);
    }
}

/// Set up for writing.
/// C `vendor/tmux/screen-write.c:350`: `static void screen_write_init(struct screen_write_ctx *ctx, struct screen *s)`
unsafe fn screen_write_init(ctx: *mut screen_write_ctx, s: *mut screen) {
    unsafe {
        memset0(ctx);

        (*ctx).s = s;

        if (*(*ctx).s).write_list.is_null() {
            screen_write_make_list((*ctx).s);
        }
        (*ctx).item = screen_write_get_citem().as_ptr();

        (*ctx).scrolled = 0;
        (*ctx).bg = 8;
    }
}

/// Initialize writing with a pane.
/// C `vendor/tmux/screen-write.c:366`: `void screen_write_start_pane(struct screen_write_ctx *ctx, struct window_pane *wp, struct screen *s)`
pub unsafe fn screen_write_start_pane(
    ctx: *mut screen_write_ctx,
    wp: *mut window_pane,
    mut s: *mut screen,
) {
    unsafe {
        if s.is_null() {
            s = (*wp).screen;
        }
        screen_write_init(ctx, s);
        (*ctx).wp = wp;

        if log_get_level() != 0 {
            // log_debug("%s: size %ux%u, pane %%%u (at %u,%u)", __func__, screen_size_x((*ctx).s), screen_size_y((*ctx).s), (*wp).id, (*wp).xoff as u32, (*wp).yoff as u32);
        }
    }
}

/// Initialize writing with a callback.
/// C `vendor/tmux/screen-write.c:383`: `void screen_write_start_callback(struct screen_write_ctx *ctx, struct screen *s, screen_write_init_ctx_cb cb, void *arg)`
pub unsafe fn screen_write_start_callback(
    ctx: *mut screen_write_ctx,
    s: *mut screen,
    cb: screen_write_init_ctx_cb,
    arg: *mut c_void,
) {
    unsafe {
        screen_write_init(ctx, s);

        (*ctx).init_ctx_cb = cb;
        (*ctx).arg = arg;

        if log_get_level() != 0 {
            // log_debug("%s: size %ux%u, with callback", __func__, screen_size_x((*ctx).s), screen_size_y((*ctx).s));
        }
    }
}

/// Initialize writing.
/// C `vendor/tmux/screen-write.c:399`: `void screen_write_start(struct screen_write_ctx *ctx, struct screen *s)`
pub unsafe fn screen_write_start(ctx: *mut screen_write_ctx, s: *mut screen) {
    unsafe {
        screen_write_init(ctx, s);

        if log_get_level() != 0 {
            // log_debug("%s: size %ux%u, no pane", __func__, screen_size_x((*ctx).s), screen_size_y((*ctx).s));
        }
    }
}

/// Finish writing.
/// C `vendor/tmux/screen-write.c:411`: `void screen_write_stop(struct screen_write_ctx *ctx)`
pub unsafe fn screen_write_stop(ctx: *mut screen_write_ctx) {
    unsafe {
        screen_write_collect_end(ctx);
        screen_write_collect_flush(ctx, 0, "screen_write_stop");

        screen_write_free_citem((*ctx).item);
    }
}

/// Reset screen state.
/// C `vendor/tmux/screen-write.c:421`: `void screen_write_reset(struct screen_write_ctx *ctx)`
pub unsafe fn screen_write_reset(ctx: *mut screen_write_ctx) {
    unsafe {
        let s = (*ctx).s;

        screen_reset_tabs(s);
        screen_write_scrollregion(ctx, 0, screen_size_y(s) - 1);

        (*s).mode = mode_flag::MODE_CURSOR | mode_flag::MODE_WRAP;

        if options_get_number_(GLOBAL_OPTIONS, "extended-keys") == 2 {
            (*s).mode = ((*s).mode & !EXTENDED_KEY_MODES) | mode_flag::MODE_KEYS_EXTENDED;
        }

        screen_write_clearscreen(ctx, 8);
        screen_write_set_cursor(ctx, 0, 0);
    }
}

/// Write character.
/// C `vendor/tmux/screen-write.c:439`: `void screen_write_putc(struct screen_write_ctx *ctx, const struct grid_cell *gcp, u_char ch)`
pub unsafe fn screen_write_putc(ctx: *mut screen_write_ctx, gcp: *const grid_cell, ch: u8) {
    unsafe {
        let mut gc: grid_cell = zeroed();
        memcpy__(&raw mut gc, gcp);

        utf8_set(&raw mut gc.data, ch);
        screen_write_cell(ctx, &raw mut gc);
    }
}

macro_rules! screen_write_strlen {
   ($fmt:literal $(, $args:expr)* $(,)?) => {
        crate::screen_write::screen_write_strlen_(format_args!($fmt $(, $args)*))
    };
}
pub(crate) use screen_write_strlen;
/// Calculate string length.
pub unsafe fn screen_write_strlen_(args: std::fmt::Arguments) -> usize {
    unsafe {
        let mut ud: utf8_data = zeroed();

        let mut size = 0;

        let mut msg = args.to_string();
        msg.push('\0');
        let mut ptr: *mut u8 = msg.as_mut_ptr();

        while *ptr != b'\0' {
            if *ptr > 0x7f && utf8_open(&raw mut ud, *ptr) == utf8_state::UTF8_MORE {
                ptr = ptr.add(1);

                let left = strlen(ptr.cast());
                if left < ud.size as usize - 1 {
                    break;
                }
                let mut more: utf8_state;
                while {
                    more = utf8_append(&raw mut ud, *ptr);
                    more == utf8_state::UTF8_MORE
                } {
                    ptr = ptr.add(1);
                }
                ptr = ptr.add(1);

                if more == utf8_state::UTF8_DONE {
                    size += ud.width;
                }
            } else {
                if *ptr > 0x1f && *ptr < 0x7f {
                    size += 1;
                }
                ptr = ptr.add(1);
            }
        }

        size as usize
    }
}

macro_rules! screen_write_text {
   ($ctx:expr, $cx:expr, $width: expr, $lines: expr, $more: expr, $gcp: expr, $fmt:literal $(, $args:expr)* $(,)?) => {
        crate::screen_write::screen_write_text_($ctx, $cx, $width, $lines, $more, $gcp, format_args!($fmt $(, $args)*))
    };
}
pub(crate) use screen_write_text;

/// Write string wrapped over lines.
pub unsafe fn screen_write_text_(
    ctx: *mut screen_write_ctx,
    cx: u32,
    width: u32,
    lines: u32,
    more: i32,
    gcp: *const grid_cell,
    args: std::fmt::Arguments,
) -> bool {
    unsafe {
        let more = more != 0;
        let s = (*ctx).s;
        let cy = (*s).cy;
        let mut idx = 0;

        let mut gc: grid_cell = zeroed();
        memcpy__(&raw mut gc, gcp);

        let mut tmp = args.to_string();
        tmp.push('\0');
        let tmp = tmp.as_mut_ptr().cast();
        let text = utf8_fromcstr(tmp);

        let mut left = (cx + width) - (*s).cx;
        loop {
            // Find the end of what can fit on the line.
            let mut at = 0;
            let mut end = idx;
            while (*text.add(end)).size != 0 {
                if (*text.add(end)).size == 1 && (*text.add(end)).data[0] == b'\n' {
                    break;
                }
                if at + (*text.add(end)).width as u32 > left {
                    break;
                }
                at += (*text.add(end)).width as u32;
                end += 1;
            }

            // If we're on a space, that's the end. If not, walk back to
            // try and find one.
            let next = if (*text.add(end)).size == 0 {
                end
            } else if ((*text.add(end)).size == 1 && (*text.add(end)).data[0] == b'\n')
                || ((*text.add(end)).size == 1 && (*text.add(end)).data[0] == b' ')
            {
                end + 1
            } else {
                let mut i = end;
                while i > idx {
                    if (*text.add(i)).size == 1 && (*text.add(i)).data[0] == b' ' {
                        break;
                    }
                    i -= 1;
                }
                if i != idx {
                    end = i;
                    i + 1
                } else {
                    end
                }
            };

            // Print the line.
            for i in idx..end {
                utf8_copy(&raw mut gc.data, text.add(i));
                screen_write_cell(ctx, &gc);
            }

            // If at the bottom, stop.
            idx = next;
            if (*s).cy == cy + lines - 1 || (*text.add(idx)).size == 0 {
                break;
            }

            screen_write_cursormove(ctx, cx as i32, (*s).cy as i32 + 1, 0);
            left = width;
        }

        // Fail if on the last line and there is more to come or at the end, or
        // if the text was not entirely consumed.
        if ((*s).cy == cy + lines - 1 && (!more || (*s).cx == cx + width))
            || (*text.add(idx)).size != 0
        {
            free_(text);
            return false;
        }
        free_(text);

        // If no more to come, move to the next line. Otherwise, leave on
        // the same line (except if at the end).
        if !more || (*s).cx == cx + width {
            screen_write_cursormove(ctx, cx as i32, (*s).cy as i32 + 1, 0);
        }
        true
    }
}

/// Write simple string (no maximum length).
macro_rules! screen_write_puts {
   ($ctx:expr, $gcp:expr, $fmt:literal $(, $args:expr)* $(,)?) => {
        crate::screen_write::screen_write_vnputs!($ctx, -1, $gcp, $fmt $(, $args)*);
   }
}
pub(crate) use screen_write_puts;

/// Write string with length limit (-1 for unlimited).
macro_rules! screen_write_nputs {
   ($ctx:expr, $maxlen:expr, $gcp:expr, $fmt:literal $(, $args:expr)* $(,)?) => {
        crate::screen_write::screen_write_vnputs!($ctx, $maxlen, $gcp, $fmt $(, $args)*);
   }
}
pub(crate) use screen_write_nputs;

macro_rules! screen_write_vnputs {
   ($ctx:expr, $maxlen:expr, $gcp:expr, $fmt:literal $(, $args:expr)* $(,)?) => {
        crate::screen_write::screen_write_vnputs_($ctx, $maxlen, $gcp, format_args!($fmt $(, $args)*))
    };
}
pub(crate) use screen_write_vnputs;

pub(crate) unsafe fn screen_write_vnputs_(
    ctx: *mut screen_write_ctx,
    maxlen: isize,
    gcp: *const grid_cell,
    args: std::fmt::Arguments,
) {
    unsafe {
        let mut gc: grid_cell = zeroed();
        let ud: *mut utf8_data = &raw mut gc.data;
        let mut size: usize = 0;

        memcpy__(&raw mut gc, gcp);
        let mut msg = args.to_string();
        msg.push('\0');

        let mut ptr: *mut u8 = msg.as_mut_ptr();
        while *ptr != b'\0' {
            if *ptr > 0x7f && utf8_open(ud, *ptr) == utf8_state::UTF8_MORE {
                ptr = ptr.add(1);

                let left = strlen(ptr.cast());
                if left < (*ud).size as usize - 1 {
                    break;
                }
                let mut more: utf8_state;
                while {
                    more = utf8_append(ud, *ptr);
                    more == utf8_state::UTF8_MORE
                } {
                    ptr = ptr.add(1);
                }
                ptr = ptr.add(1);

                if more != utf8_state::UTF8_DONE {
                    continue;
                }
                if maxlen > 0 && size + (*ud).width as usize > maxlen as usize {
                    while size < maxlen as usize {
                        screen_write_putc(ctx, &raw const gc, b' ');
                        size += 1;
                    }
                    break;
                }
                size += (*ud).width as usize;
                screen_write_cell(ctx, &raw const gc);
            } else {
                if maxlen > 0 && size + 1 > maxlen as usize {
                    break;
                }

                if *ptr == b'\x01' {
                    gc.attr ^= grid_attr::GRID_ATTR_CHARSET;
                } else if *ptr == b'\n' {
                    screen_write_linefeed(ctx, false, 8);
                    screen_write_carriagereturn(ctx);
                } else if *ptr > 0x1f && *ptr < 0x7f {
                    size += 1;
                    screen_write_putc(ctx, &gc, *ptr);
                }
                ptr = ptr.add(1);
            }
        }
    }
}

/// Copy from another screen but without the selection stuff. Assumes the target
/// region is already big enough.
/// C `vendor/tmux/screen-write.c:666`: `void screen_write_fast_copy(struct screen_write_ctx *ctx, struct screen *src, u_int px, u_int py, u_int nx, u_int ny)`
pub unsafe fn screen_write_fast_copy(
    ctx: *mut screen_write_ctx,
    src: *mut screen,
    px: u32,
    py: u32,
    nx: u32,
    ny: u32,
) {
    unsafe {
        let s = (*ctx).s;
        let gd = (*src).grid;
        let mut gc: grid_cell = zeroed();

        if nx == 0 || ny == 0 {
            return;
        }

        let mut cy = (*s).cy;
        for yy in py..(py + ny) {
            if yy >= (*gd).hsize + (*gd).sy {
                break;
            }
            let mut cx = (*s).cx;
            for xx in px..(px + nx) {
                if xx >= (*grid_get_line(gd, yy)).cellsize {
                    break;
                }
                grid_get_cell(gd, xx, yy, &raw mut gc);
                if xx + gc.data.width as u32 > px + nx {
                    break;
                }
                grid_view_set_cell((*(*ctx).s).grid, cx, cy, &gc);
                cx += 1;
            }
            cy += 1;
        }
    }
}

/// Select character set for drawing border lines.
/// C `vendor/tmux/screen-write.c:722`: `static void screen_write_box_border_set(enum box_lines lines, int cell_type, struct grid_cell *gc)`
unsafe fn screen_write_box_border_set(lines: box_lines, cell_type: cell_type, gc: *mut grid_cell) {
    unsafe {
        match lines {
            box_lines::BOX_LINES_NONE => (),
            box_lines::BOX_LINES_DOUBLE => {
                (*gc).attr &= !grid_attr::GRID_ATTR_CHARSET;
                utf8_copy(&raw mut (*gc).data, tty_acs_double_borders(cell_type));
            }
            box_lines::BOX_LINES_HEAVY => {
                (*gc).attr &= !grid_attr::GRID_ATTR_CHARSET;
                utf8_copy(&raw mut (*gc).data, tty_acs_heavy_borders(cell_type));
            }
            box_lines::BOX_LINES_ROUNDED => {
                (*gc).attr &= !grid_attr::GRID_ATTR_CHARSET;
                utf8_copy(&raw mut (*gc).data, tty_acs_rounded_borders(cell_type));
            }
            box_lines::BOX_LINES_SIMPLE => {
                (*gc).attr &= !grid_attr::GRID_ATTR_CHARSET;
                utf8_set(&raw mut (*gc).data, SIMPLE_BORDERS[cell_type as usize]);
            }
            box_lines::BOX_LINES_PADDED => {
                (*gc).attr &= !grid_attr::GRID_ATTR_CHARSET;
                utf8_set(&raw mut (*gc).data, PADDED_BORDERS[cell_type as usize]);
            }
            box_lines::BOX_LINES_SINGLE | box_lines::BOX_LINES_DEFAULT => {
                (*gc).attr |= grid_attr::GRID_ATTR_CHARSET;
                utf8_set(&raw mut (*gc).data, CELL_BORDERS[cell_type as usize]);
            }
        }
    }
}

/// Draw a horizontal line on screen.
/// C `vendor/tmux/screen-write.c:758`: `void screen_write_hline(struct screen_write_ctx *ctx, u_int nx, int left, int right, enum box_lines lines, const struct grid_cell *border_gc)`
pub unsafe fn screen_write_hline(
    ctx: *mut screen_write_ctx,
    nx: u32,
    left: i32,
    right: i32,
    lines: box_lines,
    border_gc: *const grid_cell,
) {
    unsafe {
        let s: *mut screen = (*ctx).s;
        let mut gc: grid_cell = zeroed();
        // u_int cx, cy, i;

        let cx = (*s).cx;
        let cy = (*s).cy;

        if !border_gc.is_null() {
            memcpy__(&raw mut gc, border_gc);
        } else {
            memcpy__(&raw mut gc, &raw const GRID_DEFAULT_CELL);
        }
        gc.attr |= grid_attr::GRID_ATTR_CHARSET;

        if left != 0 {
            screen_write_box_border_set(lines, cell_type::CELL_LEFTJOIN, &raw mut gc);
        } else {
            screen_write_box_border_set(lines, cell_type::CELL_LEFTRIGHT, &raw mut gc);
        }
        screen_write_cell(ctx, &gc);

        screen_write_box_border_set(lines, cell_type::CELL_LEFTRIGHT, &raw mut gc);
        for _ in 1..(nx - 1) {
            screen_write_cell(ctx, &raw mut gc);
        }

        if right != 0 {
            screen_write_box_border_set(lines, cell_type::CELL_RIGHTJOIN, &raw mut gc);
        } else {
            screen_write_box_border_set(lines, cell_type::CELL_LEFTRIGHT, &raw mut gc);
        }
        screen_write_cell(ctx, &raw const gc);

        screen_write_set_cursor(ctx, cx as i32, cy as i32);
    }
}

/// Draw a vertical line on screen.
/// C `vendor/tmux/screen-write.c:795`: `void screen_write_vline(struct screen_write_ctx *ctx, u_int ny, int top, int bottom, const struct grid_cell *gcp)`
pub unsafe fn screen_write_vline(
    ctx: *mut screen_write_ctx,
    ny: u32,
    top: i32,
    bottom: i32,
    gcp: *const grid_cell,
) {
    unsafe {
        let s = (*ctx).s;
        let mut gc: grid_cell = zeroed();

        let cx = (*s).cx;
        let cy = (*s).cy;

        if gcp.is_null() {
            memcpy__(&raw mut gc, &raw const GRID_DEFAULT_CELL);
        } else {
            memcpy__(&raw mut gc, gcp);
        }
        gc.attr |= grid_attr::GRID_ATTR_CHARSET;

        screen_write_putc(ctx, &raw const gc, if top != 0 { b'w' } else { b'x' });

        for i in 1..(ny - 1) {
            screen_write_set_cursor(ctx, cx as i32, (cy + i) as i32);
            screen_write_putc(ctx, &raw const gc, b'x');
        }
        screen_write_set_cursor(ctx, cx as i32, (cy + ny - 1) as i32);
        screen_write_putc(ctx, &raw const gc, if bottom != 0 { b'v' } else { b'x' });

        screen_write_set_cursor(ctx, cx as i32, cy as i32);
    }
}

/// Draw a menu on screen.
/// C `vendor/tmux/screen-write.c:824`: `void screen_write_menu(struct screen_write_ctx *ctx, struct menu *menu, int choice, enum box_lines lines, const struct grid_cell *menu_gc, const struct grid_cell *border_gc, const struct grid_cell *choice_gc)`
pub unsafe fn screen_write_menu(
    ctx: *mut screen_write_ctx,
    menu: *mut menu,
    choice: i32,
    lines: box_lines,
    menu_gc: *const grid_cell,
    border_gc: *const grid_cell,
    choice_gc: *const grid_cell,
) {
    unsafe {
        let s = (*ctx).s;
        let mut default_gc: grid_cell = zeroed();
        let mut gc = &raw const default_gc;

        // u_int cx, cy, i, j;
        let width = (*menu).width;

        let cx = (*s).cx;
        let cy = (*s).cy;

        memcpy__(&raw mut default_gc, menu_gc);

        screen_write_box(
            ctx,
            (*menu).width + 4,
            (*menu).items.len() as u32 + 2,
            lines,
            border_gc,
            Some(&(*menu).title),
        );

        for (i, item) in (*menu).items.iter_mut().enumerate() {
            let name: &str = &item.name;
            // TODO double check this name.is_empty() was previously name.is_null()
            if name.is_empty() {
                screen_write_cursormove(ctx, cx as i32, (cy + 1 + i as u32) as i32, 0);
                screen_write_hline(ctx, width + 4, 1, 1, lines, border_gc);
                continue;
            }

            if choice >= 0 && i as u32 == choice as u32 && !name.starts_with('-') {
                gc = choice_gc;
            }

            screen_write_cursormove(ctx, cx as i32 + 1, (cy + 1 + i as u32) as i32, 0);
            for _ in 0..(width + 2) {
                screen_write_putc(ctx, gc, b' ');
            }

            screen_write_cursormove(ctx, cx as i32 + 2, (cy + 1 + i as u32) as i32, 0);
            if let Some(stripped) = name.strip_prefix('-') {
                default_gc.attr |= grid_attr::GRID_ATTR_DIM;
                format_draw(ctx, gc, width, stripped, null_mut(), 0);
                default_gc.attr &= !grid_attr::GRID_ATTR_DIM;
                continue;
            }

            format_draw(ctx, gc, width, name, null_mut(), 0);
            gc = &raw mut default_gc;
        }

        screen_write_set_cursor(ctx, cx as i32, cy as i32);
    }
}

/// Draw a box on screen.
/// C `vendor/tmux/screen-write.c:875`: `void screen_write_box(struct screen_write_ctx *ctx, u_int nx, u_int ny, enum box_lines lines, const struct grid_cell *gcp, const char *title)`
pub unsafe fn screen_write_box(
    ctx: *mut screen_write_ctx,
    nx: u32,
    ny: u32,
    lines: box_lines,
    gcp: *const grid_cell,
    title: Option<&str>,
) {
    unsafe {
        let s = (*ctx).s;
        let mut gc: grid_cell = zeroed();

        let cx = (*s).cx;
        let cy = (*s).cy;

        if !gcp.is_null() {
            memcpy__(&raw mut gc, gcp);
        } else {
            memcpy__(&raw mut gc, &raw const GRID_DEFAULT_CELL);
        }

        gc.attr |= grid_attr::GRID_ATTR_CHARSET;
        gc.flags |= grid_flag::NOPALETTE;

        // Draw top border
        screen_write_box_border_set(lines, cell_type::CELL_TOPLEFT, &raw mut gc);
        screen_write_cell(ctx, &raw const gc);
        screen_write_box_border_set(lines, cell_type::CELL_LEFTRIGHT, &raw mut gc);
        for _ in 1..(nx - 1) {
            screen_write_cell(ctx, &raw const gc);
        }
        screen_write_box_border_set(lines, cell_type::CELL_TOPRIGHT, &raw mut gc);
        screen_write_cell(ctx, &raw const gc);

        // Draw bottom border
        screen_write_set_cursor(ctx, cx as i32, (cy + ny - 1) as i32);
        screen_write_box_border_set(lines, cell_type::CELL_BOTTOMLEFT, &raw mut gc);
        screen_write_cell(ctx, &gc);
        screen_write_box_border_set(lines, cell_type::CELL_LEFTRIGHT, &raw mut gc);
        for _ in 1..(nx - 1) {
            screen_write_cell(ctx, &raw const gc);
        }
        screen_write_box_border_set(lines, cell_type::CELL_BOTTOMRIGHT, &raw mut gc);
        screen_write_cell(ctx, &raw const gc);

        // Draw sides
        screen_write_box_border_set(lines, cell_type::CELL_TOPBOTTOM, &raw mut gc);
        for i in 1..(ny - 1) {
            // left side
            screen_write_set_cursor(ctx, cx as i32, (cy + i) as i32);
            screen_write_cell(ctx, &raw const gc);
            // right side
            screen_write_set_cursor(ctx, (cx + nx - 1) as i32, (cy + i) as i32);
            screen_write_cell(ctx, &raw const gc);
        }

        if let Some(title) = title {
            gc.attr &= !grid_attr::GRID_ATTR_CHARSET;
            screen_write_cursormove(ctx, (cx + 2) as i32, cy as i32, 0);
            format_draw(ctx, &raw const gc, nx - 4, title, null_mut(), 0);
        }

        screen_write_set_cursor(ctx, cx as i32, cy as i32);
    }
}

/// Write a preview version of a window. Assumes target area is big enough and already cleared.
/// C `vendor/tmux/screen-write.c:937`: `void screen_write_preview(struct screen_write_ctx *ctx, struct screen *src, u_int nx, u_int ny)`
pub unsafe fn screen_write_preview(ctx: *mut screen_write_ctx, src: *mut screen, nx: u32, ny: u32) {
    unsafe {
        let s = (*ctx).s;
        let mut gc: grid_cell = zeroed();

        let cx = (*s).cx;
        let cy = (*s).cy;

        // If the cursor is on, pick the area around the cursor, otherwise use
        // the top left.
        let mut px: u32;
        let mut py: u32;
        if (*src).mode.intersects(mode_flag::MODE_CURSOR) {
            px = (*src).cx;
            if px < nx / 3 {
                px = 0;
            } else {
                px -= nx / 3;
            }
            if px + nx > screen_size_x(src) {
                if nx > screen_size_x(src) {
                    px = 0;
                } else {
                    px = screen_size_x(src) - nx;
                }
            }
            py = (*src).cy;
            if py < ny / 3 {
                py = 0;
            } else {
                py -= ny / 3;
            }
            if py + ny > screen_size_y(src) {
                if ny > screen_size_y(src) {
                    py = 0;
                } else {
                    py = screen_size_y(src) - ny;
                }
            }
        } else {
            px = 0;
            py = 0;
        }

        screen_write_fast_copy(ctx, src, px, (*(*src).grid).hsize + py, nx, ny);

        if (*src).mode.intersects(mode_flag::MODE_CURSOR) {
            grid_view_get_cell((*src).grid, (*src).cx, (*src).cy, &raw mut gc);
            gc.attr |= grid_attr::GRID_ATTR_REVERSE;
            screen_write_set_cursor(
                ctx,
                cx as i32 + ((*src).cx - px) as i32,
                cy as i32 + ((*src).cy - py) as i32,
            );
            screen_write_cell(ctx, &raw const gc);
        }
    }
}

/// Set a mode.
/// C `vendor/tmux/screen-write.c:992`: `void screen_write_mode_set(struct screen_write_ctx *ctx, int mode)`
pub unsafe fn screen_write_mode_set(ctx: *mut screen_write_ctx, mode: mode_flag) {
    unsafe {
        let s = (*ctx).s;

        (*s).mode |= mode;

        if log_get_level() != 0 {
            // log_debug("%s: %s", __func__, screen_mode_to_string(mode));
        }
    }
}

/// Clear a mode.
/// C `vendor/tmux/screen-write.c:1004`: `void screen_write_mode_clear(struct screen_write_ctx *ctx, int mode)`
pub unsafe fn screen_write_mode_clear(ctx: *mut screen_write_ctx, mode: mode_flag) {
    unsafe {
        let s = (*ctx).s;

        (*s).mode &= !mode;

        if log_get_level() != 0 {
            // log_debug("%s: %s", __func__, screen_mode_to_string(mode));
        }
    }
}

/// Cursor up by ny.
/// C `vendor/tmux/screen-write.c:1064`: `void screen_write_cursorup(struct screen_write_ctx *ctx, u_int ny)`
pub unsafe fn screen_write_cursorup(ctx: *mut screen_write_ctx, mut ny: u32) {
    unsafe {
        let s = (*ctx).s;
        let mut cx: u32 = (*s).cx;
        let mut cy: u32 = (*s).cy;

        if ny == 0 {
            ny = 1;
        }

        if cy < (*s).rupper {
            // Above region.
            if ny > cy {
                ny = cy;
            }
        } else {
            // Below region.
            if ny > cy - (*s).rupper {
                ny = cy - (*s).rupper;
            }
        }
        if cx == screen_size_x(s) {
            cx -= 1;
        }

        cy -= ny;

        screen_write_set_cursor(ctx, cx as i32, cy as i32);
    }
}

/// Cursor down by ny.
/// C `vendor/tmux/screen-write.c:1091`: `void screen_write_cursordown(struct screen_write_ctx *ctx, u_int ny)`
pub unsafe fn screen_write_cursordown(ctx: *mut screen_write_ctx, mut ny: u32) {
    unsafe {
        let s: *mut screen = (*ctx).s;
        let mut cx: u32 = (*s).cx;
        let mut cy: u32 = (*s).cy;

        if ny == 0 {
            ny = 1;
        }

        if cy > (*s).rlower {
            // Below region.
            if ny > screen_size_y(s) - 1 - cy {
                ny = screen_size_y(s) - 1 - cy;
            }
        } else {
            // Above region.
            if ny > (*s).rlower - cy {
                ny = (*s).rlower - cy;
            }
        }
        if cx == screen_size_x(s) {
            cx -= 1;
        } else if ny == 0 {
            return;
        }

        cy += ny;

        screen_write_set_cursor(ctx, cx as i32, cy as i32);
    }
}

/// Cursor right by nx.
/// C `vendor/tmux/screen-write.c:1120`: `void screen_write_cursorright(struct screen_write_ctx *ctx, u_int nx)`
pub unsafe fn screen_write_cursorright(ctx: *mut screen_write_ctx, mut nx: u32) {
    unsafe {
        let s = (*ctx).s;
        let mut cx: u32 = (*s).cx;
        let cy: u32 = (*s).cy;

        if nx == 0 {
            nx = 1;
        }

        if nx > screen_size_x(s) - 1 - cx {
            nx = screen_size_x(s) - 1 - cx;
        }
        if nx == 0 {
            return;
        }

        cx += nx;

        screen_write_set_cursor(ctx, cx as i32, cy as i32);
    }
}

/// Cursor left by nx.
/// C `vendor/tmux/screen-write.c:1140`: `void screen_write_cursorleft(struct screen_write_ctx *ctx, u_int nx)`
pub unsafe fn screen_write_cursorleft(ctx: *mut screen_write_ctx, mut nx: u32) {
    unsafe {
        let s = (*ctx).s;
        let mut cx: u32 = (*s).cx;
        let cy: u32 = (*s).cy;

        if nx == 0 {
            nx = 1;
        }

        if nx > cx {
            nx = cx;
        }
        if nx == 0 {
            return;
        }

        cx -= nx;

        screen_write_set_cursor(ctx, cx as i32, cy as i32);
    }
}

/// Backspace; cursor left unless at start of wrapped line when can move up.
/// C `vendor/tmux/screen-write.c:1160`: `void screen_write_backspace(struct screen_write_ctx *ctx)`
pub unsafe fn screen_write_backspace(ctx: *mut screen_write_ctx) {
    unsafe {
        let s = (*ctx).s;
        let mut cx = (*s).cx;
        let mut cy = (*s).cy;

        if cx == 0 {
            if cy == 0 {
                return;
            }
            let gl = grid_get_line((*s).grid, (*(*s).grid).hsize + cy - 1);
            if (*gl).flags.intersects(grid_line_flag::WRAPPED) {
                cy -= 1;
                cx = screen_size_x(s) - 1;
            }
        } else {
            cx -= 1;
        }

        screen_write_set_cursor(ctx, cx as i32, cy as i32);
    }
}

/// VT100 alignment test.
/// C `vendor/tmux/screen-write.c:1301`: `void screen_write_alignmenttest(struct screen_write_ctx *ctx)`
pub unsafe fn screen_write_alignmenttest(ctx: *mut screen_write_ctx) {
    unsafe {
        let s = (*ctx).s;
        let mut ttyctx: tty_ctx = zeroed();
        let mut gc: grid_cell = zeroed();

        memcpy__(&raw mut gc, &raw const GRID_DEFAULT_CELL);
        utf8_set(&raw mut gc.data, b'E');

        #[cfg(feature = "sixel")]
        {
            if crate::image_::image_free_all(s) && !(*ctx).wp.is_null() {
                (*(*ctx).wp).flags |= window_pane_flags::PANE_REDRAW;
            }
        }

        for yy in 0..screen_size_y(s) {
            for xx in 0..screen_size_x(s) {
                grid_view_set_cell((*s).grid, xx, yy, &raw const gc);
            }
        }

        screen_write_set_cursor(ctx, 0, 0);

        (*s).rupper = 0;
        (*s).rlower = screen_size_y(s) - 1;

        screen_write_initctx(ctx, &raw mut ttyctx, 1);

        screen_write_collect_clear(ctx, 0, screen_size_y(s) - 1);
        if !screen_write_should_draw_line(ctx, (*s).cy) {
        } else if screen_write_pane_is_obscured(ctx) && !(*ctx).wp.is_null() {
            screen_write_redraw_pane(ctx, &raw mut ttyctx);
        } else {
            tty_write(tty_cmd_alignmenttest, &raw mut ttyctx);
        }
    }
}

/// Insert nx characters.
/// C `vendor/tmux/screen-write.c:1342`: `void screen_write_insertcharacter(struct screen_write_ctx *ctx, u_int nx, u_int bg)`
pub unsafe fn screen_write_insertcharacter(ctx: *mut screen_write_ctx, mut nx: u32, bg: u32) {
    unsafe {
        let s = (*ctx).s;
        let mut ttyctx: tty_ctx = zeroed();

        if nx == 0 {
            nx = 1;
        }

        if nx > screen_size_x(s) - (*s).cx {
            nx = screen_size_x(s) - (*s).cx;
        }
        if nx == 0 {
            return;
        }

        if (*s).cx > screen_size_x(s) - 1 {
            return;
        }

        #[cfg(feature = "sixel")]
        {
            if crate::image_::image_check_line(s, (*s).cy, 1) && !(*ctx).wp.is_null() {
                (*(*ctx).wp).flags |= window_pane_flags::PANE_REDRAW;
            }
        }

        screen_write_initctx(ctx, &raw mut ttyctx, 0);
        ttyctx.bg = bg;

        grid_view_insert_cells((*s).grid, (*s).cx, (*s).cy, nx, bg);

        screen_write_collect_flush(ctx, 0, "screen_write_insertcharacter");
        ttyctx.num = nx;
        if !screen_write_should_draw_line(ctx, (*s).cy) {
        } else if screen_write_pane_is_obscured(ctx) && !(*ctx).wp.is_null() {
            screen_write_redraw_pane(ctx, &raw mut ttyctx);
        } else {
            tty_write(tty_cmd_insertcharacter, &raw mut ttyctx);
        }
    }
}

/// Delete nx characters.
/// C `vendor/tmux/screen-write.c:1383`: `void screen_write_deletecharacter(struct screen_write_ctx *ctx, u_int nx, u_int bg)`
pub unsafe fn screen_write_deletecharacter(ctx: *mut screen_write_ctx, mut nx: u32, bg: u32) {
    unsafe {
        let s = (*ctx).s;
        let mut ttyctx: tty_ctx = zeroed();

        if nx == 0 {
            nx = 1;
        }

        if nx > screen_size_x(s) - (*s).cx {
            nx = screen_size_x(s) - (*s).cx;
        }
        if nx == 0 {
            return;
        }

        if (*s).cx > screen_size_x(s) - 1 {
            return;
        }

        #[cfg(feature = "sixel")]
        {
            if crate::image_::image_check_line(s, (*s).cy, 1) && !(*ctx).wp.is_null() {
                (*(*ctx).wp).flags |= window_pane_flags::PANE_REDRAW;
            }
        }

        screen_write_initctx(ctx, &raw mut ttyctx, 0);
        ttyctx.bg = bg;

        grid_view_delete_cells((*s).grid, (*s).cx, (*s).cy, nx, bg);

        screen_write_collect_flush(ctx, 0, "screen_write_deletecharacter");
        ttyctx.num = nx;
        if !screen_write_should_draw_line(ctx, (*s).cy) {
        } else if screen_write_pane_is_obscured(ctx) && !(*ctx).wp.is_null() {
            screen_write_redraw_pane(ctx, &raw mut ttyctx);
        } else {
            tty_write(tty_cmd_deletecharacter, &raw mut ttyctx);
        }
    }
}

/// Clear nx characters.
/// C `vendor/tmux/screen-write.c:1424`: `void screen_write_clearcharacter(struct screen_write_ctx *ctx, u_int nx, u_int bg)`
pub unsafe fn screen_write_clearcharacter(ctx: *mut screen_write_ctx, mut nx: u32, bg: u32) {
    unsafe {
        let s = (*ctx).s;
        let mut ttyctx: tty_ctx = zeroed();

        if nx == 0 {
            nx = 1;
        }

        if nx > screen_size_x(s) - (*s).cx {
            nx = screen_size_x(s) - (*s).cx;
        }
        if nx == 0 {
            return;
        }

        if (*s).cx > screen_size_x(s) - 1 {
            return;
        }

        #[cfg(feature = "sixel")]
        {
            if crate::image_::image_check_line(s, (*s).cy, 1) && !(*ctx).wp.is_null() {
                (*(*ctx).wp).flags |= window_pane_flags::PANE_REDRAW;
            }
        }

        screen_write_initctx(ctx, &raw mut ttyctx, 0);
        ttyctx.bg = bg;

        grid_view_clear((*s).grid, (*s).cx, (*s).cy, nx, 1, bg);

        screen_write_collect_flush(ctx, 0, "screen_write_clearcharacter");
        ttyctx.num = nx;
        if !screen_write_should_draw_line(ctx, (*s).cy) {
        } else if screen_write_pane_is_obscured(ctx) && !(*ctx).wp.is_null() {
            screen_write_redraw_pane(ctx, &raw mut ttyctx);
        } else {
            tty_write(tty_cmd_clearcharacter, &raw mut ttyctx);
        }
    }
}

/// Insert ny lines.
/// C `vendor/tmux/screen-write.c:1465`: `void screen_write_insertline(struct screen_write_ctx *ctx, u_int ny, u_int bg)`
pub unsafe fn screen_write_insertline(ctx: *mut screen_write_ctx, mut ny: u32, bg: u32) {
    unsafe {
        let s = (*ctx).s;
        let gd = (*s).grid;
        let mut ttyctx: tty_ctx = zeroed();

        if ny == 0 {
            ny = 1;
        }

        #[cfg(feature = "sixel")]
        {
            let sy = screen_size_y(s);
            if crate::image_::image_check_line(s, (*s).cy, sy - (*s).cy) && !(*ctx).wp.is_null() {
                (*(*ctx).wp).flags |= window_pane_flags::PANE_REDRAW;
            }
        }

        if (*s).cy < (*s).rupper || (*s).cy > (*s).rlower {
            if ny > screen_size_y(s) - (*s).cy {
                ny = screen_size_y(s) - (*s).cy;
            }
            if ny == 0 {
                return;
            }

            screen_write_initctx(ctx, &raw mut ttyctx, 1);
            ttyctx.bg = bg;

            grid_view_insert_lines(gd, (*s).cy, ny, bg);

            screen_write_collect_flush(ctx, 0, "screen_write_insertline");
            ttyctx.num = ny;
            if !screen_write_should_draw_line(ctx, (*s).cy) {
            } else if screen_write_pane_is_obscured(ctx) && !(*ctx).wp.is_null() {
                screen_write_redraw_pane(ctx, &raw mut ttyctx);
            } else {
                tty_write(tty_cmd_insertline, &raw mut ttyctx);
            }
            return;
        }

        if ny > (*s).rlower + 1 - (*s).cy {
            ny = (*s).rlower + 1 - (*s).cy;
        }
        if ny == 0 {
            return;
        }

        screen_write_initctx(ctx, &raw mut ttyctx, 1);
        ttyctx.bg = bg;

        if (*s).cy < (*s).rupper || (*s).cy > (*s).rlower {
            grid_view_insert_lines(gd, (*s).cy, ny, bg);
        } else {
            grid_view_insert_lines_region(gd, (*s).rlower, (*s).cy, ny, bg);
        }

        screen_write_collect_flush(ctx, 0, "screen_write_insertline");

        ttyctx.num = ny;
        if !screen_write_should_draw_line(ctx, (*s).cy) {
        } else if screen_write_pane_is_obscured(ctx) && !(*ctx).wp.is_null() {
            screen_write_redraw_pane(ctx, &raw mut ttyctx);
        } else {
            tty_write(tty_cmd_insertline, &raw mut ttyctx);
        }
    }
}

/// Delete ny lines.
/// C `vendor/tmux/screen-write.c:1533`: `void screen_write_deleteline(struct screen_write_ctx *ctx, u_int ny, u_int bg)`
pub unsafe fn screen_write_deleteline(ctx: *mut screen_write_ctx, mut ny: u32, bg: u32) {
    unsafe {
        let s = (*ctx).s;
        let gd = (*s).grid;
        let mut ttyctx: tty_ctx = zeroed();
        let sy = screen_size_y(s);

        if ny == 0 {
            ny = 1;
        }

        #[cfg(feature = "sixel")]
        {
            if crate::image_::image_check_line(s, (*s).cy, sy - (*s).cy) && !(*ctx).wp.is_null() {
                (*(*ctx).wp).flags |= window_pane_flags::PANE_REDRAW;
            }
        }

        if (*s).cy < (*s).rupper || (*s).cy > (*s).rlower {
            if ny > sy - (*s).cy {
                ny = sy - (*s).cy;
            }
            if ny == 0 {
                return;
            }

            screen_write_initctx(ctx, &raw mut ttyctx, 1);
            ttyctx.bg = bg;

            grid_view_delete_lines(gd, (*s).cy, ny, bg);

            screen_write_collect_flush(ctx, 0, "screen_write_deleteline");
            ttyctx.num = ny;
            if !screen_write_should_draw_line(ctx, (*s).cy) {
            } else if screen_write_pane_is_obscured(ctx) && !(*ctx).wp.is_null() {
                screen_write_redraw_pane(ctx, &raw mut ttyctx);
            } else {
                tty_write(tty_cmd_deleteline, &raw mut ttyctx);
            }
            return;
        }

        if ny > (*s).rlower + 1 - (*s).cy {
            ny = (*s).rlower + 1 - (*s).cy;
        }
        if ny == 0 {
            return;
        }

        screen_write_initctx(ctx, &raw mut ttyctx, 1);
        ttyctx.bg = bg;

        if (*s).cy < (*s).rupper || (*s).cy > (*s).rlower {
            grid_view_delete_lines(gd, (*s).cy, ny, bg);
        } else {
            grid_view_delete_lines_region(gd, (*s).rlower, (*s).cy, ny, bg);
        }

        screen_write_collect_flush(ctx, 0, "screen_write_deleteline");
        ttyctx.num = ny;
        if !screen_write_should_draw_line(ctx, (*s).cy) {
        } else if screen_write_pane_is_obscured(ctx) && !(*ctx).wp.is_null() {
            screen_write_redraw_pane(ctx, &raw mut ttyctx);
        } else {
            tty_write(tty_cmd_deleteline, &raw mut ttyctx);
        }
    }
}

/// Clear line at cursor.
/// C `vendor/tmux/screen-write.c:1603`: `void screen_write_clearline(struct screen_write_ctx *ctx, u_int bg)`
pub unsafe fn screen_write_clearline(ctx: *mut screen_write_ctx, bg: u32) {
    unsafe {
        let s = (*ctx).s;
        let sx = screen_size_x(s);
        let ci = (*ctx).item;

        let gl = grid_get_line((*s).grid, (*(*s).grid).hsize + (*s).cy);
        if (*gl).cellsize == 0 && COLOUR_DEFAULT(bg as i32) {
            return;
        }

        #[cfg(feature = "sixel")]
        {
            if crate::image_::image_check_line(s, (*s).cy, 1) && !(*ctx).wp.is_null() {
                (*(*ctx).wp).flags |= window_pane_flags::PANE_REDRAW;
            }
        }

        grid_view_clear((*s).grid, 0, (*s).cy, sx, 1, bg);

        screen_write_collect_clear(ctx, (*s).cy, 1);
        (*ci).x = 0;
        (*ci).used = sx;
        (*ci).type_ = screen_write_citem_type::Clear;
        (*ci).bg = bg;
        tailq_insert_tail(
            &raw mut (*(*(*ctx).s).write_list.add((*s).cy as usize)).items,
            ci,
        );
        (*ctx).item = screen_write_get_citem().as_ptr();
    }
}

/// Clear to end of line from cursor.
/// C `vendor/tmux/screen-write.c:1636`: `void screen_write_clearendofline(struct screen_write_ctx *ctx, u_int bg)`
pub unsafe fn screen_write_clearendofline(ctx: *mut screen_write_ctx, bg: u32) {
    unsafe {
        let s = (*ctx).s;
        let sx = screen_size_x(s);
        let ci = (*ctx).item;

        if (*s).cx == 0 {
            screen_write_clearline(ctx, bg);
            return;
        }

        let gl = grid_get_line((*s).grid, (*(*s).grid).hsize + (*s).cy);
        if (*s).cx > sx - 1 || ((*s).cx >= (*gl).cellsize && COLOUR_DEFAULT(bg as i32)) {
            return;
        }

        #[cfg(feature = "sixel")]
        {
            if crate::image_::image_check_line(s, (*s).cy, 1) && !(*ctx).wp.is_null() {
                (*(*ctx).wp).flags |= window_pane_flags::PANE_REDRAW;
            }
        }

        grid_view_clear((*s).grid, (*s).cx, (*s).cy, sx - (*s).cx, 1, bg);

        let before = screen_write_collect_trim(ctx, (*s).cy, (*s).cx, sx - (*s).cx, null_mut());
        (*ci).x = (*s).cx;
        (*ci).used = sx - (*s).cx;
        (*ci).type_ = screen_write_citem_type::Clear;
        (*ci).bg = bg;
        if before.is_null() {
            tailq_insert_tail(
                &raw mut (*(*(*ctx).s).write_list.add((*s).cy as usize)).items,
                ci,
            );
        } else {
            tailq_insert_before(before, ci);
        }
        (*ctx).item = screen_write_get_citem().as_ptr();
    }
}

/// Clear to start of line from cursor.
/// C `vendor/tmux/screen-write.c:1668`: `void screen_write_clearstartofline(struct screen_write_ctx *ctx, u_int bg)`
pub unsafe fn screen_write_clearstartofline(ctx: *mut screen_write_ctx, bg: u32) {
    unsafe {
        let s = (*ctx).s;
        let sx = screen_size_x(s);
        let ci = (*ctx).item;

        if (*s).cx >= sx - 1 {
            screen_write_clearline(ctx, bg);
            return;
        }

        #[cfg(feature = "sixel")]
        {
            if crate::image_::image_check_line(s, (*s).cy, 1) && !(*ctx).wp.is_null() {
                (*(*ctx).wp).flags |= window_pane_flags::PANE_REDRAW;
            }
        }

        if (*s).cx > sx - 1 {
            grid_view_clear((*s).grid, 0, (*s).cy, sx, 1, bg);
        } else {
            grid_view_clear((*s).grid, 0, (*s).cy, (*s).cx + 1, 1, bg);
        }

        let before = screen_write_collect_trim(ctx, (*s).cy, 0, (*s).cx + 1, null_mut());
        (*ci).x = 0;
        (*ci).used = (*s).cx + 1;
        (*ci).type_ = screen_write_citem_type::Clear;
        (*ci).bg = bg;
        if before.is_null() {
            tailq_insert_tail(
                &raw mut (*(*(*ctx).s).write_list.add((*s).cy as usize)).items,
                ci,
            );
        } else {
            tailq_insert_before(before, ci);
        }
        (*ctx).item = screen_write_get_citem().as_ptr();
    }
}

/// Move cursor to px,py.
/// C `vendor/tmux/screen-write.c:1698`: `void screen_write_cursormove(struct screen_write_ctx *ctx, int px, int py, int origin)`
pub unsafe fn screen_write_cursormove(
    ctx: *mut screen_write_ctx,
    mut px: i32,
    mut py: i32,
    origin: i32,
) {
    unsafe {
        let s = (*ctx).s;

        if origin != 0 && py != -1 && (*s).mode.intersects(mode_flag::MODE_ORIGIN) {
            if py as u32 > (*s).rlower - (*s).rupper {
                py = (*s).rlower as i32;
            } else {
                py += (*s).rupper as i32;
            }
        }

        if px != -1 && px as u32 > screen_size_x(s) - 1 {
            px = screen_size_x(s) as i32 - 1;
        }
        if py != -1 && py as u32 > screen_size_y(s) - 1 {
            py = screen_size_y(s) as i32 - 1;
        }

        // log_debug("%s: from %u,%u to %u,%u", __func__, (*s).cx, (*s).cy, px, py);
        screen_write_set_cursor(ctx, px, py);
    }
}

/// Reverse index (up with scroll).
/// C `vendor/tmux/screen-write.c:1721`: `void screen_write_reverseindex(struct screen_write_ctx *ctx, u_int bg)`
pub unsafe fn screen_write_reverseindex(ctx: *mut screen_write_ctx, bg: u32) {
    unsafe {
        let s = (*ctx).s;
        let mut ttyctx: tty_ctx = zeroed();

        if (*s).cy == (*s).rupper {
            #[cfg(feature = "sixel")]
            {
                if crate::image_::image_free_all(s) && !(*ctx).wp.is_null() {
                    (*(*ctx).wp).flags |= window_pane_flags::PANE_REDRAW;
                }
            }

            grid_view_scroll_region_down((*s).grid, (*s).rupper, (*s).rlower, bg);
            screen_write_collect_flush(ctx, 0, "screen_write_reverseindex");

            screen_write_initctx(ctx, &raw mut ttyctx, 1);
            ttyctx.bg = bg;

            if !screen_write_should_draw_line(ctx, (*s).cy) {
            } else if screen_write_pane_is_obscured(ctx) && !(*ctx).wp.is_null() {
                screen_write_redraw_pane(ctx, &raw mut ttyctx);
            } else {
                tty_write(tty_cmd_reverseindex, &raw mut ttyctx);
            }
        } else if (*s).cy > 0 {
            screen_write_set_cursor(ctx, -1, (*s).cy as i32 - 1);
        }
    }
}

/// Set scroll region.
/// C `vendor/tmux/screen-write.c:1757`: `void screen_write_scrollregion(struct screen_write_ctx *ctx, u_int rupper, u_int rlower)`
pub unsafe fn screen_write_scrollregion(
    ctx: *mut screen_write_ctx,
    mut rupper: u32,
    mut rlower: u32,
) {
    unsafe {
        let s = (*ctx).s;

        if rupper > screen_size_y(s) - 1 {
            rupper = screen_size_y(s) - 1;
        }
        if rlower > screen_size_y(s) - 1 {
            rlower = screen_size_y(s) - 1;
        }
        if rupper >= rlower {
            return;
        } // cannot be one line

        screen_write_collect_flush(ctx, 0, "screen_write_scrollregion");

        // Cursor moves to top-left.
        screen_write_set_cursor(ctx, 0, 0);

        (*s).rupper = rupper;
        (*s).rlower = rlower;
    }
}

/// Line feed.
/// C `vendor/tmux/screen-write.c:1780`: `void screen_write_linefeed(struct screen_write_ctx *ctx, int wrapped, u_int bg)`
pub unsafe fn screen_write_linefeed(ctx: *mut screen_write_ctx, wrapped: bool, bg: u32) {
    unsafe {
        let s = (*ctx).s;
        let gd = (*s).grid;

        let rupper = (*s).rupper;
        let rlower = (*s).rlower;

        let gl = grid_get_line(gd, (*gd).hsize + (*s).cy);
        if wrapped {
            (*gl).flags |= grid_line_flag::WRAPPED;
        }

        log_debug!(
            "screen_write_linefeed: at {},{} (region {}-{})",
            (*s).cx,
            (*s).cy,
            rupper,
            rlower
        );

        if bg != (*ctx).bg {
            screen_write_collect_flush(ctx, 1, "screen_write_linefeed");
            (*ctx).bg = bg;
        }

        if (*s).cy == (*s).rlower {
            #[cfg(feature = "sixel")]
            {
                let redraw = if rlower == screen_size_y(s) - 1 {
                    crate::image_::image_scroll_up(s, 1)
                } else {
                    crate::image_::image_check_line(s, rupper, rlower - rupper)
                };
                if redraw && !(*ctx).wp.is_null() {
                    (*(*ctx).wp).flags |= window_pane_flags::PANE_REDRAW;
                }
            }
            grid_view_scroll_region_up(gd, (*s).rupper, (*s).rlower, bg);
            screen_write_collect_scroll(ctx, bg);
            (*ctx).scrolled += 1;
        } else if (*s).cy < screen_size_y(s) - 1 {
            screen_write_set_cursor(ctx, -1, (*s).cy as i32 + 1);
        }
    }
}

/// Scroll up.
/// C `vendor/tmux/screen-write.c:1824`: `void screen_write_scrollup(struct screen_write_ctx *ctx, u_int lines, u_int bg)`
pub unsafe fn screen_write_scrollup(ctx: *mut screen_write_ctx, mut lines: u32, bg: u32) {
    unsafe {
        let s = (*ctx).s;
        let gd = (*s).grid;

        if lines == 0 {
            lines = 1;
        } else if lines > (*s).rlower - (*s).rupper + 1 {
            lines = (*s).rlower - (*s).rupper + 1;
        }

        if bg != (*ctx).bg {
            screen_write_collect_flush(ctx, 1, "screen_write_scrollup");
            (*ctx).bg = bg;
        }

        #[cfg(feature = "sixel")]
        {
            if crate::image_::image_scroll_up(s, lines) && !(*ctx).wp.is_null() {
                (*(*ctx).wp).flags |= window_pane_flags::PANE_REDRAW;
            }
        }

        for _ in 0..lines {
            grid_view_scroll_region_up(gd, (*s).rupper, (*s).rlower, bg);
            screen_write_collect_scroll(ctx, bg);
        }
        (*ctx).scrolled += lines;
    }
}

/// Scroll down.
/// C `vendor/tmux/screen-write.c:1854`: `void screen_write_scrolldown(struct screen_write_ctx *ctx, u_int lines, u_int bg)`
pub unsafe fn screen_write_scrolldown(ctx: *mut screen_write_ctx, mut lines: u32, bg: u32) {
    unsafe {
        let s = (*ctx).s;
        let gd = (*s).grid;
        let mut ttyctx: tty_ctx = zeroed();

        screen_write_initctx(ctx, &raw mut ttyctx, 1);
        ttyctx.bg = bg;

        if lines == 0 {
            lines = 1;
        } else if lines > (*s).rlower - (*s).rupper + 1 {
            lines = (*s).rlower - (*s).rupper + 1;
        }

        #[cfg(feature = "sixel")]
        {
            if crate::image_::image_free_all(s) && !(*ctx).wp.is_null() {
                (*(*ctx).wp).flags |= window_pane_flags::PANE_REDRAW;
            }
        }

        for _ in 0..lines {
            grid_view_scroll_region_down(gd, (*s).rupper, (*s).rlower, bg);
        }

        screen_write_collect_flush(ctx, 0, "screen_write_scrolldown");
        ttyctx.num = lines;
        if !screen_write_should_draw_line(ctx, (*s).cy) {
        } else if screen_write_pane_is_obscured(ctx) && !(*ctx).wp.is_null() {
            screen_write_redraw_pane(ctx, &raw mut ttyctx);
        } else {
            tty_write(tty_cmd_scrolldown, &raw mut ttyctx);
        }
    }
}

/// Carriage return (cursor to start of line).
/// C `vendor/tmux/screen-write.c:1893`: `void screen_write_carriagereturn(struct screen_write_ctx *ctx)`
pub unsafe fn screen_write_carriagereturn(ctx: *mut screen_write_ctx) {
    unsafe {
        screen_write_set_cursor(ctx, 0, -1);
    }
}

/// Clear to end of screen from cursor.
/// C `vendor/tmux/screen-write.c:1900`: `void screen_write_clearendofscreen(struct screen_write_ctx *ctx, u_int bg)`
pub unsafe fn screen_write_clearendofscreen(ctx: *mut screen_write_ctx, bg: u32) {
    unsafe {
        let s = (*ctx).s;
        let gd = (*s).grid;
        let mut ttyctx: tty_ctx = zeroed();
        let sx = screen_size_x(s);
        let sy = screen_size_y(s);

        #[cfg(feature = "sixel")]
        {
            if crate::image_::image_check_line(s, (*s).cy, sy - (*s).cy) && !(*ctx).wp.is_null() {
                (*(*ctx).wp).flags |= window_pane_flags::PANE_REDRAW;
            }
        }

        screen_write_initctx(ctx, &raw mut ttyctx, 1);
        ttyctx.bg = bg;

        // Scroll into history if it is enabled and clearing entire screen.
        if (*s).cx == 0
            && (*s).cy == 0
            && ((*gd).flags & GRID_HISTORY != 0)
            && !(*ctx).wp.is_null()
            && options_get_number_((*(*ctx).wp).options, "scroll-on-clear") != 0
        {
            grid_view_clear_history(gd, bg);
        } else {
            if (*s).cx < sx {
                grid_view_clear(gd, (*s).cx, (*s).cy, sx - (*s).cx, 1, bg);
            }
            grid_view_clear(gd, 0, (*s).cy + 1, sx, sy - ((*s).cy + 1), bg);
        }

        screen_write_collect_clear(ctx, (*s).cy + 1, sy - ((*s).cy + 1));
        screen_write_collect_flush(ctx, 0, "screen_write_clearendofscreen");

        if !screen_write_pane_is_obscured(ctx) {
            tty_write(tty_cmd_clearendofscreen, &raw mut ttyctx);
            return;
        }

        // Can't clear the whole screen in one escape: a floating pane sits over
        // part of it, so queue a clear per visible span instead.
        let (ocx, ocy) = ((*s).cx, (*s).cy);
        let (xoff, yoff) = if (*ctx).wp.is_null() {
            (0, 0)
        } else {
            ((*(*ctx).wp).xoff as c_int, (*(*ctx).wp).yoff as c_int)
        };

        // First line (containing the cursor).
        if (*s).cx < sx {
            let r = window_visible_ranges(
                (*ctx).wp,
                xoff + (*s).cx as c_int,
                yoff + (*s).cy as c_int,
                sx - (*s).cx,
                null_mut(),
            );
            for i in 0..(*r).used as usize {
                let ri = *(*r).ranges.add(i);
                if ri.nx == 0 {
                    continue;
                }
                screen_write_collect_insert_clear(ctx, (ri.px as c_int - xoff) as u32, ri.nx, bg);
            }
        }

        // Below cursor to bottom.
        for y in (*s).cy + 1..sy {
            screen_write_set_cursor(ctx, 0, y as i32);
            let r =
                window_visible_ranges((*ctx).wp, xoff, yoff + y as c_int, sx, null_mut());
            for i in 0..(*r).used as usize {
                let ri = *(*r).ranges.add(i);
                if ri.nx == 0 {
                    continue;
                }
                screen_write_collect_insert_clear(ctx, (ri.px as c_int - xoff) as u32, ri.nx, bg);
            }
        }
        screen_write_set_cursor(ctx, ocx as i32, ocy as i32);
    }
}

/// Clear to start of screen.
/// C `vendor/tmux/screen-write.c:1982`: `void screen_write_clearstartofscreen(struct screen_write_ctx *ctx, u_int bg)`
pub unsafe fn screen_write_clearstartofscreen(ctx: *mut screen_write_ctx, bg: u32) {
    unsafe {
        let s = (*ctx).s;
        let mut ttyctx: tty_ctx = zeroed();
        let sx = screen_size_x(s);

        #[cfg(feature = "sixel")]
        {
            if crate::image_::image_check_line(s, 0, (*s).cy - 1) && !(*ctx).wp.is_null() {
                (*(*ctx).wp).flags |= window_pane_flags::PANE_REDRAW;
            }
        }

        screen_write_initctx(ctx, &raw mut ttyctx, 1);
        ttyctx.bg = bg;

        if (*s).cy > 0 {
            grid_view_clear((*s).grid, 0, 0, sx, (*s).cy, bg);
        }
        if (*s).cx > sx - 1 {
            grid_view_clear((*s).grid, 0, (*s).cy, sx, 1, bg);
        } else {
            grid_view_clear((*s).grid, 0, (*s).cy, (*s).cx + 1, 1, bg);
        }

        screen_write_collect_clear(ctx, 0, (*s).cy);
        screen_write_collect_flush(ctx, 0, "screen_write_clearstartofscreen");

        if !screen_write_pane_is_obscured(ctx) {
            tty_write(tty_cmd_clearstartofscreen, &raw mut ttyctx);
            return;
        }

        let (ocx, ocy) = ((*s).cx, (*s).cy);
        let (xoff, yoff) = if (*ctx).wp.is_null() {
            (0, 0)
        } else {
            ((*(*ctx).wp).xoff as c_int, (*(*ctx).wp).yoff as c_int)
        };

        // Top to above the cursor.
        for y in 0..(*s).cy {
            screen_write_set_cursor(ctx, 0, y as i32);
            let r = window_visible_ranges((*ctx).wp, xoff, yoff + y as c_int, sx, null_mut());
            for i in 0..(*r).used as usize {
                let ri = *(*r).ranges.add(i);
                if ri.nx == 0 {
                    continue;
                }
                screen_write_collect_insert_clear(ctx, (ri.px as c_int - xoff) as u32, ri.nx, bg);
            }
        }

        // Last line (containing the cursor).
        screen_write_set_cursor(ctx, 0, (*s).cy as i32);
        let r = window_visible_ranges(
            (*ctx).wp,
            xoff,
            yoff + ocy as c_int,
            (*s).cx + 1,
            null_mut(),
        );
        for i in 0..(*r).used as usize {
                let ri = *(*r).ranges.add(i);
                if ri.nx == 0 {
                    continue;
                }
                screen_write_collect_insert_clear(ctx, (ri.px as c_int - xoff) as u32, ri.nx, bg);
            }
        screen_write_set_cursor(ctx, ocx as i32, ocy as i32);
    }
}

/// Clear entire screen.
/// C `vendor/tmux/screen-write.c:2055`: `void screen_write_clearscreen(struct screen_write_ctx *ctx, u_int bg)`
pub unsafe fn screen_write_clearscreen(ctx: *mut screen_write_ctx, bg: u32) {
    unsafe {
        let s = (*ctx).s;
        let mut ttyctx: tty_ctx = zeroed();
        let sx = screen_size_x(s);
        let sy = screen_size_y(s);

        #[cfg(feature = "sixel")]
        {
            if crate::image_::image_free_all(s) && !(*ctx).wp.is_null() {
                (*(*ctx).wp).flags |= window_pane_flags::PANE_REDRAW;
            }
        }

        screen_write_initctx(ctx, &raw mut ttyctx, 1);
        ttyctx.bg = bg;

        // Scroll into history if it is enabled.
        if ((*(*s).grid).flags & GRID_HISTORY != 0)
            && !(*ctx).wp.is_null()
            && options_get_number_((*(*ctx).wp).options, "scroll-on-clear") != 0
        {
            grid_view_clear_history((*s).grid, bg);
        } else {
            grid_view_clear((*s).grid, 0, 0, sx, sy, bg);
        }

        screen_write_collect_clear(ctx, 0, sy);

        if !screen_write_pane_is_obscured(ctx) {
            tty_write(tty_cmd_clearscreen, &raw mut ttyctx);
            return;
        }

        let (ocx, ocy) = ((*s).cx, (*s).cy);
        let (xoff, yoff) = if (*ctx).wp.is_null() {
            (0, 0)
        } else {
            ((*(*ctx).wp).xoff as c_int, (*(*ctx).wp).yoff as c_int)
        };

        // Clear every line, skipping what a floating pane covers.
        for y in 0..sy {
            screen_write_set_cursor(ctx, 0, y as i32);
            let r = window_visible_ranges((*ctx).wp, xoff, yoff + y as c_int, sx, null_mut());
            for i in 0..(*r).used as usize {
                let ri = *(*r).ranges.add(i);
                if ri.nx == 0 {
                    continue;
                }
                screen_write_collect_insert_clear(ctx, (ri.px as c_int - xoff) as u32, ri.nx, bg);
            }
        }
        screen_write_set_cursor(ctx, ocx as i32, ocy as i32);
    }
}

/// Clear entire history.
/// C `vendor/tmux/screen-write.c:2117`: `void screen_write_clearhistory(struct screen_write_ctx *ctx)`
pub unsafe fn screen_write_clearhistory(ctx: *mut screen_write_ctx) {
    unsafe {
        grid_clear_history((*(*ctx).s).grid);
    }
}

/// Force a full redraw.
/// C `vendor/tmux/screen-write.c:2124`: `void screen_write_fullredraw(struct screen_write_ctx *ctx)`
pub unsafe fn screen_write_fullredraw(ctx: *mut screen_write_ctx) {
    unsafe {
        let mut ttyctx: tty_ctx = zeroed();

        screen_write_collect_flush(ctx, 0, "screen_write_fullredraw");

        screen_write_initctx(ctx, &raw mut ttyctx, 1);
        if let Some(redraw_cb) = ttyctx.redraw_cb {
            redraw_cb(&raw const ttyctx);
        }
    }
}

/// Trim collected items.
/// C `vendor/tmux/screen-write.c:2137`: `static struct screen_write_citem *screen_write_collect_trim(struct screen_write_ctx *ctx, u_int y, u_int x, u_int used, int *wrapped)`
pub unsafe fn screen_write_collect_trim(
    ctx: *mut screen_write_ctx,
    y: u32,
    x: u32,
    used: u32,
    wrapped: *mut bool,
) -> *mut screen_write_citem {
    unsafe {
        let cl = (*(*ctx).s).write_list.add(y as usize);
        let mut before = null_mut();
        let sx = x;
        let ex = x + used - 1;

        if tailq_empty(&raw const (*cl).items) {
            return null_mut();
        }
        for ci in tailq_foreach(&raw mut (*cl).items).map(NonNull::as_ptr) {
            let csx = (*ci).x;
            let cex = (*ci).x + (*ci).used - 1;

            // Item is entirely before.
            if cex < sx {
                continue;
            } // log_debug("%s: %p %u-%u before %u-%u", __func__, ci, csx, cex, sx, ex);

            // Item is entirely after.
            if csx > ex {
                // log_debug("%s: %p %u-%u after %u-%u", __func__, ci, csx, cex, sx, ex);
                before = ci;
                break;
            }

            // Item is entirely inside.
            if csx >= sx && cex <= ex {
                // log_debug("%s: %p %u-%u inside %u-%u", __func__, ci, csx, cex, sx, ex);
                tailq_remove(&raw mut (*cl).items, ci);
                screen_write_free_citem(ci);
                if csx == 0 && (*ci).wrapped && !wrapped.is_null() {
                    *wrapped = true;
                }
                continue;
            }

            // Item under the start.
            if csx < sx && cex >= sx && cex <= ex {
                // log_debug("%s: %p %u-%u start %u-%u", __func__, ci, csx, cex, sx, ex);
                (*ci).used = sx - csx;
                // log_debug("%s: %p now %u-%u", __func__, ci, (*ci).x, (*ci).x + (*ci).used + 1);
                continue;
            }

            // Item covers the end.
            if cex > ex && csx >= sx && csx <= ex {
                // log_debug("%s: %p %u-%u end %u-%u", __func__, ci, csx, cex, sx, ex);
                (*ci).x = ex + 1;
                (*ci).used = cex - ex;
                // log_debug("%s: %p now %u-%u", __func__, ci, (*ci).x, (*ci).x + (*ci).used + 1);
                before = ci;
                break;
            }

            // Item must cover both sides.
            // log_debug("%s: %p %u-%u under %u-%u", __func__, ci, csx, cex, sx, ex);
            let ci2 = screen_write_get_citem().as_ptr();
            (*ci2).type_ = (*ci).type_;
            (*ci2).bg = (*ci).bg;
            memcpy__(&raw mut (*ci2).gc, &raw mut (*ci).gc);
            tailq_insert_after(&raw mut (*cl).items, ci, ci2);

            (*ci).used = sx - csx;
            (*ci2).x = ex + 1;
            (*ci2).used = cex - ex;

            // log_debug("%s: %p now %u-%u (%p) and %u-%u (%p)", __func__, ci, (*ci).x, (*ci).x + (*ci).used - 1, ci, (*ci2).x, (*ci2).x + (*ci2).used - 1, ci2);
            before = ci2;
            break;
        }
        before
    }
}

/// Clear collected lines.
/// C `vendor/tmux/screen-write.c:2223`: `static void screen_write_collect_clear(struct screen_write_ctx *ctx, u_int y, u_int n)`
pub unsafe fn screen_write_collect_clear(ctx: *mut screen_write_ctx, y: u32, n: u32) {
    unsafe {
        for i in y..(y + n) {
            let cl = (*(*ctx).s).write_list.add(i as usize);
            tailq_concat(&raw mut SCREEN_WRITE_CITEM_FREELIST, &raw mut (*cl).items);
        }
    }
}

/// Scroll collected lines up.
/// C `vendor/tmux/screen-write.c:2236`: `static void screen_write_collect_scroll(struct screen_write_ctx *ctx, u_int bg)`
pub unsafe fn screen_write_collect_scroll(ctx: *mut screen_write_ctx, bg: u32) {
    unsafe {
        let s = (*ctx).s;
        // log_debug("%s: at %u,%u (region %u-%u)", __func__, (*s).cx, (*s).cy, (*s).rupper, (*s).rlower);

        screen_write_collect_clear(ctx, (*s).rupper, 1);
        let saved = (*(*(*ctx).s).write_list.add((*s).rupper as usize)).data;
        for y in (*s).rupper..(*s).rlower {
            let cl = (*(*ctx).s).write_list.add(y as usize + 1);
            tailq_concat(
                &raw mut (*(*(*ctx).s).write_list.add(y as usize)).items,
                &raw mut (*cl).items,
            );
            (*(*(*ctx).s).write_list.add(y as usize)).data = (*cl).data;
        }
        (*(*(*ctx).s).write_list.add((*s).rlower as usize)).data = saved;

        let ci = screen_write_get_citem().as_ptr();
        (*ci).x = 0;
        (*ci).used = screen_size_x(s);
        (*ci).type_ = screen_write_citem_type::Clear;
        (*ci).bg = bg;
        tailq_insert_tail(
            &raw mut (*(*(*ctx).s).write_list.add((*s).rlower as usize)).items,
            ci,
        );
    }
}

/// Flush collected lines.
/// C `vendor/tmux/screen-write.c:2392`: `static void screen_write_collect_flush(struct screen_write_ctx *ctx, int scroll_only, const char *from)`
pub unsafe fn screen_write_collect_flush(ctx: *mut screen_write_ctx, scroll_only: u32, from: &str) {
    unsafe {
        let s = (*ctx).s;
        let wp = (*ctx).wp;
        let mut items = 0;

        'discard: {
            if !wp.is_null()
                && (*wp)
                    .flags
                    .intersects(window_pane_flags::PANE_REDRAW | window_pane_flags::PANE_DROP)
            {
                break 'discard;
            }
            if (*s).mode.intersects(mode_flag::MODE_SYNC) {
                for y in 0..screen_size_y(s) {
                    let cl = (*s).write_list.add(y as usize);
                    if tailq_first(&raw mut (*cl).items).is_null() {
                        continue;
                    }
                    screen_write_should_draw_line(ctx, y);
                }
                break 'discard;
            }

            if (*ctx).scrolled != 0 {
                // A zero return means the scroll was serviced by redrawing the
                // whole pane, so the per-line items are already on screen and
                // flushing them would paint the same frame twice.
                if screen_write_collect_flush_scrolled(ctx) == 0 {
                    break 'discard;
                }
                (*ctx).scrolled = 0;
            }
            (*ctx).bg = 8;

            if scroll_only != 0 {
                return;
            }

            let cx = (*s).cx;
            let cy = (*s).cy;
            for y in 0..screen_size_y(s) {
                items += screen_write_collect_flush_line(ctx, y);
            }
            (*s).cx = cx;
            (*s).cy = cy;

            log_debug!("screen_write_collect_flush: flushed {items} items ({from})",);
            return;
        }

        for y in 0..screen_size_y(s) {
            let cl = (*s).write_list.add(y as usize);
            for ci in tailq_foreach(&raw mut (*cl).items).map(NonNull::as_ptr) {
                tailq_remove(&raw mut (*cl).items, ci);
                screen_write_free_citem(ci);
            }
        }
        (*ctx).scrolled = 0;
        (*ctx).bg = 8;
    }
}

/// Flush collected scrolling.
/// C `vendor/tmux/screen-write.c:2265`: `static int screen_write_collect_flush_scrolled(struct screen_write_ctx *ctx)`
///
/// A scroll escape moves whole terminal rows, so it cannot be clipped per cell:
/// it would drag a floating pane's rows along with the pane underneath. When a
/// float overlaps, redraw the pane instead of scrolling.
unsafe fn screen_write_collect_flush_scrolled(ctx: *mut screen_write_ctx) -> c_int {
    unsafe {
        let wp = (*ctx).wp;
        let s = (*ctx).s;
        let mut ttyctx: tty_ctx = zeroed();

        screen_write_initctx(ctx, &raw mut ttyctx, 1);
        if screen_write_pane_is_obscured(ctx) && !wp.is_null() {
            screen_write_redraw_pane(ctx, &raw mut ttyctx);
            return 0;
        }
        // A terminal scroll moves whole rows, which would drag an overlay
        // scrollbar's cells up with the pane's own. Redraw instead.
        if !wp.is_null() && window_pane_scrollbar_overlay_visible(wp) != 0 {
            (*wp).flags |= window_pane_flags::PANE_REDRAW;
            return 0;
        }

        log_debug!(
            "screen_write_collect_flush_scrolled: scrolled {} (region {}-{})",
            (*ctx).scrolled,
            (*s).rupper,
            (*s).rlower,
        );
        if (*ctx).scrolled > (*s).rlower - (*s).rupper + 1 {
            (*ctx).scrolled = (*s).rlower - (*s).rupper + 1;
        }

        // A pane hanging off the bottom scrolls a shorter region.
        if !wp.is_null() {
            let past = (*wp).yoff as c_int + (*wp).sy as c_int - (*(*wp).window).sy as c_int;
            if past > 0 {
                ttyctx.orlower -= past as u32;
            }
        }
        ttyctx.num = (*ctx).scrolled;
        ttyctx.bg = (*ctx).bg;
        tty_write(tty_cmd_scrollup, &raw mut ttyctx);

        // The scrollback grew, so the slider is now in the wrong place.
        if !wp.is_null() {
            window_pane_scrollbar_redraw(wp);
        }
        1
    }
}

/// Flush one collected line, writing only the spans of it that are not covered
/// by a floating pane above this one. Returns the number of items written.
/// C `vendor/tmux/screen-write.c:2300`: `static u_int screen_write_collect_flush_line(struct screen_write_ctx *ctx, u_int y)`
unsafe fn screen_write_collect_flush_line(ctx: *mut screen_write_ctx, y: u32) -> u32 {
    unsafe {
        let wp = (*ctx).wp;
        let s = (*ctx).s;
        let cl = (*s).write_list.add(y as usize);
        let mut last = u32::MAX;
        let mut items = 0;
        let mut ttyctx: tty_ctx = zeroed();

        let (wsx, wsy, xoff, yoff) = if wp.is_null() {
            (screen_size_x(s), screen_size_y(s), 0, 0)
        } else {
            (
                (*(*wp).window).sx,
                (*(*wp).window).sy,
                (*wp).xoff as c_int,
                (*wp).yoff as c_int,
            )
        };
        if y as c_int + yoff >= wsy as c_int {
            return 0;
        }

        let r = window_visible_ranges(wp, 0, y as c_int + yoff, wsx, null_mut());
        for ci in tailq_foreach(&raw mut (*cl).items).map(NonNull::as_ptr) {
            if last != u32::MAX && (*ci).x <= last {
                fatalx_!("collect list bad order: {} <= {}", (*ci).x, last);
            }

            let mut written = false;
            for i in 0..(*r).used as usize {
                let ri = *(*r).ranges.add(i);
                if ri.nx == 0 {
                    continue;
                }

                let r_start = ri.px as c_int;
                let r_end = (ri.px + ri.nx) as c_int;
                let c_start = (*ci).x as c_int;
                let c_end = ((*ci).x + (*ci).used) as c_int;

                if c_start + xoff >= r_end || c_end + xoff <= r_start {
                    continue;
                }
                let w_start = if r_start > c_start + xoff {
                    r_start - xoff
                } else {
                    c_start
                };
                let w_end = if c_end + xoff > r_end {
                    r_end - xoff
                } else {
                    c_end
                };
                if w_end <= w_start {
                    continue;
                }
                let w_length = (w_end - w_start) as u32;

                screen_write_set_cursor(ctx, w_start, y as i32);
                if (*ci).type_ == screen_write_citem_type::Clear {
                    screen_write_initctx(ctx, &raw mut ttyctx, 1);
                    ttyctx.bg = (*ci).bg;
                    ttyctx.num = w_length;
                    if screen_write_pane_is_obscured(ctx) && !(*ctx).wp.is_null() {
                        screen_write_redraw_pane(ctx, &raw mut ttyctx);
                    } else {
                        tty_write(tty_cmd_clearcharacter, &raw mut ttyctx);
                    }
                } else {
                    screen_write_initctx(ctx, &raw mut ttyctx, 0);
                    ttyctx.cell = &(*ci).gc;
                    ttyctx.wrapped = (*ci).wrapped;
                    ttyctx.ptr = (*cl).data.add(w_start as usize).cast();
                    ttyctx.num = w_length;
                    tty_write(tty_cmd_cells, &raw mut ttyctx);
                }
                items += 1;
                written = true;
            }
            if written {
                last = (*ci).x;
                tailq_remove(&raw mut (*cl).items, ci);
                screen_write_free_citem(ci);
            }
        }
        items
    }
}

/// Finish and store collected cells.
/// C `vendor/tmux/screen-write.c:2478`: `void screen_write_collect_end(struct screen_write_ctx *ctx)`
pub unsafe fn screen_write_collect_end(ctx: *mut screen_write_ctx) {
    unsafe {
        let s = (*ctx).s;
        let ci = (*ctx).item;
        let cl = (*s).write_list.add((*s).cy as usize);
        let mut gc: grid_cell = zeroed();
        let mut wrapped = (*ci).wrapped;

        if (*ci).used == 0 {
            return;
        }

        let before = screen_write_collect_trim(ctx, (*s).cy, (*s).cx, (*ci).used, &raw mut wrapped);
        (*ci).x = (*s).cx;
        (*ci).wrapped = wrapped;
        if before.is_null() {
            tailq_insert_tail(&raw mut (*cl).items, ci);
        } else {
            tailq_insert_before(before, ci);
        }
        (*ctx).item = screen_write_get_citem().as_ptr();

        // log_debug("%s: %u %.*s (at %u,%u)", __func__, (*ci).used, (int)(*ci).used, (*cl).data + (*ci).x, (*s).cx, (*s).cy);

        if (*s).cx != 0 {
            let mut xx = (*s).cx;
            while xx > 0 {
                grid_view_get_cell((*s).grid, xx, (*s).cy, &raw mut gc);
                if !gc.flags.intersects(grid_flag::PADDING) {
                    break;
                }
                grid_view_set_cell((*s).grid, xx, (*s).cy, &GRID_DEFAULT_CELL);
                xx -= 1;
            }
            if gc.data.width > 1 {
                grid_view_set_cell((*s).grid, xx, (*s).cy, &GRID_DEFAULT_CELL);
            }
        }

        #[cfg(feature = "sixel")]
        {
            if crate::image_::image_check_area(s, (*s).cx, (*s).cy, (*ci).used, 1)
                && !(*ctx).wp.is_null()
            {
                (*(*ctx).wp).flags |= window_pane_flags::PANE_REDRAW;
            }
        }

        grid_view_set_cells(
            (*s).grid,
            (*s).cx,
            (*s).cy,
            &(*ci).gc,
            (*cl).data.add((*ci).x as usize),
            (*ci).used as usize,
        );
        screen_write_set_cursor(ctx, ((*s).cx + (*ci).used) as i32, -1);

        for xx in (*s).cx..screen_size_x(s) {
            grid_view_get_cell((*s).grid, xx, (*s).cy, &raw mut gc);
            if !gc.flags.intersects(grid_flag::PADDING) {
                break;
            }
            grid_view_set_cell((*s).grid, xx, (*s).cy, &GRID_DEFAULT_CELL);
        }
    }
}

/// Write cell data, collecting if necessary.
/// C `vendor/tmux/screen-write.c:2560`: `void screen_write_collect_add(struct screen_write_ctx *ctx, const struct grid_cell *gc)`
pub unsafe fn screen_write_collect_add(ctx: *mut screen_write_ctx, gc: *const grid_cell) {
    unsafe {
        let s = (*ctx).s;
        let sx = screen_size_x(s);

        // Don't need to check that the attributes and whatnot are still the
        // same - input_parse will end the collection when anything that isn't
        // a plain character is encountered.

        if ((*gc).data.width != 1 || (*gc).data.size != 1 || (*gc).data.data[0] >= 0x7f)
            || (*gc).flags.contains(grid_flag::TAB)
            || (*gc).attr.intersects(grid_attr::GRID_ATTR_CHARSET)
            || !(*s).mode.intersects(mode_flag::MODE_WRAP)
            || (*s).mode.intersects(mode_flag::MODE_INSERT)
            || !(*s).sel.is_null()
        {
            screen_write_collect_end(ctx);
            screen_write_collect_flush(ctx, 0, "screen_write_collect_add");
            screen_write_cell(ctx, gc);
            return;
        }

        if (*s).cx > sx - 1 || (*(*ctx).item).used > sx - 1 - (*s).cx {
            screen_write_collect_end(ctx);
        }
        let ci = (*ctx).item; // may have changed

        if (*s).cx > sx - 1 {
            // log_debug!("%s: wrapped at %u,%u", __func__, (*s).cx, (*s).cy);
            (*ci).wrapped = true;
            screen_write_linefeed(ctx, true, 8);
            screen_write_set_cursor(ctx, 0, -1);
        }

        if (*ci).used == 0 {
            memcpy__(&raw mut (*ci).gc, gc);
        }
        if (*(*(*ctx).s).write_list.add((*s).cy as usize))
            .data
            .is_null()
        {
            (*(*(*ctx).s).write_list.add((*s).cy as usize)).data =
                xmalloc(screen_size_x((*ctx).s) as usize).as_ptr().cast();
        }
        *(*(*(*ctx).s).write_list.add((*s).cy as usize))
            .data
            .add(((*s).cx + (*ci).used) as usize) = (*gc).data.data[0];
        (*ci).used += 1;
    }
}

/// Write cell data.
/// C `vendor/tmux/screen-write.c:2614`: `void screen_write_cell(struct screen_write_ctx *ctx, const struct grid_cell *gc)`
pub unsafe fn screen_write_cell(ctx: *mut screen_write_ctx, gc: *const grid_cell) {
    unsafe {
        let s = (*ctx).s;
        let gd = (*s).grid;
        let ud = &raw const (*gc).data;

        let gce: *mut grid_cell_entry;

        let mut tmp_gc: grid_cell = zeroed();
        let mut now_gc: grid_cell = zeroed();
        let mut ttyctx: tty_ctx = zeroed();

        let sx = screen_size_x(s);
        let sy = screen_size_y(s);

        let width = (*ud).width as u32;
        // xx, not_wrap;
        let mut skip = true;

        // Ignore padding cells.
        if (*gc).flags.intersects(grid_flag::PADDING) {
            return;
        }

        // Get the previous cell to check for combining.
        if screen_write_combine(ctx, gc) != 0 {
            return;
        }

        // Flush any existing scrolling.
        screen_write_collect_flush(ctx, 1, "screen_write_cell");

        // If this character doesn't fit, ignore it.
        if !(*s).mode.intersects(mode_flag::MODE_WRAP)
            && width > 1
            && (width > sx || ((*s).cx != sx && (*s).cx > sx - width))
        {
            return;
        }

        // If in insert mode, make space for the cells.
        if (*s).mode.intersects(mode_flag::MODE_INSERT) {
            grid_view_insert_cells((*s).grid, (*s).cx, (*s).cy, width, 8);
            skip = false;
        }

        // Check this will fit on the current line and wrap if not.
        if (*s).mode.intersects(mode_flag::MODE_WRAP) && (*s).cx > sx - width {
            // log_debug("%s: wrapped at %u,%u", __func__, (*s).cx, (*s).cy);
            screen_write_linefeed(ctx, true, 8);
            screen_write_set_cursor(ctx, 0, -1);
            screen_write_collect_flush(ctx, 1, "screen_write_cell");
        }

        // Sanity check cursor position.
        if (*s).cx > sx - width || (*s).cy > sy - 1 {
            return;
        }
        screen_write_initctx(ctx, &raw mut ttyctx, 0);

        // Handle overwriting of UTF-8 characters.
        let gl: *mut grid_line = grid_get_line((*s).grid, (*(*s).grid).hsize + (*s).cy);
        if (*gl).flags.intersects(grid_line_flag::EXTENDED) {
            grid_view_get_cell(gd, (*s).cx, (*s).cy, &raw mut now_gc);
            if screen_write_overwrite(ctx, &raw mut now_gc, width) != 0 {
                skip = false;
            }
        }

        // If the new character is UTF-8 wide, fill in padding cells. Have
        // already ensured there is enough room.
        for xx in ((*s).cx + 1)..((*s).cx + width) {
            // log_debug("%s: new padding at %u,%u", __func__, xx, (*s).cy);
            grid_view_set_padding(gd, xx, (*s).cy);
            skip = false;
        }

        // If no change, do not draw.
        if skip {
            if (*s).cx >= (*gl).cellsize {
                skip = grid_cells_equal(gc, &GRID_DEFAULT_CELL);
            } else {
                gce = (*gl).celldata.add((*s).cx as usize);
                if (*gce).flags.intersects(grid_flag::EXTENDED)
                    || (*gc).flags != (*gce).flags
                    || (*gc).attr.bits() != (*gce).union_.data.attr as u16
                    || (*gc).fg != (*gce).union_.data.fg as i32
                    || (*gc).bg != (*gce).union_.data.bg as i32
                    || (*gc).data.width != 1
                    || (*gc).data.size != 1
                    || (*gce).union_.data.data != (*gc).data.data[0]
                {
                    skip = false;
                }
            }
        }

        // Update the selected flag and set the cell.
        let selected = screen_check_selection(s, (*s).cx, (*s).cy) != 0;
        if selected && !(*gc).flags.intersects(grid_flag::SELECTED) {
            memcpy__(&raw mut tmp_gc, gc);
            tmp_gc.flags |= grid_flag::SELECTED;
            grid_view_set_cell(gd, (*s).cx, (*s).cy, &raw const tmp_gc);
        } else if !selected && ((*gc).flags.intersects(grid_flag::SELECTED)) {
            memcpy__(&raw mut tmp_gc, gc);
            tmp_gc.flags &= !grid_flag::SELECTED;
            grid_view_set_cell(gd, (*s).cx, (*s).cy, &tmp_gc);
        } else if !skip {
            grid_view_set_cell(gd, (*s).cx, (*s).cy, gc);
        }
        if selected {
            skip = false;
        }

        // Get visible ranges for the character before moving the cursor, so a
        // floating pane above this one is not drawn over.
        let wp = (*ctx).wp;
        let (xoff, yoff) = if wp.is_null() {
            (0, 0)
        } else {
            ((*wp).xoff as c_int, (*wp).yoff as c_int)
        };
        let r = window_visible_ranges(
            wp,
            xoff + (*s).cx as c_int,
            (*s).cy as c_int + yoff,
            width,
            null_mut(),
        );

        // Move the cursor. If not wrapping, stick at the last character and
        // replace it.
        let not_wrap = !((*s).mode.intersects(mode_flag::MODE_WRAP)) as i32;
        if (*s).cx <= (sx as i32 - not_wrap - width as i32) as u32 {
            screen_write_set_cursor(ctx, ((*s).cx + width) as i32, -1);
        } else {
            screen_write_set_cursor(ctx, sx as i32 - not_wrap, -1);
        }

        // Create space for character in insert mode.
        if (*s).mode.intersects(mode_flag::MODE_INSERT) {
            screen_write_collect_flush(ctx, 0, "screen_write_cell");
            ttyctx.num = width;
            if !screen_write_should_draw_line(ctx, (*s).cy) {
            } else if screen_write_pane_is_obscured(ctx) && !(*ctx).wp.is_null() {
                screen_write_redraw_pane(ctx, &raw mut ttyctx);
            } else {
                tty_write(tty_cmd_insertcharacter, &raw mut ttyctx);
            }
        }

        // Write to the screen.
        if skip {
            return;
        }

        // Work out the cell attributes.
        if selected {
            screen_select_cell(s, &raw mut tmp_gc, gc);
        } else {
            memcpy__(&raw mut tmp_gc, gc);
        }
        ttyctx.cell = &raw const tmp_gc;

        if !screen_write_should_draw_line(ctx, (*s).cy) {
            return;
        }

        // If the cell is fully visible, it can be written entirely.
        let mut vis = 0;
        for i in 0..(*r).used as usize {
            vis += (*(*r).ranges.add(i)).nx;
        }
        if vis >= width {
            tty_write(tty_cmd_cell, &raw mut ttyctx);
            return;
        }

        // Otherwise this is a wide character or tab partly obscured by a
        // floating pane. Write spaces in the visible regions only.
        utf8_set(&raw mut tmp_gc.data, b' ');
        for i in 0..(*r).used as usize {
            let ri = *(*r).ranges.add(i);
            if ri.nx == 0 {
                continue;
            }
            for n in 0..ri.nx {
                ttyctx.ocx = (ri.px as c_int - xoff + n as c_int) as u32;
                tty_write(tty_cmd_cell, &raw mut ttyctx);
            }
        }
    }
}

/// Combine a UTF-8 zero-width character onto the previous if necessary.
/// C `vendor/tmux/screen-write.c:2798`: `static int screen_write_combine(struct screen_write_ctx *ctx, const struct grid_cell *gc)`
pub unsafe fn screen_write_combine(ctx: *mut screen_write_ctx, gc: *const grid_cell) -> i32 {
    unsafe {
        let s = (*ctx).s;
        let gd = (*s).grid;
        let ud: *const utf8_data = &raw const (*gc).data;
        let mut cx = (*s).cx;
        let cy = (*s).cy;

        let mut last: grid_cell = zeroed();
        let mut ttyctx: tty_ctx = zeroed();

        let mut force_wide = 0;
        let mut zero_width = 0;

        // Is this character which makes no sense without being combined? If
        // this is true then flag it here and discard the character (return 1)
        // if we cannot combine it.
        if utf8_is_zwj(ud) {
            zero_width = 1;
        } else if utf8_is_vs(ud) {
            zero_width = 1;
            // C screen-write.c:2824: only forced wide when the option says so.
            // This used to force it unconditionally, which was masked by the
            // constant above never matching.
            if options_get_number_(GLOBAL_OPTIONS, "variation-selector-always-wide") != 0 {
                force_wide = 1;
            }
        } else if (*ud).width == 0 {
            zero_width = 1;
        }

        // Cannot combine empty character or at left.
        if (*ud).size < 2 || cx == 0 {
            return zero_width;
        }
        // log_debug("%s: character %.*s at %u,%u (width %u)", __func__, (int)(*ud).size, (*ud).data, cx, cy, (*ud).width);

        // Find the cell to combine with.
        let mut n = 1;
        grid_view_get_cell(gd, cx - n, cy, &raw mut last);
        if cx != 1 && last.flags.intersects(grid_flag::PADDING) {
            n = 2;
            grid_view_get_cell(gd, cx - n, cy, &raw mut last);
        }
        if n != last.data.width as u32 || last.flags.intersects(grid_flag::PADDING) {
            return zero_width;
        }

        // Check if we need to combine characters. This could be zero width
        // (set above), a modifier character (with an existing Unicode
        // character) or a previous ZWJ.
        if zero_width == 0 {
            if utf8_is_modifier(ud) {
                if last.data.size < 2 {
                    return 0;
                }
                force_wide = 1;
            } else if !utf8_has_zwj(&raw mut last.data) {
                return 0;
            }
        }

        // Check if this combined character would be too long.
        if last.data.size + (*ud).size > UTF8_SIZE as u8 {
            return 0;
        }

        // Combining; flush any pending output.
        screen_write_collect_flush(ctx, 0, "screen_write_combine");

        // log_debug("%s: %.*s -> %.*s at %u,%u (offset %u, width %u)", __func__, (int)(*ud).size, (*ud).data, (int)last.data.size, last.data.data, cx - n, cy, n, last.data.width);

        // Append the data.
        libc::memcpy(
            (&raw mut last.data.data[last.data.size as usize]).cast(),
            (&raw const (*ud).data).cast(),
            (*ud).size as usize,
        );
        last.data.size += (*ud).size;

        // Force the width to 2 for modifiers and variation selector.
        if last.data.width == 1 && force_wide != 0 {
            last.data.width = 2;
            n = 2;
            cx += 1;
        } else {
            force_wide = 0;
        }

        // Set the new cell.
        grid_view_set_cell(gd, cx - n, cy, &last);
        if force_wide != 0 {
            grid_view_set_padding(gd, cx - 1, cy);
        }

        // Check if all of this character is visible. No character is obscured
        // in the middle, only on the left or right, but there can be an empty
        // range in between so add them all up.
        let wp = (*ctx).wp;
        let yoff = if wp.is_null() {
            0
        } else {
            (*wp).yoff as c_int
        };
        let r = window_visible_ranges(
            wp,
            (cx - n) as c_int,
            cy as c_int + yoff,
            n,
            null_mut(),
        );
        let mut vis = 0;
        for i in 0..(*r).used as usize {
            vis += (*(*r).ranges.add(i)).nx;
        }
        if vis < n {
            // Part of this character is obscured by a floating pane. Return 1
            // and let screen_write_cell write a space.
            return 1;
        }

        // Redraw the combined cell. If forcing the cell to width 2, reset the
        // cached cursor position in the tty, since we don't really know
        // whether the terminal thought the character was width 1 or width 2
        // and what it is going to do now.
        screen_write_set_cursor(ctx, cx as i32 - n as i32, cy as i32);
        screen_write_initctx(ctx, &raw mut ttyctx, 0);
        ttyctx.cell = &raw const last;
        ttyctx.num = force_wide; // reset cached cursor position
        tty_write(tty_cmd_cell, &raw mut ttyctx);
        screen_write_set_cursor(ctx, cx as i32, cy as i32);

        1
    }
}

// UTF-8 wide characters are a bit of an annoyance. They take up more than one
// cell on the screen, so following cells must not be drawn by marking them as
// padding.
//
// So far, so good. The problem is, when overwriting a padding cell, or a UTF-8
// character, it is necessary to also overwrite any other cells which covered
// by the same character.

/// C `vendor/tmux/screen-write.c:2943`: `static int screen_write_overwrite(struct screen_write_ctx *ctx, struct grid_cell *gc, u_int width)`
pub unsafe fn screen_write_overwrite(
    ctx: *mut screen_write_ctx,
    gc: *mut grid_cell,
    width: u32,
) -> i32 {
    unsafe {
        let s = (*ctx).s;
        let gd = (*s).grid;

        let mut tmp_gc: grid_cell = zeroed();
        let mut done = 0;

        if (*gc).flags.intersects(grid_flag::PADDING) {
            // A padding cell, so clear any following and leading padding
            // cells back to the character. Don't overwrite the current
            // cell as that happens later anyway.
            let mut xx = (*s).cx + 1;
            while {
                xx -= 1;
                xx > 0
            } {
                grid_view_get_cell(gd, xx, (*s).cy, &raw mut tmp_gc);
                if !tmp_gc.flags.intersects(grid_flag::PADDING) {
                    break;
                }
                // log_debug("%s: padding at %u,%u", __func__, xx, (*s).cy);
                grid_view_set_cell(gd, xx, (*s).cy, &raw const GRID_DEFAULT_CELL);
            }

            // Overwrite the character at the start of this padding.
            // log_debug("%s: character at %u,%u", __func__, xx, (*s).cy);
            grid_view_set_cell(gd, xx, (*s).cy, &raw const GRID_DEFAULT_CELL);
            done = 1;
        }

        // Overwrite any padding cells that belong to any UTF-8 characters
        // we'll be overwriting with the current character.
        if width != 1 || (*gc).data.width != 1 || (*gc).flags.intersects(grid_flag::PADDING) {
            let mut xx = (*s).cx + width - 1;
            while {
                xx += 1;
                xx < screen_size_x(s)
            } {
                grid_view_get_cell(gd, xx, (*s).cy, &raw mut tmp_gc);
                if !tmp_gc.flags.intersects(grid_flag::PADDING) {
                    break;
                }
                // log_debug("%s: overwrite at %u,%u", __func__, xx, (*s).cy);
                if (*gc).flags.contains(grid_flag::TAB) {
                    memcpy__(&raw mut tmp_gc, gc);
                    tmp_gc.data.data = [0; UTF8_SIZE];
                    tmp_gc.data.data[0] = b' ';
                    tmp_gc.data.width = 1;
                    tmp_gc.data.size = 1;
                    tmp_gc.data.have = 1;
                    grid_view_set_cell(gd, xx, (*s).cy, &raw const tmp_gc);
                } else {
                    grid_view_set_cell(gd, xx, (*s).cy, &raw const GRID_DEFAULT_CELL);
                }
                done = 1;
            }
        }

        done
    }
}

/// Set external clipboard.
/// C `vendor/tmux/screen-write.c:3007`: `void screen_write_setselection(struct screen_write_ctx *ctx, const char *clip, u_char *str, u_int len)`
pub unsafe fn screen_write_setselection(
    ctx: *mut screen_write_ctx,
    flags: *const u8,
    str: *mut u8,
    len: u32,
) {
    unsafe {
        let mut ttyctx: tty_ctx = zeroed();

        screen_write_initctx(ctx, &raw mut ttyctx, 0);
        ttyctx.ptr = str.cast();
        ttyctx.ptr2 = flags as *mut c_void; // TODO casting away const
        ttyctx.num = len;

        tty_write(tty_cmd_setselection, &raw mut ttyctx);
    }
}

/// Write unmodified string.
/// C `vendor/tmux/screen-write.c:3022`: `void screen_write_rawstring(struct screen_write_ctx *ctx, u_char *str, u_int len, int allow_invisible_panes)`
pub unsafe fn screen_write_rawstring(
    ctx: *mut screen_write_ctx,
    str: *mut u8,
    len: u32,
    allow_invisible_panes: i32,
) {
    unsafe {
        let mut ttyctx: tty_ctx = zeroed();

        screen_write_initctx(ctx, &raw mut ttyctx, 0);
        ttyctx.ptr = str.cast();
        ttyctx.num = len;
        ttyctx.allow_invisible_panes = allow_invisible_panes;

        tty_write(tty_cmd_rawstring, &raw mut ttyctx);
    }
}

/// Write a SIXEL image.
#[cfg(feature = "sixel")]
/// C `vendor/tmux/screen-write.c:3039`: `void screen_write_sixelimage(struct screen_write_ctx *ctx, struct sixel_image *si, u_int bg)`
pub(crate) unsafe fn screen_write_sixelimage(
    ctx: *mut screen_write_ctx,
    mut si: *mut sixel_image,
    bg: u32,
) {
    use crate::image_::{image_scroll_up, image_store};
    use crate::image_sixel::{sixel_free, sixel_scale, sixel_size_in_cells};

    unsafe {
        let s = (*ctx).s;
        let gd = (*s).grid;
        let mut ttyctx: tty_ctx = zeroed();

        let sx: u32;
        let mut sy: u32;
        let cx: u32 = (*s).cx;
        let cy: u32 = (*s).cy;
        let new: *mut sixel_image;

        let (mut x, mut y) = sixel_size_in_cells(&*si);
        if x > screen_size_x(s) || y > screen_size_y(s) {
            if x > screen_size_x(s) - cx {
                sx = screen_size_x(s) - cx;
            } else {
                sx = x;
            }
            if y > screen_size_y(s) - 1 {
                sy = screen_size_y(s) - 1;
            } else {
                sy = y;
            }
            new = sixel_scale(si, 0, 0, 0, y - sy, sx, sy, 1);
            sixel_free(si);
            si = new;

            // Bail out if the image cannot be scaled.
            if si.is_null() {
                return;
            }
            #[expect(unused_assignments)]
            {
                (x, y) = sixel_size_in_cells(&*si);
            }
        }

        sy = screen_size_y(s) - cy;
        if sy < y {
            let lines = y - sy + 1;
            if image_scroll_up(s, lines) && !(*ctx).wp.is_null() {
                (*(*ctx).wp).flags |= window_pane_flags::PANE_REDRAW;
            }
            for _ in 0..lines {
                grid_view_scroll_region_up(gd, 0, screen_size_y(s) - 1, bg);
                screen_write_collect_scroll(ctx, bg);
            }
            (*ctx).scrolled += lines;
            if lines > cy {
                screen_write_cursormove(ctx, -1, 0, 0);
            } else {
                screen_write_cursormove(ctx, -1, cy as i32 - lines as i32, 0);
            }
        }
        screen_write_collect_flush(ctx, 0, "screen_write_sixelimage");

        log_debug!("before screen_write_initctx");
        screen_write_initctx(ctx, &raw mut ttyctx, 0);
        log_debug!("before image_store");
        ttyctx.ptr = image_store(s, si).cast();

        log_debug!("before tty_write");
        tty_write(crate::tty_::tty_cmd_sixelimage, &raw mut ttyctx);

        log_debug!("before screen_write_cursormove");
        screen_write_cursormove(ctx, 0, (cy + y) as i32, 0);
    }
}

/// Turn alternate screen on.
/// C `vendor/tmux/screen-write.c:3098`: `void screen_write_alternateon(struct screen_write_ctx *ctx, struct grid_cell *gc, int cursor)`
pub unsafe fn screen_write_alternateon(
    ctx: *mut screen_write_ctx,
    gc: *mut grid_cell,
    cursor: i32,
) {
    unsafe {
        let mut ttyctx: tty_ctx = zeroed();
        let wp = (*ctx).wp;

        if !wp.is_null() && options_get_number_((*wp).options, "alternate-screen") == 0 {
            return;
        }

        screen_write_collect_flush(ctx, 0, "screen_write_alternateon");
        screen_alternate_on((*ctx).s, gc, cursor);

        screen_write_initctx(ctx, &raw mut ttyctx, 1);
        if let Some(redraw_cb) = ttyctx.redraw_cb {
            redraw_cb(&raw const ttyctx);
        }
    }
}

/// Turn alternate screen off.
/// C `vendor/tmux/screen-write.c:3124`: `void screen_write_alternateoff(struct screen_write_ctx *ctx, struct grid_cell *gc, int cursor)`
pub unsafe fn screen_write_alternateoff(
    ctx: *mut screen_write_ctx,
    gc: *mut grid_cell,
    cursor: i32,
) {
    unsafe {
        let mut ttyctx: tty_ctx = zeroed();
        let wp = (*ctx).wp;
        if !wp.is_null() && options_get_number_((*wp).options, "alternate-screen") == 0 {
            return;
        }

        screen_write_collect_flush(ctx, 0, "screen_write_alternateoff");
        screen_alternate_off((*ctx).s, gc, cursor);

        screen_write_initctx(ctx, &raw mut ttyctx, 1);
        if let Some(redraw_cb) = ttyctx.redraw_cb {
            redraw_cb(&raw mut ttyctx);
        }
    }
}
