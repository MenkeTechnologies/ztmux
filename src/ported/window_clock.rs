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
use crate::options_::*;

pub static WINDOW_CLOCK_MODE: window_mode = window_mode {
    name: "clock-mode",

    init: window_clock_init,
    free: window_clock_free,
    resize: window_clock_resize,
    key: Some(window_clock_key),
    default_format: None,
    update: None,
    key_table: None,
    command: None,
    formats: None,
};

#[repr(C)]
pub struct window_clock_mode_data {
    pub screen: screen,
    pub tim: time_t,
    pub timer: event,
}

#[rustfmt::skip]
pub static WINDOW_CLOCK_TABLE: [[[u8; 5]; 5]; 14] = [
    [
        [1, 1, 1, 1, 1], /* 0 */
        [1, 0, 0, 0, 1],
        [1, 0, 0, 0, 1],
        [1, 0, 0, 0, 1],
        [1, 1, 1, 1, 1],
    ],
    [
        [0, 0, 0, 0, 1], /* 1 */
        [0, 0, 0, 0, 1],
        [0, 0, 0, 0, 1],
        [0, 0, 0, 0, 1],
        [0, 0, 0, 0, 1],
    ],
    [
        [1, 1, 1, 1, 1], /* 2 */
        [0, 0, 0, 0, 1],
        [1, 1, 1, 1, 1],
        [1, 0, 0, 0, 0],
        [1, 1, 1, 1, 1],
    ],
    [
        [1, 1, 1, 1, 1], /* 3 */
        [0, 0, 0, 0, 1],
        [1, 1, 1, 1, 1],
        [0, 0, 0, 0, 1],
        [1, 1, 1, 1, 1],
    ],
    [
        [1, 0, 0, 0, 1], /* 4 */
        [1, 0, 0, 0, 1],
        [1, 1, 1, 1, 1],
        [0, 0, 0, 0, 1],
        [0, 0, 0, 0, 1],
    ],
    [
        [1, 1, 1, 1, 1], /* 5 */
        [1, 0, 0, 0, 0],
        [1, 1, 1, 1, 1],
        [0, 0, 0, 0, 1],
        [1, 1, 1, 1, 1],
    ],
    [
        [1, 1, 1, 1, 1], /* 6 */
        [1, 0, 0, 0, 0],
        [1, 1, 1, 1, 1],
        [1, 0, 0, 0, 1],
        [1, 1, 1, 1, 1],
    ],
    [
        [1, 1, 1, 1, 1], /* 7 */
        [0, 0, 0, 0, 1],
        [0, 0, 0, 0, 1],
        [0, 0, 0, 0, 1],
        [0, 0, 0, 0, 1],
    ],
    [
        [1, 1, 1, 1, 1], /* 8 */
        [1, 0, 0, 0, 1],
        [1, 1, 1, 1, 1],
        [1, 0, 0, 0, 1],
        [1, 1, 1, 1, 1],
    ],
    [
        [1, 1, 1, 1, 1], /* 9 */
        [1, 0, 0, 0, 1],
        [1, 1, 1, 1, 1],
        [0, 0, 0, 0, 1],
        [1, 1, 1, 1, 1],
    ],
    [
        [0, 0, 0, 0, 0], /* : */
        [0, 0, 1, 0, 0],
        [0, 0, 0, 0, 0],
        [0, 0, 1, 0, 0],
        [0, 0, 0, 0, 0],
    ],
    [
        [1, 1, 1, 1, 1], /* A */
        [1, 0, 0, 0, 1],
        [1, 1, 1, 1, 1],
        [1, 0, 0, 0, 1],
        [1, 0, 0, 0, 1],
    ],
    [
        [1, 1, 1, 1, 1], /* P */
        [1, 0, 0, 0, 1],
        [1, 1, 1, 1, 1],
        [1, 0, 0, 0, 0],
        [1, 0, 0, 0, 0],
    ],
    [
        [1, 0, 0, 0, 1], /* M */
        [1, 1, 0, 1, 1],
        [1, 0, 1, 0, 1],
        [1, 0, 0, 0, 1],
        [1, 0, 0, 0, 1],
    ],
];

