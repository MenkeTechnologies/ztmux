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

/// Selected area in screen.
#[repr(C)]
pub struct screen_sel {
    pub hidden: i32,
    pub rectangle: i32,
    pub modekeys: modekey,

    pub sx: u32,
    pub sy: u32,

    pub ex: u32,
    pub ey: u32,

    pub cell: grid_cell,
}

impl_tailq_entry!(screen_title_entry, entry, tailq_entry<screen_title_entry>);
/// Entry on title stack.
#[repr(C)]
pub struct screen_title_entry {
    pub text: *mut u8,

    pub entry: tailq_entry<screen_title_entry>,
}
pub type screen_titles = tailq_head<screen_title_entry>;

/// Free titles stack.
/// C `vendor/tmux/screen.c:57`: `static void screen_free_titles(struct screen *s)`
pub unsafe fn screen_free_titles(s: *mut screen) {
    unsafe {
        if (*s).titles.is_null() {
            return;
        }

        while let Some(title_entry) = NonNull::new(tailq_first((*s).titles)) {
            let title_entry = title_entry.as_ptr();
            tailq_remove((*s).titles, title_entry);
            free_((*title_entry).text);
            free_(title_entry);
        }

        free_((*s).titles);
        (*s).titles = null_mut();
    }
}

/// Create a new screen.
/// C `vendor/tmux/screen.c:76`: `void screen_init(struct screen *s, u_int sx, u_int sy, u_int hlimit)`
pub unsafe fn screen_init(s: *mut screen, sx: u32, sy: u32, hlimit: u32) {
    unsafe {
        (*s).grid = grid_create(sx, sy, hlimit);
        (*s).saved_grid = null_mut();

        (*s).title = xstrdup_(c"").as_ptr();
        (*s).titles = null_mut();
        (*s).path = null_mut();

        (*s).cstyle = screen_cursor_style::SCREEN_CURSOR_DEFAULT;
        (*s).default_cstyle = screen_cursor_style::SCREEN_CURSOR_DEFAULT;
        (*s).mode = mode_flag::MODE_CURSOR;
        (*s).default_mode = mode_flag::empty();
        (*s).ccolour = -1;
        (*s).default_ccolour = -1;
        (*s).tabs = None;
        (*s).sel = null_mut();

        #[cfg(feature = "sixel")]
        tailq_init(&raw mut (*s).images);

        (*s).write_list = null_mut();
        (*s).hyperlinks = null_mut();

        screen_reinit(s);
    }
}

/// Reinitialise screen.
/// C `vendor/tmux/screen.c:107`: `void screen_reinit(struct screen *s)`
pub unsafe fn screen_reinit(s: *mut screen) {
    unsafe {
        (*s).cx = 0;
        (*s).cy = 0;

        (*s).rupper = 0;
        (*s).rlower = screen_size_y(s) - 1;

        (*s).mode =
            mode_flag::MODE_CURSOR | mode_flag::MODE_WRAP | ((*s).mode & mode_flag::MODE_CRLF);

        if options_get_number_(GLOBAL_OPTIONS, "extended-keys") == 2 {
            (*s).mode = ((*s).mode & !EXTENDED_KEY_MODES) | mode_flag::MODE_KEYS_EXTENDED;
        }

        if !(*s).saved_grid.is_null() {
            screen_alternate_off(s, null_mut(), 0);
        }
        (*s).saved_cx = u32::MAX;
        (*s).saved_cy = u32::MAX;

        screen_reset_tabs(s);

        grid_clear_lines((*s).grid, (*(*s).grid).hsize, (*(*s).grid).sy, 8);

        screen_clear_selection(s);
        screen_free_titles(s);

        #[cfg(feature = "sixel")]
        crate::image_::image_free_all(s);

        screen_reset_hyperlinks(s);
    }
}

/// Reset hyperlinks of a screen.
/// C `vendor/tmux/screen.c:142`: `void screen_reset_hyperlinks(struct screen *s)`
pub unsafe fn screen_reset_hyperlinks(s: *mut screen) {
    unsafe {
        if (*s).hyperlinks.is_null() {
            (*s).hyperlinks = hyperlinks_init();
        } else {
            hyperlinks_reset((*s).hyperlinks);
        }
    }
}

/// Destroy a screen.
/// C `vendor/tmux/screen.c:152`: `void screen_free(struct screen *s)`
pub unsafe fn screen_free(s: *mut screen) {
    unsafe {
        free_((*s).sel);
        (*s).tabs = None;
        free_((*s).path);
        free_((*s).title);

        if !(*s).write_list.is_null() {
            screen_write_free_list(s);
        }

        if !(*s).saved_grid.is_null() {
            grid_destroy((*s).saved_grid);
        }
        grid_destroy((*s).grid);

        if !(*s).hyperlinks.is_null() {
            hyperlinks_free((*s).hyperlinks);
        }
        screen_free_titles(s);

        #[cfg(feature = "sixel")]
        crate::image_::image_free_all(s);
    }
}

/// Reset tabs to default, eight spaces apart.
/// C `vendor/tmux/screen.c:177`: `void screen_reset_tabs(struct screen *s)`
pub unsafe fn screen_reset_tabs(s: *mut screen) {
    unsafe {
        (*s).tabs = Some(Rc::new(RefCell::new(BitStr::new(screen_size_x(s)))));

        let mut i = 8;
        while i < screen_size_x(s) {
            (*s).tabs.as_ref().unwrap().borrow_mut().bit_set(i);
            i += 8;
        }
    }
}

/// Set screen cursor style and mode.
/// C `vendor/tmux/screen.c:206`: `void screen_set_cursor_style(u_int style, enum screen_cursor_style *cstyle, int *mode)`
pub unsafe fn screen_set_cursor_style(
    style: u32,
    cstyle: *mut screen_cursor_style,
    mode: *mut mode_flag,
) {
    unsafe {
        match style {
            0 => *cstyle = screen_cursor_style::SCREEN_CURSOR_DEFAULT,
            1 => {
                *cstyle = screen_cursor_style::SCREEN_CURSOR_BLOCK;
                *mode |= mode_flag::MODE_CURSOR_BLINKING;
            }
            2 => {
                *cstyle = screen_cursor_style::SCREEN_CURSOR_BLOCK;
                *mode &= !mode_flag::MODE_CURSOR_BLINKING;
            }
            3 => {
                *cstyle = screen_cursor_style::SCREEN_CURSOR_UNDERLINE;
                *mode |= mode_flag::MODE_CURSOR_BLINKING;
            }
            4 => {
                *cstyle = screen_cursor_style::SCREEN_CURSOR_UNDERLINE;
                *mode &= !mode_flag::MODE_CURSOR_BLINKING;
            }
            5 => {
                *cstyle = screen_cursor_style::SCREEN_CURSOR_BAR;
                *mode |= mode_flag::MODE_CURSOR_BLINKING;
            }
            6 => {
                *cstyle = screen_cursor_style::SCREEN_CURSOR_BAR;
                *mode &= !mode_flag::MODE_CURSOR_BLINKING;
            }
            _ => (),
        }
    }
}

/// Set screen cursor colour.
/// C `vendor/tmux/screen.c:242`: `void screen_set_cursor_colour(struct screen *s, int colour)`
pub unsafe fn screen_set_cursor_colour(s: *mut screen, colour: c_int) {
    unsafe {
        (*s).ccolour = colour;
    }
}

/// Set screen title.
/// C `vendor/tmux/screen.c:249`: `int screen_set_title(struct screen *s, const char *title, int untrusted)`
pub unsafe fn screen_set_title(s: *mut screen, title: *const u8) -> c_int {
    unsafe {
        if !utf8_isvalid(title) {
            return 0;
        }
        free_((*s).title);
        (*s).title = xstrdup(title).as_ptr();
        1
    }
}

