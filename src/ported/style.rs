// Copyright (c) 2007 Nicholas Marriott <nicholas.marriott@gmail.com>
// Copyright (c) 2014 Tiago Cunha <tcunha@users.sourceforge.net>
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
use crate::libc::{snprintf, strchr, strcspn, strncasecmp, strspn};
use crate::*;
use crate::options_::*;

// #define STYLE_ATTR_MASK (~0)

pub static mut STYLE_DEFAULT: style = style {
    gc: grid_cell::new(
        utf8_data::new([b' '], 0, 1, 1),
        grid_attr::empty(),
        grid_flag::empty(),
        8,
        8,
        0,
        0,
    ),
    ignore: 0,

    fill: 8,
    align: style_align::STYLE_ALIGN_DEFAULT,
    list: style_list::STYLE_LIST_OFF,

    range_type: style_range_type::STYLE_RANGE_NONE,
    range_argument: 0,
    range_string: [0; 16], // ""

    default_type: style_default_type::STYLE_DEFAULT_BASE,
};

/// C `vendor/tmux/style.c:58`: `static void style_set_range_string(struct style *sy, const char *s)`
pub unsafe fn style_set_range_string(sy: *mut style, s: *const u8) {
    unsafe {
        strlcpy(&raw mut (*sy).range_string as _, s, 16); // TODO use better sizeof
    }
}

