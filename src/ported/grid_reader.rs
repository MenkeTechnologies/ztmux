// Copyright (c) 2020 Anindya Mukherjee <anindya49@hotmail.com>
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

/// C `vendor/tmux/grid-reader.c:24`: `void grid_reader_start(struct grid_reader *gr, struct grid *gd, u_int cx, u_int cy)`
pub unsafe fn grid_reader_start(gr: *mut grid_reader, gd: *mut grid, cx: u32, cy: u32) {
    unsafe {
        (*gr).gd = gd;
        (*gr).cx = cx;
        (*gr).cy = cy;
    }
}

/// C `vendor/tmux/grid-reader.c:33`: `void grid_reader_get_cursor(struct grid_reader *gr, u_int *cx, u_int *cy)`
pub unsafe fn grid_reader_get_cursor(gr: *mut grid_reader, cx: *mut u32, cy: *mut u32) {
    unsafe {
        *cx = (*gr).cx;
        *cy = (*gr).cy;
    }
}

/// C `vendor/tmux/grid-reader.c:41`: `u_int grid_reader_line_length(struct grid_reader *gr)`
pub unsafe fn grid_reader_line_length(gr: *mut grid_reader) -> u32 {
    unsafe { grid_line_length((*gr).gd, (*gr).cy) }
}

/// C `vendor/tmux/grid-reader.c:48`: `void grid_reader_cursor_right(struct grid_reader *gr, int wrap, int all, int onemore)`
pub unsafe fn grid_reader_cursor_right(gr: *mut grid_reader, wrap: u32, all: i32) {
    unsafe {
        let mut gc = MaybeUninit::<grid_cell>::uninit();

        let px = if all != 0 {
            (*(*gr).gd).sx
        } else {
            grid_reader_line_length(gr)
        };

        if wrap != 0 && (*gr).cx >= px && (*gr).cy < (*(*gr).gd).hsize + (*(*gr).gd).sy - 1 {
            grid_reader_cursor_start_of_line(gr, 0);
            grid_reader_cursor_down(gr);
        } else if (*gr).cx < px {
            (*gr).cx += 1;
            while (*gr).cx < px {
                grid_get_cell((*gr).gd, (*gr).cx, (*gr).cy, gc.as_mut_ptr());
                if !(*gc.as_ptr()).flags.intersects(grid_flag::PADDING) {
                    break;
                }
                (*gr).cx += 1;
            }
        }
    }
}

/// C `vendor/tmux/grid-reader.c:79`: `void grid_reader_cursor_left(struct grid_reader *gr, int wrap)`
pub unsafe fn grid_reader_cursor_left(gr: *mut grid_reader, wrap: i32) {
    unsafe {
        let mut gc = MaybeUninit::<grid_cell>::uninit();

        while (*gr).cx > 0 {
            grid_get_cell((*gr).gd, (*gr).cx, (*gr).cy, gc.as_mut_ptr());
            if !(*gc.as_ptr()).flags.intersects(grid_flag::PADDING) {
                break;
            }
            (*gr).cx -= 1;
        }
        if (*gr).cx == 0
            && (*gr).cy > 0
            && (wrap != 0
                || (*grid_get_line((*gr).gd, (*gr).cy - 1))
                    .flags
                    .intersects(grid_line_flag::WRAPPED))
        {
            grid_reader_cursor_up(gr);
            grid_reader_cursor_end_of_line(gr, 0, 0);
        } else if (*gr).cx > 0 {
            (*gr).cx -= 1;
        }
    }
}

/// C `vendor/tmux/grid-reader.c:100`: `void grid_reader_cursor_down(struct grid_reader *gr)`
pub unsafe fn grid_reader_cursor_down(gr: *mut grid_reader) {
    unsafe {
        let mut gc = MaybeUninit::<grid_cell>::uninit();
        let gc = gc.as_mut_ptr();

        if (*gr).cy < (*(*gr).gd).hsize + (*(*gr).gd).sy - 1 {
            (*gr).cy += 1;
        }
        while (*gr).cx > 0 {
            grid_get_cell((*gr).gd, (*gr).cx, (*gr).cy, gc);
            if !(*gc).flags.intersects(grid_flag::PADDING) {
                break;
            }
            (*gr).cx -= 1;
        }
    }
}

/// C `vendor/tmux/grid-reader.c:116`: `void grid_reader_cursor_up(struct grid_reader *gr)`
pub unsafe fn grid_reader_cursor_up(gr: *mut grid_reader) {
    unsafe {
        let mut gc = MaybeUninit::<grid_cell>::uninit();
        let gc = gc.as_mut_ptr();

        if (*gr).cy > 0 {
            (*gr).cy -= 1;
        }
        while (*gr).cx > 0 {
            grid_get_cell((*gr).gd, (*gr).cx, (*gr).cy, gc);
            if !(*gc).flags.intersects(grid_flag::PADDING) {
                break;
            }
            (*gr).cx -= 1;
        }
    }
}

/// C `vendor/tmux/grid-reader.c:132`: `void grid_reader_cursor_start_of_line(struct grid_reader *gr, int wrap)`
pub unsafe fn grid_reader_cursor_start_of_line(gr: *mut grid_reader, wrap: i32) {
    unsafe {
        if wrap != 0 {
            while (*gr).cy > 0
                && (*grid_get_line((*gr).gd, (*gr).cy - 1))
                    .flags
                    .intersects(grid_line_flag::WRAPPED)
            {
                (*gr).cy -= 1;
            }
        }
        (*gr).cx = 0;
    }
}