/// Set screen path.
/// C `vendor/tmux/screen.c:263`: `int screen_set_path(struct screen *s, const char *path, int untrusted)`
pub unsafe fn screen_set_path(s: *mut screen, path: *const u8) {
    unsafe {
        free_((*s).path);
        utf8_stravis(
            &mut (*s).path,
            path,
            vis_flags::VIS_OCTAL | vis_flags::VIS_CSTYLE | vis_flags::VIS_TAB | vis_flags::VIS_NL,
        );
    }
}

/// Push the current title onto the stack.
/// C `vendor/tmux/screen.c:277`: `void screen_push_title(struct screen *s)`
pub unsafe fn screen_push_title(s: *mut screen) {
    unsafe {
        if (*s).titles.is_null() {
            (*s).titles = Box::leak(Box::new(screen_titles {
                tqh_first: null_mut(),
                tqh_last: null_mut(),
            }));
            tailq_init((*s).titles);
        }

        let title_entry = Box::leak(Box::new(screen_title_entry {
            text: xstrdup((*s).title).as_ptr(),
            entry: tailq_entry::default(),
        }));
        tailq_insert_head((*s).titles, title_entry);
    }
}

/// Pop a title from the stack and set it as the screen title. If the stack is empty, do nothing.
/// C `vendor/tmux/screen.c:306`: `void screen_pop_title(struct screen *s)`
pub unsafe fn screen_pop_title(s: *mut screen) {
    unsafe {
        if (*s).titles.is_null() {
            return;
        }

        if let Some(title_entry) = NonNull::new(tailq_first((*s).titles)) {
            screen_set_title(s, (*title_entry.as_ptr()).text);

            tailq_remove((*s).titles, title_entry.as_ptr());
            free_((*title_entry.as_ptr()).text);
            free_(title_entry.as_ptr());
        }
    }
}

/// Resize screen with options.
/// C `vendor/tmux/screen.c:339`: `void screen_resize_cursor(struct screen *s, u_int sx, u_int sy, int reflow, int eat_empty, int cursor)`
pub unsafe fn screen_resize_cursor(
    s: *mut screen,
    sx: u32,
    sy: u32,
    mut reflow: i32,
    eat_empty: i32,
    cursor: i32,
) {
    let __func__ = "screen_resize_cursor";
    unsafe {
        let mut cx = (*s).cx;
        let mut cy = (*(*s).grid).hsize + (*s).cy;

        if !(*s).write_list.is_null() {
            screen_write_free_list(s);
        }

        log_debug!(
            "{}: new size {}{}, now {}x{} (cursor {},{} = {},{})",
            __func__,
            sx,
            sy,
            screen_size_x(s),
            screen_size_y(s),
            (*s).cx,
            (*s).cy,
            cx,
            cy,
        );

        let sx = if sx < 1 { 1 } else { sx };
        let sy = if sy < 1 { 1 } else { sy };

        if sx != screen_size_x(s) {
            (*(*s).grid).sx = sx;
            screen_reset_tabs(s);
        } else {
            reflow = 0;
        }

        if sy != screen_size_y(s) {
            screen_resize_y(s, sy, eat_empty, &mut cy);
        }

        #[cfg(feature = "sixel")]
        crate::image_::image_free_all(s);

        if reflow != 0 {
            screen_reflow(s, sx, &mut cx, &mut cy, cursor);
        }

        if cy >= (*(*s).grid).hsize {
            (*s).cx = cx;
            (*s).cy = cy - (*(*s).grid).hsize;
        } else {
            (*s).cx = 0;
            (*s).cy = 0;
        }

        log_debug!(
            "{}: cursor finished at {},{} = {},{}",
            __func__,
            (*s).cx,
            (*s).cy,
            cx,
            cy,
        );

        if !(*s).write_list.is_null() {
            screen_write_make_list(s);
        }
    }
}

/// Resize screen.
/// C `vendor/tmux/screen.c:389`: `void screen_resize(struct screen *s, u_int sx, u_int sy, int reflow)`
pub unsafe fn screen_resize(s: *mut screen, sx: u32, sy: u32, reflow: i32) {
    unsafe {
        screen_resize_cursor(s, sx, sy, reflow, 1, 1);
    }
}

/// Resize screen vertically.
/// C `vendor/tmux/screen.c:395`: `static void screen_resize_y(struct screen *s, u_int sy, int eat_empty, u_int *cy)`
unsafe fn screen_resize_y(s: *mut screen, sy: u32, eat_empty: i32, cy: *mut u32) {
    unsafe {
        let gd = (*s).grid;

        if sy == 0 {
            fatalx("zero size");
        }
        let oldy = screen_size_y(s);

        // When resizing:
        //
        // If the height is decreasing, delete lines from the bottom until
        // hitting the cursor, then push lines from the top into the history.
        //
        // When increasing, pull as many lines as possible from scrolled
        // history (not explicitly cleared from view) to the top, then fill the
        // remaining with blanks at the bottom.

        // Size decreasing
        if sy < oldy {
            let mut needed = oldy - sy;

            // Delete as many lines as possible from the bottom
            if eat_empty != 0 {
                let mut available = oldy - 1 - (*s).cy;
                if available > 0 {
                    if available > needed {
                        available = needed;
                    }
                    grid_view_delete_lines(gd, oldy - available, available, 8);
                }
                needed -= available;
            }

            // Now just increase the history size, if possible, to take
            // over the lines which are left. If history is off, delete
            // lines from the top.
            let mut available = (*s).cy;
            if (*gd).flags & GRID_HISTORY != 0 {
                (*gd).hscrolled += needed;
                (*gd).hsize += needed;
            } else if needed > 0 && available > 0 {
                if available > needed {
                    available = needed;
                }
                grid_view_delete_lines(gd, 0, available, 8);
                *cy -= available;
            }
        }

        // Resize line array
        grid_adjust_lines(gd, (*gd).hsize + sy);

        // Size increasing
        if sy > oldy {
            let mut needed = sy - oldy;

            // Try to pull as much as possible out of scrolled history, if
            // it is enabled.
            let mut available = (*gd).hscrolled;
            if (*gd).flags & GRID_HISTORY != 0 && available > 0 {
                if available > needed {
                    available = needed;
                }
                (*gd).hscrolled -= available;
                (*gd).hsize -= available;
            } else {
                available = 0;
            }
            needed -= available;

            // Then fill the rest in with blanks
            for i in ((*gd).hsize + sy - needed)..((*gd).hsize + sy) {
                grid_empty_line(gd, i, 8);
            }
        }

        // Set the new size, and reset the scroll region
        (*gd).sy = sy;
        (*s).rupper = 0;
        (*s).rlower = screen_size_y(s) - 1;
    }
}

/// Set selection.
/// C `vendor/tmux/screen.c:482`: `void screen_set_selection(struct screen *s, u_int sx, u_int sy, u_int ex, u_int ey, u_int rectangle, u_int clipx, int modekeys, struct grid_cell *gc)`
pub unsafe fn screen_set_selection(
    s: *mut screen,
    sx: u32,
    sy: u32,
    ex: u32,
    ey: u32,
    rectangle: u32,
    modekeys: modekey,
    gc: *mut grid_cell,
) {
    unsafe {
        if (*s).sel.is_null() {
            (*s).sel = xcalloc1::<screen_sel>() as *mut screen_sel;
        }

        memcpy__(&raw mut (*(*s).sel).cell, gc);
        (*(*s).sel).hidden = 0;
        (*(*s).sel).rectangle = rectangle as i32;
        (*(*s).sel).modekeys = modekeys;

        (*(*s).sel).sx = sx;
        (*(*s).sel).sy = sy;
        (*(*s).sel).ex = ex;
        (*(*s).sel).ey = ey;
    }
}

