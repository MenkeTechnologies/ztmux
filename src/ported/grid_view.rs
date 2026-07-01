// Copyright (c) 2008 Nicholas Marriott <nicholas.marriott@gmail.com>
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

fn grid_view_x(_gd: *mut grid, x: u32) -> u32 {
    x
}
unsafe fn grid_view_y(gd: *mut grid, y: u32) -> u32 {
    unsafe { (*gd).hsize + (y) }
}

/// C `vendor/tmux/grid-view.c:35`: `void grid_view_get_cell(struct grid *gd, u_int px, u_int py, struct grid_cell *gc)`
pub unsafe fn grid_view_get_cell(gd: *mut grid, px: u32, py: u32, gc: *mut grid_cell) {
    unsafe {
        grid_get_cell(gd, grid_view_x(gd, px), grid_view_y(gd, py), gc);
    }
}

/// C `vendor/tmux/grid-view.c:42`: `void grid_view_set_cell(struct grid *gd, u_int px, u_int py, const struct grid_cell *gc)`
pub unsafe fn grid_view_set_cell(gd: *mut grid, px: u32, py: u32, gc: *const grid_cell) {
    unsafe {
        grid_set_cell(gd, grid_view_x(gd, px), grid_view_y(gd, py), gc);
    }
}

/// C `vendor/tmux/grid-view.c:50`: `void grid_view_set_padding(struct grid *gd, u_int px, u_int py)`
pub unsafe fn grid_view_set_padding(gd: *mut grid, px: u32, py: u32) {
    unsafe {
        grid_set_padding(gd, grid_view_x(gd, px), grid_view_y(gd, py));
    }
}

/// C `vendor/tmux/grid-view.c:57`: `void grid_view_set_cells(struct grid *gd, u_int px, u_int py, const struct grid_cell *gc, const char *s, size_t slen)`
pub unsafe fn grid_view_set_cells(
    gd: *mut grid,
    px: u32,
    py: u32,
    gc: *const grid_cell,
    s: *const u8,
    slen: usize,
) {
    unsafe {
        grid_set_cells(gd, grid_view_x(gd, px), grid_view_y(gd, py), gc, s, slen);
    }
}

/// C `vendor/tmux/grid-view.c:66`: `void grid_view_clear_history(struct grid *gd, u_int bg)`
pub unsafe fn grid_view_clear_history(gd: *mut grid, bg: u32) {
    unsafe {
        let mut last = 0u32;

        for yy in 0..(*gd).sy {
            let gl = grid_get_line(gd, grid_view_y(gd, yy));
            if (*gl).cellused != 0 {
                last = yy + 1;
            }
        }
        if last == 0 {
            grid_view_clear(gd, 0, 0, (*gd).sx, (*gd).sy, bg);
            return;
        }

        for _ in 0..(*gd).sy {
            grid_collect_history(gd);
            grid_scroll_history(gd, bg);
        }
        if last < (*gd).sy {
            grid_view_clear(gd, 0, 0, (*gd).sx, (*gd).sy - last, bg);
        }
        (*gd).hscrolled = 0;
    }
}

/// C `vendor/tmux/grid-view.c:95`: `void grid_view_clear(struct grid *gd, u_int px, u_int py, u_int nx, u_int ny, u_int bg)`
pub unsafe fn grid_view_clear(gd: *mut grid, mut px: u32, mut py: u32, nx: u32, ny: u32, bg: u32) {
    unsafe {
        px = grid_view_x(gd, px);
        py = grid_view_y(gd, py);

        grid_clear(gd, px, py, nx, ny, bg);
    }
}

/// C `vendor/tmux/grid-view.c:106`: `void grid_view_scroll_region_up(struct grid *gd, u_int rupper, u_int rlower, u_int bg)`
pub unsafe fn grid_view_scroll_region_up(gd: *mut grid, mut rupper: u32, mut rlower: u32, bg: u32) {
    unsafe {
        if (*gd).flags & GRID_HISTORY != 0 {
            grid_collect_history(gd);
            if rupper == 0 && rlower == (*gd).sy - 1 {
                grid_scroll_history(gd, bg);
            } else {
                rupper = grid_view_y(gd, rupper);
                rlower = grid_view_y(gd, rlower);
                grid_scroll_history_region(gd, rupper, rlower, bg);
            }
        } else {
            rupper = grid_view_y(gd, rupper);
            rlower = grid_view_y(gd, rlower);
            grid_move_lines(gd, rupper, rupper + 1, rlower - rupper, bg);
        }
    }
}