/// C `vendor/tmux/style.c:69`: `int style_parse(struct style *sy, const struct grid_cell *base, const char *in)`
pub unsafe fn style_parse(sy: *mut style, base: *const grid_cell, mut in_: *const u8) -> i32 {
    unsafe {
        let delimiters = c!(" ,\n");

        type tmp_type = [u8; 256];
        let mut tmp_bak: tmp_type = [0; 256];
        let tmp = tmp_bak.as_mut_ptr();

        let mut found: *mut u8;
        let mut end: usize;

        if *in_ == b'\0' {
            return 0;
        }

        let mut saved = MaybeUninit::<style>::uninit();
        style_copy(saved.as_mut_ptr(), sy);
        let saved = saved.assume_init();

        'error: {
            log_debug!("{}: {}", "style_parse", _s(in_));
            loop {
                while *in_ != b'\0' && !strchr(delimiters, *in_ as _).is_null() {
                    in_ = in_.add(1);
                }
                if *in_ == b'\0' {
                    break;
                }

                end = strcspn(in_, delimiters);
                if end > size_of::<tmp_type>() - 1 {
                    break 'error;
                }
                memcpy_(tmp, in_, end);
                *tmp.add(end) = b'\0' as _;

                log_debug!("{}: {}", "style_parse", _s(tmp));
                if strcaseeq_(tmp, "default") {
                    (*sy).gc.fg = (*base).fg;
                    (*sy).gc.bg = (*base).bg;
                    (*sy).gc.us = (*base).us;
                    (*sy).gc.attr = (*base).attr;
                    (*sy).gc.flags = (*base).flags;
                } else if strcaseeq_(tmp, "ignore") {
                    (*sy).ignore = 1;
                } else if strcaseeq_(tmp, "noignore") {
                    (*sy).ignore = 0;
                } else if strcaseeq_(tmp, "push-default") {
                    (*sy).default_type = style_default_type::STYLE_DEFAULT_PUSH;
                } else if strcaseeq_(tmp, "pop-default") {
                    (*sy).default_type = style_default_type::STYLE_DEFAULT_POP;
                } else if strcaseeq_(tmp, "nolist") {
                    (*sy).list = style_list::STYLE_LIST_OFF;
                } else if strncasecmp(tmp, c!("list="), 5) == 0 {
                    if strcaseeq_(tmp.add(5), "on") {
                        (*sy).list = style_list::STYLE_LIST_ON;
                    } else if strcaseeq_(tmp.add(5), "focus") {
                        (*sy).list = style_list::STYLE_LIST_FOCUS;
                    } else if strcaseeq_(tmp.add(5), "left-marker") {
                        (*sy).list = style_list::STYLE_LIST_LEFT_MARKER;
                    } else if strcaseeq_(tmp.add(5), "right-marker") {
                        (*sy).list = style_list::STYLE_LIST_RIGHT_MARKER;
                    } else {
                        break 'error;
                    }
                } else if strcaseeq_(tmp, "norange") {
                    (*sy).range_type = STYLE_DEFAULT.range_type;
                    (*sy).range_argument = STYLE_DEFAULT.range_type as u32;
                    strlcpy(
                        &raw mut (*sy).range_string as *mut u8,
                        &raw const STYLE_DEFAULT.range_string as *const u8,
                        16,
                    );
                } else if end > 6 && strncasecmp(tmp, c!("range="), 6) == 0 {
                    found = strchr(tmp.add(6), b'|' as i32);
                    if !found.is_null() {
                        *found = b'\0' as _;
                        found = found.add(1);
                        if *found == b'\0' {
                            break 'error;
                        }
                    }
                    if strcaseeq_(tmp.add(6), "left") {
                        if !found.is_null() {
                            break 'error;
                        }
                        (*sy).range_type = style_range_type::STYLE_RANGE_LEFT;
                        (*sy).range_argument = 0;
                        style_set_range_string(sy, c!(""));
                    } else if strcaseeq_(tmp.add(6), "right") {
                        if !found.is_null() {
                            break 'error;
                        }
                        (*sy).range_type = style_range_type::STYLE_RANGE_RIGHT;
                        (*sy).range_argument = 0;
                        style_set_range_string(sy, c!(""));
                    } else if strcaseeq_(tmp.add(6), "pane") {
                        if found.is_null() {
                            break 'error;
                        }
                        if *found != b'%' || *found.add(1) == b'\0' {
                            break 'error;
                        }
                        let Ok(n) = strtonum(found.add(1), 0, u32::MAX) else {
                            break 'error;
                        };
                        (*sy).range_type = style_range_type::STYLE_RANGE_PANE;
                        (*sy).range_argument = n;
                        style_set_range_string(sy, c!(""));
                    } else if strcaseeq_(tmp.add(6), "window") {
                        if found.is_null() {
                            break 'error;
                        }
                        let Ok(n) = strtonum(found, 0, u32::MAX) else {
                            break 'error;
                        };
                        (*sy).range_type = style_range_type::STYLE_RANGE_WINDOW;
                        (*sy).range_argument = n;
                        style_set_range_string(sy, c!(""));
                    } else if strcaseeq_(tmp.add(6), "session") {
                        if found.is_null() {
                            break 'error;
                        }
                        if *found != b'$' || *found.add(1) == b'\0' {
                            break 'error;
                        }
                        let Ok(n) = strtonum(found.add(1), 0, u32::MAX) else {
                            break 'error;
                        };
                        (*sy).range_type = style_range_type::STYLE_RANGE_SESSION;
                        (*sy).range_argument = n;
                        style_set_range_string(sy, c!(""));
                    } else if strcaseeq_(tmp.add(6), "user") {
                        if found.is_null() {
                            break 'error;
                        }
                        (*sy).range_type = style_range_type::STYLE_RANGE_USER;
                        (*sy).range_argument = 0;
                        style_set_range_string(sy, found);
                    }
                } else if strcaseeq_(tmp, "noalign") {
                    (*sy).align = STYLE_DEFAULT.align;
                } else if end > 6 && strncasecmp(tmp, c!("align="), 6) == 0 {
                    if strcaseeq_(tmp.add(6), "left") {
                        (*sy).align = style_align::STYLE_ALIGN_LEFT;
                    } else if strcaseeq_(tmp.add(6), "centre") {
                        (*sy).align = style_align::STYLE_ALIGN_CENTRE;
                    } else if strcaseeq_(tmp.add(6), "right") {
                        (*sy).align = style_align::STYLE_ALIGN_RIGHT;
                    } else if strcaseeq_(tmp.add(6), "absolute-centre") {
                        (*sy).align = style_align::STYLE_ALIGN_ABSOLUTE_CENTRE;
                    } else {
                        break 'error;
                    }
                } else if end > 5 && strncasecmp(tmp, c!("fill="), 5) == 0 {
                    let value = colour_fromstring(cstr_to_str(tmp.add(5)));
                    if value == -1 {
                        break 'error;
                    }
                    (*sy).fill = value;
                } else if end > 3 && strncasecmp(tmp.add(1), c!("g="), 2) == 0 {
                    let value = colour_fromstring(cstr_to_str(tmp.add(3)));
                    if value == -1 {
                        break 'error;
                    }
                    if *in_ == b'f' || *in_ == b'F' {
                        if value != 8 {
                            (*sy).gc.fg = value;
                        } else {
                            (*sy).gc.fg = (*base).fg;
                        }
                    } else if *in_ == b'b' || *in_ == b'B' {
                        if value != 8 {
                            (*sy).gc.bg = value;
                        } else {
                            (*sy).gc.bg = (*base).bg;
                        }
                    } else {
                        break 'error;
                    }
                } else if end > 3 && strncasecmp(tmp, c!("us="), 3) == 0 {
                    let value = colour_fromstring(cstr_to_str(tmp.add(3)));
                    if value == -1 {
                        break 'error;
                    }
                    if value != 8 {
                        (*sy).gc.us = value;
                    } else {
                        (*sy).gc.us = (*base).us;
                    }
                } else if strcaseeq_(tmp, "none") {
                    (*sy).gc.attr = grid_attr::empty();
                } else if end > 2 && strncasecmp(tmp, c!("no"), 2) == 0 {
                    let Ok(value) = attributes_fromstring(cstr_to_str(tmp.add(2))) else {
                        break 'error;
                    };
                    (*sy).gc.attr &= !value;
                } else {
                    let Ok(value) = attributes_fromstring(cstr_to_str(tmp)) else {
                        break 'error;
                    };
                    (*sy).gc.attr |= value;
                }

                in_ = in_.add(end + strspn(in_.add(end), delimiters));
                if *in_ == b'\0' {
                    break;
                }
            }

            return 0;
        }

        // error:
        style_copy(sy, &raw const saved);
        -1
    }
}