/// Clear selection.
/// C `vendor/tmux/screen.c:503`: `void screen_clear_selection(struct screen *s)`
pub unsafe fn screen_clear_selection(s: *mut screen) {
    unsafe {
        free_((*s).sel);
        (*s).sel = null_mut();
    }
}

/// Hide selection.
/// C `vendor/tmux/screen.c:511`: `void screen_hide_selection(struct screen *s)`
pub unsafe fn screen_hide_selection(s: *mut screen) {
    unsafe {
        if !(*s).sel.is_null() {
            (*(*s).sel).hidden = 1;
        }
    }
}

/// Check if cell in selection.
/// C `vendor/tmux/screen.c:519`: `int screen_check_selection(struct screen *s, u_int px, u_int py)`
pub unsafe fn screen_check_selection(s: *mut screen, px: u32, py: u32) -> c_int {
    unsafe {
        let sel = (*s).sel;
        let xx: u32;

        if sel.is_null() || (*sel).hidden != 0 {
            return 0;
        }

        if (*sel).rectangle != 0 {
            match (*sel).sy.cmp(&(*sel).ey) {
                cmp::Ordering::Less => {
                    // start line < end line -- downward selection.
                    if py < (*sel).sy || py > (*sel).ey {
                        return 0;
                    }
                }
                cmp::Ordering::Greater => {
                    // start line > end line -- upward selection.
                    if py > (*sel).sy || py < (*sel).ey {
                        return 0;
                    }
                }
                cmp::Ordering::Equal => {
                    // starting line == ending line.
                    if py != (*sel).sy {
                        return 0;
                    }
                }
            }

            // Need to include the selection start row, but not the cursor
            // row, which means the selection changes depending on which
            // one is on the left.
            if (*sel).ex < (*sel).sx {
                // Cursor (ex) is on the left.
                if px < (*sel).ex {
                    return 0;
                }

                if px > (*sel).sx {
                    return 0;
                }
            } else {
                // Selection start (sx) is on the left.
                if px < (*sel).sx {
                    return 0;
                }

                if px > (*sel).ex {
                    return 0;
                }
            }
        } else {

            // Like emacs, keep the top-left-most character, and drop the
            // bottom-right-most, regardless of copy direction.
            match (*sel).sy.cmp(&((*sel).ey)) {
                cmp::Ordering::Less => {
                    // starting line < ending line -- downward selection.
                    if py < (*sel).sy || py > (*sel).ey {
                        return 0;
                    }

                    if py == (*sel).sy && px < (*sel).sx {
                        return 0;
                    }

                    if (*sel).modekeys == modekey::MODEKEY_EMACS {
                        xx = if (*sel).ex == 0 { 0 } else { (*sel).ex - 1 };
                    } else {
                        xx = (*sel).ex;
                    }
                    if py == (*sel).ey && px > xx {
                        return 0;
                    }
                }
                cmp::Ordering::Greater => {
                    // starting line > ending line -- upward selection.
                    if py > (*sel).sy || py < (*sel).ey {
                        return 0;
                    }

                    if py == (*sel).ey && px < (*sel).ex {
                        return 0;
                    }

                    if (*sel).modekeys == modekey::MODEKEY_EMACS {
                        xx = (*sel).sx - 1;
                    } else {
                        xx = (*sel).sx;
                    }
                    if py == (*sel).sy && ((*sel).sx == 0 || px > xx) {
                        return 0;
                    }
                }
                cmp::Ordering::Equal => {
                    // starting line == ending line.
                    if py != (*sel).sy {
                        return 0;
                    }

                    if (*sel).ex < (*sel).sx {
                        // cursor (ex) is on the left
                        if (*sel).modekeys == modekey::MODEKEY_EMACS {
                            xx = (*sel).sx - 1;
                        } else {
                            xx = (*sel).sx;
                        }
                        if px > xx || px < (*sel).ex {
                            return 0;
                        }
                    } else {
                        // selection start (sx) is on the left
                        if (*sel).modekeys == modekey::MODEKEY_EMACS {
                            xx = if (*sel).ex == 0 { 0 } else { (*sel).ex - 1 };
                        } else {
                            xx = (*sel).ex;
                        }
                        if px < (*sel).sx || px > xx {
                            return 0;
                        }
                    }
                }
            }
        }

        1
    }
}

/// Get selected grid cell.
/// C `vendor/tmux/screen.c:627`: `int screen_select_cell(struct screen *s, struct grid_cell *dst, const struct grid_cell *src)`
pub unsafe fn screen_select_cell(s: *mut screen, dst: *mut grid_cell, src: *const grid_cell) {
    unsafe {
        if (*s).sel.is_null() || (*(*s).sel).hidden != 0 {
            return;
        }

        memcpy__(dst, &raw const (*(*s).sel).cell);

        utf8_copy(&mut (*dst).data, &(*src).data);
        (*dst).attr &= !grid_attr::GRID_ATTR_CHARSET;
        (*dst).attr |= (*src).attr & grid_attr::GRID_ATTR_CHARSET;
        (*dst).flags = (*src).flags;
    }
}

/// Reflow wrapped lines.
/// C `vendor/tmux/screen.c:650`: `static void screen_reflow(struct screen *s, u_int new_x, u_int *cx, u_int *cy, int cursor)`
unsafe fn screen_reflow(s: *mut screen, new_x: u32, cx: *mut u32, cy: *mut u32, cursor: i32) {
    unsafe {
        let mut wx: u32 = 0;
        let mut wy: u32 = 0;

        if cursor != 0 {
            grid_wrap_position((*s).grid, *cx, *cy, &mut wx, &mut wy);
            log_debug!(
                "{}: cursor {},{} is {},{}",
                "screen_reflow",
                *cx,
                *cy,
                wx,
                wy,
            );
        }

        grid_reflow((*s).grid, new_x);

        if cursor != 0 {
            grid_unwrap_position((*s).grid, cx, cy, wx, wy);
            log_debug!("{}: new cursor is {},{}", "screen_reflow", *cx, *cy);
        } else {
            *cx = 0;
            *cy = (*(*s).grid).hsize;
        }
    }
}

/// Enter alternative screen mode. A copy of the visible screen is saved and the
/// history is not updated.
/// C `vendor/tmux/screen.c:677`: `int screen_alternate_on(struct screen *s, struct grid_cell *gc, int cursor)`
pub unsafe fn screen_alternate_on(s: *mut screen, gc: *mut grid_cell, cursor: i32) {
    unsafe {
        if !(*s).saved_grid.is_null() {
            return;
        }
        let sx = screen_size_x(s);
        let sy = screen_size_y(s);

        (*s).saved_grid = grid_create(sx, sy, 0);
        grid_duplicate_lines((*s).saved_grid, 0, (*s).grid, screen_hsize(s), sy);
        if cursor != 0 {
            (*s).saved_cx = (*s).cx;
            (*s).saved_cy = (*s).cy;
        }
        memcpy__(&raw mut (*s).saved_cell, gc);

        grid_view_clear((*s).grid, 0, 0, sx, sy, 8);

        (*s).saved_flags = (*(*s).grid).flags;
        (*(*s).grid).flags &= !GRID_HISTORY;
    }
}