/// C `vendor/tmux/grid-view.c:127`: `void grid_view_scroll_region_down(struct grid *gd, u_int rupper, u_int rlower, u_int bg)`
pub unsafe fn grid_view_scroll_region_down(
    gd: *mut grid,
    mut rupper: u32,
    mut rlower: u32,
    bg: u32,
) {
    unsafe {
        rupper = grid_view_y(gd, rupper);
        rlower = grid_view_y(gd, rlower);

        grid_move_lines(gd, rupper + 1, rupper, rlower - rupper, bg);
    }
}

/// C `vendor/tmux/grid-view.c:138`: `void grid_view_insert_lines(struct grid *gd, u_int py, u_int ny, u_int bg)`
pub unsafe fn grid_view_insert_lines(gd: *mut grid, mut py: u32, ny: u32, bg: u32) {
    unsafe {
        py = grid_view_y(gd, py);

        let sy = grid_view_y(gd, (*gd).sy);

        grid_move_lines(gd, py + ny, py, sy - py - ny, bg);
    }
}

/// Insert lines in region.
/// C `vendor/tmux/grid-view.c:151`: `void grid_view_insert_lines_region(struct grid *gd, u_int rlower, u_int py, u_int ny, u_int bg)`
pub unsafe fn grid_view_insert_lines_region(
    gd: *mut grid,
    mut rlower: u32,
    mut py: u32,
    ny: u32,
    bg: u32,
) {
    unsafe {
        rlower = grid_view_y(gd, rlower);

        py = grid_view_y(gd, py);

        let ny2 = rlower + 1 - py - ny;
        grid_move_lines(gd, rlower + 1 - ny2, py, ny2, bg);
        // TODO does this bug exist upstream?
        grid_clear(gd, 0, py + ny2, (*gd).sx, ny.saturating_sub(ny2), bg);
    }
}

/// Delete lines.
/// C `vendor/tmux/grid-view.c:167`: `void grid_view_delete_lines(struct grid *gd, u_int py, u_int ny, u_int bg)`
pub unsafe fn grid_view_delete_lines(gd: *mut grid, mut py: u32, ny: u32, bg: u32) {
    unsafe {
        py = grid_view_y(gd, py);

        let sy = grid_view_y(gd, (*gd).sy);

        grid_move_lines(gd, py, py + ny, sy - py - ny, bg);
        grid_clear(gd, 0, sy.saturating_sub(ny), (*gd).sx, ny, bg);
    }
}

/// Delete lines inside scroll region.
/// C `vendor/tmux/grid-view.c:181`: `void grid_view_delete_lines_region(struct grid *gd, u_int rlower, u_int py, u_int ny, u_int bg)`
pub unsafe fn grid_view_delete_lines_region(
    gd: *mut grid,
    mut rlower: u32,
    mut py: u32,
    ny: u32,
    bg: u32,
) {
    unsafe {
        rlower = grid_view_y(gd, rlower);

        py = grid_view_y(gd, py);

        let ny2 = rlower + 1 - py - ny;
        grid_move_lines(gd, py, py + ny, ny2, bg);
        // TODO does this bug exist in the tmux source code too
        grid_clear(gd, 0, py + ny2, (*gd).sx, ny.saturating_sub(ny2), bg);
    }
}

/// Insert characters.
/// C `vendor/tmux/grid-view.c:197`: `void grid_view_insert_cells(struct grid *gd, u_int px, u_int py, u_int nx, u_int bg)`
pub unsafe fn grid_view_insert_cells(gd: *mut grid, mut px: u32, mut py: u32, nx: u32, bg: u32) {
    unsafe {
        px = grid_view_x(gd, px);
        py = grid_view_y(gd, py);

        let sx = grid_view_x(gd, (*gd).sx);

        if px >= sx - 1 {
            grid_clear(gd, px, py, 1, 1, bg);
        } else {
            grid_move_cells(gd, px + nx, px, py, sx - px - nx, bg);
        }
    }
}

/// Delete characters.
/// C `vendor/tmux/grid-view.c:214`: `void grid_view_delete_cells(struct grid *gd, u_int px, u_int py, u_int nx, u_int bg)`
pub unsafe fn grid_view_delete_cells(gd: *mut grid, mut px: u32, mut py: u32, nx: u32, bg: u32) {
    unsafe {
        px = grid_view_x(gd, px);
        py = grid_view_y(gd, py);

        let sx = grid_view_x(gd, (*gd).sx);

        grid_move_cells(gd, px, px + nx, py, sx - px - nx, bg);
        grid_clear(gd, sx - nx, py, nx, 1, bg);
    }
}