/// C `vendor/tmux/style.c:303`: `const char *style_tostring(struct style *sy)`
pub unsafe fn style_tostring(sy: *const style) -> *const u8 {
    type s_type = [i8; 256];
    static mut S_BUF: MaybeUninit<s_type> = MaybeUninit::<s_type>::uninit();

    unsafe {
        let gc = &raw const (*sy).gc;
        let mut off: i32 = 0;
        let mut comma = c!("");
        let mut tmp = c!("");
        type b_type = [i8; 21];
        let mut b: b_type = [0; 21];

        let s = &raw mut S_BUF as *mut u8;
        *s = b'\0';

        if (*sy).list != style_list::STYLE_LIST_OFF {
            if (*sy).list == style_list::STYLE_LIST_ON {
                tmp = c!("on");
            } else if (*sy).list == style_list::STYLE_LIST_FOCUS {
                tmp = c!("focus");
            } else if (*sy).list == style_list::STYLE_LIST_LEFT_MARKER {
                tmp = c!("left-marker");
            } else if (*sy).list == style_list::STYLE_LIST_RIGHT_MARKER {
                tmp = c!("right-marker");
            }
            off += xsnprintf_!(
                s.add(off as usize),
                size_of::<s_type>() - off as usize,
                "{}list={}",
                _s(comma),
                _s(tmp),
            )
            .unwrap() as i32;
            comma = c!(",");
        }
        if (*sy).range_type != style_range_type::STYLE_RANGE_NONE {
            if (*sy).range_type == style_range_type::STYLE_RANGE_LEFT {
                tmp = c!("left");
            } else if (*sy).range_type == style_range_type::STYLE_RANGE_RIGHT {
                tmp = c!("right");
            } else if (*sy).range_type == style_range_type::STYLE_RANGE_PANE {
                snprintf(
                    &raw mut b as _,
                    size_of::<b_type>(),
                    c"pane|%%%u".as_ptr(),
                    (*sy).range_argument,
                );
                tmp = &raw const b as _;
            } else if (*sy).range_type == style_range_type::STYLE_RANGE_WINDOW {
                snprintf(
                    &raw mut b as _,
                    size_of::<b_type>(),
                    c"window|%u".as_ptr(),
                    (*sy).range_argument,
                );
                tmp = &raw const b as _;
            } else if (*sy).range_type == style_range_type::STYLE_RANGE_SESSION {
                snprintf(
                    &raw mut b as _,
                    size_of::<b_type>(),
                    c"session|$%u".as_ptr(),
                    (*sy).range_argument,
                );
                tmp = &raw const b as _;
            } else if (*sy).range_type == style_range_type::STYLE_RANGE_USER {
                snprintf(
                    &raw mut b as _,
                    size_of::<b_type>(),
                    c"user|%s".as_ptr(),
                    // C passes the char[] which decays to a pointer; passing the
                    // array by value to a variadic %s is UB (segfault).
                    (*sy).range_string.as_ptr(),
                );
                tmp = &raw const b as _;
            }
            off += xsnprintf_!(
                s.add(off as usize),
                size_of::<s_type>() - off as usize,
                "{}range={}",
                _s(comma),
                _s(tmp),
            )
            .unwrap() as i32;
            comma = c!(",");
        }
        if (*sy).align != style_align::STYLE_ALIGN_DEFAULT {
            if (*sy).align == style_align::STYLE_ALIGN_LEFT {
                tmp = c!("left");
            } else if (*sy).align == style_align::STYLE_ALIGN_CENTRE {
                tmp = c!("centre");
            } else if (*sy).align == style_align::STYLE_ALIGN_RIGHT {
                tmp = c!("right");
            } else if (*sy).align == style_align::STYLE_ALIGN_ABSOLUTE_CENTRE {
                tmp = c!("absolute-centre");
            }
            off += xsnprintf_!(
                s.add(off as usize),
                size_of::<s_type>() - off as usize,
                "{}align={}",
                _s(comma),
                _s(tmp),
            )
            .unwrap() as i32;
            comma = c!(",");
        }
        if (*sy).default_type != style_default_type::STYLE_DEFAULT_BASE {
            if (*sy).default_type == style_default_type::STYLE_DEFAULT_PUSH {
                tmp = c!("push-default");
            } else if (*sy).default_type == style_default_type::STYLE_DEFAULT_POP {
                tmp = c!("pop-default");
            }
            off += xsnprintf_!(
                s.add(off as usize),
                size_of::<s_type>() - off as usize,
                "{}{}",
                _s(comma),
                _s(tmp),
            )
            .unwrap() as i32;
            comma = c!(",");
        }
        if (*sy).fill != 8 {
            off += xsnprintf_!(
                s.add(off as usize),
                size_of::<s_type>() - off as usize,
                "{}fill={}",
                _s(comma),
                colour_tostring((*sy).fill),
            )
            .unwrap() as i32;
            comma = c!(",");
        }
        if (*gc).fg != 8 {
            off += xsnprintf_!(
                s.add(off as usize),
                size_of::<s_type>() - off as usize,
                "{}fg={}",
                _s(comma),
                colour_tostring((*gc).fg),
            )
            .unwrap() as i32;
            comma = c!(",");
        }
        if (*gc).bg != 8 {
            off += xsnprintf_!(
                s.add(off as usize),
                size_of::<s_type>() - off as usize,
                "{}bg={}",
                _s(comma),
                colour_tostring((*gc).bg),
            )
            .unwrap() as i32;
            comma = c!(",");
        }
        if (*gc).us != 8 {
            off += xsnprintf_!(
                s.add(off as usize),
                size_of::<s_type>() - off as usize,
                "{}us={}",
                _s(comma),
                colour_tostring((*gc).us),
            )
            .unwrap() as i32;
            comma = c!(",");
        }
        #[expect(unused_assignments)]
        if !(*gc).attr.is_empty() {
            _ = xsnprintf_!(
                s.add(off as usize),
                size_of::<s_type>() - off as usize,
                "{}{}",
                _s(comma),
                attributes_tostring((*gc).attr),
            );
            comma = c!(",");
        }

        if *s == b'\0' {
            return c!("default");
        }
        s
    }
}

/// C `vendor/tmux/style.c:441`: `struct style *style_add(struct grid_cell *gc, struct options *oo, const char *name, struct format_tree *ft)`
pub unsafe fn style_add(
    gc: *mut grid_cell,
    oo: *mut options,
    name: *const u8,
    mut ft: *mut format_tree,
) {
    unsafe {
        let mut ft0: *mut format_tree = null_mut();

        if ft.is_null() {
            ft0 = format_create(null_mut(), null_mut(), 0, format_flags::FORMAT_NOJOBS);
            ft = ft0;
        }

        let mut sy = options_string_to_style(oo, cstr_to_str(name), ft);
        if sy.is_null() {
            sy = &raw mut STYLE_DEFAULT;
        }
        if (*sy).gc.fg != 8 {
            (*gc).fg = (*sy).gc.fg;
        }
        if (*sy).gc.bg != 8 {
            (*gc).bg = (*sy).gc.bg;
        }
        if (*sy).gc.us != 8 {
            (*gc).us = (*sy).gc.us;
        }
        (*gc).attr |= (*sy).gc.attr;

        if !ft0.is_null() {
            format_free(ft0);
        }
    }
}