/// Exit alternate screen mode and restore the copied grid.
/// C `vendor/tmux/screen.c:713`: `int screen_alternate_off(struct screen *s, struct grid_cell *gc, int cursor)`
pub unsafe fn screen_alternate_off(s: *mut screen, gc: *mut grid_cell, cursor: i32) {
    unsafe {
        let sx = screen_size_x(s);
        let sy = screen_size_y(s);

        // If the current size is different, temporarily resize to the old size
        // before copying back.
        if !(*s).saved_grid.is_null() {
            screen_resize(s, (*(*s).saved_grid).sx, (*(*s).saved_grid).sy, 0);
        }

        // Restore the cursor position and cell. This happens even if not
        // currently in the alternate screen.
        if cursor != 0 && (*s).saved_cx != u32::MAX && (*s).saved_cy != u32::MAX {
            (*s).cx = (*s).saved_cx;
            (*s).cy = (*s).saved_cy;
            if !gc.is_null() {
                memcpy__(gc, &raw const (*s).saved_cell);
            }
        }

        // If not in the alternate screen, do nothing more.
        if (*s).saved_grid.is_null() {
            if (*s).cx > screen_size_x(s) - 1 {
                (*s).cx = screen_size_x(s) - 1;
            }
            if (*s).cy > screen_size_y(s) - 1 {
                (*s).cy = screen_size_y(s) - 1;
            }
            return;
        }

        // Restore the saved grid.
        grid_duplicate_lines(
            (*s).grid,
            screen_hsize(s),
            (*s).saved_grid,
            0,
            (*(*s).saved_grid).sy,
        );

        // Turn history back on (so resize can use it) and then resize back to
        // the current size.
        if (*s).saved_flags & GRID_HISTORY != 0 {
            (*(*s).grid).flags |= GRID_HISTORY;
        }
        screen_resize(s, sx, sy, 1);

        grid_destroy((*s).saved_grid);
        (*s).saved_grid = null_mut();

        if (*s).cx > screen_size_x(s) - 1 {
            (*s).cx = screen_size_x(s) - 1;
        }
        if (*s).cy > screen_size_y(s) - 1 {
            (*s).cy = screen_size_y(s) - 1;
        }
    }
}