/// C `vendor/tmux/grid-reader.c:145`: `void grid_reader_cursor_end_of_line(struct grid_reader *gr, int wrap, int all)`
pub unsafe fn grid_reader_cursor_end_of_line(gr: *mut grid_reader, wrap: i32, all: i32) {
    unsafe {
        if wrap != 0 {
            let yy = (*(*gr).gd).hsize + (*(*gr).gd).sy - 1;
            while (*gr).cy < yy
                && (*grid_get_line((*gr).gd, (*gr).cy))
                    .flags
                    .intersects(grid_line_flag::WRAPPED)
            {
                (*gr).cy += 1;
            }
        }
        if all != 0 {
            (*gr).cx = (*(*gr).gd).sx;
        } else {
            (*gr).cx = grid_reader_line_length(gr);
        }
    }
}

/// C `vendor/tmux/grid-reader.c:163`: `static int grid_reader_handle_wrap(struct grid_reader *gr, u_int *xx, u_int *yy)`
pub unsafe fn grid_reader_handle_wrap(gr: *mut grid_reader, xx: *mut u32, yy: *mut u32) -> i32 {
    unsafe {
        while (*gr).cx > *xx {
            if (*gr).cy == *yy {
                return 0;
            }
            grid_reader_cursor_start_of_line(gr, 0);
            grid_reader_cursor_down(gr);

            if (*grid_get_line((*gr).gd, (*gr).cy))
                .flags
                .intersects(grid_line_flag::WRAPPED)
            {
                *xx = (*(*gr).gd).sx - 1;
            } else {
                *xx = grid_reader_line_length(gr);
            }
        }
    }
    1
}

/// C `vendor/tmux/grid-reader.c:186`: `int grid_reader_in_set(struct grid_reader *gr, const char *set)`
pub unsafe fn grid_reader_in_set(gr: *mut grid_reader, set: *const u8) -> bool {
    unsafe {
        let mut gc = MaybeUninit::<grid_cell>::uninit();
        let gc = gc.as_mut_ptr();

        grid_get_cell((*gr).gd, (*gr).cx, (*gr).cy, gc);
        if (*gc).flags.intersects(grid_flag::PADDING) {
            return false;
        }
        utf8_cstrhas(set, &raw mut (*gc).data)
    }
}

/// C `vendor/tmux/grid-reader.c:193`: `void grid_reader_cursor_next_word(struct grid_reader *gr, const char *separators)`
pub unsafe fn grid_reader_cursor_next_word(gr: *mut grid_reader, separators: *const u8) {
    unsafe {
        // Do not break up wrapped words.
        let mut xx = if (*grid_get_line((*gr).gd, (*gr).cy))
            .flags
            .intersects(grid_line_flag::WRAPPED)
        {
            (*(*gr).gd).sx - 1
        } else {
            grid_reader_line_length(gr)
        };
        let mut yy = (*(*gr).gd).hsize + (*(*gr).gd).sy - 1;

        if grid_reader_handle_wrap(gr, &raw mut xx, &raw mut yy) == 0 {
            return;
        }
        if !grid_reader_in_set(gr, WHITESPACE) {
            if grid_reader_in_set(gr, separators) {
                loop {
                    (*gr).cx += 1;

                    if !(grid_reader_handle_wrap(gr, &raw mut xx, &raw mut yy) != 0
                        && grid_reader_in_set(gr, separators)
                        && !grid_reader_in_set(gr, WHITESPACE))
                    {
                        break;
                    }
                }
            } else {
                loop {
                    (*gr).cx += 1;
                    if !(grid_reader_handle_wrap(gr, &raw mut xx, &raw mut yy) != 0
                        && (!grid_reader_in_set(gr, separators)
                            || grid_reader_in_set(gr, WHITESPACE)))
                    {
                        break;
                    }
                }
            }
        }
        while grid_reader_handle_wrap(gr, &raw mut xx, &raw mut yy) != 0
            && grid_reader_in_set(gr, WHITESPACE)
        {
            (*gr).cx += 1;
        }
    }
}

/// C `vendor/tmux/grid-reader.c:238`: `void grid_reader_cursor_next_word_end(struct grid_reader *gr, const char *separators)`
pub unsafe fn grid_reader_cursor_next_word_end(gr: *mut grid_reader, separators: *const u8) {
    unsafe {
        // Do not break up wrapped words.
        let mut xx = if (*grid_get_line((*gr).gd, (*gr).cy))
            .flags
            .intersects(grid_line_flag::WRAPPED)
        {
            (*(*gr).gd).sx - 1
        } else {
            grid_reader_line_length(gr)
        };
        let mut yy = (*(*gr).gd).hsize + (*(*gr).gd).sy - 1;

        while grid_reader_handle_wrap(gr, &raw mut xx, &raw mut yy) != 0 {
            if grid_reader_in_set(gr, WHITESPACE) {
                (*gr).cx += 1;
            } else if grid_reader_in_set(gr, separators) {
                loop {
                    (*gr).cx += 1;

                    if !(grid_reader_handle_wrap(gr, &raw mut xx, &raw mut yy) != 0
                        && grid_reader_in_set(gr, separators)
                        && !grid_reader_in_set(gr, WHITESPACE))
                    {
                        break;
                    }
                }
                return;
            } else {
                loop {
                    (*gr).cx += 1;

                    if !(grid_reader_handle_wrap(gr, &raw mut xx, &raw mut yy) != 0
                        && !(grid_reader_in_set(gr, WHITESPACE)
                            || grid_reader_in_set(gr, separators)))
                    {
                        break;
                    }
                }
                return;
            }
        }
    }
}