/// C `vendor/tmux/window-clock.c:147`: `static void window_clock_timer_callback(__unused int fd, __unused short events, void *arg)`
pub unsafe extern "C-unwind" fn window_clock_timer_callback(
    _fd: i32,
    _events: i16,
    wme: NonNull<window_mode_entry>,
) {
    unsafe {
        let wp = (*wme.as_ptr()).wp;
        let data = (*wme.as_ptr()).data as *mut window_clock_mode_data;
        let mut now: libc::tm = zeroed();
        let mut then: libc::tm = zeroed();
        let mut t: time_t;
        let tv: timeval = timeval {
            tv_sec: 1,
            tv_usec: 0,
        };

        evtimer_del(&raw mut (*data).timer);
        evtimer_add(&raw mut (*data).timer, &tv);

        if tailq_first(&raw mut (*wp).modes) != wme.as_ptr() {
            return;
        }

        t = libc::time(null_mut());
        libc::gmtime_r(&raw mut t, &raw mut now);
        libc::gmtime_r(&raw mut (*data).tim, &raw mut then);
        if now.tm_min == then.tm_min {
            return;
        }
        (*data).tim = t;

        window_clock_draw_screen(wme);
        (*wp).flags |= window_pane_flags::PANE_REDRAW;
    }
}

/// C `vendor/tmux/window-clock.c:171`: `static struct screen *window_clock_init(struct window_mode_entry *wme, __unused struct cmd_find_state *fs, __unused struct args *args)`
pub unsafe fn window_clock_init(
    wme: NonNull<window_mode_entry>,
    _fs: *mut cmd_find_state,
    _args: *mut args,
) -> *mut screen {
    unsafe {
        let wp: *mut window_pane = (*wme.as_ptr()).wp;
        let mut tv = timeval {
            tv_sec: 1,
            tv_usec: 0,
        };

        let data = Box::leak(Box::new(window_clock_mode_data {
            screen: zeroed(),
            tim: libc::time(null_mut()),
            timer: zeroed(),
        })) as *mut window_clock_mode_data;
        (*wme.as_ptr()).data = data.cast();

        evtimer_set(&raw mut (*data).timer, window_clock_timer_callback, wme);
        evtimer_add(&raw mut (*data).timer, &raw mut tv);

        let s = &raw mut (*data).screen;
        screen_init(
            s,
            screen_size_x(&raw mut (*wp).base),
            screen_size_y(&raw mut (*wp).base),
            0,
        );
        (*s).mode &= !mode_flag::MODE_CURSOR;

        window_clock_draw_screen(wme);

        s
    }
}

/// C `vendor/tmux/window-clock.c:194`: `static void window_clock_free(struct window_mode_entry *wme)`
pub unsafe fn window_clock_free(wme: NonNull<window_mode_entry>) {
    unsafe {
        let data = (*wme.as_ptr()).data as *mut window_clock_mode_data;

        evtimer_del(&raw mut (*data).timer);
        screen_free(&raw mut (*data).screen);
        free_(data);
    }
}

/// C `vendor/tmux/window-clock.c:204`: `static void window_clock_resize(struct window_mode_entry *wme, u_int sx, u_int sy)`
pub unsafe fn window_clock_resize(wme: NonNull<window_mode_entry>, sx: u32, sy: u32) {
    unsafe {
        let data = (*wme.as_ptr()).data as *mut window_clock_mode_data;
        let s = &raw mut (*data).screen;

        screen_resize(s, sx, sy, 0);
        window_clock_draw_screen(wme);
    }
}

/// C `vendor/tmux/window-clock.c:214`: `static void window_clock_key(struct window_mode_entry *wme, __unused struct client *c, __unused struct session *s, __unused struct winlink *wl, __unused key_code key, __unused struct mouse_event *m)`
pub unsafe fn window_clock_key(
    wme: NonNull<window_mode_entry>,
    _c: *mut client,
    _s: *mut session,
    _wl: *mut winlink,
    _key: key_code,
    _m: *mut mouse_event,
) {
    unsafe {
        window_pane_reset_mode((*wme.as_ptr()).wp);
    }
}