/// Convert cells into a string.
/// C `vendor/tmux/grid-view.c:229`: `char *grid_view_string_cells(struct grid *gd, u_int px, u_int py, u_int nx)`
pub unsafe fn grid_view_string_cells(gd: *mut grid, mut px: u32, mut py: u32, nx: u32) -> *mut u8 {
    unsafe {
        px = grid_view_x(gd, px);
        py = grid_view_y(gd, py);

        grid_string_cells(
            gd,
            px,
            py,
            nx,
            null_mut(),
            grid_string_flags::empty(),
            null_mut(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a simple, non-extended `grid_cell`: a single one-column ASCII
    /// character with default attributes/flags, plain (256-colour-free) fg/bg,
    /// us=8 and no hyperlink. Mirrors the helper used in grid.rs tests so cells
    /// round-trip through the packed cell entry.
    fn make_cell(ch: u8, fg: i32, bg: i32) -> grid_cell {
        grid_cell::new(
            utf8_data::new([ch], 0, 1, 1),
            grid_attr::empty(),
            grid_flag::empty(),
            fg,
            bg,
            8,
            0,
        )
    }

    /// Read the char stored in a grid cell at view coordinates.
    unsafe fn view_char(gd: *mut grid, px: u32, py: u32) -> u8 {
        unsafe {
            let mut out = zeroed::<grid_cell>();
            grid_view_get_cell(gd, px, py, &mut out);
            out.data.data[0]
        }
    }

    /// Read the char stored at raw grid coordinates (no history offset).
    unsafe fn view_char_raw(gd: *mut grid, px: u32, py: u32) -> u8 {
        unsafe {
            let mut out = zeroed::<grid_cell>();
            grid_get_cell(gd, px, py, &mut out);
            out.data.data[0]
        }
    }

    // grid_view_x(gd, x) == x and grid_view_y(gd, y) == gd->hsize + y
    // (grid-view.c:30-31). With no history (hlimit == 0) hsize is 0, so view
    // coordinates map straight onto grid coordinates. grid_view_set_cell /
    // grid_view_get_cell then delegate to grid_set_cell / grid_get_cell at the
    // same (x, y) (grid-view.c:37,45).
    #[test]
    fn test_grid_view_set_get_cell_roundtrip_no_history() {
        let gd = grid_create(80, 24, 0);
        unsafe {
            let gc = make_cell(b'A', 2, 5);
            grid_view_set_cell(gd, 3, 4, &gc);

            // Read back through the view helper.
            let mut out = zeroed::<grid_cell>();
            grid_view_get_cell(gd, 3, 4, &mut out);
            assert!(grid_cells_equal(&gc, &out));

            // With hsize == 0 the view coordinate equals the grid coordinate,
            // so grid_get_cell at the same position sees the same cell.
            let mut raw = zeroed::<grid_cell>();
            grid_get_cell(gd, 3, 4, &mut raw);
            assert!(grid_cells_equal(&gc, &raw));

            grid_destroy(gd);
        }
    }

    // An unset cell read through the view returns a copy of GRID_DEFAULT_CELL,
    // since grid_view_get_cell just forwards to grid_get_cell which copies the
    // default cell for out-of-range / unwritten cells (grid.c:653).
    #[test]
    fn test_grid_view_get_cell_unset_is_default() {
        let gd = grid_create(40, 10, 0);
        unsafe {
            let mut out = zeroed::<grid_cell>();
            grid_view_get_cell(gd, 0, 0, &mut out);
            assert!(grid_cells_equal(&GRID_DEFAULT_CELL, &out));
            grid_destroy(gd);
        }
    }

    // grid_view_y adds gd->hsize. After pushing lines into the history the
    // visible row py maps to grid row hsize + py (grid-view.c:31). Verify that
    // a view write lands at the offset grid row and NOT at the raw history row.
    #[test]
    fn test_grid_view_y_history_offset() {
        let gd = grid_create(80, 24, 100);
        unsafe {
            // Push three lines into history so hsize becomes 3.
            grid_scroll_history(gd, 8);
            grid_scroll_history(gd, 8);
            grid_scroll_history(gd, 8);
            assert_eq!((*gd).hsize, 3);

            let gc = make_cell(b'Q', 1, 2);
            grid_view_set_cell(gd, 2, 1, &gc);

            // Landed at grid row hsize + 1 == 4.
            let mut raw = zeroed::<grid_cell>();
            grid_get_cell(gd, 2, 4, &mut raw);
            assert!(grid_cells_equal(&gc, &raw));

            // The view read at py == 1 agrees.
            let mut out = zeroed::<grid_cell>();
            grid_view_get_cell(gd, 2, 1, &mut out);
            assert!(grid_cells_equal(&gc, &out));

            // The raw history row 1 was untouched (still default).
            let mut hist = zeroed::<grid_cell>();
            grid_get_cell(gd, 2, 1, &mut hist);
            assert!(grid_cells_equal(&GRID_DEFAULT_CELL, &hist));

            grid_destroy(gd);
        }
    }

    // grid_view_set_padding forwards to grid_set_padding, which stores the
    // PADDING cell (grid.c:687, GRID_PADDING_CELL is a zero-width padding cell).
    #[test]
    fn test_grid_view_set_padding() {
        let gd = grid_create(20, 5, 0);
        unsafe {
            grid_view_set_padding(gd, 4, 2);
            let mut out = zeroed::<grid_cell>();
            grid_view_get_cell(gd, 4, 2, &mut out);
            assert!(out.flags.intersects(grid_flag::PADDING));
            grid_destroy(gd);
        }
    }

    // grid_view_set_cells writes a run of cells sharing one style, one glyph per
    // byte of s (grid.c:694). Read them back individually through the view.
    #[test]
    fn test_grid_view_set_cells_run() {
        let gd = grid_create(20, 5, 0);
        unsafe {
            let gc = make_cell(b' ', 4, 6);
            let s = c!("hey");
            grid_view_set_cells(gd, 1, 3, &gc, s, 3);
            assert_eq!(view_char(gd, 1, 3), b'h');
            assert_eq!(view_char(gd, 2, 3), b'e');
            assert_eq!(view_char(gd, 3, 3), b'y');
            grid_destroy(gd);
        }
    }

    // grid_view_clear translates coordinates then calls grid_clear (grid-view.c:98).
    // Cleared cells become spaces carrying the requested background colour.
    #[test]
    fn test_grid_view_clear_region() {
        let gd = grid_create(80, 24, 0);
        unsafe {
            // Fill row 5, columns 0..10 with 'X'.
            for x in 0..10u32 {
                let gc = make_cell(b'X', 1, 2);
                grid_view_set_cell(gd, x, 5, &gc);
            }
            // Clear columns 2..5 (nx == 3) on row 5 with bg 3.
            grid_view_clear(gd, 2, 5, 3, 1, 3);

            // Cleared cells: space glyph, background 3.
            for x in 2..5u32 {
                let mut out = zeroed::<grid_cell>();
                grid_view_get_cell(gd, x, 5, &mut out);
                assert_eq!(out.data.data[0], b' ');
                assert_eq!(out.bg, 3);
            }
            // Untouched cells remain 'X'.
            assert_eq!(view_char(gd, 1, 5), b'X');
            assert_eq!(view_char(gd, 5, 5), b'X');

            grid_destroy(gd);
        }
    }

    // grid_view_delete_cells shifts the tail of the row left by nx, filling the
    // right edge with cleared cells (grid-view.c:214, grid_move_cells).
    #[test]
    fn test_grid_view_delete_cells() {
        let gd = grid_create(80, 1, 0);
        unsafe {
            for (i, ch) in b"ABCDEF".iter().enumerate() {
                let gc = make_cell(*ch, 1, 2);
                grid_view_set_cell(gd, i as u32, 0, &gc);
            }
            // Delete 2 cells starting at column 2: 'C','D' removed, tail slides in.
            grid_view_delete_cells(gd, 2, 0, 2, 8);

            assert_eq!(view_char(gd, 0, 0), b'A');
            assert_eq!(view_char(gd, 1, 0), b'B');
            assert_eq!(view_char(gd, 2, 0), b'E');
            assert_eq!(view_char(gd, 3, 0), b'F');

            grid_destroy(gd);
        }
    }

    // grid_view_insert_cells shifts the row right by nx and blanks the opened
    // gap (grid-view.c:197, grid_move_cells wipes the source columns).
    #[test]
    fn test_grid_view_insert_cells() {
        let gd = grid_create(80, 1, 0);
        unsafe {
            for (i, ch) in b"ABCDEF".iter().enumerate() {
                let gc = make_cell(*ch, 1, 2);
                grid_view_set_cell(gd, i as u32, 0, &gc);
            }
            // Insert 2 blank cells at column 2: 'C'.. shift right to columns 4,5.
            grid_view_insert_cells(gd, 2, 0, 2, 8);

            assert_eq!(view_char(gd, 0, 0), b'A');
            assert_eq!(view_char(gd, 1, 0), b'B');
            // Opened gap is blanked.
            assert_eq!(view_char(gd, 2, 0), b' ');
            assert_eq!(view_char(gd, 3, 0), b' ');
            // Original tail shifted right.
            assert_eq!(view_char(gd, 4, 0), b'C');
            assert_eq!(view_char(gd, 5, 0), b'D');

            grid_destroy(gd);
        }
    }

    // grid_view_insert_cells clamps at the last column: when px >= sx - 1 it just
    // clears the single cell instead of moving anything (grid-view.c:206).
    #[test]
    fn test_grid_view_insert_cells_last_column() {
        let gd = grid_create(10, 1, 0);
        unsafe {
            let gc = make_cell(b'Z', 1, 2);
            grid_view_set_cell(gd, 9, 0, &gc); // last column, sx - 1
            grid_view_insert_cells(gd, 9, 0, 3, 8);
            assert_eq!(view_char(gd, 9, 0), b' ');
            grid_destroy(gd);
        }
    }

    // Mark column 0 of each row with its row index digit.
    unsafe fn mark_rows(gd: *mut grid, n: u32) {
        unsafe {
            for y in 0..n {
                let gc = make_cell(b'0' + y as u8, 1, 2);
                grid_view_set_cell(gd, 0, y, &gc);
            }
        }
    }

    // grid_view_delete_lines moves the lines below py up over the deleted band
    // (grid-view.c:167). Deleting one line at py == 1 pulls rows 2,3.. up by one.
    #[test]
    fn test_grid_view_delete_lines() {
        let gd = grid_create(80, 24, 0);
        unsafe {
            mark_rows(gd, 4);
            grid_view_delete_lines(gd, 1, 1, 8);

            assert_eq!(view_char(gd, 0, 0), b'0'); // above deletion untouched
            assert_eq!(view_char(gd, 0, 1), b'2'); // old row 2 pulled up
            assert_eq!(view_char(gd, 0, 2), b'3'); // old row 3 pulled up

            grid_destroy(gd);
        }
    }

    // grid_view_insert_lines opens a gap at py by moving the lines down; the
    // opened line is left empty (grid-view.c:138, grid_move_lines wipes source).
    #[test]
    fn test_grid_view_insert_lines() {
        let gd = grid_create(80, 24, 0);
        unsafe {
            mark_rows(gd, 3);
            grid_view_insert_lines(gd, 1, 1, 8);

            assert_eq!(view_char(gd, 0, 0), b'0'); // above insertion untouched
            assert_eq!(view_char(gd, 0, 1), b' '); // opened line is blank
            assert_eq!(view_char(gd, 0, 2), b'1'); // old row 1 pushed down
            assert_eq!(view_char(gd, 0, 3), b'2'); // old row 2 pushed down

            grid_destroy(gd);
        }
    }

    // grid_view_scroll_region_down moves rupper..rlower down by one within the
    // region: grid_move_lines(dy = rupper+1, py = rupper, ny = rlower - rupper)
    // (grid-view.c:127).
    #[test]
    fn test_grid_view_scroll_region_down() {
        let gd = grid_create(80, 24, 0);
        unsafe {
            mark_rows(gd, 5);
            grid_view_scroll_region_down(gd, 1, 3, 8);

            assert_eq!(view_char(gd, 0, 0), b'0'); // outside region
            assert_eq!(view_char(gd, 0, 1), b' '); // top of region vacated
            assert_eq!(view_char(gd, 0, 2), b'1'); // shifted down
            assert_eq!(view_char(gd, 0, 3), b'2'); // shifted down
            assert_eq!(view_char(gd, 0, 4), b'4'); // outside region

            grid_destroy(gd);
        }
    }

    // Without GRID_HISTORY, grid_view_scroll_region_up moves rupper+1..rlower up
    // by one: grid_move_lines(dy = rupper, py = rupper+1, ny = rlower - rupper)
    // (grid-view.c:119-121). hlimit == 0 leaves GRID_HISTORY unset.
    #[test]
    fn test_grid_view_scroll_region_up_no_history() {
        let gd = grid_create(80, 24, 0);
        unsafe {
            assert_eq!((*gd).flags & GRID_HISTORY, 0);
            mark_rows(gd, 5);
            grid_view_scroll_region_up(gd, 1, 3, 8);

            assert_eq!(view_char(gd, 0, 0), b'0'); // outside region
            assert_eq!(view_char(gd, 0, 1), b'2'); // shifted up
            assert_eq!(view_char(gd, 0, 2), b'3'); // shifted up
            assert_eq!(view_char(gd, 0, 3), b' '); // bottom of region vacated
            assert_eq!(view_char(gd, 0, 4), b'4'); // outside region

            grid_destroy(gd);
        }
    }

    // grid_view_string_cells converts the visible cells of a row into a C string
    // (grid-view.c:229). With empty flags it stops at cellused and does no space
    // trimming.
    #[test]
    fn test_grid_view_string_cells() {
        let gd = grid_create(80, 5, 0);
        unsafe {
            for (i, ch) in b"Hi!".iter().enumerate() {
                let gc = make_cell(*ch, 1, 2);
                grid_view_set_cell(gd, i as u32, 2, &gc);
            }
            let out = grid_view_string_cells(gd, 0, 2, 5);
            let s = std::ffi::CStr::from_ptr(out.cast());
            assert_eq!(s.to_bytes(), b"Hi!");
            free_(out);
            grid_destroy(gd);
        }
    }

    // grid_view_delete_lines fills the vacated bottom rows with cleared cells
    // carrying the requested background (grid-view.c:167, grid_clear tail).
    #[test]
    fn test_grid_view_delete_lines_fills_bg() {
        let gd = grid_create(80, 4, 0);
        unsafe {
            mark_rows(gd, 4);
            grid_view_delete_lines(gd, 0, 2, 3); // delete rows 0,1 with bg 3

            // Old rows 2,3 slid up to 0,1.
            assert_eq!(view_char(gd, 0, 0), b'2');
            assert_eq!(view_char(gd, 0, 1), b'3');
            // Bottom rows filled with cleared bg-3 spaces.
            let mut out = zeroed::<grid_cell>();
            grid_view_get_cell(gd, 0, 3, &mut out);
            assert_eq!(out.data.data[0], b' ');
            assert_eq!(out.bg, 3);
            grid_destroy(gd);
        }
    }

    // grid_view_insert_lines_region opens a gap inside a bounded region, pushing
    // the region's lines down and blanking the opened rows (grid-view.c:151).
    #[test]
    fn test_grid_view_insert_lines_region() {
        let gd = grid_create(80, 6, 0);
        unsafe {
            mark_rows(gd, 6);
            // Region bottom rlower=4, insert 1 line at py=1.
            grid_view_insert_lines_region(gd, 4, 1, 1, 8);

            assert_eq!(view_char(gd, 0, 0), b'0'); // above region untouched
            assert_eq!(view_char(gd, 0, 1), b' '); // opened line blank
            assert_eq!(view_char(gd, 0, 2), b'1'); // pushed down
            assert_eq!(view_char(gd, 0, 3), b'2'); // pushed down
            grid_destroy(gd);
        }
    }

    // grid_view_delete_lines_region removes lines inside a bounded region,
    // pulling the region's lower lines up (grid-view.c:181).
    #[test]
    fn test_grid_view_delete_lines_region() {
        let gd = grid_create(80, 6, 0);
        unsafe {
            mark_rows(gd, 6);
            // Region bottom rlower=4, delete 1 line at py=1.
            grid_view_delete_lines_region(gd, 4, 1, 1, 8);

            assert_eq!(view_char(gd, 0, 0), b'0'); // above region untouched
            assert_eq!(view_char(gd, 0, 1), b'2'); // pulled up over deleted row 1
            assert_eq!(view_char(gd, 0, 2), b'3'); // pulled up
            assert_eq!(view_char(gd, 0, 3), b'4'); // pulled up
            grid_destroy(gd);
        }
    }

    // With GRID_HISTORY set and the region spanning the whole screen,
    // grid_view_scroll_region_up pushes the top line into the history via
    // grid_scroll_history (grid-view.c:106).
    #[test]
    fn test_grid_view_scroll_region_up_full_history() {
        let gd = grid_create(80, 3, 100);
        unsafe {
            assert_ne!((*gd).flags & GRID_HISTORY, 0);
            mark_rows(gd, 3);
            // Full-screen region: rupper=0, rlower=sy-1==2.
            grid_view_scroll_region_up(gd, 0, 2, 8);

            // hsize grew by one; the old top view row scrolled into history.
            assert_eq!((*gd).hsize, 1);
            // The scrolled-out '0' now lives at grid (history) row 0.
            let mut out = zeroed::<grid_cell>();
            grid_get_cell(gd, 0, 0, &mut out);
            assert_eq!(out.data.data[0], b'0');
            // View row 0 now shows what was row 1.
            assert_eq!(view_char(gd, 0, 0), b'1');
            grid_destroy(gd);
        }
    }

    // A sub-region scroll-up with history routes through
    // grid_scroll_history_region (grid-view.c:106, the else branch): the top
    // region line is pushed into the history (hsize grows by one) and the region
    // shifts up within the now-offset screen (grid.c:529).
    #[test]
    fn test_grid_view_scroll_region_up_subregion_history() {
        let gd = grid_create(80, 5, 100);
        unsafe {
            mark_rows(gd, 5);
            // Partial region rupper=1, rlower=3 (not the whole screen).
            grid_view_scroll_region_up(gd, 1, 3, 8);

            // The region top line ('1') was moved into the history.
            assert_eq!((*gd).hsize, 1);
            assert_eq!(view_char_raw(gd, 0, 0), b'1');
            // View is now offset by hsize; row 0 shows the old top-of-screen '0'.
            assert_eq!(view_char(gd, 0, 0), b'0');
            assert_eq!(view_char(gd, 0, 1), b'2'); // region shifted up
            assert_eq!(view_char(gd, 0, 2), b'3'); // region shifted up
            grid_destroy(gd);
        }
    }

    // grid_view_clear_history with no populated cells just clears the visible
    // area and leaves the history empty (grid-view.c:66, the last==0 fast path).
    #[test]
    fn test_grid_view_clear_history_empty() {
        let gd = grid_create(20, 4, 100);
        unsafe {
            grid_view_clear_history(gd, 8);
            assert_eq!((*gd).hsize, 0);
            // Visible cells are default/cleared.
            let mut out = zeroed::<grid_cell>();
            grid_view_get_cell(gd, 0, 0, &mut out);
            assert!(grid_cells_equal(&GRID_DEFAULT_CELL, &out));
            grid_destroy(gd);
        }
    }

    // grid_view_clear_history with populated rows scrolls every visible line into
    // the history and resets hscrolled (grid-view.c:66).
    #[test]
    fn test_grid_view_clear_history_with_content() {
        let gd = grid_create(20, 3, 100);
        unsafe {
            mark_rows(gd, 3); // all three rows have content at col 0
            grid_view_clear_history(gd, 8);

            // All visible rows were pushed into history.
            assert_eq!((*gd).hsize, 3);
            assert_eq!((*gd).hscrolled, 0);
            // History rows retain the marked glyphs.
            assert_eq!(view_char_raw(gd, 0, 0), b'0');
            assert_eq!(view_char_raw(gd, 0, 1), b'1');
            assert_eq!(view_char_raw(gd, 0, 2), b'2');
            grid_destroy(gd);
        }
    }

    // grid_view_string_cells honours the history offset: it reads visible row py
    // from grid row hsize + py (grid-view.c:229).
    #[test]
    fn test_grid_view_string_cells_history_offset() {
        let gd = grid_create(80, 3, 100);
        unsafe {
            grid_scroll_history(gd, 8);
            grid_scroll_history(gd, 8);
            assert_eq!((*gd).hsize, 2);
            for (i, ch) in b"ok".iter().enumerate() {
                let gc = make_cell(*ch, 1, 2);
                grid_view_set_cell(gd, i as u32, 0, &gc);
            }
            let out = grid_view_string_cells(gd, 0, 0, 5);
            let s = std::ffi::CStr::from_ptr(out.cast());
            assert_eq!(s.to_bytes(), b"ok");
            free_(out);
            grid_destroy(gd);
        }
    }

    // grid_view_scroll_region_down over the whole screen (rupper=0,
    // rlower=sy-1) always uses grid_move_lines (grid-view.c:127); it never
    // touches the history. Every row shifts down one and the bottom row is lost.
    #[test]
    fn test_grid_view_scroll_region_down_full_screen() {
        let gd = grid_create(80, 4, 0);
        unsafe {
            mark_rows(gd, 4);
            grid_view_scroll_region_down(gd, 0, 3, 8);

            assert_eq!(view_char(gd, 0, 0), b' '); // top vacated
            assert_eq!(view_char(gd, 0, 1), b'0'); // shifted down
            assert_eq!(view_char(gd, 0, 2), b'1');
            assert_eq!(view_char(gd, 0, 3), b'2'); // old '3' scrolled off
            grid_destroy(gd);
        }
    }

    // grid_view_scroll_region_up over the whole screen with history OFF takes the
    // grid_move_lines else-branch (grid-view.c:107-111): rows shift up one and the
    // bottom row is vacated, with nothing pushed into a (non-existent) history.
    #[test]
    fn test_grid_view_scroll_region_up_no_history_full() {
        let gd = grid_create(80, 4, 0);
        unsafe {
            assert_eq!((*gd).flags & GRID_HISTORY, 0);
            mark_rows(gd, 4);
            grid_view_scroll_region_up(gd, 0, 3, 8);

            assert_eq!(view_char(gd, 0, 0), b'1'); // shifted up
            assert_eq!(view_char(gd, 0, 1), b'2');
            assert_eq!(view_char(gd, 0, 2), b'3');
            assert_eq!(view_char(gd, 0, 3), b' '); // bottom vacated
            assert_eq!((*gd).hsize, 0); // no history growth
            grid_destroy(gd);
        }
    }

    // grid_view_delete_cells fills the right edge (columns sx-nx .. sx) with
    // cleared cells carrying the requested background (grid-view.c:223, the
    // grid_clear tail). The earlier delete_cells test checks only the leftward
    // shift, not the fill colour.
    #[test]
    fn test_grid_view_delete_cells_fills_right_edge_bg() {
        let gd = grid_create(80, 1, 0);
        unsafe {
            for (i, ch) in b"ABCDEF".iter().enumerate() {
                let gc = make_cell(*ch, 1, 2);
                grid_view_set_cell(gd, i as u32, 0, &gc);
            }
            grid_view_delete_cells(gd, 2, 0, 2, 3); // bg 3

            // Right-edge columns are cleared bg-3 spaces.
            for x in 78..80u32 {
                let mut out = zeroed::<grid_cell>();
                grid_view_get_cell(gd, x, 0, &mut out);
                assert_eq!(out.data.data[0], b' ');
                assert_eq!(out.bg, 3);
            }
            grid_destroy(gd);
        }
    }

    // grid_view_insert_cells blanks the opened gap by wiping the moved-from source
    // columns with the requested background (grid-view.c:208, grid_move_cells
    // clears vacated cells). The earlier insert_cells test checks the glyph is a
    // space but not the background colour.
    #[test]
    fn test_grid_view_insert_cells_gap_carries_bg() {
        let gd = grid_create(80, 1, 0);
        unsafe {
            for (i, ch) in b"ABCDEF".iter().enumerate() {
                let gc = make_cell(*ch, 1, 2);
                grid_view_set_cell(gd, i as u32, 0, &gc);
            }
            grid_view_insert_cells(gd, 2, 0, 2, 5); // bg 5

            for x in 2..4u32 {
                let mut out = zeroed::<grid_cell>();
                grid_view_get_cell(gd, x, 0, &mut out);
                assert_eq!(out.data.data[0], b' ');
                assert_eq!(out.bg, 5);
            }
            grid_destroy(gd);
        }
    }

    // grid_view_clear of a full-width row with the default background routes
    // through grid_clear -> grid_clear_lines (grid.c:696), emptying the whole
    // line so it reads back as GRID_DEFAULT_CELL.
    #[test]
    fn test_grid_view_clear_full_row_default_bg() {
        let gd = grid_create(6, 2, 0);
        unsafe {
            for x in 0..6u32 {
                let gc = make_cell(b'X', 1, 2);
                grid_view_set_cell(gd, x, 0, &gc);
            }
            grid_view_clear(gd, 0, 0, 6, 1, 8);
            for x in 0..6u32 {
                let mut out = zeroed::<grid_cell>();
                grid_view_get_cell(gd, x, 0, &mut out);
                assert!(grid_cells_equal(&GRID_DEFAULT_CELL, &out));
            }
            grid_destroy(gd);
        }
    }

    // grid_view_insert_lines opens a blank line at py and pushes the lines below
    // down, dropping the bottom row off the screen (grid-view.c:138,
    // grid_move_lines of sy-py-ny lines). Verified on a small grid where the
    // bottom loss is visible.
    #[test]
    fn test_grid_view_insert_lines_bottom_pushed_off() {
        let gd = grid_create(80, 4, 0);
        unsafe {
            mark_rows(gd, 4);
            grid_view_insert_lines(gd, 1, 1, 8);

            assert_eq!(view_char(gd, 0, 0), b'0'); // above insertion untouched
            assert_eq!(view_char(gd, 0, 1), b' '); // opened blank line
            assert_eq!(view_char(gd, 0, 2), b'1'); // pushed down
            assert_eq!(view_char(gd, 0, 3), b'2'); // old '3' pushed off screen
            grid_destroy(gd);
        }
    }

    // grid_view_set_cells honours the history offset: with hsize == 2 a write at
    // view row 0 lands at grid row 2 and leaves the raw history rows untouched
    // (grid-view.c:45 via grid_view_y).
    #[test]
    fn test_grid_view_set_cells_history_offset() {
        let gd = grid_create(80, 3, 100);
        unsafe {
            grid_scroll_history(gd, 8);
            grid_scroll_history(gd, 8);
            assert_eq!((*gd).hsize, 2);

            let gc = make_cell(b' ', 1, 2);
            grid_view_set_cells(gd, 0, 0, &gc, c!("hi"), 2);

            // Landed at grid row hsize + 0 == 2.
            assert_eq!(view_char_raw(gd, 0, 2), b'h');
            assert_eq!(view_char_raw(gd, 1, 2), b'i');
            // Raw history rows 0 and 1 stay default.
            let mut out = zeroed::<grid_cell>();
            grid_get_cell(gd, 0, 0, &mut out);
            assert!(grid_cells_equal(&GRID_DEFAULT_CELL, &out));
            grid_destroy(gd);
        }
    }

    // grid_view_set_cell / grid_view_get_cell forward extended cells (here an RGB
    // foreground) through the packed-vs-extended machinery unchanged
    // (grid-view.c:37/45 delegate to grid_set_cell / grid_get_cell).
    #[test]
    fn test_grid_view_get_cell_extended_roundtrip() {
        let gd = grid_create(20, 5, 0);
        unsafe {
            let mut gc = make_cell(b'r', 8, 8);
            gc.fg = COLOUR_FLAG_RGB | 0x0a_0b_0c;
            grid_view_set_cell(gd, 3, 2, &gc);
            let mut out = zeroed::<grid_cell>();
            grid_view_get_cell(gd, 3, 2, &mut out);
            assert_eq!(out.fg, COLOUR_FLAG_RGB | 0x0a_0b_0c);
            assert!(grid_cells_equal(&gc, &out));
            grid_destroy(gd);
        }
    }
}