/// C `vendor/tmux/grid-reader.c:283`: `void grid_reader_cursor_previous_word(struct grid_reader *gr, const char *separators, int already, int stop_at_eol)`
pub unsafe fn grid_reader_cursor_previous_word(
    gr: *mut grid_reader,
    separators: *const u8,
    already: i32,
    stop_at_eol: bool,
) {
    unsafe {
        let mut oldx: i32;
        let word_is_letters;

        if already != 0 || grid_reader_in_set(gr, WHITESPACE) {
            loop {
                if (*gr).cx > 0 {
                    (*gr).cx -= 1;
                    if !grid_reader_in_set(gr, WHITESPACE) {
                        word_is_letters = !grid_reader_in_set(gr, separators);
                        break;
                    }
                } else {
                    if (*gr).cy == 0 {
                        return;
                    }
                    grid_reader_cursor_up(gr);
                    grid_reader_cursor_end_of_line(gr, 0, 0);

                    if stop_at_eol && (*gr).cx > 0 {
                        oldx = (*gr).cx as i32;
                        (*gr).cx -= 1;
                        let at_eol = grid_reader_in_set(gr, WHITESPACE);
                        (*gr).cx = oldx as u32;
                        if at_eol {
                            word_is_letters = false;
                            break;
                        }
                    }
                }
            }
        } else {
            word_is_letters = !grid_reader_in_set(gr, separators);
        }

        let mut oldx;
        let mut oldy;
        loop {
            oldx = (*gr).cx;
            oldy = (*gr).cy;
            if (*gr).cx == 0 {
                if (*gr).cy == 0
                    || (!(*grid_get_line((*gr).gd, (*gr).cy - 1))
                        .flags
                        .intersects(grid_line_flag::WRAPPED))
                {
                    break;
                }
                grid_reader_cursor_up(gr);
                grid_reader_cursor_end_of_line(gr, 0, 1);
            }
            if (*gr).cx > 0 {
                (*gr).cx -= 1;
            }

            if grid_reader_in_set(gr, WHITESPACE)
                || word_is_letters == grid_reader_in_set(gr, separators)
            {
                break;
            }
        }
        (*gr).cx = oldx;
        (*gr).cy = oldy;
    }
}

/// C `vendor/tmux/grid-reader.c:357`: `int grid_reader_cursor_jump(struct grid_reader *gr, const struct utf8_data *jc)`
pub unsafe fn grid_reader_cursor_jump(gr: *mut grid_reader, jc: *const utf8_data) -> i32 {
    unsafe {
        let mut gc = MaybeUninit::<grid_cell>::uninit();
        let gc = gc.as_mut_ptr();

        let mut px = (*gr).cx;
        let yy = (*(*gr).gd).hsize + (*(*gr).gd).sy - 1;

        let mut py = (*gr).cy;
        while py <= yy {
            let xx = grid_line_length((*gr).gd, py);
            while px < xx {
                grid_get_cell((*gr).gd, px, py, gc);
                if !(*gc).flags.intersects(grid_flag::PADDING)
                    && (*gc).data.size == (*jc).size
                    && memcmp(
                        (*gc).data.data.as_ptr().cast(),
                        (*jc).data.as_ptr().cast(),
                        (*gc).data.size as usize,
                    ) == 0
                {
                    (*gr).cx = px;
                    (*gr).cy = py;
                    return 1;
                }
                px += 1;
            }

            if py == yy
                || !(*grid_get_line((*gr).gd, py))
                    .flags
                    .intersects(grid_line_flag::WRAPPED)
            {
                return 0;
            }
            px = 0;
            py += 1;
        }
    }
    0
}

/// C `vendor/tmux/grid-reader.c:387`: `int grid_reader_cursor_jump_back(struct grid_reader *gr, const struct utf8_data *jc)`
pub unsafe fn grid_reader_cursor_jump_back(gr: *mut grid_reader, jc: *mut utf8_data) -> i32 {
    unsafe {
        let mut gc = MaybeUninit::<grid_cell>::uninit();
        let gc = gc.as_mut_ptr();

        let mut xx = (*gr).cx + 1;

        let mut py = (*gr).cy + 1;
        let mut px;
        while py > 0 {
            px = xx;
            while px > 0 {
                grid_get_cell((*gr).gd, px - 1, py - 1, gc);
                if !(*gc).flags.intersects(grid_flag::PADDING)
                    && (*gc).data.size == (*jc).size
                    && memcmp(
                        (*gc).data.data.as_ptr().cast(),
                        (*jc).data.as_ptr().cast(),
                        (*gc).data.size as usize,
                    ) == 0
                {
                    (*gr).cx = px - 1;
                    (*gr).cy = py - 1;
                    return 1;
                }
                px -= 1;
            }

            if py == 1
                || !(*grid_get_line((*gr).gd, py - 2))
                    .flags
                    .intersects(grid_line_flag::WRAPPED)
            {
                return 0;
            }
            xx = grid_line_length((*gr).gd, py - 2);
            py -= 1;
        }
    }
    0
}

