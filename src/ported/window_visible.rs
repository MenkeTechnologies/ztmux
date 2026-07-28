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

/// Scratch ranges used when there is no pane to hang them off.
/// C `vendor/tmux/window-visible.c:57`: `static struct visible_ranges sr`
static mut STATIC_RANGES: visible_ranges = visible_ranges {
    ranges: null_mut(),
    used: 0,
    size: 0,
};

/// Check if a single character is within a visible range (not obscured by a
/// floating pane).
/// C `vendor/tmux/window-visible.c:31`: `int window_position_is_visible(struct visible_ranges *r, u_int px)`
///
/// The C's only caller is `screen_write_fast_copy`, which in next-3.7 writes
/// each copied cell to the tty. ztmux's `screen_write_fast_copy` is the older
/// form that only sets grid cells and emits no tty output, so there is nothing
/// to gate there yet; ported for when that function is brought forward.
#[expect(dead_code)]
pub unsafe fn window_position_is_visible(r: *mut visible_ranges, px: u32) -> c_int {
    unsafe {
        if r.is_null() {
            return 1;
        }
        for i in 0..(*r).used as usize {
            let ri = *(*r).ranges.add(i);
            if ri.nx != 0 && px >= ri.px && px < ri.px + ri.nx {
                return 1;
            }
        }
        0
    }
}

/// Construct the ranges of the line starting at `px,py` of `width` cells of
/// `base_wp` that are unobstructed. All ranges are in window coordinates.
/// C `vendor/tmux/window-visible.c:51`: `struct visible_ranges *window_visible_ranges(struct window_pane *base_wp, int px, int py, u_int width, struct visible_ranges *r)`
///
/// ztmux has no pane scrollbars, so the C's `sb_w`/`sb_pos` adjustments to the
/// left and right edges have no counterpart here.
pub unsafe fn window_visible_ranges(
    base_wp: *mut window_pane,
    mut px: c_int,
    py: c_int,
    mut width: u32,
    mut r: *mut visible_ranges,
) -> *mut visible_ranges {
    unsafe {
        if py < 0 || width == 0 {
            return window_visible_ranges_empty(r);
        }
        if px < 0 {
            if (-px) as u32 >= width {
                return window_visible_ranges_empty(r);
            }
            width -= (-px) as u32;
            px = 0;
        }

        if base_wp.is_null() {
            if !r.is_null() {
                return r;
            }
            let sr = &raw mut STATIC_RANGES;
            server_client_ensure_ranges(sr, 1);
            (*(*sr).ranges).px = px as u32;
            (*(*sr).ranges).nx = width;
            (*sr).used = 1;
            return sr;
        }

        let w = (*base_wp).window;
        if py as u32 >= (*w).sy {
            return window_visible_ranges_empty(r);
        }
        if px as u32 + width > (*w).sx {
            width = (*w).sx - px as u32;
        }

        if r.is_null() {
            // Start with the entire width of the range.
            r = &raw mut (*base_wp).r;
            server_client_ensure_ranges(r, 1);
            (*(*r).ranges).px = px as u32;
            (*(*r).ranges).nx = width;
            (*r).used = 1;
        }

        // Walk the z-index from the bottom. Only panes found AFTER base_wp are
        // above it, so only those can obscure it.
        let mut found_self = false;
        for wp in tailq_foreach_reverse::<_, discr_zentry>(&raw mut (*w).z_index).map(NonNull::as_ptr)
        {
            if wp == base_wp {
                found_self = true;
                continue;
            }

            let no_border = window_pane_is_floating(wp) != 0
                && window_pane_get_pane_lines(wp) == pane_lines::PANE_LINES_NONE;
            let (tb, bb) = if no_border {
                ((*wp).yoff as c_int, (*wp).yoff as c_int + (*wp).sy as c_int - 1)
            } else {
                (
                    if (*wp).yoff > 0 { (*wp).yoff as c_int - 1 } else { 0 },
                    (*wp).yoff as c_int + (*wp).sy as c_int,
                )
            };
            if !found_self || !window_pane_visible(wp) || py < tb || py > bb {
                continue;
            }
            // A tiled pane's own top and bottom border rows obscure nothing.
            if window_pane_is_floating(wp) == 0 && (py == tb || py == bb) {
                continue;
            }

            let mut i = 0;
            while i < (*r).used as usize {
                let ri = (*r).ranges.add(i);
                if (*ri).nx == 0 {
                    i += 1;
                    continue;
                }

                let mut lb;
                let mut rb;
                if no_border {
                    lb = (*wp).xoff as c_int;
                    rb = (*wp).xoff as c_int + (*wp).sx as c_int - 1;
                } else {
                    lb = if (*wp).xoff > 0 { (*wp).xoff as c_int - 1 } else { 0 };
                    rb = (*wp).xoff as c_int + (*wp).sx as c_int;
                }
                if lb < 0 {
                    lb = 0;
                }
                if rb < 0 {
                    i += 1;
                    continue;
                }
                // A borderless pane clamps one column earlier: `window-visible.c:160`
                // uses `>=` for it and `>` for a bordered pane.
                if (no_border && rb >= (*w).sx as c_int) || (!no_border && rb > (*w).sx as c_int) {
                    rb = (*w).sx as c_int - 1;
                }
                if lb > rb {
                    i += 1;
                    continue;
                }

                let sx = (*ri).px as c_int;
                let ex = sx + (*ri).nx as c_int - 1;
                if lb > sx && lb <= ex && rb > ex {
                    // The pane's left edge falls inside this range and its
                    // right edge covers past the end: shrink the range.
                    (*ri).nx = (lb - sx) as u32;
                } else if rb >= sx && rb <= ex && lb <= sx {
                    // The pane's right edge falls inside this range and its
                    // left edge covers the start: move the range start past it.
                    (*ri).nx = (ex - rb) as u32;
                    (*ri).px = (rb + 1) as u32;
                } else if lb > sx && rb <= ex {
                    // The pane is fully inside the range: split it in two.
                    server_client_ensure_ranges(r, (*r).used + 1);
                    let ranges = (*r).ranges;
                    let mut s = (*r).used as usize;
                    while s > i {
                        *ranges.add(s) = *ranges.add(s - 1);
                        s -= 1;
                    }
                    let ri = ranges.add(i);
                    (*ranges.add(i + 1)).px = (rb + 1) as u32;
                    (*ranges.add(i + 1)).nx = (ex - rb) as u32;
                    // ri->px was copied, unchanged.
                    (*ri).nx = (lb - sx) as u32;
                    (*r).used += 1;
                } else if lb <= sx && rb > ex {
                    // The pane completely covers this range: delete it by
                    // making it zero length.
                    (*ri).nx = 0;
                }
                // Otherwise the range is already obscured, do nothing.
                i += 1;
            }
        }
        r
    }
}

/// C `vendor/tmux/window-visible.c:218`: the `empty:` label.
unsafe fn window_visible_ranges_empty(r: *mut visible_ranges) -> *mut visible_ranges {
    unsafe {
        if r.is_null() {
            let sr = &raw mut STATIC_RANGES;
            server_client_ensure_ranges(sr, 1);
            (*sr).used = 0;
            return sr;
        }
        (*r).used = 0;
        r
    }
}