/// C `vendor/tmux/style.c:468`: `void style_apply(struct grid_cell *gc, struct options *oo, const char *name, struct format_tree *ft)`
pub unsafe fn style_apply(
    gc: *mut grid_cell,
    oo: *mut options,
    name: *const u8,
    ft: *mut format_tree,
) {
    unsafe {
        memcpy__(gc, &raw const GRID_DEFAULT_CELL);
        style_add(gc, oo, name, ft);
    }
}

/// C `vendor/tmux/style.c:500`: `void style_set(struct style *sy, const struct grid_cell *gc)`
pub unsafe fn style_set(sy: *mut style, gc: *const grid_cell) {
    unsafe {
        memcpy__(sy, &raw const STYLE_DEFAULT);
        memcpy__(&raw mut (*sy).gc, gc);
    }
}

/// C `vendor/tmux/style.c:508`: `void style_copy(struct style *dst, struct style *src)`
pub unsafe fn style_copy(dst: *mut style, src: *const style) {
    unsafe {
        memcpy__(dst, src);
    }
}

#[cfg(test)]
mod tests {
    // Notes on two latent bugs in this port that these tests deliberately work
    // around (they are in the production code, not the tests, so they are not
    // "fixed" here):
    //
    // 1. style_tostring off-by-one: xsnprintf__ (src/xmalloc.rs) returns the
    //    formatted length *including* the NUL, but style_tostring advances `off`
    //    like C's xsnprintf (which excludes the NUL). So every field after the
    //    first is written past an embedded NUL and is lost to strlen. Renders of
    //    styles with two or more fields are therefore truncated to the first
    //    field. Consequently tostring is only asserted for single-field styles;
    //    multi-field cases are verified via the parsed struct fields instead.
    //
    // 2. range=user render segfaults: style_tostring passes the range_string
    //    [u8; 16] array by value to a variadic snprintf("%s"), which is UB. The
    //    user-range case is verified via fields only.
    use super::*;
    use std::ffi::CString;
    use std::sync::Mutex;

    // style_tostring writes into a shared `static mut S_BUF`, so serialise every
    // test that renders a style. STYLE_DEFAULT is also read via style_set /
    // style_parse; the lock keeps that access single-threaded too.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    // Acquire the serialisation lock, tolerating poisoning from an unrelated
    // failing test so failures don't cascade into spurious PoisonErrors.
    fn lock() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    // A fresh style initialised from the default grid cell, matching how tmux
    // callers seed a style before style_parse (vendor/tmux/style.c:500
    // style_set copies style_default then the cell: fg=bg=us=8, attr=0).
    unsafe fn make_style() -> style {
        unsafe {
            let mut sy = MaybeUninit::<style>::uninit();
            style_set(sy.as_mut_ptr(), &raw const GRID_DEFAULT_CELL);
            sy.assume_init()
        }
    }

    // Parse `input` against `base` into a freshly initialised style.
    unsafe fn parse_with(base: *const grid_cell, input: &str) -> (i32, style) {
        unsafe {
            let mut sy = make_style();
            let cs = CString::new(input).unwrap();
            let rc = style_parse(&raw mut sy, base, cs.as_ptr().cast::<u8>());
            (rc, sy)
        }
    }

    // Parse against the default grid cell (the common case).
    unsafe fn parse(input: &str) -> (i32, style) {
        unsafe { parse_with(&raw const GRID_DEFAULT_CELL, input) }
    }

    // Render a style to an owned String; caller must already hold TEST_LOCK
    // because style_tostring uses a shared static buffer.
    unsafe fn tostring(sy: *const style) -> String {
        unsafe { cstr_to_str(style_tostring(sy)).to_owned() }
    }

    #[test]
    fn empty_input_is_noop() {
        let _g = lock();
        unsafe {
            // vendor/tmux/style.c:78 - *in == '\0' returns 0 without touching sy.
            let (rc, sy) = parse("");
            assert_eq!(rc, 0);
            assert_eq!(sy.gc.fg, 8);
            assert_eq!(sy.gc.bg, 8);
            assert_eq!(sy.gc.us, 8);
            assert!(sy.gc.attr.is_empty());
            // A style equal to the default renders as "default"
            // (vendor/tmux/style.c:421).
            assert_eq!(tostring(&raw const sy), "default");
        }
    }

    #[test]
    fn parse_fg_bg_colours() {
        let _g = lock();
        unsafe {
            // colour_fromstring: red=1, blue=4 (vendor/tmux/colour.c).
            let (rc, sy) = parse("fg=red,bg=blue");
            assert_eq!(rc, 0);
            assert_eq!(sy.gc.fg, 1);
            assert_eq!(sy.gc.bg, 4);

            // Single-field renders are exact; each colour matches C
            // (vendor/tmux/style.c:382,387). NOTE: the port cannot be asked to
            // render fg AND bg together here - see the module note on the
            // style_tostring off-by-one - so the two colours are checked
            // separately.
            let (_, syfg) = parse("fg=red");
            assert_eq!(tostring(&raw const syfg), "fg=red");
            let (_, sybg) = parse("bg=blue");
            assert_eq!(tostring(&raw const sybg), "bg=blue");
        }
    }