/// C `vendor/tmux/grid-reader.c:414`: `void grid_reader_cursor_back_to_indentation(struct grid_reader *gr)`
pub unsafe fn grid_reader_cursor_back_to_indentation(gr: *mut grid_reader) {
    unsafe {
        let mut gc = MaybeUninit::<grid_cell>::uninit();
        let gc = gc.as_mut_ptr();
        // u_int px, py, xx, yy, oldx, oldy;

        let yy = (*(*gr).gd).hsize + (*(*gr).gd).sy - 1;
        let oldx = (*gr).cx;
        let oldy = (*gr).cy;
        grid_reader_cursor_start_of_line(gr, 1);

        for py in (*gr).cy..=yy {
            let xx = grid_line_length((*gr).gd, py);
            for px in 0..xx {
                grid_get_cell((*gr).gd, px, py, gc);
                if (*gc).data.size != 1 || (*gc).data.data[0] != b' ' {
                    (*gr).cx = px;
                    (*gr).cy = py;
                    return;
                }
            }
            if !(*grid_get_line((*gr).gd, py))
                .flags
                .intersects(grid_line_flag::WRAPPED)
            {
                break;
            }
        }
        (*gr).cx = oldx;
        (*gr).cy = oldy;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a simple, non-extended one-column ASCII `grid_cell` with default
    /// attributes and flags (in particular no PADDING flag), mirroring the
    /// `make_cell` helper used in the grid.rs tests.
    fn make_cell(ch: u8) -> grid_cell {
        grid_cell::new(
            utf8_data::new([ch], 0, 1, 1),
            grid_attr::empty(),
            grid_flag::empty(),
            8,
            8,
            8,
            0,
        )
    }

    /// Write an ASCII string into row `py` starting at column 0.
    unsafe fn set_line(gd: *mut grid, py: u32, s: &[u8]) {
        unsafe {
            for (i, &ch) in s.iter().enumerate() {
                let gc = make_cell(ch);
                grid_set_cell(gd, i as u32, py, &gc);
            }
        }
    }

    fn reader(gd: *mut grid, cx: u32, cy: u32) -> grid_reader {
        grid_reader { gd, cx, cy }
    }

    // grid_reader_start just stores gd/cx/cy; grid_reader_get_cursor reads them
    // back (grid-reader.c:24 and grid-reader.c:33).
    #[test]
    fn test_start_and_get_cursor() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            let mut gr = reader(null_mut(), 0, 0);
            grid_reader_start(&mut gr, gd, 5, 2);
            assert_eq!(gr.cx, 5);
            assert_eq!(gr.cy, 2);
            assert!(std::ptr::eq(gr.gd, gd));

            let mut cx = 0u32;
            let mut cy = 0u32;
            grid_reader_get_cursor(&mut gr, &mut cx, &mut cy);
            assert_eq!(cx, 5);
            assert_eq!(cy, 2);

            grid_destroy(gd);
        }
    }

    // grid_reader_line_length delegates to grid_line_length, which trims
    // trailing spaces (grid-reader.c:41).
    #[test]
    fn test_line_length() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            set_line(gd, 0, b"hello world");
            set_line(gd, 1, b"  hi"); // leading spaces counted, no trailing
            // row 2 left empty

            let mut gr = reader(gd, 0, 0);
            assert_eq!(grid_reader_line_length(&mut gr), 11);
            gr.cy = 1;
            assert_eq!(grid_reader_line_length(&mut gr), 4);
            gr.cy = 2;
            assert_eq!(grid_reader_line_length(&mut gr), 0);

            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_right: px is line length when all==0 (the Rust port
    // folds the C `onemore` case in), or the grid width when all!=0. Cursor
    // advances while below px (grid-reader.c:48).
    #[test]
    fn test_cursor_right_basic() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            set_line(gd, 0, b"abc"); // length 3
            let mut gr = reader(gd, 0, 0);

            grid_reader_cursor_right(&mut gr, 0, 0);
            assert_eq!(gr.cx, 1);
            grid_reader_cursor_right(&mut gr, 0, 0);
            assert_eq!(gr.cx, 2);
            grid_reader_cursor_right(&mut gr, 0, 0);
            assert_eq!(gr.cx, 3); // reached px == line length
            // At px with no wrap: no movement.
            grid_reader_cursor_right(&mut gr, 0, 0);
            assert_eq!(gr.cx, 3);

            // With all!=0, px becomes the grid width (20), so it advances again.
            grid_reader_cursor_right(&mut gr, 0, 1);
            assert_eq!(gr.cx, 4);

            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_right with wrap: at/after px and not on the last row,
    // move to start of the next line (grid-reader.c:63).
    #[test]
    fn test_cursor_right_wrap() {
        let gd = grid_create(3, 2, 0);
        unsafe {
            set_line(gd, 0, b"abc"); // length 3 == px
            let mut gr = reader(gd, 3, 0);

            grid_reader_cursor_right(&mut gr, 1, 0);
            assert_eq!(gr.cx, 0);
            assert_eq!(gr.cy, 1);

            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_left: decrement, and at column 0 of a wrapped/forced
    // line move up to the end of the previous line (grid-reader.c:79).
    #[test]
    fn test_cursor_left_basic() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            set_line(gd, 0, b"abc");
            let mut gr = reader(gd, 3, 0);

            grid_reader_cursor_left(&mut gr, 0);
            assert_eq!(gr.cx, 2);
            grid_reader_cursor_left(&mut gr, 0);
            assert_eq!(gr.cx, 1);
            grid_reader_cursor_left(&mut gr, 0);
            assert_eq!(gr.cx, 0);
            // At origin (cx==0, cy==0): no movement.
            grid_reader_cursor_left(&mut gr, 0);
            assert_eq!(gr.cx, 0);
            assert_eq!(gr.cy, 0);

            grid_destroy(gd);
        }
    }

    #[test]
    fn test_cursor_left_wrap_up() {
        let gd = grid_create(3, 2, 0);
        unsafe {
            set_line(gd, 0, b"abc");
            set_line(gd, 1, b"de");
            // Mark row 0 as wrapped so left from (0,1) climbs to end of row 0.
            (*grid_get_line(gd, 0)).flags |= grid_line_flag::WRAPPED;

            let mut gr = reader(gd, 0, 1);
            grid_reader_cursor_left(&mut gr, 0);
            assert_eq!(gr.cy, 0);
            assert_eq!(gr.cx, 3); // end of line == length of "abc"

            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_down / _up move cy within [0, hsize+sy-1]
    // (grid-reader.c:100 and grid-reader.c:116).
    #[test]
    fn test_cursor_up_down() {
        let gd = grid_create(20, 3, 0); // max cy == 2
        unsafe {
            let mut gr = reader(gd, 5, 0);

            grid_reader_cursor_down(&mut gr);
            assert_eq!((gr.cx, gr.cy), (5, 1));
            grid_reader_cursor_down(&mut gr);
            assert_eq!((gr.cx, gr.cy), (5, 2));
            grid_reader_cursor_down(&mut gr); // clamped at bottom
            assert_eq!((gr.cx, gr.cy), (5, 2));

            grid_reader_cursor_up(&mut gr);
            assert_eq!(gr.cy, 1);
            grid_reader_cursor_up(&mut gr);
            assert_eq!(gr.cy, 0);
            grid_reader_cursor_up(&mut gr); // clamped at top
            assert_eq!(gr.cy, 0);

            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_start_of_line sets cx=0; cursor_end_of_line sets cx to
    // line length (all==0) or grid width (all==1) (grid-reader.c:132/145).
    #[test]
    fn test_start_and_end_of_line() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            set_line(gd, 0, b"hello world"); // length 11
            let mut gr = reader(gd, 5, 0);

            grid_reader_cursor_end_of_line(&mut gr, 0, 0);
            assert_eq!(gr.cx, 11);

            grid_reader_cursor_start_of_line(&mut gr, 0);
            assert_eq!(gr.cx, 0);

            grid_reader_cursor_end_of_line(&mut gr, 0, 1);
            assert_eq!(gr.cx, 20); // grid width

            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_next_word: starting on whitespace exercises the final
    // whitespace-skip loop, which advances to the start of the next word
    // (grid-reader.c:231). (Starting mid-word exercises a separate branch that
    // diverges from the C source in this port; see the test module notes.)
    #[test]
    fn test_next_word_from_whitespace() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            set_line(gd, 0, b"foo bar baz");
            let sep = crate::c!("");

            // Cursor sits on the space (index 3) between "foo" and "bar".
            let mut gr = reader(gd, 3, 0);
            grid_reader_cursor_next_word(&mut gr, sep);
            assert_eq!((gr.cx, gr.cy), (4, 0)); // start of "bar"

            // And again from the space at index 7 before "baz".
            let mut gr = reader(gd, 7, 0);
            grid_reader_cursor_next_word(&mut gr, sep);
            assert_eq!((gr.cx, gr.cy), (8, 0)); // start of "baz"

            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_next_word_end: from a word char, advance to the first
    // following whitespace/separator; from whitespace, skip it then the word
    // (grid-reader.c:238).
    #[test]
    fn test_next_word_end() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            set_line(gd, 0, b"foo bar baz");
            let sep = crate::c!("");

            // From start of "foo": stop at the space after it (index 3).
            let mut gr = reader(gd, 0, 0);
            grid_reader_cursor_next_word_end(&mut gr, sep);
            assert_eq!((gr.cx, gr.cy), (3, 0));

            // From the space at index 3: skip it, cross "bar", stop at space 7.
            let mut gr = reader(gd, 3, 0);
            grid_reader_cursor_next_word_end(&mut gr, sep);
            assert_eq!((gr.cx, gr.cy), (7, 0));

            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_previous_word: move to the beginning of the current or
    // previous word (grid-reader.c:283).
    #[test]
    fn test_previous_word() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            set_line(gd, 0, b"foo bar baz");
            let sep = crate::c!("");

            let mut gr = reader(gd, 8, 0); // start of "baz"
            grid_reader_cursor_previous_word(&mut gr, sep, 1, false);
            assert_eq!((gr.cx, gr.cy), (4, 0)); // start of "bar"
            grid_reader_cursor_previous_word(&mut gr, sep, 1, false);
            assert_eq!((gr.cx, gr.cy), (0, 0)); // start of "foo"
            // Already at the very first word: no movement.
            grid_reader_cursor_previous_word(&mut gr, sep, 1, false);
            assert_eq!((gr.cx, gr.cy), (0, 0));

            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_jump scans forward from cx on the current line (and
    // across wrapped continuations) for the first cell matching jc
    // (grid-reader.c:357). Found -> cursor moves onto it and returns 1.
    #[test]
    fn test_cursor_jump_forward_found() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            set_line(gd, 0, b"foo.bar");
            let jc = utf8_data::new([b'.'], 0, 1, 1);

            let mut gr = reader(gd, 0, 0);
            assert_eq!(grid_reader_cursor_jump(&mut gr, &jc), 1);
            assert_eq!((gr.cx, gr.cy), (3, 0)); // landed on '.'

            grid_destroy(gd);
        }
    }

    // No match on the (unwrapped) line -> returns 0 and leaves the cursor put
    // (grid-reader.c:377).
    #[test]
    fn test_cursor_jump_forward_not_found() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            set_line(gd, 0, b"foobar");
            let jc = utf8_data::new([b'z'], 0, 1, 1);

            let mut gr = reader(gd, 2, 0);
            assert_eq!(grid_reader_cursor_jump(&mut gr, &jc), 0);
            assert_eq!((gr.cx, gr.cy), (2, 0));

            grid_destroy(gd);
        }
    }

    // The starting cell itself is included in the scan: jumping to the char the
    // cursor already sits on returns 1 without moving (px starts at gr->cx).
    #[test]
    fn test_cursor_jump_forward_on_current_cell() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            set_line(gd, 0, b"a.b");
            let jc = utf8_data::new([b'.'], 0, 1, 1);
            let mut gr = reader(gd, 1, 0); // already on '.'
            assert_eq!(grid_reader_cursor_jump(&mut gr, &jc), 1);
            assert_eq!((gr.cx, gr.cy), (1, 0));
            grid_destroy(gd);
        }
    }

    // A wrapped line continues the scan onto the next row (grid-reader.c:377:
    // only stops when py==yy or the line is not WRAPPED).
    #[test]
    fn test_cursor_jump_forward_across_wrap() {
        let gd = grid_create(3, 3, 0);
        unsafe {
            set_line(gd, 0, b"abc");
            set_line(gd, 1, b"d.e");
            (*grid_get_line(gd, 0)).flags |= grid_line_flag::WRAPPED;
            let jc = utf8_data::new([b'.'], 0, 1, 1);

            let mut gr = reader(gd, 0, 0);
            assert_eq!(grid_reader_cursor_jump(&mut gr, &jc), 1);
            assert_eq!((gr.cx, gr.cy), (1, 1)); // '.' on the wrapped row

            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_jump_back scans left from the cursor and lands on the
    // first cell whose data equals jc, using the same non-padding match test as
    // grid_reader_cell_equals_data (grid-reader.c:342): `!PADDING && size==size &&
    // memcmp==0`. The earlier port folded the PADDING check inside a negated AND
    // (`!(PADDING && ...)`), which was true for every ordinary cell and so
    // "matched" the current cell immediately; fixed to mirror the forward
    // grid_reader_cursor_jump. Starting on 'd' (cx=4) of "a.bcd", a back-scan for
    // '.' skips d/c/b and stops on '.' at cx=1.
    #[test]
    fn test_cursor_jump_back_finds_char_to_left() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            set_line(gd, 0, b"a.bcd");
            let mut jc = utf8_data::new([b'.'], 0, 1, 1);

            let mut gr = reader(gd, 4, 0);
            assert_eq!(grid_reader_cursor_jump_back(&mut gr, &mut jc), 1);
            assert_eq!((gr.cx, gr.cy), (1, 0));

            // No matching char to the left of the cursor: returns 0, no move.
            let mut miss = utf8_data::new([b'z'], 0, 1, 1);
            let mut gr2 = reader(gd, 4, 0);
            assert_eq!(grid_reader_cursor_jump_back(&mut gr2, &mut miss), 0);
            assert_eq!((gr2.cx, gr2.cy), (4, 0));

            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_back_to_indentation moves to the first non-blank cell of
    // the (logical, un-wrapped) line (grid-reader.c:414).
    #[test]
    fn test_back_to_indentation() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            set_line(gd, 0, b"   foo");
            let mut gr = reader(gd, 5, 0);
            grid_reader_cursor_back_to_indentation(&mut gr);
            assert_eq!((gr.cx, gr.cy), (3, 0)); // first non-space

            grid_destroy(gd);
        }
    }

    // An all-blank line has no indentation target, so the cursor is restored to
    // where it started (grid-reader.c:439 oldx/oldy).
    #[test]
    fn test_back_to_indentation_all_blank_restores() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            set_line(gd, 0, b"     "); // spaces trim to length 0
            let mut gr = reader(gd, 4, 0);
            grid_reader_cursor_back_to_indentation(&mut gr);
            assert_eq!((gr.cx, gr.cy), (4, 0));

            grid_destroy(gd);
        }
    }

    // back_to_indentation follows a WRAPPED line back to its start before
    // scanning (grid_reader_cursor_start_of_line(gr, 1)), so indentation is found
    // on the first physical row of the logical line.
    #[test]
    fn test_back_to_indentation_wrapped() {
        let gd = grid_create(3, 3, 0);
        unsafe {
            set_line(gd, 0, b" ab");
            set_line(gd, 1, b"cde");
            (*grid_get_line(gd, 0)).flags |= grid_line_flag::WRAPPED;
            // Start on the wrapped continuation row.
            let mut gr = reader(gd, 2, 1);
            grid_reader_cursor_back_to_indentation(&mut gr);
            assert_eq!((gr.cx, gr.cy), (1, 0)); // 'a' at col 1 of physical row 0

            grid_destroy(gd);
        }
    }

    // With a separator set, grid_reader_cursor_next_word treats separator runs as
    // their own word: from a word char it advances to the following separator run
    // and then skips trailing whitespace (grid-reader.c:193).
    #[test]
    fn test_next_word_with_separators() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            set_line(gd, 0, b"foo.bar baz");
            let sep = crate::c!(".");

            // Starting on the separator '.' (index 3): it is its own word, so the
            // cursor advances past it to the next word "bar" at index 4.
            let mut gr = reader(gd, 3, 0);
            grid_reader_cursor_next_word(&mut gr, sep);
            assert_eq!((gr.cx, gr.cy), (4, 0));

            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_next_word_end with a separator set stops at the end of a
    // separator run when starting inside one (grid-reader.c:238).
    #[test]
    fn test_next_word_end_with_separators() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            set_line(gd, 0, b"a...b");
            let sep = crate::c!(".");

            // From the first '.' (index 1) advance across the "..." run; the loop
            // stops at the first non-separator ('b', index 4).
            let mut gr = reader(gd, 1, 0);
            grid_reader_cursor_next_word_end(&mut gr, sep);
            assert_eq!((gr.cx, gr.cy), (4, 0));

            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_start_of_line(wrap=1) climbs to the first row of a
    // wrapped logical line; end_of_line(wrap=1) descends to the last
    // (grid-reader.c:132/145).
    #[test]
    fn test_start_end_of_line_wrap() {
        let gd = grid_create(3, 4, 0);
        unsafe {
            set_line(gd, 0, b"abc");
            set_line(gd, 1, b"def");
            set_line(gd, 2, b"gh");
            (*grid_get_line(gd, 0)).flags |= grid_line_flag::WRAPPED;
            (*grid_get_line(gd, 1)).flags |= grid_line_flag::WRAPPED;

            // From the middle physical row, wrap-start climbs to row 0.
            let mut gr = reader(gd, 1, 1);
            grid_reader_cursor_start_of_line(&mut gr, 1);
            assert_eq!((gr.cx, gr.cy), (0, 0));

            // Wrap-end descends to the last physical row (row 2, length 2).
            grid_reader_cursor_end_of_line(&mut gr, 1, 0);
            assert_eq!((gr.cx, gr.cy), (2, 2));

            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_right skips PADDING cells: a wide glyph occupies a real
    // cell plus a trailing padding cell, so moving right lands past the padding
    // (grid-reader.c:54).
    #[test]
    fn test_cursor_right_skips_padding() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            // Column 0 'a', column 1 real, column 2 padding, column 3 'b'.
            set_line(gd, 0, b"a");
            let mut wide = make_cell(b'W');
            wide.data.width = 2;
            grid_set_cell(gd, 1, 0, &wide);
            grid_set_padding(gd, 2, 0);
            let b = make_cell(b'b');
            grid_set_cell(gd, 3, 0, &b);

            let mut gr = reader(gd, 1, 0);
            // From the wide cell, right should skip the padding at 2 and reach 3.
            grid_reader_cursor_right(&mut gr, 0, 1);
            assert_eq!(gr.cx, 3);

            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_previous_word with stop_at_eol=true stops at a line
    // boundary rather than crossing into the previous line's trailing word
    // (grid-reader.c:305).
    #[test]
    fn test_previous_word_stop_at_eol() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            set_line(gd, 0, b"foo");
            set_line(gd, 1, b"bar");
            // Rows are NOT wrapped: independent logical lines.
            let sep = crate::c!("");

            // From the start of "bar" on row 1, previous-word with stop_at_eol
            // climbs to row 0 and stops at its end rather than entering "foo".
            let mut gr = reader(gd, 0, 1);
            grid_reader_cursor_previous_word(&mut gr, sep, 1, true);
            assert_eq!(gr.cy, 0);

            grid_destroy(gd);
        }
    }

    // grid_reader_handle_wrap returns 0 when the cursor is past the line end
    // (cx > *xx) but already on the last physical row (cy == *yy), signalling the
    // caller there is nothing further to scan (grid-reader.c:171). The cursor is
    // left untouched.
    #[test]
    fn test_handle_wrap_at_end_returns_zero() {
        let gd = grid_create(3, 1, 0);
        unsafe {
            let mut gr = reader(gd, 2, 0);
            let mut xx = 0u32; // cursor (cx=2) is already past xx
            let mut yy = 0u32; // and on the last row
            assert_eq!(grid_reader_handle_wrap(&mut gr, &mut xx, &mut yy), 0);
            assert_eq!((gr.cx, gr.cy), (2, 0));
            grid_destroy(gd);
        }
    }

    // grid_reader_handle_wrap advances onto the next physical row when the cursor
    // is past the line end and the current row is not the last (grid-reader.c:174,
    // start_of_line + cursor_down). It stops once cx is within the new line's
    // extent and returns 1.
    #[test]
    fn test_handle_wrap_advances_on_wrapped_line() {
        let gd = grid_create(3, 2, 0);
        unsafe {
            set_line(gd, 0, b"abc");
            set_line(gd, 1, b"de");
            (*grid_get_line(gd, 0)).flags |= grid_line_flag::WRAPPED;

            let mut gr = reader(gd, 3, 0); // past end of row 0
            let mut xx = 0u32;
            let mut yy = 1u32; // last row index
            assert_eq!(grid_reader_handle_wrap(&mut gr, &mut xx, &mut yy), 1);
            assert_eq!((gr.cx, gr.cy), (0, 1)); // dropped to start of row 1
            grid_destroy(gd);
        }
    }

    // grid_reader_in_set matches the cell glyph against a set and always reports
    // false for a PADDING cell (grid-reader.c:196-201), regardless of the set.
    #[test]
    fn test_in_set_matches_and_padding() {
        let gd = grid_create(20, 2, 0);
        unsafe {
            set_line(gd, 0, b"a.");
            grid_set_padding(gd, 5, 0);

            let mut gr = reader(gd, 0, 0); // on 'a'
            assert!(grid_reader_in_set(&mut gr, crate::c!("abc")));
            assert!(!grid_reader_in_set(&mut gr, crate::c!(".")));

            gr.cx = 1; // on '.'
            assert!(grid_reader_in_set(&mut gr, crate::c!(".")));

            gr.cx = 5; // on the padding cell
            assert!(!grid_reader_in_set(&mut gr, crate::c!(".")));
            assert!(!grid_reader_in_set(&mut gr, crate::c!(" ")));
            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_jump_back continues the leftward scan up onto a wrapped
    // predecessor row (grid-reader.c:444-451): from 'f' on the wrapped
    // continuation "def", a back-scan for '.' walks up into "a.c" and stops on the
    // '.' at (1,0).
    #[test]
    fn test_cursor_jump_back_across_wrap() {
        let gd = grid_create(3, 3, 0);
        unsafe {
            set_line(gd, 0, b"a.c");
            set_line(gd, 1, b"def");
            (*grid_get_line(gd, 0)).flags |= grid_line_flag::WRAPPED;
            let mut jc = utf8_data::new([b'.'], 0, 1, 1);

            let mut gr = reader(gd, 2, 1); // on 'f'
            assert_eq!(grid_reader_cursor_jump_back(&mut gr, &mut jc), 1);
            assert_eq!((gr.cx, gr.cy), (1, 0));
            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_down lands on the next row and then backs the cursor off
    // any PADDING cell it falls on, so the cursor rests on the real wide-character
    // cell to its left (grid-reader.c:101-107).
    #[test]
    fn test_cursor_down_skips_padding() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            // Row 1: wide 'W' at col 1 (real), padding at col 2.
            let mut wide = make_cell(b'W');
            wide.data.width = 2;
            grid_set_cell(gd, 1, 1, &wide);
            grid_set_padding(gd, 2, 1);

            let mut gr = reader(gd, 2, 0); // column 2, row 0
            grid_reader_cursor_down(&mut gr);
            assert_eq!((gr.cx, gr.cy), (1, 1)); // backed off padding onto 'W'
            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_up mirrors cursor_down: it moves up a row and backs the
    // cursor off a PADDING cell onto the real cell (grid-reader.c:120-126).
    #[test]
    fn test_cursor_up_skips_padding() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            let mut wide = make_cell(b'W');
            wide.data.width = 2;
            grid_set_cell(gd, 1, 1, &wide);
            grid_set_padding(gd, 2, 1);

            let mut gr = reader(gd, 2, 2); // column 2, row 2
            grid_reader_cursor_up(&mut gr);
            assert_eq!((gr.cx, gr.cy), (1, 1));
            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_right with all!=0 uses the grid width as the limit and,
    // with wrap==0, clamps at that width rather than advancing off the row
    // (grid-reader.c:48-61).
    #[test]
    fn test_cursor_right_all_clamps_at_grid_width() {
        let gd = grid_create(5, 1, 0);
        unsafe {
            set_line(gd, 0, b"abc");
            let mut gr = reader(gd, 3, 0);
            grid_reader_cursor_right(&mut gr, 0, 1);
            assert_eq!(gr.cx, 4);
            grid_reader_cursor_right(&mut gr, 0, 1);
            assert_eq!(gr.cx, 5); // reached grid width
            grid_reader_cursor_right(&mut gr, 0, 1);
            assert_eq!(gr.cx, 5); // clamped: no wrap, no further motion
            grid_destroy(gd);
        }
    }

    // grid_reader_cursor_previous_word with a separator set treats the separator
    // as a word boundary: starting inside "bar" of "foo.bar", the back-scan stops
    // at the start of "bar" (index 4) rather than merging across the '.' into
    // "foo" (grid-reader.c:337-366).
    #[test]
    fn test_previous_word_with_separator_midword() {
        let gd = grid_create(20, 3, 0);
        unsafe {
            set_line(gd, 0, b"foo.bar");
            let sep = crate::c!(".");

            let mut gr = reader(gd, 6, 0); // on 'r', the last char of "bar"
            grid_reader_cursor_previous_word(&mut gr, sep, 1, false);
            assert_eq!((gr.cx, gr.cy), (4, 0)); // start of "bar"
            grid_destroy(gd);
        }
    }
}