/// Get mode as a string.
/// C `vendor/tmux/screen.c:779`: `const char *screen_mode_to_string(int mode)`
pub unsafe fn screen_mode_to_string(mode: mode_flag) -> *const u8 {
    const TMP_LEN: usize = 1024;
    static mut TMP: [MaybeUninit<u8>; 1024] = [MaybeUninit::uninit(); 1024];

    unsafe {
        if mode == mode_flag::empty() {
            return c!("NONE");
        }
        if mode.is_all() {
            return c!("ALL");
        }

        *TMP[0].as_mut_ptr().cast() = 0i8;

        if mode.intersects(mode_flag::MODE_CURSOR) {
            strlcat(addr_of_mut!(TMP).cast(), c!("CURSOR,"), TMP_LEN);
        }
        if mode.intersects(mode_flag::MODE_INSERT) {
            strlcat(addr_of_mut!(TMP).cast(), c!("INSERT,"), TMP_LEN);
        }
        if mode.intersects(mode_flag::MODE_KCURSOR) {
            strlcat(addr_of_mut!(TMP).cast(), c!("KCURSOR,"), TMP_LEN);
        }
        if mode.intersects(mode_flag::MODE_KKEYPAD) {
            strlcat(addr_of_mut!(TMP).cast(), c!("KKEYPAD,"), TMP_LEN);
        }
        if mode.intersects(mode_flag::MODE_WRAP) {
            strlcat(addr_of_mut!(TMP).cast(), c!("WRAP,"), TMP_LEN);
        }
        if mode.intersects(mode_flag::MODE_MOUSE_STANDARD) {
            strlcat(addr_of_mut!(TMP).cast(), c!("MOUSE_STANDARD,"), TMP_LEN);
        }
        if mode.intersects(mode_flag::MODE_MOUSE_BUTTON) {
            strlcat(addr_of_mut!(TMP).cast(), c!("MOUSE_BUTTON,"), TMP_LEN);
        }
        if mode.intersects(mode_flag::MODE_CURSOR_BLINKING) {
            strlcat(addr_of_mut!(TMP).cast(), c!("CURSOR_BLINKING,"), TMP_LEN);
        }
        if mode.intersects(mode_flag::MODE_CURSOR_VERY_VISIBLE) {
            strlcat(
                addr_of_mut!(TMP).cast(),
                c!("CURSOR_VERY_VISIBLE,"),
                TMP_LEN,
            );
        }
        if mode.intersects(mode_flag::MODE_MOUSE_UTF8) {
            strlcat(addr_of_mut!(TMP).cast(), c!("MOUSE_UTF8,"), TMP_LEN);
        }
        if mode.intersects(mode_flag::MODE_MOUSE_SGR) {
            strlcat(addr_of_mut!(TMP).cast(), c!("MOUSE_SGR,"), TMP_LEN);
        }
        if mode.intersects(mode_flag::MODE_BRACKETPASTE) {
            strlcat(addr_of_mut!(TMP).cast(), c!("BRACKETPASTE,"), TMP_LEN);
        }
        if mode.intersects(mode_flag::MODE_FOCUSON) {
            strlcat(addr_of_mut!(TMP).cast(), c!("FOCUSON,"), TMP_LEN);
        }
        if mode.intersects(mode_flag::MODE_MOUSE_ALL) {
            strlcat(addr_of_mut!(TMP).cast(), c!("MOUSE_ALL,"), TMP_LEN);
        }
        if mode.intersects(mode_flag::MODE_ORIGIN) {
            strlcat(addr_of_mut!(TMP).cast(), c!("ORIGIN,"), TMP_LEN);
        }
        if mode.intersects(mode_flag::MODE_CRLF) {
            strlcat(addr_of_mut!(TMP).cast(), c!("CRLF,"), TMP_LEN);
        }
        if mode.intersects(mode_flag::MODE_KEYS_EXTENDED) {
            strlcat(addr_of_mut!(TMP).cast(), c!("KEYS_EXTENDED,"), TMP_LEN);
        }
        if mode.intersects(mode_flag::MODE_KEYS_EXTENDED_2) {
            strlcat(addr_of_mut!(TMP).cast(), c!("KEYS_EXTENDED_2,"), TMP_LEN);
        }

        let len = strlen(addr_of!(TMP).cast());
        if len > 0 {
            *TMP[len - 1].as_mut_ptr().cast() = 0i8;
        }
        &raw mut TMP as *mut u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // screen_init -> screen_reinit reads options_get_number(global_options,
    // "extended-keys") (vendor/tmux/screen.c:117). In a unit-test process
    // tmux's main() never ran, so GLOBAL_OPTIONS is NULL. screen_free /
    // screen_set_selection also touch no other process-globals, but several
    // tests here mutate GLOBAL_OPTIONS during first-time setup, so serialize
    // every test in this module behind one lock.
    static SCREEN_LOCK: Mutex<()> = Mutex::new(());

    // Populate GLOBAL_OPTIONS with the server-scope defaults exactly the way
    // tmux.rs does at startup (tmux.rs:515) so the "extended-keys" lookup in
    // screen_reinit succeeds. Idempotent: only creates the tree once.
    unsafe fn ensure_global_options() {
        unsafe {
            if GLOBAL_OPTIONS.is_null() {
                GLOBAL_OPTIONS = options_create(null_mut());
                for oe in &OPTIONS_TABLE {
                    if oe.scope & OPTIONS_TABLE_SERVER != 0 {
                        options_default(GLOBAL_OPTIONS, oe);
                    }
                }
            }
        }
    }

    // utf8_isvalid consults the C locale for multibyte sequences; match the
    // idiom in src/utf8.rs tests and pin a UTF-8 locale for the process.
    fn ensure_utf8_locale() {
        use std::sync::Once;
        static ONCE: Once = Once::new();
        ONCE.call_once(|| unsafe {
            if crate::libc::setlocale(::libc::LC_CTYPE, crate::c!("en_US.UTF-8")).is_null()
                && crate::libc::setlocale(::libc::LC_CTYPE, crate::c!("C.UTF-8")).is_null()
            {
                crate::libc::setlocale(::libc::LC_CTYPE, crate::c!(""));
            }
        });
    }

    // Allocate a zeroed `screen` on the heap and run the real screen_init.
    // (Option<Rc>/raw-pointer/bitflags fields all have a valid all-zero value,
    // and screen_init overwrites everything it uses.)
    unsafe fn new_screen(sx: u32, sy: u32, hlimit: u32) -> *mut screen {
        unsafe {
            let s = Box::into_raw(Box::new(zeroed::<screen>()));
            screen_init(s, sx, sy, hlimit);
            s
        }
    }

    unsafe fn free_screen(s: *mut screen) {
        unsafe {
            screen_free(s);
            drop(Box::from_raw(s));
        }
    }

    // Copy a NUL-terminated C string's bytes (excluding the terminator).
    unsafe fn cstr_bytes(p: *const u8) -> Vec<u8> {
        unsafe { std::slice::from_raw_parts(p, strlen(p)).to_vec() }
    }

    // screen_init (screen.c:76) then screen_reinit (screen.c:107): fresh screen
    // has empty title, no path/titles/selection, default cursor style/colour,
    // MODE_CURSOR|MODE_WRAP, sentinel saved cursor, and a scroll region that
    // spans the whole grid. screen_free (screen.c:152) tears it all down.
    #[test]
    fn test_screen_init_defaults_and_free() {
        let _g = SCREEN_LOCK.lock().unwrap();
        unsafe {
            ensure_global_options();
            let s = new_screen(80, 24, 100);

            assert_eq!(screen_size_x(s), 80);
            assert_eq!(screen_size_y(s), 24);
            assert_eq!(screen_hsize(s), 0);

            assert!(!(*s).title.is_null());
            assert_eq!(cstr_to_str((*s).title), "");
            assert!((*s).path.is_null());
            assert!((*s).titles.is_null());
            assert!((*s).sel.is_null());
            assert!((*s).saved_grid.is_null());

            assert!((*s).cstyle == screen_cursor_style::SCREEN_CURSOR_DEFAULT);
            assert!((*s).default_cstyle == screen_cursor_style::SCREEN_CURSOR_DEFAULT);
            assert_eq!((*s).ccolour, -1);
            assert_eq!((*s).default_ccolour, -1);

            // screen_reinit: cursor home, scroll region full, mode reset.
            assert_eq!((*s).cx, 0);
            assert_eq!((*s).cy, 0);
            assert_eq!((*s).rupper, 0);
            assert_eq!((*s).rlower, 23);
            assert_eq!((*s).saved_cx, u32::MAX);
            assert_eq!((*s).saved_cy, u32::MAX);
            assert!((*s).mode == mode_flag::MODE_CURSOR | mode_flag::MODE_WRAP);

            free_screen(s);
        }
    }

    // screen_set_title (screen.rs:233): valid UTF-8 is accepted (returns 1 and
    // xstrdup's the bytes verbatim), invalid UTF-8 is rejected (returns 0 and
    // leaves the previous title untouched). NB: the Rust port gates on
    // utf8_isvalid rather than clean_name(title, untrusted) as C does
    // (screen.c:249); we test the port's actual behaviour.
    #[test]
    #[expect(
        clippy::manual_c_str_literals,
        reason = "raw byte pointers, incl. invalid UTF-8, are the point of these FFI tests"
    )]
    fn test_screen_set_title_valid_and_invalid() {
        let _g = SCREEN_LOCK.lock().unwrap();
        ensure_utf8_locale();
        unsafe {
            ensure_global_options();
            let s = new_screen(80, 24, 0);

            // Plain ASCII accepted, stored verbatim.
            assert_eq!(screen_set_title(s, b"hello\0".as_ptr()), 1);
            assert_eq!(cstr_to_str((*s).title), "hello");

            // Valid multibyte UTF-8 ("A" + euro sign) accepted, stored verbatim.
            assert_eq!(screen_set_title(s, b"A\xe2\x82\xac\0".as_ptr()), 1);
            assert_eq!(cstr_bytes((*s).title), b"A\xe2\x82\xac");

            // Invalid UTF-8 (lone continuation byte) rejected; title unchanged.
            assert_eq!(screen_set_title(s, b"\x82\0".as_ptr()), 0);
            assert_eq!(cstr_bytes((*s).title), b"A\xe2\x82\xac");

            free_screen(s);
        }
    }

    // screen_set_path (screen.rs:246): frees any old path and stores a
    // strvis-visualised copy. A plain path with no special characters
    // round-trips unchanged; a second call replaces (and frees) the first.
    #[test]
    #[expect(
        clippy::manual_c_str_literals,
        reason = "raw byte pointers, incl. invalid UTF-8, are the point of these FFI tests"
    )]
    fn test_screen_set_path_roundtrip_and_replace() {
        let _g = SCREEN_LOCK.lock().unwrap();
        unsafe {
            ensure_global_options();
            let s = new_screen(80, 24, 0);

            screen_set_path(s, b"/home/user/dir\0".as_ptr());
            assert!(!(*s).path.is_null());
            assert_eq!(cstr_to_str((*s).path), "/home/user/dir");

            // Replacing frees the previous allocation and stores the new value.
            screen_set_path(s, b"/tmp\0".as_ptr());
            assert_eq!(cstr_to_str((*s).path), "/tmp");

            free_screen(s);
        }
    }

    // screen_set_cursor_style (screen.c:206): maps style codes 0..=6 to a
    // cursor shape and toggles MODE_CURSOR_BLINKING; unknown codes are no-ops.
    #[test]
    fn test_screen_set_cursor_style_all_cases() {
        let _g = SCREEN_LOCK.lock().unwrap();
        use screen_cursor_style::*;
        unsafe {
            // Helper: run one style over a fresh (cstyle, mode) pair.
            let run = |style: u32, mode_in: mode_flag| -> (screen_cursor_style, mode_flag) {
                let mut cstyle = SCREEN_CURSOR_BLOCK; // deliberately non-default
                let mut mode = mode_in;
                screen_set_cursor_style(style, &mut cstyle, &mut mode);
                (cstyle, mode)
            };

            // 0 -> default, mode untouched (blinking bit preserved as-is).
            let (c, m) = run(0, mode_flag::MODE_CURSOR_BLINKING);
            assert!(c == SCREEN_CURSOR_DEFAULT);
            assert!(m.intersects(mode_flag::MODE_CURSOR_BLINKING));

            // 1 -> block, blinking set.
            let (c, m) = run(1, mode_flag::empty());
            assert!(c == SCREEN_CURSOR_BLOCK);
            assert!(m.intersects(mode_flag::MODE_CURSOR_BLINKING));
            // 2 -> block, blinking cleared.
            let (c, m) = run(2, mode_flag::MODE_CURSOR_BLINKING);
            assert!(c == SCREEN_CURSOR_BLOCK);
            assert!(!m.intersects(mode_flag::MODE_CURSOR_BLINKING));

            // 3 -> underline, blinking set.
            let (c, m) = run(3, mode_flag::empty());
            assert!(c == SCREEN_CURSOR_UNDERLINE);
            assert!(m.intersects(mode_flag::MODE_CURSOR_BLINKING));
            // 4 -> underline, blinking cleared.
            let (c, m) = run(4, mode_flag::MODE_CURSOR_BLINKING);
            assert!(c == SCREEN_CURSOR_UNDERLINE);
            assert!(!m.intersects(mode_flag::MODE_CURSOR_BLINKING));

            // 5 -> bar, blinking set.
            let (c, m) = run(5, mode_flag::empty());
            assert!(c == SCREEN_CURSOR_BAR);
            assert!(m.intersects(mode_flag::MODE_CURSOR_BLINKING));
            // 6 -> bar, blinking cleared.
            let (c, m) = run(6, mode_flag::MODE_CURSOR_BLINKING);
            assert!(c == SCREEN_CURSOR_BAR);
            assert!(!m.intersects(mode_flag::MODE_CURSOR_BLINKING));

            // Out-of-range: no case matches, cstyle and mode are left as-is.
            let (c, m) = run(99, mode_flag::MODE_WRAP);
            assert!(c == SCREEN_CURSOR_BLOCK);
            assert!(m == mode_flag::MODE_WRAP);
        }
    }

    // screen_set_cursor_colour (screen.c:242): writes s->ccolour directly.
    #[test]
    fn test_screen_set_cursor_colour() {
        let _g = SCREEN_LOCK.lock().unwrap();
        unsafe {
            ensure_global_options();
            let s = new_screen(80, 24, 0);
            assert_eq!((*s).ccolour, -1);

            screen_set_cursor_colour(s, 4);
            assert_eq!((*s).ccolour, 4);
            screen_set_cursor_colour(s, -1);
            assert_eq!((*s).ccolour, -1);

            free_screen(s);
        }
    }

    // screen_resize (screen.c:389 -> screen_resize_cursor): changes the grid
    // dimensions and resets the scroll region to span the new height. sx/sy
    // below 1 are clamped to 1 (screen.c:351-354).
    #[test]
    fn test_screen_resize_dims_and_clamp() {
        let _g = SCREEN_LOCK.lock().unwrap();
        unsafe {
            ensure_global_options();
            let s = new_screen(80, 24, 100);

            // Grow.
            screen_resize(s, 120, 40, 0);
            assert_eq!(screen_size_x(s), 120);
            assert_eq!(screen_size_y(s), 40);
            assert_eq!((*s).rupper, 0);
            assert_eq!((*s).rlower, 39);

            // Shrink.
            screen_resize(s, 10, 5, 0);
            assert_eq!(screen_size_x(s), 10);
            assert_eq!(screen_size_y(s), 5);
            assert_eq!((*s).rlower, 4);

            // Zero dimensions clamp to 1x1.
            screen_resize(s, 0, 0, 0);
            assert_eq!(screen_size_x(s), 1);
            assert_eq!(screen_size_y(s), 1);
            assert_eq!((*s).rlower, 0);

            free_screen(s);
        }
    }

    // screen_alternate_on/off (screen.c:677/713): entering saves the grid and
    // cursor, disables history, and clears the view; exiting restores them and
    // frees the saved grid. Re-entering while already alternate is a no-op.
    #[test]
    fn test_screen_alternate_on_off_roundtrip() {
        let _g = SCREEN_LOCK.lock().unwrap();
        unsafe {
            ensure_global_options();
            let s = new_screen(80, 24, 100);

            // hlimit>0 means the grid starts with history enabled.
            assert_ne!((*(*s).grid).flags & GRID_HISTORY, 0);

            // Position the cursor so we can verify it is saved and restored.
            (*s).cx = 5;
            (*s).cy = 7;

            let mut gc = GRID_DEFAULT_CELL;
            screen_alternate_on(s, &mut gc, 1);

            // Now in the alternate screen: saved grid present, cursor saved,
            // history turned off on the live grid.
            assert!(!(*s).saved_grid.is_null());
            assert_eq!((*s).saved_cx, 5);
            assert_eq!((*s).saved_cy, 7);
            assert_eq!((*(*s).grid).flags & GRID_HISTORY, 0);
            let saved_ptr = (*s).saved_grid;

            // Re-entering is a no-op: the saved grid pointer does not change.
            screen_alternate_on(s, &mut gc, 1);
            assert_eq!((*s).saved_grid, saved_ptr);

            // Move the cursor while in the alternate screen.
            (*s).cx = 1;
            (*s).cy = 1;

            // Exit: cursor restored, saved grid released, history back on.
            let mut out_gc = GRID_DEFAULT_CELL;
            screen_alternate_off(s, &mut out_gc, 1);
            assert!((*s).saved_grid.is_null());
            assert_eq!((*s).cx, 5);
            assert_eq!((*s).cy, 7);
            assert_ne!((*(*s).grid).flags & GRID_HISTORY, 0);

            free_screen(s);
        }
    }

    // screen_alternate_off (screen.c:738-745): when NOT in the alternate screen
    // (no saved grid) it still clamps an out-of-range cursor to the last
    // row/column and does nothing else.
    #[test]
    fn test_screen_alternate_off_clamps_cursor_when_not_alternate() {
        let _g = SCREEN_LOCK.lock().unwrap();
        unsafe {
            ensure_global_options();
            let s = new_screen(80, 24, 0);
            assert!((*s).saved_grid.is_null());

            // Force the cursor out of bounds.
            (*s).cx = 999;
            (*s).cy = 999;

            screen_alternate_off(s, null_mut(), 0);
            assert_eq!((*s).cx, 79); // screen_size_x - 1
            assert_eq!((*s).cy, 23); // screen_size_y - 1

            free_screen(s);
        }
    }

    // screen_set_selection / screen_check_selection / screen_hide_selection /
    // screen_clear_selection (screen.c:482/519/511/503): a rectangle selection
    // includes points within [sx,ex] x [sy,ey]; hiding or clearing makes every
    // point test negative.
    #[test]
    fn test_screen_selection_set_check_hide_clear() {
        let _g = SCREEN_LOCK.lock().unwrap();
        unsafe {
            ensure_global_options();
            let s = new_screen(80, 24, 0);

            let mut gc = GRID_DEFAULT_CELL;
            // Rectangle from (2,1) to (5,3).
            screen_set_selection(s, 2, 1, 5, 3, 1, modekey::MODEKEY_EMACS, &mut gc);
            assert!(!(*s).sel.is_null());

            // Inside the rectangle.
            assert_eq!(screen_check_selection(s, 3, 2), 1);
            // Left of sx, and above sy: outside.
            assert_eq!(screen_check_selection(s, 1, 2), 0);
            assert_eq!(screen_check_selection(s, 3, 0), 0);
            // Right of ex, and below ey: outside.
            assert_eq!(screen_check_selection(s, 6, 2), 0);
            assert_eq!(screen_check_selection(s, 3, 4), 0);

            // Hidden selection: every point reads as unselected.
            screen_hide_selection(s);
            assert_eq!((*(*s).sel).hidden, 1);
            assert_eq!(screen_check_selection(s, 3, 2), 0);

            // Cleared selection: pointer gone, checks return 0.
            screen_clear_selection(s);
            assert!((*s).sel.is_null());
            assert_eq!(screen_check_selection(s, 3, 2), 0);

            free_screen(s);
        }
    }

    // screen_reset_tabs (screen.c:177): default tab stops are every 8 columns
    // starting at column 8; columns 0..7 and any non-multiple of 8 are clear.
    // A width change re-derives the stops for the new width.
    #[test]
    fn test_screen_reset_tabs_default_stops() {
        let _g = SCREEN_LOCK.lock().unwrap();
        unsafe {
            ensure_global_options();
            let s = new_screen(40, 24, 0);

            let is_tab = |col: u32| -> bool {
                (*s).tabs.as_ref().unwrap().borrow().bit_test(col)
            };
            // Columns 0 and 7 are not tab stops; 8, 16, 24, 32 are.
            assert!(!is_tab(0));
            assert!(!is_tab(7));
            assert!(is_tab(8));
            assert!(is_tab(16));
            assert!(is_tab(24));
            assert!(is_tab(32));
            // Between stops is clear.
            assert!(!is_tab(9));
            assert!(!is_tab(15));

            // Resizing width past 40 re-runs reset_tabs, adding a stop at 40/48.
            screen_resize(s, 60, 24, 0);
            assert!((*s).tabs.as_ref().unwrap().borrow().bit_test(40));
            assert!((*s).tabs.as_ref().unwrap().borrow().bit_test(48));

            free_screen(s);
        }
    }

    // screen_push_title / screen_pop_title (screen.c:277/306): push snapshots the
    // current title onto a LIFO stack; pop restores the most recently pushed one.
    #[test]
    fn test_screen_title_push_pop_stack() {
        let _g = SCREEN_LOCK.lock().unwrap();
        ensure_utf8_locale();
        unsafe {
            ensure_global_options();
            let s = new_screen(80, 24, 0);

            assert_eq!(screen_set_title(s, c!("first")), 1);
            screen_push_title(s); // stack: [first]
            assert!(!(*s).titles.is_null());

            assert_eq!(screen_set_title(s, c!("second")), 1);
            screen_push_title(s); // stack: [second, first]

            // Change the live title, then pop twice to walk back down the stack.
            assert_eq!(screen_set_title(s, c!("third")), 1);
            screen_pop_title(s);
            assert_eq!(cstr_to_str((*s).title), "second");
            screen_pop_title(s);
            assert_eq!(cstr_to_str((*s).title), "first");

            // Extra pop on the now-empty stack is a no-op (title unchanged).
            screen_pop_title(s);
            assert_eq!(cstr_to_str((*s).title), "first");

            free_screen(s);
        }
    }

    // screen_pop_title on a screen that never pushed (titles == NULL) returns
    // immediately without touching the title (screen.c:306-310).
    #[test]
    fn test_screen_pop_title_empty_is_noop() {
        let _g = SCREEN_LOCK.lock().unwrap();
        ensure_utf8_locale();
        unsafe {
            ensure_global_options();
            let s = new_screen(80, 24, 0);
            assert_eq!(screen_set_title(s, c!("keep")), 1);
            assert!((*s).titles.is_null());

            screen_pop_title(s);
            assert_eq!(cstr_to_str((*s).title), "keep");

            free_screen(s);
        }
    }

    // screen_check_selection non-rectangle single line (screen.c:611-639): in
    // EMACS mode the bottom-right cell (ex) is dropped (xx = ex-1), while in VI
    // mode it is kept (xx = ex). Start (sx) is always included.
    #[test]
    fn test_screen_check_selection_emacs_vs_vi_dropped_cell() {
        let _g = SCREEN_LOCK.lock().unwrap();
        unsafe {
            ensure_global_options();
            let mut gc = GRID_DEFAULT_CELL;

            // EMACS: sx=2, ex=5 on one line. Cells 2,3,4 selected; 5 dropped.
            let s = new_screen(80, 24, 0);
            screen_set_selection(s, 2, 1, 5, 1, 0, modekey::MODEKEY_EMACS, &mut gc);
            assert_eq!(screen_check_selection(s, 2, 1), 1);
            assert_eq!(screen_check_selection(s, 4, 1), 1);
            assert_eq!(screen_check_selection(s, 5, 1), 0); // bottom-right dropped
            assert_eq!(screen_check_selection(s, 1, 1), 0); // before start
            screen_clear_selection(s);

            // VI: same coordinates, but the end cell (5) is kept.
            screen_set_selection(s, 2, 1, 5, 1, 0, modekey::MODEKEY_VI, &mut gc);
            assert_eq!(screen_check_selection(s, 5, 1), 1);
            assert_eq!(screen_check_selection(s, 2, 1), 1);

            free_screen(s);
        }
    }

    // screen_select_cell (screen.c:627): copies the stored selection cell into
    // dst, then overlays the source glyph data and charset/flags. When there is
    // no selection (or it is hidden), dst is left untouched.
    #[test]
    fn test_screen_select_cell_overlay_and_guards() {
        let _g = SCREEN_LOCK.lock().unwrap();
        unsafe {
            ensure_global_options();
            let s = new_screen(80, 24, 0);

            let mut selcell = GRID_DEFAULT_CELL;
            selcell.fg = 42; // distinctive selection foreground
            screen_set_selection(s, 0, 0, 9, 0, 1, modekey::MODEKEY_EMACS, &mut selcell);

            let src = GRID_DEFAULT_CELL; // fg = 8
            let mut dst = GRID_DEFAULT_CELL;
            dst.fg = -100; // sentinel we expect to be overwritten
            screen_select_cell(s, &mut dst, &src);
            // dst inherits the selection cell's colour, not the source's.
            assert_eq!(dst.fg, 42);

            // Hidden selection: dst is left exactly as-is.
            screen_hide_selection(s);
            let mut dst2 = GRID_DEFAULT_CELL;
            dst2.fg = -100;
            screen_select_cell(s, &mut dst2, &src);
            assert_eq!(dst2.fg, -100);

            // No selection at all: also a no-op.
            screen_clear_selection(s);
            let mut dst3 = GRID_DEFAULT_CELL;
            dst3.fg = -100;
            screen_select_cell(s, &mut dst3, &src);
            assert_eq!(dst3.fg, -100);

            free_screen(s);
        }
    }

    // screen_mode_to_string (screen.c:779): NONE for empty, and a comma-joined
    // list in a fixed order for a combination. The freshly-initialised screen
    // mode (MODE_CURSOR|MODE_WRAP) renders as "CURSOR,WRAP".
    #[test]
    fn test_screen_mode_to_string() {
        let _g = SCREEN_LOCK.lock().unwrap();
        unsafe {
            assert_eq!(cstr_to_str(screen_mode_to_string(mode_flag::empty())), "NONE");
            assert_eq!(
                cstr_to_str(screen_mode_to_string(
                    mode_flag::MODE_CURSOR | mode_flag::MODE_WRAP
                )),
                "CURSOR,WRAP"
            );
            // Order follows the function's if-ladder, not the argument order.
            assert_eq!(
                cstr_to_str(screen_mode_to_string(
                    mode_flag::MODE_INSERT | mode_flag::MODE_CURSOR
                )),
                "CURSOR,INSERT"
            );
        }
    }

    // screen_resize with reflow=1 (screen.c:389) still yields the requested
    // dimensions and a scroll region spanning the new height.
    #[test]
    fn test_screen_resize_with_reflow() {
        let _g = SCREEN_LOCK.lock().unwrap();
        unsafe {
            ensure_global_options();
            let s = new_screen(80, 24, 100);

            screen_resize(s, 100, 30, 1);
            assert_eq!(screen_size_x(s), 100);
            assert_eq!(screen_size_y(s), 30);
            assert_eq!((*s).rupper, 0);
            assert_eq!((*s).rlower, 29);

            free_screen(s);
        }
    }

    // screen_reinit (screen.c:107) homes the cursor, resets the scroll region to
    // the full height, rebuilds MODE_CURSOR|MODE_WRAP, restores the saved-cursor
    // sentinels and frees the title stack, even after the screen has been driven
    // away from its initial state.
    #[test]
    fn test_screen_reinit_resets_state() {
        let _g = SCREEN_LOCK.lock().unwrap();
        ensure_utf8_locale();
        unsafe {
            ensure_global_options();
            let s = new_screen(80, 24, 0);

            // Drive the screen away from defaults.
            (*s).cx = 10;
            (*s).cy = 5;
            (*s).rupper = 3;
            (*s).rlower = 10;
            (*s).saved_cx = 4;
            (*s).saved_cy = 4;
            (*s).mode = mode_flag::MODE_INSERT | mode_flag::MODE_ORIGIN;
            assert_eq!(screen_set_title(s, c!("t")), 1);
            screen_push_title(s);
            assert!(!(*s).titles.is_null());

            screen_reinit(s);

            assert_eq!((*s).cx, 0);
            assert_eq!((*s).cy, 0);
            assert_eq!((*s).rupper, 0);
            assert_eq!((*s).rlower, 23);
            assert_eq!((*s).saved_cx, u32::MAX);
            assert_eq!((*s).saved_cy, u32::MAX);
            assert!((*s).mode.intersects(mode_flag::MODE_CURSOR));
            assert!((*s).mode.intersects(mode_flag::MODE_WRAP));
            assert!(!(*s).mode.intersects(mode_flag::MODE_INSERT));
            assert!((*s).titles.is_null()); // title stack freed
            free_screen(s);
        }
    }

    // screen_reset_hyperlinks (screen.c:142): the first call on a screen with no
    // hyperlink cache allocates one; a subsequent call resets the existing cache
    // in place. Either way the pointer is left non-null.
    #[test]
    fn test_screen_reset_hyperlinks_inits_and_resets() {
        let _g = SCREEN_LOCK.lock().unwrap();
        unsafe {
            ensure_global_options();
            let s = new_screen(80, 24, 0);

            // Force the "not yet allocated" branch.
            (*s).hyperlinks = null_mut();
            screen_reset_hyperlinks(s);
            assert!(!(*s).hyperlinks.is_null());

            // Second call takes the reset-in-place branch; still non-null.
            let before = (*s).hyperlinks;
            screen_reset_hyperlinks(s);
            assert!(!(*s).hyperlinks.is_null());
            assert_eq!((*s).hyperlinks, before);
            free_screen(s);
        }
    }

    // screen_resize growing with no history keeps the cursor at its original
    // position (screen.c:297 screen_resize_cursor): with hsize == 0 and reflow==0
    // the cursor's grid row equals its screen row, so it maps straight back.
    #[test]
    fn test_screen_resize_grow_preserves_cursor() {
        let _g = SCREEN_LOCK.lock().unwrap();
        unsafe {
            ensure_global_options();
            let s = new_screen(80, 24, 0);
            (*s).cx = 10;
            (*s).cy = 5;

            screen_resize(s, 120, 40, 0);
            assert_eq!((*s).cx, 10);
            assert_eq!((*s).cy, 5);
            free_screen(s);
        }
    }

    // screen_resize shrinking below the cursor with history enabled pushes the
    // extra top lines into the scrollback rather than deleting them, and the
    // cursor's screen row drops by exactly the number of lines absorbed
    // (screen.c:418-421). Here 24->10 with cy=20 absorbs 11 lines, so cy becomes 9.
    #[test]
    fn test_screen_resize_shrink_history_pushes_lines() {
        let _g = SCREEN_LOCK.lock().unwrap();
        unsafe {
            ensure_global_options();
            let s = new_screen(80, 24, 100);
            (*s).cx = 0;
            (*s).cy = 20;

            screen_resize(s, 80, 10, 0);
            assert_eq!(screen_size_y(s), 10);
            assert_eq!((*(*s).grid).hsize, 11); // 14 needed, 3 eaten from bottom
            assert_eq!((*s).cy, 9); // 20 - 11 absorbed
            free_screen(s);
        }
    }

    // screen_check_selection for a downward (sy<ey) non-rectangle selection
    // (screen.c:572): the middle rows are fully selected; the start row drops
    // px<sx; the end row (EMACS) drops the bottom-right cell (xx = ex-1).
    #[test]
    fn test_screen_check_selection_downward_multiline() {
        let _g = SCREEN_LOCK.lock().unwrap();
        unsafe {
            ensure_global_options();
            let s = new_screen(80, 24, 0);
            let mut gc = GRID_DEFAULT_CELL;

            // sx=5,sy=1 -> ex=3,ey=3, EMACS, non-rectangle.
            screen_set_selection(s, 5, 1, 3, 3, 0, modekey::MODEKEY_EMACS, &mut gc);

            // Middle row: every column selected.
            assert_eq!(screen_check_selection(s, 0, 2), 1);
            assert_eq!(screen_check_selection(s, 79, 2), 1);
            // Start row: before sx dropped, at/after sx kept.
            assert_eq!(screen_check_selection(s, 4, 1), 0);
            assert_eq!(screen_check_selection(s, 5, 1), 1);
            assert_eq!(screen_check_selection(s, 10, 1), 1);
            // End row: EMACS drops the ex cell, keeps up to ex-1.
            assert_eq!(screen_check_selection(s, 3, 3), 0);
            assert_eq!(screen_check_selection(s, 2, 3), 1);
            assert_eq!(screen_check_selection(s, 0, 3), 1);
            // Off the selection entirely.
            assert_eq!(screen_check_selection(s, 3, 0), 0);
            assert_eq!(screen_check_selection(s, 3, 4), 0);

            screen_clear_selection(s);
            free_screen(s);
        }
    }

    // screen_check_selection for a rectangle whose cursor (ex) is left of the
    // start (sx) includes the column band [ex, sx] on every row in [sy, ey]
    // (screen.c:549-557).
    #[test]
    fn test_screen_check_selection_rectangle_cursor_on_left() {
        let _g = SCREEN_LOCK.lock().unwrap();
        unsafe {
            ensure_global_options();
            let s = new_screen(80, 24, 0);
            let mut gc = GRID_DEFAULT_CELL;

            // Rectangle sx=5 -> ex=2 (cursor on the left), rows 1..3.
            screen_set_selection(s, 5, 1, 2, 3, 1, modekey::MODEKEY_EMACS, &mut gc);

            assert_eq!(screen_check_selection(s, 3, 2), 1); // inside band
            assert_eq!(screen_check_selection(s, 2, 2), 1); // left edge (ex)
            assert_eq!(screen_check_selection(s, 5, 2), 1); // right edge (sx)
            assert_eq!(screen_check_selection(s, 1, 2), 0); // left of ex
            assert_eq!(screen_check_selection(s, 6, 2), 0); // right of sx
            assert_eq!(screen_check_selection(s, 3, 0), 0); // above sy
            assert_eq!(screen_check_selection(s, 3, 4), 0); // below ey

            screen_clear_selection(s);
            free_screen(s);
        }
    }

    // screen_alternate_on with cursor==0 enters the alternate screen (allocates
    // the saved grid) but does NOT snapshot the cursor, leaving the saved-cursor
    // sentinels untouched (screen.c:707-710).
    #[test]
    fn test_screen_alternate_on_without_cursor() {
        let _g = SCREEN_LOCK.lock().unwrap();
        unsafe {
            ensure_global_options();
            let s = new_screen(80, 24, 100);
            (*s).cx = 5;
            (*s).cy = 7;

            let mut gc = GRID_DEFAULT_CELL;
            screen_alternate_on(s, &mut gc, 0);

            assert!(!(*s).saved_grid.is_null()); // entered alternate
            assert_eq!((*s).saved_cx, u32::MAX); // cursor not saved
            assert_eq!((*s).saved_cy, u32::MAX);
            free_screen(s);
        }
    }

    // screen_mode_to_string emits "ALL" when every mode bit is set (screen.c:786)
    // and otherwise joins the set flags in the function's fixed if-ladder order,
    // independent of argument order (screen.c:790+).
    #[test]
    fn test_screen_mode_to_string_all_and_ordering() {
        let _g = SCREEN_LOCK.lock().unwrap();
        unsafe {
            assert_eq!(cstr_to_str(screen_mode_to_string(mode_flag::all())), "ALL");
            // Ladder order: WRAP before MOUSE_SGR before ORIGIN.
            assert_eq!(
                cstr_to_str(screen_mode_to_string(
                    mode_flag::MODE_ORIGIN | mode_flag::MODE_MOUSE_SGR | mode_flag::MODE_WRAP
                )),
                "WRAP,MOUSE_SGR,ORIGIN"
            );
        }
    }
}