    #[test]
    fn parse_us_colour() {
        let _g = lock();
        unsafe {
            // us= sets the underscore colour (vendor/tmux/style.c:236).
            let (rc, sy) = parse("us=green");
            assert_eq!(rc, 0);
            assert_eq!(sy.gc.us, 2);
            assert_eq!(tostring(&raw const sy), "us=green");
        }
    }

    #[test]
    fn parse_256_and_rgb_colour() {
        let _g = lock();
        unsafe {
            // colour123 -> 123 | COLOUR_FLAG_256; #112233 -> RGB flag.
            let (rc, sy) = parse("fg=colour123,bg=#112233");
            assert_eq!(rc, 0);
            assert_eq!(sy.gc.fg & 0xff, 123);
            // COLOUR_FLAG_256 (0x01000000) is set, so the value is not a plain
            // palette index (vendor/tmux/colour.c:387 colour_fromstring).
            assert_eq!(sy.gc.fg, 123 | 0x01000000);
            // #112233 sets COLOUR_FLAG_RGB (0x02000000).
            assert!(sy.gc.bg & 0x02000000 != 0);
            // Each colour renders exactly through colour_tostring
            // (vendor/tmux/colour.c) - checked one field at a time.
            let (_, sy256) = parse("fg=colour123");
            assert_eq!(tostring(&raw const sy256), "fg=colour123");
            let (_, syrgb) = parse("bg=#112233");
            assert_eq!(tostring(&raw const syrgb), "bg=#112233");
        }
    }

    #[test]
    fn colour_eight_falls_back_to_base() {
        let _g = lock();
        unsafe {
            // colour_fromstring("default") == 8; style.c:225 uses base->fg then.
            let mut base = GRID_DEFAULT_CELL;
            base.fg = 2;
            base.bg = 6;
            let (rc, sy) = parse_with(&raw const base, "fg=default,bg=default");
            assert_eq!(rc, 0);
            assert_eq!(sy.gc.fg, 2);
            assert_eq!(sy.gc.bg, 6);
        }
    }

    #[test]
    fn parse_attributes() {
        let _g = lock();
        unsafe {
            // "bold" is an alias for GRID_ATTR_BRIGHT; multiple attrs OR together
            // (vendor/tmux/style.c:286).
            let (rc, sy) = parse("bold,underscore");
            assert_eq!(rc, 0);
            assert!(sy.gc.attr.intersects(grid_attr::GRID_ATTR_BRIGHT));
            assert!(sy.gc.attr.intersects(grid_attr::GRID_ATTR_UNDERSCORE));
            // attributes_tostring emits table order with a trailing comma.
            assert_eq!(tostring(&raw const sy), "bright,underscore,");
        }
    }

    #[test]
    fn no_prefix_clears_attribute() {
        let _g = lock();
        unsafe {
            // "no<attr>" clears the bit: attr &= ~value (vendor/tmux/style.c:254).
            let (rc, sy) = parse("bold,underscore,nobright");
            assert_eq!(rc, 0);
            assert!(!sy.gc.attr.intersects(grid_attr::GRID_ATTR_BRIGHT));
            assert!(sy.gc.attr.intersects(grid_attr::GRID_ATTR_UNDERSCORE));
            assert_eq!(tostring(&raw const sy), "underscore,");
        }
    }

    #[test]
    fn none_clears_all_attributes() {
        let _g = lock();
        unsafe {
            // "none" -> gc.attr = 0 (vendor/tmux/style.c:243).
            let (rc, sy) = parse("bold,underscore,none");
            assert_eq!(rc, 0);
            assert!(sy.gc.attr.is_empty());
            assert_eq!(tostring(&raw const sy), "default");
        }
    }

    #[test]
    fn parse_align() {
        let _g = lock();
        unsafe {
            let (rc, sy) = parse("align=centre");
            assert_eq!(rc, 0);
            assert!(sy.align == style_align::STYLE_ALIGN_CENTRE);
            assert_eq!(tostring(&raw const sy), "align=centre");

            let (rc, sy) = parse("align=absolute-centre");
            assert_eq!(rc, 0);
            assert!(sy.align == style_align::STYLE_ALIGN_ABSOLUTE_CENTRE);
            assert_eq!(tostring(&raw const sy), "align=absolute-centre");
        }
    }

    #[test]
    fn noalign_resets_align() {
        let _g = lock();
        unsafe {
            // "noalign" -> style_default.align == STYLE_ALIGN_DEFAULT (style.c:197).
            let (rc, sy) = parse("align=right,noalign");
            assert_eq!(rc, 0);
            assert!(sy.align == style_align::STYLE_ALIGN_DEFAULT);
            assert_eq!(tostring(&raw const sy), "default");
        }
    }

    #[test]
    fn parse_fill() {
        let _g = lock();
        unsafe {
            // fill defaults to 8 and is only rendered when != 8 (style.c:372).
            let (rc, sy) = parse("fill=magenta");
            assert_eq!(rc, 0);
            assert_eq!(sy.fill, 5);
            assert_eq!(tostring(&raw const sy), "fill=magenta");
        }
    }

    #[test]
    fn parse_ignore_flags() {
        let _g = lock();
        unsafe {
            // ignore / noignore toggle sy->ignore (style.c:103-106). Not rendered.
            let (rc, sy) = parse("ignore");
            assert_eq!(rc, 0);
            assert_eq!(sy.ignore, 1);

            let (rc, sy) = parse("ignore,noignore");
            assert_eq!(rc, 0);
            assert_eq!(sy.ignore, 0);
        }
    }