/// C `vendor/tmux/window-clock.c:222`: `static void window_clock_draw_screen(struct window_mode_entry *wme)`
pub unsafe fn window_clock_draw_screen(wme: NonNull<window_mode_entry>) {
    unsafe {
        let wp = (*wme.as_ptr()).wp;
        let data = (*wme.as_ptr()).data as *mut window_clock_mode_data;
        let mut ctx: screen_write_ctx = zeroed();
        let s = &raw mut (*data).screen;
        const SIZEOF_TIM: usize = 64;
        let mut tim: [u8; 64] = [0; 64];
        let mut x: u32;
        let y: u32;
        let mut idx: u32;

        let colour: i32 = options_get_number_((*(*wp).window).options, "clock-mode-colour") as i32;
        let style: i32 = options_get_number_((*(*wp).window).options, "clock-mode-style") as i32;

        screen_write_start(&raw mut ctx, s);

        let mut t = libc::time(null_mut());
        let tm = libc::localtime(&raw mut t);
        if style == 0 {
            libc::strftime(
                &raw mut tim as _,
                SIZEOF_TIM,
                c!("%l:%M "),
                libc::localtime(&raw mut t),
            );
            if (*tm).tm_hour >= 12 {
                strlcat(&raw mut tim as _, c!("PM"), SIZEOF_TIM);
            } else {
                strlcat(&raw mut tim as _, c!("AM"), SIZEOF_TIM);
            }
        } else {
            libc::strftime(&raw mut tim as _, SIZEOF_TIM, c!("%H:%M"), tm);
        }

        screen_write_clearscreen(&raw mut ctx, 8);

        let mut gc = MaybeUninit::<grid_cell>::uninit();
        let tim_len = strlen(&raw const tim as _) as u32;
        if screen_size_x(s) < 6 * tim_len || screen_size_y(s) < 6 {
            if screen_size_x(s) >= tim_len && screen_size_y(s) != 0 {
                x = (screen_size_x(s) / 2) - (tim_len / 2);
                y = screen_size_y(s) / 2;
                screen_write_cursormove(&raw mut ctx, x as i32, y as i32, 0);

                gc.write(GRID_DEFAULT_CELL);
                (*gc.as_mut_ptr()).flags |= grid_flag::NOPALETTE;
                (*gc.as_mut_ptr()).fg = colour;
                screen_write_puts!(
                    &raw mut ctx,
                    gc.as_mut_ptr(),
                    "{}",
                    _s((&raw const tim).cast::<u8>())
                );
            }

            screen_write_stop(&raw mut ctx);
            return;
        }

        x = (screen_size_x(s) / 2) - 3 * tim_len;
        y = (screen_size_y(s) / 2) - 3;

        gc.write(GRID_DEFAULT_CELL);
        (*gc.as_mut_ptr()).flags |= grid_flag::NOPALETTE;
        (*gc.as_mut_ptr()).bg = colour;
        let mut ptr = &raw mut tim as *mut u8;
        while *ptr != b'\0' {
            if *ptr >= b'0' && *ptr <= b'9' {
                idx = (*ptr - b'0') as u32;
            } else if *ptr == b':' {
                idx = 10;
            } else if *ptr == b'A' {
                idx = 11;
            } else if *ptr == b'P' {
                idx = 12;
            } else if *ptr == b'M' {
                idx = 13;
            } else {
                x += 6;
                ptr = ptr.add(1);
                continue;
            }

            for j in 0..5 {
                for i in 0..5 {
                    screen_write_cursormove(&raw mut ctx, (x + i) as i32, (y + j) as i32, 0);
                    if WINDOW_CLOCK_TABLE[idx as usize][j as usize][i as usize] != 0 {
                        screen_write_putc(&raw mut ctx, gc.as_ptr(), b' ');
                    }
                }
            }
            x += 6;
            ptr = ptr.add(1);
        }

        screen_write_stop(&raw mut ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The functions in this file (window_clock_init/free/resize/key/
    // draw_screen/timer_callback) all require a live `window_mode_entry`
    // with an attached `window_pane`, an event loop and a screen, none of
    // which can be constructed here without a running server. They are
    // therefore SKIPPED. What remains pure and self-contained is the static
    // digit bitmap `WINDOW_CLOCK_TABLE` and the `WINDOW_CLOCK_MODE`
    // descriptor, which are exercised below.
    //
    // Expected values are derived from vendor/tmux/window-clock.c:53-124
    // (`const char window_clock_table[14][5][5]`).

    // C: vendor/tmux/window-clock.c:38-45 `window_clock_mode.name = "clock-mode"`.
    #[test]
    fn mode_name_is_clock_mode() {
        assert_eq!(WINDOW_CLOCK_MODE.name, "clock-mode");
    }

    // C: `const char window_clock_table[14][5][5]` — 14 glyphs, 5 rows, 5 cols.
    #[test]
    fn table_dimensions() {
        assert_eq!(WINDOW_CLOCK_TABLE.len(), 14);
        for glyph in &WINDOW_CLOCK_TABLE {
            assert_eq!(glyph.len(), 5);
            for row in glyph {
                assert_eq!(row.len(), 5);
            }
        }
    }

    // Every cell is a boolean flag: only 0 or 1 appear in the C table.
    #[test]
    fn table_cells_are_binary() {
        for glyph in &WINDOW_CLOCK_TABLE {
            for row in glyph {
                for &c in row {
                    assert!(c == 0 || c == 1, "cell must be 0 or 1, got {c}");
                }
            }
        }
    }

    // C: window-clock.c:54-58, digit 0 is a hollow rectangle.
    #[test]
    fn digit_zero_pattern() {
        assert_eq!(
            WINDOW_CLOCK_TABLE[0],
            [
                [1, 1, 1, 1, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 1, 1, 1, 1],
            ]
        );
    }

    // C: window-clock.c:59-63, digit 1 is the rightmost column only.
    #[test]
    fn digit_one_pattern() {
        assert_eq!(
            WINDOW_CLOCK_TABLE[1],
            [
                [0, 0, 0, 0, 1],
                [0, 0, 0, 0, 1],
                [0, 0, 0, 0, 1],
                [0, 0, 0, 0, 1],
                [0, 0, 0, 0, 1],
            ]
        );
        // Only the last column is lit.
        for row in &WINDOW_CLOCK_TABLE[1] {
            assert_eq!(&row[..4], &[0, 0, 0, 0]);
            assert_eq!(row[4], 1);
        }
    }

    // C: window-clock.c:104-108, the ':' glyph (index 10): two dots in the
    // center column, rows 1 and 3, all else blank.
    #[test]
    fn colon_glyph_pattern() {
        let colon = &WINDOW_CLOCK_TABLE[10];
        assert_eq!(
            *colon,
            [
                [0, 0, 0, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 0, 0, 0],
            ]
        );
        // Exactly two lit cells for the colon.
        let lit: u32 = colon.iter().flatten().map(|&c| c as u32).sum();
        assert_eq!(lit, 2);
    }

    // draw_screen maps chars to table indices as: '0'-'9' -> 0..=9,
    // ':' -> 10, 'A' -> 11, 'P' -> 12, 'M' -> 13 (window-clock.c:288-302).
    // Verify the letter glyphs occupy the expected trailing indices and are
    // non-empty (so AM/PM/24h separators actually render something).
    #[test]
    fn letter_glyphs_present_and_nonempty() {
        for &idx in &[11usize, 12, 13] {
            let lit: u32 = WINDOW_CLOCK_TABLE[idx]
                .iter()
                .flatten()
                .map(|&c| c as u32)
                .sum();
            assert!(lit > 0, "glyph {idx} should have lit cells");
        }
        // 'M' (index 13): C window-clock.c:119-123 first and last rows are
        // the two outer columns lit only.
        assert_eq!(WINDOW_CLOCK_TABLE[13][0], [1, 0, 0, 0, 1]);
        assert_eq!(WINDOW_CLOCK_TABLE[13][1], [1, 1, 0, 1, 1]);
        assert_eq!(WINDOW_CLOCK_TABLE[13][2], [1, 0, 1, 0, 1]);
    }

    // Digits 8 and 0 differ: 8 has a lit middle row, 0 has a hollow middle.
    // Cross-check against C window-clock.c:94-98.
    #[test]
    fn digit_eight_has_full_middle_row() {
        assert_eq!(WINDOW_CLOCK_TABLE[8][2], [1, 1, 1, 1, 1]);
        assert_eq!(WINDOW_CLOCK_TABLE[0][2], [1, 0, 0, 0, 1]);
    }

    // C: window-clock.c:68-72, digit 2.
    #[test]
    fn digit_two_pattern() {
        assert_eq!(
            WINDOW_CLOCK_TABLE[2],
            [
                [1, 1, 1, 1, 1],
                [0, 0, 0, 0, 1],
                [1, 1, 1, 1, 1],
                [1, 0, 0, 0, 0],
                [1, 1, 1, 1, 1],
            ]
        );
    }

    // C: window-clock.c:113-117, 'A' glyph (index 11).
    #[test]
    fn letter_a_pattern() {
        assert_eq!(
            WINDOW_CLOCK_TABLE[11],
            [
                [1, 1, 1, 1, 1],
                [1, 0, 0, 0, 1],
                [1, 1, 1, 1, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
            ]
        );
    }

    // C: window-clock.c:118-122, 'P' glyph (index 12): the bottom two rows have
    // only the leftmost column lit (the tail of the P bowl).
    #[test]
    fn letter_p_pattern() {
        assert_eq!(
            WINDOW_CLOCK_TABLE[12],
            [
                [1, 1, 1, 1, 1],
                [1, 0, 0, 0, 1],
                [1, 1, 1, 1, 1],
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
            ]
        );
    }

    // C: window-clock.c:38-45 — WINDOW_CLOCK_MODE wires the clock lifecycle:
    // key is present; the format/update/command hooks are None.
    #[test]
    fn mode_descriptor_optional_hooks() {
        assert!(WINDOW_CLOCK_MODE.key.is_some());
        assert!(WINDOW_CLOCK_MODE.default_format.is_none());
        assert!(WINDOW_CLOCK_MODE.update.is_none());
        assert!(WINDOW_CLOCK_MODE.command.is_none());
        assert!(WINDOW_CLOCK_MODE.formats.is_none());
    }
}