    #[test]
    fn parse_default_type() {
        let _g = lock();
        unsafe {
            let (rc, sy) = parse("push-default");
            assert_eq!(rc, 0);
            assert!(sy.default_type == style_default_type::STYLE_DEFAULT_PUSH);
            assert_eq!(tostring(&raw const sy), "push-default");

            let (rc, sy) = parse("pop-default");
            assert_eq!(rc, 0);
            assert!(sy.default_type == style_default_type::STYLE_DEFAULT_POP);
            assert_eq!(tostring(&raw const sy), "pop-default");
        }
    }

    #[test]
    fn parse_list() {
        let _g = lock();
        unsafe {
            let (rc, sy) = parse("list=focus");
            assert_eq!(rc, 0);
            assert!(sy.list == style_list::STYLE_LIST_FOCUS);
            assert_eq!(tostring(&raw const sy), "list=focus");

            // nolist resets to STYLE_LIST_OFF (style.c:113).
            let (rc, sy) = parse("list=on,nolist");
            assert_eq!(rc, 0);
            assert!(sy.list == style_list::STYLE_LIST_OFF);
            assert_eq!(tostring(&raw const sy), "default");
        }
    }

    #[test]
    fn parse_range_left_and_window() {
        let _g = lock();
        unsafe {
            let (rc, sy) = parse("range=left");
            assert_eq!(rc, 0);
            assert!(sy.range_type == style_range_type::STYLE_RANGE_LEFT);
            assert_eq!(sy.range_argument, 0);
            assert_eq!(tostring(&raw const sy), "range=left");

            // window|N carries a numeric argument (style.c:170).
            let (rc, sy) = parse("range=window|5");
            assert_eq!(rc, 0);
            assert!(sy.range_type == style_range_type::STYLE_RANGE_WINDOW);
            assert_eq!(sy.range_argument, 5);
            assert_eq!(tostring(&raw const sy), "range=window|5");
        }
    }

    #[test]
    fn parse_range_pane_and_user() {
        let _g = lock();
        unsafe {
            // pane requires a %<n> argument (style.c:148).
            let (rc, sy) = parse("range=pane|%3");
            assert_eq!(rc, 0);
            assert!(sy.range_type == style_range_type::STYLE_RANGE_PANE);
            assert_eq!(sy.range_argument, 3);
            assert_eq!(tostring(&raw const sy), "range=pane|%3");

            // user carries a free-form string copied into range_string
            // (style.c:184 style_set_range_string). Verified via fields; the
            // port's style_tostring for user ranges is not exercised here
            // because it passes the range_string array by value to a variadic
            // snprintf (see module note).
            let (rc, sy) = parse("range=user|myid");
            assert_eq!(rc, 0);
            assert!(sy.range_type == style_range_type::STYLE_RANGE_USER);
            assert_eq!(sy.range_argument, 0);
            assert_eq!(&sy.range_string[..4], b"myid");
            assert_eq!(sy.range_string[4], 0);
        }
    }

    #[test]
    fn norange_resets_range() {
        let _g = lock();
        unsafe {
            // norange restores the default (STYLE_RANGE_NONE) (style.c:126).
            let (rc, sy) = parse("range=left,norange");
            assert_eq!(rc, 0);
            assert!(sy.range_type == style_range_type::STYLE_RANGE_NONE);
            assert_eq!(tostring(&raw const sy), "default");
        }
    }

    #[test]
    fn default_keyword_copies_base_cell() {
        let _g = lock();
        unsafe {
            // "default" copies fg/bg/us/attr/flags from base (style.c:96).
            let mut base = GRID_DEFAULT_CELL;
            base.fg = 2;
            base.bg = 3;
            base.us = 5;
            base.attr = grid_attr::GRID_ATTR_ITALICS;
            let (rc, sy) = parse_with(&raw const base, "default");
            assert_eq!(rc, 0);
            assert_eq!(sy.gc.fg, 2);
            assert_eq!(sy.gc.bg, 3);
            assert_eq!(sy.gc.us, 5);
            assert!(sy.gc.attr.intersects(grid_attr::GRID_ATTR_ITALICS));
        }
    }

    #[test]
    fn whitespace_and_multiple_delimiters() {
        let _g = lock();
        unsafe {
            // Leading/trailing/duplicated delimiters (" ,\n") are skipped
            // (style.c:84 and style.c:291).
            let (rc, sy) = parse("  fg=red , , bg=blue  ");
            assert_eq!(rc, 0);
            assert_eq!(sy.gc.fg, 1);
            assert_eq!(sy.gc.bg, 4);
        }
    }

    #[test]
    fn bad_colour_returns_minus_one() {
        let _g = lock();
        unsafe {
            // colour_fromstring failure -> goto error -> return -1 (style.c:222).
            let (rc, sy) = parse("fg=notacolour");
            assert_eq!(rc, -1);
            // On error the style is restored to its saved copy: fg stays default.
            assert_eq!(sy.gc.fg, 8);
        }
    }

    #[test]
    fn bad_align_and_list_and_attr_return_minus_one() {
        let _g = lock();
        unsafe {
            assert_eq!(parse("align=bogus").0, -1);
            assert_eq!(parse("list=bogus").0, -1);
            assert_eq!(parse("fill=bogus").0, -1);
            // Unknown bare token is treated as an attribute and rejected.
            assert_eq!(parse("definitelynotanattr").0, -1);
        }
    }

    #[test]
    fn error_restores_earlier_tokens() {
        let _g = lock();
        unsafe {
            // "fg=red" succeeds, then the bad token aborts the whole parse and
            // style_copy(sy, &saved) rolls fg back to the default (style.c:296).
            let (rc, sy) = parse("fg=red,notanattr");
            assert_eq!(rc, -1);
            assert_eq!(sy.gc.fg, 8);
        }
    }

    #[test]
    fn parse_order_independent_fields() {
        let _g = lock();
        unsafe {
            // Parsing collects every token regardless of order; the resulting
            // fields match C (vendor/tmux/style.c:83-292). tostring ordering is
            // not asserted here because the port's multi-field render is broken
            // (see module note); we check the parsed state instead.
            let (rc, sy) = parse("bold,bg=blue,fg=red,align=left");
            assert_eq!(rc, 0);
            assert_eq!(sy.gc.fg, 1);
            assert_eq!(sy.gc.bg, 4);
            assert!(sy.gc.attr.intersects(grid_attr::GRID_ATTR_BRIGHT));
            assert!(sy.align == style_align::STYLE_ALIGN_LEFT);
        }
    }

    #[test]
    fn parse_tostring_round_trip_single_field() {
        let _g = lock();
        unsafe {
            // For a single rendered field, tostring is exact, so parse -> render
            // -> parse is a stable round-trip. (Multi-field round-trips are not
            // testable while the port's style_tostring off-by-one stands.)
            for (input, rendered) in [
                ("fg=colour200", "fg=colour200"),
                ("bg=green", "bg=green"),
                ("us=blue", "us=blue"),
                ("align=right", "align=right"),
                ("fill=red", "fill=red"),
                ("underscore", "underscore,"),
            ] {
                let (rc, sy) = parse(input);
                assert_eq!(rc, 0, "parse {input} failed");
                let s1 = tostring(&raw const sy);
                assert_eq!(s1, rendered, "render of {input}");
                let (rc2, sy2) = parse(&s1);
                assert_eq!(rc2, 0, "reparse {s1} failed");
                let s2 = tostring(&raw const sy2);
                assert_eq!(s1, s2, "round-trip of {input}");
            }
        }
    }

    #[test]
    fn parse_range_right() {
        let _g = lock();
        unsafe {
            // range=right takes no argument (style.c:141) and renders "range=right".
            let (rc, sy) = parse("range=right");
            assert_eq!(rc, 0);
            assert!(sy.range_type == style_range_type::STYLE_RANGE_RIGHT);
            assert_eq!(sy.range_argument, 0);
            assert_eq!(tostring(&raw const sy), "range=right");
        }
    }

    #[test]
    fn parse_range_session() {
        let _g = lock();
        unsafe {
            // session requires a $<n> argument (style.c:171); the '$' is consumed
            // and n parsed from the following digits.
            let (rc, sy) = parse("range=session|$7");
            assert_eq!(rc, 0);
            assert!(sy.range_type == style_range_type::STYLE_RANGE_SESSION);
            assert_eq!(sy.range_argument, 7);
            // tostring renders "session|$%u" (style.c:328).
            assert_eq!(tostring(&raw const sy), "range=session|$7");
        }
    }

    #[test]
    fn parse_list_markers() {
        let _g = lock();
        unsafe {
            for (input, want_render, want) in [
                ("list=on", "list=on", style_list::STYLE_LIST_ON),
                (
                    "list=left-marker",
                    "list=left-marker",
                    style_list::STYLE_LIST_LEFT_MARKER,
                ),
                (
                    "list=right-marker",
                    "list=right-marker",
                    style_list::STYLE_LIST_RIGHT_MARKER,
                ),
            ] {
                let (rc, sy) = parse(input);
                assert_eq!(rc, 0, "parse {input}");
                assert!(sy.list == want, "list value for {input}");
                assert_eq!(tostring(&raw const sy), want_render);
            }
        }
    }

    #[test]
    fn parse_dim_reverse_italics_render() {
        let _g = lock();
        unsafe {
            // attributes_tostring emits table order with a trailing comma
            // (vendor/tmux/attributes.c:35). Each single attribute renders alone.
            for (input, render, mask) in [
                ("dim", "dim,", grid_attr::GRID_ATTR_DIM),
                ("reverse", "reverse,", grid_attr::GRID_ATTR_REVERSE),
                ("italics", "italics,", grid_attr::GRID_ATTR_ITALICS),
                (
                    "strikethrough",
                    "strikethrough,",
                    grid_attr::GRID_ATTR_STRIKETHROUGH,
                ),
                ("overline", "overline,", grid_attr::GRID_ATTR_OVERLINE),
            ] {
                let (rc, sy) = parse(input);
                assert_eq!(rc, 0, "parse {input}");
                assert!(sy.gc.attr.intersects(mask), "attr set for {input}");
                assert_eq!(tostring(&raw const sy), render);
            }
        }
    }

    #[test]
    fn no_prefix_clears_one_of_many() {
        let _g = lock();
        unsafe {
            // "no<attr>" only clears its own bit (style.c:254); the others stay.
            let (rc, sy) = parse("bright,dim,italics,nodim");
            assert_eq!(rc, 0);
            assert!(sy.gc.attr.intersects(grid_attr::GRID_ATTR_BRIGHT));
            assert!(!sy.gc.attr.intersects(grid_attr::GRID_ATTR_DIM));
            assert!(sy.gc.attr.intersects(grid_attr::GRID_ATTR_ITALICS));
        }
    }

    #[test]
    fn bg_default_falls_back_to_base() {
        let _g = lock();
        unsafe {
            // bg=default: colour_fromstring == 8, so base->bg is used (style.c:227).
            let mut base = GRID_DEFAULT_CELL;
            base.bg = 4;
            let (rc, sy) = parse_with(&raw const base, "bg=default");
            assert_eq!(rc, 0);
            assert_eq!(sy.gc.bg, 4);
        }
    }

    #[test]
    fn us_default_falls_back_to_base() {
        let _g = lock();
        unsafe {
            // us=default: value 8 -> base->us (style.c:240).
            let mut base = GRID_DEFAULT_CELL;
            base.us = 3;
            let (rc, sy) = parse_with(&raw const base, "us=default");
            assert_eq!(rc, 0);
            assert_eq!(sy.gc.us, 3);
        }
    }

    #[test]
    fn range_pane_requires_percent_prefixed_arg() {
        let _g = lock();
        unsafe {
            // pane needs "%<n>": missing '|' argument (style.c:149) is an error.
            assert_eq!(parse("range=pane").0, -1);
            // present but without a leading '%' is also an error (style.c:152).
            assert_eq!(parse("range=pane|3").0, -1);
            // a bare '%' with no digits is rejected too.
            assert_eq!(parse("range=pane|%").0, -1);
        }
    }

    #[test]
    fn range_session_and_window_bad_args() {
        let _g = lock();
        unsafe {
            // window needs an argument at all (style.c:162): none -> error.
            assert_eq!(parse("range=window").0, -1);
            // session needs "$<n>" (style.c:175): missing '$' -> error.
            assert_eq!(parse("range=session|4").0, -1);
            assert_eq!(parse("range=session").0, -1);
        }
    }

    #[test]
    fn error_restores_range_and_align() {
        let _g = lock();
        unsafe {
            // Valid range + align tokens are applied, then a bad final token
            // aborts and style_copy(sy, &saved) rolls the WHOLE style back to the
            // seed (default range/align) (style.c:296).
            let (rc, sy) = parse("range=left,align=centre,notacolour");
            assert_eq!(rc, -1);
            assert!(sy.range_type == style_range_type::STYLE_RANGE_NONE);
            assert!(sy.align == style_align::STYLE_ALIGN_DEFAULT);
        }
    }

    #[test]
    fn multifield_render_fg_bg_attr() {
        let _g = lock();
        unsafe {
            // tostring order is fill, fg, bg, us, attr (style.c:372-447). Two
            // colours plus an attribute render together (the multi-field
            // truncation bug the module note describes is fixed).
            let (rc, sy) = parse("fg=red,bg=blue,bold");
            assert_eq!(rc, 0);
            assert_eq!(tostring(&raw const sy), "fg=red,bg=blue,bright,");
        }
    }

    #[test]
    fn multifield_render_roundtrip() {
        let _g = lock();
        unsafe {
            // A full render now re-parses to an equal struct.
            let (rc, sy) = parse("align=left,fg=green,bg=colour200,underscore");
            assert_eq!(rc, 0);
            let s1 = tostring(&raw const sy);
            let (rc2, sy2) = parse(&s1);
            assert_eq!(rc2, 0, "reparse {s1}");
            assert_eq!(sy2.gc.fg, sy.gc.fg);
            assert_eq!(sy2.gc.bg, sy.gc.bg);
            assert_eq!(sy2.gc.attr, sy.gc.attr);
            assert!(sy2.align == sy.align);
            assert_eq!(tostring(&raw const sy2), s1);
        }
    }

    #[test]
    fn newline_delimiter_between_tokens() {
        let _g = lock();
        unsafe {
            // '\n' is one of the delimiters " ,\n" (style.c:71).
            let (rc, sy) = parse("fg=red\nbg=blue");
            assert_eq!(rc, 0);
            assert_eq!(sy.gc.fg, 1);
            assert_eq!(sy.gc.bg, 4);
        }
    }

    #[test]
    fn no_attr_on_empty_is_noop() {
        let _g = lock();
        unsafe {
            // Clearing a bit that isn't set leaves attr empty and succeeds
            // (style.c:244 attr &= ~value with value unset).
            let (rc, sy) = parse("nobright");
            assert_eq!(rc, 0);
            assert!(sy.gc.attr.is_empty());
        }
    }

    // --- Known ztmux port divergences (ignored until fixed) ----------------
    //
    // These two assert the CORRECT tmux behavior for the latent bugs the
    // module comment above describes, so they flip to passing once fixed.

    // ztmux BUG: style_tostring truncates any style with 2+ fields to just the
    // first field. xsnprintf__ (src/xmalloc.rs) returns the formatted length
    // INCLUDING the NUL, but style_tostring advances `off` like C's xsnprintf,
    // which EXCLUDES it (vendor/tmux/style.c). So every field after the first is
    // written one byte past an embedded NUL and lost to strlen. Remove #[ignore]
    // once xsnprintf__ returns the length excluding the NUL, like C.
    #[test]
    fn bug_tostring_multifield_truncated() {
        let _g = lock();
        unsafe {
            let mut sy = make_style();
            sy.align = style_align::STYLE_ALIGN_LEFT;
            sy.fill = 1; // red
            // tmux renders both fields; ztmux currently yields only "align=left".
            assert_eq!(tostring(&raw const sy), "align=left,fill=red");
        }
    }

    // ztmux BUG: the range=user path passes the range_string [u8; 16] array BY
    // VALUE to a variadic snprintf("user|%s", ...) instead of a pointer, which is
    // undefined behavior. WARNING: unlike the other ignored bug-tests, this one
    // CRASHES the process (SIGSEGV) when actually run (e.g. via `--ignored`) — it
    // does not fail cleanly, so run it in isolation. Remove #[ignore] once
    // range_string is passed as `.as_ptr()`, matching vendor/tmux/style.c.
    #[test]
    fn bug_tostring_user_range_crashes() {
        let _g = lock();
        unsafe {
            let mut sy = make_style();
            sy.range_type = style_range_type::STYLE_RANGE_USER;
            sy.range_string[..3].copy_from_slice(b"foo");
            assert_eq!(tostring(&raw const sy), "range=user|foo");
        }
    }
}
