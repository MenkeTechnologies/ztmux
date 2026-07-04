// Copyright (c) 2008 Nicholas Marriott <nicholas.marriott@gmail.com>
// Copyright (c) 2016 Avi Halachmi <avihpit@yahoo.com>
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
use std::borrow::Cow;

use crate::*;
use crate::options_::{options, options_array_first, options_array_item_index, options_array_item_value, options_array_next, options_get};

const COLOUR_FLAG_256: i32 = 0x01000000;
const COLOUR_FLAG_RGB: i32 = 0x02000000;
const COLOUR_FLAG_THEME: i32 = 0x04000000;
const COLOUR_THEME_COUNT: u32 = 10;

// Theme colour slots and their server options.
// vendor/tmux/colour.c:29 `static const struct { ... } colour_theme_table[]`.
// The anonymous C struct is (name, dark_option, light_option, terminal_colour);
// only the terminal_colour column is consumed so far (by
// colour_theme_terminal_colour) — the name/option columns are carried verbatim
// for the not-yet-ported theme option lookups (colour_theme_option, etc.).
static COLOUR_THEME_TABLE: [(&str, &str, &str, i32); 10] = [
    ("themeblack", "dark-theme-black", "light-theme-black", 0),
    ("themewhite", "dark-theme-white", "light-theme-white", 7),
    ("themelightgrey", "dark-theme-light-grey", "light-theme-light-grey", 7),
    ("themedarkgrey", "dark-theme-dark-grey", "light-theme-dark-grey", 0),
    ("themegreen", "dark-theme-green", "light-theme-green", 2),
    ("themeyellow", "dark-theme-yellow", "light-theme-yellow", 3),
    ("themered", "dark-theme-red", "light-theme-red", 1),
    ("themeblue", "dark-theme-blue", "light-theme-blue", 4),
    ("themecyan", "dark-theme-cyan", "light-theme-cyan", 6),
    ("thememagenta", "dark-theme-magenta", "light-theme-magenta", 5),
];

/// Get theme terminal colour.
/// C `vendor/tmux/colour.c:101`: `int colour_theme_terminal_colour(u_int n)`
pub fn colour_theme_terminal_colour(n: u32) -> i32 {
    if n as usize >= COLOUR_THEME_TABLE.len() {
        return 8;
    }
    COLOUR_THEME_TABLE[n as usize].3
}

/// C `vendor/tmux/colour.c:109`: `static int colour_dist_sq(int R, int G, int B, int r, int g, int b)`
fn colour_dist_sq(r1: i32, g1: i32, b1: i32, r2: i32, g2: i32, b2: i32) -> i32 {
    (r1 - r2) * (r1 - r2) + (g1 - g2) * (g1 - g2) + (b1 - b2) * (b1 - b2)
}

/// C `vendor/tmux/colour.c:115`: `static int colour_to_6cube(int v)`
fn colour_to_6cube(v: i32) -> i32 {
    if v < 48 {
        0
    } else if v < 114 {
        1
    } else {
        (v - 35) / 40
    }
}

/// Convert an RGB triplet to the xterm(1) 256 colour palette.
///
/// xterm provides a 6x6x6 colour cube (16 - 231) and 24 greys (232 - 255). We
/// map our RGB colour to the closest in the cube, also work out the closest
/// grey, and use the nearest of the two.
///
/// Note that the xterm has much lower resolution for darker colours (they are
/// not evenly spread out), so our 6 levels are not evenly spread: 0x0, 0x5f
/// (95), 0x87 (135), 0xaf (175), 0xd7 (215) and 0xff (255). Greys are more
/// evenly spread (8, 18, 28 ... 238).
/// C `vendor/tmux/colour.c:137`: `int colour_find_rgb(u_char r, u_char g, u_char b)`
pub fn colour_find_rgb(r: u8, g: u8, b: u8) -> i32 {
    // convert to i32 to better match c's integer promotion rules
    let r = r as i32;
    let g = g as i32;
    let b = b as i32;

    const Q2C: [i32; 6] = [0x00, 0x5f, 0x87, 0xaf, 0xd7, 0xff];

    // Map RGB to 6x6x6 cube.
    let qr = colour_to_6cube(r);
    let qg = colour_to_6cube(g);
    let qb = colour_to_6cube(b);
    let cr = Q2C[qr as usize];
    let cg = Q2C[qg as usize];
    let cb = Q2C[qb as usize];

    // If we have hit the colour exactly, return early.
    if cr == r && cg == g && cb == b {
        return (16 + (36 * qr) + (6 * qg) + qb) | COLOUR_FLAG_256;
    }

    // Work out the closest grey (average of RGB).
    let grey_avg = (r + g + b) / 3;
    let grey_idx = if grey_avg > 238 {
        23
    } else {
        (grey_avg - 3) / 10
    };
    let grey = 8 + (10 * grey_idx);

    // Is grey or 6x6x6 colour closest?
    let d = colour_dist_sq(cr, cg, cb, r, g, b);
    let idx = if colour_dist_sq(grey, grey, grey, r, g, b) < d {
        232 + grey_idx
    } else {
        16 + (36 * qr) + (6 * qg) + qb
    };

    idx | COLOUR_FLAG_256
}

/// Join RGB into a colour.
/// C `vendor/tmux/colour.c:171`: `int colour_join_rgb(u_char r, u_char g, u_char b)`
pub fn colour_join_rgb(r: u8, g: u8, b: u8) -> i32 {
    (((r as i32) << 16) | ((g as i32) << 8) | (b as i32)) | COLOUR_FLAG_RGB
}

/// Split colour into RGB.
#[inline]
/// C `vendor/tmux/colour.c:180`: `void colour_split_rgb(int c, u_char *r, u_char *g, u_char *b)`
pub fn colour_split_rgb(c: i32) -> (u8 /* red */, u8 /* green */, u8 /* blue */) {
    (
        ((c >> 16) & 0xff) as u8,
        ((c >> 8) & 0xff) as u8,
        (c & 0xff) as u8,
    )
}

/// Force colour to RGB if not already.
/// C `vendor/tmux/colour.c:189`: `int colour_force_rgb(int c)`
pub fn colour_force_rgb(c: i32) -> i32 {
    if c & COLOUR_FLAG_RGB != 0 {
        c
    } else if c & COLOUR_FLAG_256 != 0 || (0..=7).contains(&c) {
        colour_256_to_rgb(c)
    } else if (90..=97).contains(&c) {
        colour_256_to_rgb(8 + c - 90)
    } else {
        -1
    }
}

/// Convert colour to a string.
/// C `vendor/tmux/colour.c:226`: `const char *colour_tostring(int c)`
pub fn colour_tostring(c: i32) -> Cow<'static, str> {
    if c == -1 {
        return Cow::Borrowed("none");
    }

    if c & COLOUR_FLAG_THEME != 0 {
        let n = c & 0xff;
        if n >= 0 && (n as usize) < COLOUR_THEME_TABLE.len() {
            return Cow::Borrowed(COLOUR_THEME_TABLE[n as usize].0);
        }
        return Cow::Borrowed("invalid");
    }

    if c & COLOUR_FLAG_RGB != 0 {
        let (r, g, b) = colour_split_rgb(c);
        return Cow::Owned(format!("#{r:02x}{g:02x}{b:02x}"));
    }

    if c & COLOUR_FLAG_256 != 0 {
        return Cow::Owned(format!("colour{}", c & 0xff));
    }

    Cow::Borrowed(match c {
        0 => "black",
        1 => "red",
        2 => "green",
        3 => "yellow",
        4 => "blue",
        5 => "magenta",
        6 => "cyan",
        7 => "white",
        8 => "default",
        9 => "terminal",
        90 => "brightblack",
        91 => "brightred",
        92 => "brightgreen",
        93 => "brightyellow",
        94 => "brightblue",
        95 => "brightmagenta",
        96 => "brightcyan",
        97 => "brightwhite",
        _ => "invalid",
    })
}

/// Convert colour to an SGR escape sequence.
/// C `vendor/tmux/colour.c:295`: `const char *colour_toescape(struct client *c, int colour, int bg)`
pub unsafe fn colour_toescape(c: *mut client, mut colour: i32, bg: i32) -> *const u8 {
    use crate::xmalloc::xsnprintf_;
    unsafe {
        static mut S: [u8; 32] = [0; 32];
        let n: i32;
        let mut flags: i32 = (term_flags::TERM_256COLOURS | term_flags::TERM_RGBCOLOURS).bits();
        let o: u32 = if bg != 0 { 40 } else { 30 };

        if !c.is_null()
            && (*c).tty.flags.intersects(tty_flags::TTY_OPENED)
            && !(*c).tty.term.is_null()
        {
            flags = (*(*c).tty.term).flags.bits();
        }

        if colour & COLOUR_FLAG_THEME != 0 {
            n = colour & 0xff;
            if !c.is_null() && (n as u32) < COLOUR_THEME_COUNT {
                colour = (*c).theme_colours[n as usize];
            } else {
                colour = colour_theme_terminal_colour(n as u32);
            }
        }

        if colour == 8 || colour == 9 {
            let _ = xsnprintf_!((&raw mut S).cast(), 32, "\x1b[{}m", o + 9);
            return (&raw const S).cast();
        }

        // Note: TERM_RGBCOLOURS (0x10) and COLOUR_FLAG_RGB (0x02000000) occupy
        // disjoint bits, so `(~flags & TERM_RGBCOLOURS) & (colour &
        // COLOUR_FLAG_RGB)` is always 0 — this branch never fires, matching the
        // vendored tmux (colour_toescape does not down-convert here). Ported
        // verbatim regardless, per the C.
        if (!flags & term_flags::TERM_RGBCOLOURS.bits()) & (colour & COLOUR_FLAG_RGB) != 0 {
            let (r, g, b) = colour_split_rgb(colour);
            colour = colour_find_rgb(r, g, b);
        }
        if (!flags & term_flags::TERM_256COLOURS.bits()) & (colour & COLOUR_FLAG_256) != 0 {
            colour = colour_256to16(colour);
        }

        if colour & COLOUR_FLAG_RGB != 0 {
            let (r, g, b) = colour_split_rgb(colour);
            let _ = xsnprintf_!((&raw mut S).cast(), 32, "\x1b[{};2;{};{};{}m", o + 8, r, g, b);
            return (&raw const S).cast();
        }
        if colour & COLOUR_FLAG_256 != 0 {
            let _ = xsnprintf_!((&raw mut S).cast(), 32, "\x1b[{};5;{}m", o + 8, colour & 0xff);
            return (&raw const S).cast();
        }
        if (0..=7).contains(&colour) {
            let _ = xsnprintf_!((&raw mut S).cast(), 32, "\x1b[{}m", colour + o as i32);
            return (&raw const S).cast();
        }
        if (90..=97).contains(&colour) {
            let _ = xsnprintf_!((&raw mut S).cast(), 32, "\x1b[{}m", colour + o as i32 - 30);
            return (&raw const S).cast();
        }
        null()
    }
}

/// Convert colour from string.
/// C `vendor/tmux/colour.c:387`: `int colour_fromstring(const char *s)`
pub fn colour_fromstring(s: &str) -> i32 {
    // C colour.c:262 `if (*s == '#' && strlen(s) == 7)`: the six chars after '#'
    // must all be hex digits (`for (cp=s+1; isxdigit(*cp); cp++); if (*cp) -1`),
    // then sscanf "%2hhx%2hhx%2hhx". Work on bytes: a 7-*byte* string can hold a
    // multibyte char (e.g. an invalid byte lossily mapped to U+FFFD), so slicing
    // `s[1..3]` would panic on a non-char-boundary. The all-hex check guarantees
    // ASCII before any str slice.
    let b = s.as_bytes();
    if b.first() == Some(&b'#') && b.len() == 7 {
        if !b[1..7].iter().all(u8::is_ascii_hexdigit) {
            return -1;
        }
        let r = u8::from_str_radix(&s[1..3], 16).unwrap();
        let g = u8::from_str_radix(&s[3..5], 16).unwrap();
        let bl = u8::from_str_radix(&s[5..7], 16).unwrap();
        return colour_join_rgb(r, g, bl);
    }

    // Byte-based prefix test (C uses strncasecmp): slicing `s[..6]` panics when
    // byte 6 falls inside a multibyte char, so compare the raw bytes. When the
    // ASCII prefix matches, byte 6/5 is guaranteed a char boundary, so `&s[6..]`
    // is then safe.
    if s.len() > 6 && s.as_bytes()[..6].eq_ignore_ascii_case(b"colour") {
        let Ok(n) = strtonum_(&s[6..], 0i32, 255) else {
            return -1;
        };
        return n | COLOUR_FLAG_256;
    }

    if s.len() > 5 && s.as_bytes()[..5].eq_ignore_ascii_case(b"color") {
        let Ok(n) = strtonum_(&s[5..], 0i32, 255) else {
            return -1;
        };
        return n | COLOUR_FLAG_256;
    }

    match s {
        "0" => return 0,
        "1" => return 1,
        "2" => return 2,
        "3" => return 3,
        "4" => return 4,
        "5" => return 5,
        "6" => return 6,
        "7" => return 7,
        "90" => return 90,
        "91" => return 91,
        "92" => return 92,
        "93" => return 93,
        "94" => return 94,
        "95" => return 95,
        "96" => return 96,
        "97" => return 97,
        _ => (),
    }

    for (colour_name, colour_code) in [
        ("default", 8),
        ("terminal", 9),
        ("black", 0),
        ("red", 1),
        ("green", 2),
        ("yellow", 3),
        ("blue", 4),
        ("magenta", 5),
        ("cyan", 6),
        ("white", 7),
        ("brightblack", 90),
        ("brightred", 91),
        ("brightgreen", 92),
        ("brightyellow", 93),
        ("brightblue", 94),
        ("brightmagenta", 95),
        ("brightcyan", 96),
        ("brightwhite", 97),
    ] {
        if s.eq_ignore_ascii_case(colour_name) {
            return colour_code;
        }
    }

    // C colour.c:424: a theme colour name (themered, themeblue, …) parses to
    // its table index OR'd with COLOUR_FLAG_THEME; resolved to a real colour at
    // render time via the client's theme palette (falling back to the ANSI
    // terminal colour).
    for (i, entry) in COLOUR_THEME_TABLE.iter().enumerate() {
        if s.eq_ignore_ascii_case(entry.0) {
            return (i as i32) | COLOUR_FLAG_THEME;
        }
    }

    colour_byname(s)
}

/// Convert 256 colour to RGB colour.
fn colour_256_to_rgb(c: i32) -> i32 {
    const TABLE: [i32; 256] = [
        0x000000, 0x800000, 0x008000, 0x808000, 0x000080, 0x800080, 0x008080, 0xc0c0c0, 0x808080,
        0xff0000, 0x00ff00, 0xffff00, 0x0000ff, 0xff00ff, 0x00ffff, 0xffffff, 0x000000, 0x00005f,
        0x000087, 0x0000af, 0x0000d7, 0x0000ff, 0x005f00, 0x005f5f, 0x005f87, 0x005faf, 0x005fd7,
        0x005fff, 0x008700, 0x00875f, 0x008787, 0x0087af, 0x0087d7, 0x0087ff, 0x00af00, 0x00af5f,
        0x00af87, 0x00afaf, 0x00afd7, 0x00afff, 0x00d700, 0x00d75f, 0x00d787, 0x00d7af, 0x00d7d7,
        0x00d7ff, 0x00ff00, 0x00ff5f, 0x00ff87, 0x00ffaf, 0x00ffd7, 0x00ffff, 0x5f0000, 0x5f005f,
        0x5f0087, 0x5f00af, 0x5f00d7, 0x5f00ff, 0x5f5f00, 0x5f5f5f, 0x5f5f87, 0x5f5faf, 0x5f5fd7,
        0x5f5fff, 0x5f8700, 0x5f875f, 0x5f8787, 0x5f87af, 0x5f87d7, 0x5f87ff, 0x5faf00, 0x5faf5f,
        0x5faf87, 0x5fafaf, 0x5fafd7, 0x5fafff, 0x5fd700, 0x5fd75f, 0x5fd787, 0x5fd7af, 0x5fd7d7,
        0x5fd7ff, 0x5fff00, 0x5fff5f, 0x5fff87, 0x5fffaf, 0x5fffd7, 0x5fffff, 0x870000, 0x87005f,
        0x870087, 0x8700af, 0x8700d7, 0x8700ff, 0x875f00, 0x875f5f, 0x875f87, 0x875faf, 0x875fd7,
        0x875fff, 0x878700, 0x87875f, 0x878787, 0x8787af, 0x8787d7, 0x8787ff, 0x87af00, 0x87af5f,
        0x87af87, 0x87afaf, 0x87afd7, 0x87afff, 0x87d700, 0x87d75f, 0x87d787, 0x87d7af, 0x87d7d7,
        0x87d7ff, 0x87ff00, 0x87ff5f, 0x87ff87, 0x87ffaf, 0x87ffd7, 0x87ffff, 0xaf0000, 0xaf005f,
        0xaf0087, 0xaf00af, 0xaf00d7, 0xaf00ff, 0xaf5f00, 0xaf5f5f, 0xaf5f87, 0xaf5faf, 0xaf5fd7,
        0xaf5fff, 0xaf8700, 0xaf875f, 0xaf8787, 0xaf87af, 0xaf87d7, 0xaf87ff, 0xafaf00, 0xafaf5f,
        0xafaf87, 0xafafaf, 0xafafd7, 0xafafff, 0xafd700, 0xafd75f, 0xafd787, 0xafd7af, 0xafd7d7,
        0xafd7ff, 0xafff00, 0xafff5f, 0xafff87, 0xafffaf, 0xafffd7, 0xafffff, 0xd70000, 0xd7005f,
        0xd70087, 0xd700af, 0xd700d7, 0xd700ff, 0xd75f00, 0xd75f5f, 0xd75f87, 0xd75faf, 0xd75fd7,
        0xd75fff, 0xd78700, 0xd7875f, 0xd78787, 0xd787af, 0xd787d7, 0xd787ff, 0xd7af00, 0xd7af5f,
        0xd7af87, 0xd7afaf, 0xd7afd7, 0xd7afff, 0xd7d700, 0xd7d75f, 0xd7d787, 0xd7d7af, 0xd7d7d7,
        0xd7d7ff, 0xd7ff00, 0xd7ff5f, 0xd7ff87, 0xd7ffaf, 0xd7ffd7, 0xd7ffff, 0xff0000, 0xff005f,
        0xff0087, 0xff00af, 0xff00d7, 0xff00ff, 0xff5f00, 0xff5f5f, 0xff5f87, 0xff5faf, 0xff5fd7,
        0xff5fff, 0xff8700, 0xff875f, 0xff8787, 0xff87af, 0xff87d7, 0xff87ff, 0xffaf00, 0xffaf5f,
        0xffaf87, 0xffafaf, 0xffafd7, 0xffafff, 0xffd700, 0xffd75f, 0xffd787, 0xffd7af, 0xffd7d7,
        0xffd7ff, 0xffff00, 0xffff5f, 0xffff87, 0xffffaf, 0xffffd7, 0xffffff, 0x080808, 0x121212,
        0x1c1c1c, 0x262626, 0x303030, 0x3a3a3a, 0x444444, 0x4e4e4e, 0x585858, 0x626262, 0x6c6c6c,
        0x767676, 0x808080, 0x8a8a8a, 0x949494, 0x9e9e9e, 0xa8a8a8, 0xb2b2b2, 0xbcbcbc, 0xc6c6c6,
        0xd0d0d0, 0xdadada, 0xe4e4e4, 0xeeeeee,
    ];

    TABLE[c as u8 as usize] | COLOUR_FLAG_RGB
}

/// C `vendor/tmux/colour.c:540`: `int colour_256to16(int c)`
pub fn colour_256to16(c: i32) -> i32 {
    const TABLE: [u8; 256] = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0, 4, 4, 4, 12, 12, 2, 6, 4, 4, 12,
        12, 2, 2, 6, 4, 12, 12, 2, 2, 2, 6, 12, 12, 10, 10, 10, 10, 14, 12, 10, 10, 10, 10, 10, 14,
        1, 5, 4, 4, 12, 12, 3, 8, 4, 4, 12, 12, 2, 2, 6, 4, 12, 12, 2, 2, 2, 6, 12, 12, 10, 10, 10,
        10, 14, 12, 10, 10, 10, 10, 10, 14, 1, 1, 5, 4, 12, 12, 1, 1, 5, 4, 12, 12, 3, 3, 8, 4, 12,
        12, 2, 2, 2, 6, 12, 12, 10, 10, 10, 10, 14, 12, 10, 10, 10, 10, 10, 14, 1, 1, 1, 5, 12, 12,
        1, 1, 1, 5, 12, 12, 1, 1, 1, 5, 12, 12, 3, 3, 3, 7, 12, 12, 10, 10, 10, 10, 14, 12, 10, 10,
        10, 10, 10, 14, 9, 9, 9, 9, 13, 12, 9, 9, 9, 9, 13, 12, 9, 9, 9, 9, 13, 12, 9, 9, 9, 9, 13,
        12, 11, 11, 11, 11, 7, 12, 10, 10, 10, 10, 10, 14, 9, 9, 9, 9, 9, 13, 9, 9, 9, 9, 9, 13, 9,
        9, 9, 9, 9, 13, 9, 9, 9, 9, 9, 13, 9, 9, 9, 9, 9, 13, 11, 11, 11, 11, 11, 15, 0, 0, 0, 0,
        0, 0, 8, 8, 8, 8, 8, 8, 7, 7, 7, 7, 7, 7, 15, 15, 15, 15, 15, 15,
    ];
    TABLE[c as u8 as usize] as i32
}

/// C `vendor/tmux/colour.c:566`: `int colour_byname(const char *name)`
pub fn colour_byname(name: &str) -> i32 {
    const COLOURS: [(&str, i32); 578] = [
        ("AliceBlue", 0xf0f8ff),
        ("AntiqueWhite", 0xfaebd7),
        ("AntiqueWhite1", 0xffefdb),
        ("AntiqueWhite2", 0xeedfcc),
        ("AntiqueWhite3", 0xcdc0b0),
        ("AntiqueWhite4", 0x8b8378),
        ("BlanchedAlmond", 0xffebcd),
        ("BlueViolet", 0x8a2be2),
        ("CadetBlue", 0x5f9ea0),
        ("CadetBlue1", 0x98f5ff),
        ("CadetBlue2", 0x8ee5ee),
        ("CadetBlue3", 0x7ac5cd),
        ("CadetBlue4", 0x53868b),
        ("CornflowerBlue", 0x6495ed),
        ("DarkBlue", 0x00008b),
        ("DarkCyan", 0x008b8b),
        ("DarkGoldenrod", 0xb8860b),
        ("DarkGoldenrod1", 0xffb90f),
        ("DarkGoldenrod2", 0xeead0e),
        ("DarkGoldenrod3", 0xcd950c),
        ("DarkGoldenrod4", 0x8b6508),
        ("DarkGray", 0xa9a9a9),
        ("DarkGreen", 0x006400),
        ("DarkGrey", 0xa9a9a9),
        ("DarkKhaki", 0xbdb76b),
        ("DarkMagenta", 0x8b008b),
        ("DarkOliveGreen", 0x556b2f),
        ("DarkOliveGreen1", 0xcaff70),
        ("DarkOliveGreen2", 0xbcee68),
        ("DarkOliveGreen3", 0xa2cd5a),
        ("DarkOliveGreen4", 0x6e8b3d),
        ("DarkOrange", 0xff8c00),
        ("DarkOrange1", 0xff7f00),
        ("DarkOrange2", 0xee7600),
        ("DarkOrange3", 0xcd6600),
        ("DarkOrange4", 0x8b4500),
        ("DarkOrchid", 0x9932cc),
        ("DarkOrchid1", 0xbf3eff),
        ("DarkOrchid2", 0xb23aee),
        ("DarkOrchid3", 0x9a32cd),
        ("DarkOrchid4", 0x68228b),
        ("DarkRed", 0x8b0000),
        ("DarkSalmon", 0xe9967a),
        ("DarkSeaGreen", 0x8fbc8f),
        ("DarkSeaGreen1", 0xc1ffc1),
        ("DarkSeaGreen2", 0xb4eeb4),
        ("DarkSeaGreen3", 0x9bcd9b),
        ("DarkSeaGreen4", 0x698b69),
        ("DarkSlateBlue", 0x483d8b),
        ("DarkSlateGray", 0x2f4f4f),
        ("DarkSlateGray1", 0x97ffff),
        ("DarkSlateGray2", 0x8deeee),
        ("DarkSlateGray3", 0x79cdcd),
        ("DarkSlateGray4", 0x528b8b),
        ("DarkSlateGrey", 0x2f4f4f),
        ("DarkTurquoise", 0x00ced1),
        ("DarkViolet", 0x9400d3),
        ("DeepPink", 0xff1493),
        ("DeepPink1", 0xff1493),
        ("DeepPink2", 0xee1289),
        ("DeepPink3", 0xcd1076),
        ("DeepPink4", 0x8b0a50),
        ("DeepSkyBlue", 0x00bfff),
        ("DeepSkyBlue1", 0x00bfff),
        ("DeepSkyBlue2", 0x00b2ee),
        ("DeepSkyBlue3", 0x009acd),
        ("DeepSkyBlue4", 0x00688b),
        ("DimGray", 0x696969),
        ("DimGrey", 0x696969),
        ("DodgerBlue", 0x1e90ff),
        ("DodgerBlue1", 0x1e90ff),
        ("DodgerBlue2", 0x1c86ee),
        ("DodgerBlue3", 0x1874cd),
        ("DodgerBlue4", 0x104e8b),
        ("FloralWhite", 0xfffaf0),
        ("ForestGreen", 0x228b22),
        ("GhostWhite", 0xf8f8ff),
        ("GreenYellow", 0xadff2f),
        ("HotPink", 0xff69b4),
        ("HotPink1", 0xff6eb4),
        ("HotPink2", 0xee6aa7),
        ("HotPink3", 0xcd6090),
        ("HotPink4", 0x8b3a62),
        ("IndianRed", 0xcd5c5c),
        ("IndianRed1", 0xff6a6a),
        ("IndianRed2", 0xee6363),
        ("IndianRed3", 0xcd5555),
        ("IndianRed4", 0x8b3a3a),
        ("LavenderBlush", 0xfff0f5),
        ("LavenderBlush1", 0xfff0f5),
        ("LavenderBlush2", 0xeee0e5),
        ("LavenderBlush3", 0xcdc1c5),
        ("LavenderBlush4", 0x8b8386),
        ("LawnGreen", 0x7cfc00),
        ("LemonChiffon", 0xfffacd),
        ("LemonChiffon1", 0xfffacd),
        ("LemonChiffon2", 0xeee9bf),
        ("LemonChiffon3", 0xcdc9a5),
        ("LemonChiffon4", 0x8b8970),
        ("LightBlue", 0xadd8e6),
        ("LightBlue1", 0xbfefff),
        ("LightBlue2", 0xb2dfee),
        ("LightBlue3", 0x9ac0cd),
        ("LightBlue4", 0x68838b),
        ("LightCoral", 0xf08080),
        ("LightCyan", 0xe0ffff),
        ("LightCyan1", 0xe0ffff),
        ("LightCyan2", 0xd1eeee),
        ("LightCyan3", 0xb4cdcd),
        ("LightCyan4", 0x7a8b8b),
        ("LightGoldenrod", 0xeedd82),
        ("LightGoldenrod1", 0xffec8b),
        ("LightGoldenrod2", 0xeedc82),
        ("LightGoldenrod3", 0xcdbe70),
        ("LightGoldenrod4", 0x8b814c),
        ("LightGoldenrodYellow", 0xfafad2),
        ("LightGray", 0xd3d3d3),
        ("LightGreen", 0x90ee90),
        ("LightGrey", 0xd3d3d3),
        ("LightPink", 0xffb6c1),
        ("LightPink1", 0xffaeb9),
        ("LightPink2", 0xeea2ad),
        ("LightPink3", 0xcd8c95),
        ("LightPink4", 0x8b5f65),
        ("LightSalmon", 0xffa07a),
        ("LightSalmon1", 0xffa07a),
        ("LightSalmon2", 0xee9572),
        ("LightSalmon3", 0xcd8162),
        ("LightSalmon4", 0x8b5742),
        ("LightSeaGreen", 0x20b2aa),
        ("LightSkyBlue", 0x87cefa),
        ("LightSkyBlue1", 0xb0e2ff),
        ("LightSkyBlue2", 0xa4d3ee),
        ("LightSkyBlue3", 0x8db6cd),
        ("LightSkyBlue4", 0x607b8b),
        ("LightSlateBlue", 0x8470ff),
        ("LightSlateGray", 0x778899),
        ("LightSlateGrey", 0x778899),
        ("LightSteelBlue", 0xb0c4de),
        ("LightSteelBlue1", 0xcae1ff),
        ("LightSteelBlue2", 0xbcd2ee),
        ("LightSteelBlue3", 0xa2b5cd),
        ("LightSteelBlue4", 0x6e7b8b),
        ("LightYellow", 0xffffe0),
        ("LightYellow1", 0xffffe0),
        ("LightYellow2", 0xeeeed1),
        ("LightYellow3", 0xcdcdb4),
        ("LightYellow4", 0x8b8b7a),
        ("LimeGreen", 0x32cd32),
        ("MediumAquamarine", 0x66cdaa),
        ("MediumBlue", 0x0000cd),
        ("MediumOrchid", 0xba55d3),
        ("MediumOrchid1", 0xe066ff),
        ("MediumOrchid2", 0xd15fee),
        ("MediumOrchid3", 0xb452cd),
        ("MediumOrchid4", 0x7a378b),
        ("MediumPurple", 0x9370db),
        ("MediumPurple1", 0xab82ff),
        ("MediumPurple2", 0x9f79ee),
        ("MediumPurple3", 0x8968cd),
        ("MediumPurple4", 0x5d478b),
        ("MediumSeaGreen", 0x3cb371),
        ("MediumSlateBlue", 0x7b68ee),
        ("MediumSpringGreen", 0x00fa9a),
        ("MediumTurquoise", 0x48d1cc),
        ("MediumVioletRed", 0xc71585),
        ("MidnightBlue", 0x191970),
        ("MintCream", 0xf5fffa),
        ("MistyRose", 0xffe4e1),
        ("MistyRose1", 0xffe4e1),
        ("MistyRose2", 0xeed5d2),
        ("MistyRose3", 0xcdb7b5),
        ("MistyRose4", 0x8b7d7b),
        ("NavajoWhite", 0xffdead),
        ("NavajoWhite1", 0xffdead),
        ("NavajoWhite2", 0xeecfa1),
        ("NavajoWhite3", 0xcdb38b),
        ("NavajoWhite4", 0x8b795e),
        ("NavyBlue", 0x000080),
        ("OldLace", 0xfdf5e6),
        ("OliveDrab", 0x6b8e23),
        ("OliveDrab1", 0xc0ff3e),
        ("OliveDrab2", 0xb3ee3a),
        ("OliveDrab3", 0x9acd32),
        ("OliveDrab4", 0x698b22),
        ("OrangeRed", 0xff4500),
        ("OrangeRed1", 0xff4500),
        ("OrangeRed2", 0xee4000),
        ("OrangeRed3", 0xcd3700),
        ("OrangeRed4", 0x8b2500),
        ("PaleGoldenrod", 0xeee8aa),
        ("PaleGreen", 0x98fb98),
        ("PaleGreen1", 0x9aff9a),
        ("PaleGreen2", 0x90ee90),
        ("PaleGreen3", 0x7ccd7c),
        ("PaleGreen4", 0x548b54),
        ("PaleTurquoise", 0xafeeee),
        ("PaleTurquoise1", 0xbbffff),
        ("PaleTurquoise2", 0xaeeeee),
        ("PaleTurquoise3", 0x96cdcd),
        ("PaleTurquoise4", 0x668b8b),
        ("PaleVioletRed", 0xdb7093),
        ("PaleVioletRed1", 0xff82ab),
        ("PaleVioletRed2", 0xee799f),
        ("PaleVioletRed3", 0xcd6889),
        ("PaleVioletRed4", 0x8b475d),
        ("PapayaWhip", 0xffefd5),
        ("PeachPuff", 0xffdab9),
        ("PeachPuff1", 0xffdab9),
        ("PeachPuff2", 0xeecbad),
        ("PeachPuff3", 0xcdaf95),
        ("PeachPuff4", 0x8b7765),
        ("PowderBlue", 0xb0e0e6),
        ("RebeccaPurple", 0x663399),
        ("RosyBrown", 0xbc8f8f),
        ("RosyBrown1", 0xffc1c1),
        ("RosyBrown2", 0xeeb4b4),
        ("RosyBrown3", 0xcd9b9b),
        ("RosyBrown4", 0x8b6969),
        ("RoyalBlue", 0x4169e1),
        ("RoyalBlue1", 0x4876ff),
        ("RoyalBlue2", 0x436eee),
        ("RoyalBlue3", 0x3a5fcd),
        ("RoyalBlue4", 0x27408b),
        ("SaddleBrown", 0x8b4513),
        ("SandyBrown", 0xf4a460),
        ("SeaGreen", 0x2e8b57),
        ("SeaGreen1", 0x54ff9f),
        ("SeaGreen2", 0x4eee94),
        ("SeaGreen3", 0x43cd80),
        ("SeaGreen4", 0x2e8b57),
        ("SkyBlue", 0x87ceeb),
        ("SkyBlue1", 0x87ceff),
        ("SkyBlue2", 0x7ec0ee),
        ("SkyBlue3", 0x6ca6cd),
        ("SkyBlue4", 0x4a708b),
        ("SlateBlue", 0x6a5acd),
        ("SlateBlue1", 0x836fff),
        ("SlateBlue2", 0x7a67ee),
        ("SlateBlue3", 0x6959cd),
        ("SlateBlue4", 0x473c8b),
        ("SlateGray", 0x708090),
        ("SlateGray1", 0xc6e2ff),
        ("SlateGray2", 0xb9d3ee),
        ("SlateGray3", 0x9fb6cd),
        ("SlateGray4", 0x6c7b8b),
        ("SlateGrey", 0x708090),
        ("SpringGreen", 0x00ff7f),
        ("SpringGreen1", 0x00ff7f),
        ("SpringGreen2", 0x00ee76),
        ("SpringGreen3", 0x00cd66),
        ("SpringGreen4", 0x008b45),
        ("SteelBlue", 0x4682b4),
        ("SteelBlue1", 0x63b8ff),
        ("SteelBlue2", 0x5cacee),
        ("SteelBlue3", 0x4f94cd),
        ("SteelBlue4", 0x36648b),
        ("VioletRed", 0xd02090),
        ("VioletRed1", 0xff3e96),
        ("VioletRed2", 0xee3a8c),
        ("VioletRed3", 0xcd3278),
        ("VioletRed4", 0x8b2252),
        ("WebGray", 0x808080),
        ("WebGreen", 0x008000),
        ("WebGrey", 0x808080),
        ("WebMaroon", 0x800000),
        ("WebPurple", 0x800080),
        ("WhiteSmoke", 0xf5f5f5),
        ("X11Gray", 0xbebebe),
        ("X11Green", 0x00ff00),
        ("X11Grey", 0xbebebe),
        ("X11Maroon", 0xb03060),
        ("X11Purple", 0xa020f0),
        ("YellowGreen", 0x9acd32),
        ("alice blue", 0xf0f8ff),
        ("antique white", 0xfaebd7),
        ("aqua", 0x00ffff),
        ("aquamarine", 0x7fffd4),
        ("aquamarine1", 0x7fffd4),
        ("aquamarine2", 0x76eec6),
        ("aquamarine3", 0x66cdaa),
        ("aquamarine4", 0x458b74),
        ("azure", 0xf0ffff),
        ("azure1", 0xf0ffff),
        ("azure2", 0xe0eeee),
        ("azure3", 0xc1cdcd),
        ("azure4", 0x838b8b),
        ("beige", 0xf5f5dc),
        ("bisque", 0xffe4c4),
        ("bisque1", 0xffe4c4),
        ("bisque2", 0xeed5b7),
        ("bisque3", 0xcdb79e),
        ("bisque4", 0x8b7d6b),
        ("black", 0x000000),
        ("blanched almond", 0xffebcd),
        ("blue violet", 0x8a2be2),
        ("blue", 0x0000ff),
        ("blue1", 0x0000ff),
        ("blue2", 0x0000ee),
        ("blue3", 0x0000cd),
        ("blue4", 0x00008b),
        ("brown", 0xa52a2a),
        ("brown1", 0xff4040),
        ("brown2", 0xee3b3b),
        ("brown3", 0xcd3333),
        ("brown4", 0x8b2323),
        ("burlywood", 0xdeb887),
        ("burlywood1", 0xffd39b),
        ("burlywood2", 0xeec591),
        ("burlywood3", 0xcdaa7d),
        ("burlywood4", 0x8b7355),
        ("cadet blue", 0x5f9ea0),
        ("chartreuse", 0x7fff00),
        ("chartreuse1", 0x7fff00),
        ("chartreuse2", 0x76ee00),
        ("chartreuse3", 0x66cd00),
        ("chartreuse4", 0x458b00),
        ("chocolate", 0xd2691e),
        ("chocolate1", 0xff7f24),
        ("chocolate2", 0xee7621),
        ("chocolate3", 0xcd661d),
        ("chocolate4", 0x8b4513),
        ("coral", 0xff7f50),
        ("coral1", 0xff7256),
        ("coral2", 0xee6a50),
        ("coral3", 0xcd5b45),
        ("coral4", 0x8b3e2f),
        ("cornflower blue", 0x6495ed),
        ("cornsilk", 0xfff8dc),
        ("cornsilk1", 0xfff8dc),
        ("cornsilk2", 0xeee8cd),
        ("cornsilk3", 0xcdc8b1),
        ("cornsilk4", 0x8b8878),
        ("crimson", 0xdc143c),
        ("cyan", 0x00ffff),
        ("cyan1", 0x00ffff),
        ("cyan2", 0x00eeee),
        ("cyan3", 0x00cdcd),
        ("cyan4", 0x008b8b),
        ("dark blue", 0x00008b),
        ("dark cyan", 0x008b8b),
        ("dark goldenrod", 0xb8860b),
        ("dark gray", 0xa9a9a9),
        ("dark green", 0x006400),
        ("dark grey", 0xa9a9a9),
        ("dark khaki", 0xbdb76b),
        ("dark magenta", 0x8b008b),
        ("dark olive green", 0x556b2f),
        ("dark orange", 0xff8c00),
        ("dark orchid", 0x9932cc),
        ("dark red", 0x8b0000),
        ("dark salmon", 0xe9967a),
        ("dark sea green", 0x8fbc8f),
        ("dark slate blue", 0x483d8b),
        ("dark slate gray", 0x2f4f4f),
        ("dark slate grey", 0x2f4f4f),
        ("dark turquoise", 0x00ced1),
        ("dark violet", 0x9400d3),
        ("deep pink", 0xff1493),
        ("deep sky blue", 0x00bfff),
        ("dim gray", 0x696969),
        ("dim grey", 0x696969),
        ("dodger blue", 0x1e90ff),
        ("firebrick", 0xb22222),
        ("firebrick1", 0xff3030),
        ("firebrick2", 0xee2c2c),
        ("firebrick3", 0xcd2626),
        ("firebrick4", 0x8b1a1a),
        ("floral white", 0xfffaf0),
        ("forest green", 0x228b22),
        ("fuchsia", 0xff00ff),
        ("gainsboro", 0xdcdcdc),
        ("ghost white", 0xf8f8ff),
        ("gold", 0xffd700),
        ("gold1", 0xffd700),
        ("gold2", 0xeec900),
        ("gold3", 0xcdad00),
        ("gold4", 0x8b7500),
        ("goldenrod", 0xdaa520),
        ("goldenrod1", 0xffc125),
        ("goldenrod2", 0xeeb422),
        ("goldenrod3", 0xcd9b1d),
        ("goldenrod4", 0x8b6914),
        ("green yellow", 0xadff2f),
        ("green", 0x00ff00),
        ("green1", 0x00ff00),
        ("green2", 0x00ee00),
        ("green3", 0x00cd00),
        ("green4", 0x008b00),
        ("honeydew", 0xf0fff0),
        ("honeydew1", 0xf0fff0),
        ("honeydew2", 0xe0eee0),
        ("honeydew3", 0xc1cdc1),
        ("honeydew4", 0x838b83),
        ("hot pink", 0xff69b4),
        ("indian red", 0xcd5c5c),
        ("indigo", 0x4b0082),
        ("ivory", 0xfffff0),
        ("ivory1", 0xfffff0),
        ("ivory2", 0xeeeee0),
        ("ivory3", 0xcdcdc1),
        ("ivory4", 0x8b8b83),
        ("khaki", 0xf0e68c),
        ("khaki1", 0xfff68f),
        ("khaki2", 0xeee685),
        ("khaki3", 0xcdc673),
        ("khaki4", 0x8b864e),
        ("lavender blush", 0xfff0f5),
        ("lavender", 0xe6e6fa),
        ("lawn green", 0x7cfc00),
        ("lemon chiffon", 0xfffacd),
        ("light blue", 0xadd8e6),
        ("light coral", 0xf08080),
        ("light cyan", 0xe0ffff),
        ("light goldenrod yellow", 0xfafad2),
        ("light goldenrod", 0xeedd82),
        ("light gray", 0xd3d3d3),
        ("light green", 0x90ee90),
        ("light grey", 0xd3d3d3),
        ("light pink", 0xffb6c1),
        ("light salmon", 0xffa07a),
        ("light sea green", 0x20b2aa),
        ("light sky blue", 0x87cefa),
        ("light slate blue", 0x8470ff),
        ("light slate gray", 0x778899),
        ("light slate grey", 0x778899),
        ("light steel blue", 0xb0c4de),
        ("light yellow", 0xffffe0),
        ("lime green", 0x32cd32),
        ("lime", 0x00ff00),
        ("linen", 0xfaf0e6),
        ("magenta", 0xff00ff),
        ("magenta1", 0xff00ff),
        ("magenta2", 0xee00ee),
        ("magenta3", 0xcd00cd),
        ("magenta4", 0x8b008b),
        ("maroon", 0xb03060),
        ("maroon1", 0xff34b3),
        ("maroon2", 0xee30a7),
        ("maroon3", 0xcd2990),
        ("maroon4", 0x8b1c62),
        ("medium aquamarine", 0x66cdaa),
        ("medium blue", 0x0000cd),
        ("medium orchid", 0xba55d3),
        ("medium purple", 0x9370db),
        ("medium sea green", 0x3cb371),
        ("medium slate blue", 0x7b68ee),
        ("medium spring green", 0x00fa9a),
        ("medium turquoise", 0x48d1cc),
        ("medium violet red", 0xc71585),
        ("midnight blue", 0x191970),
        ("mint cream", 0xf5fffa),
        ("misty rose", 0xffe4e1),
        ("moccasin", 0xffe4b5),
        ("navajo white", 0xffdead),
        ("navy blue", 0x000080),
        ("navy", 0x000080),
        ("old lace", 0xfdf5e6),
        ("olive drab", 0x6b8e23),
        ("olive", 0x808000),
        ("orange red", 0xff4500),
        ("orange", 0xffa500),
        ("orange1", 0xffa500),
        ("orange2", 0xee9a00),
        ("orange3", 0xcd8500),
        ("orange4", 0x8b5a00),
        ("orchid", 0xda70d6),
        ("orchid1", 0xff83fa),
        ("orchid2", 0xee7ae9),
        ("orchid3", 0xcd69c9),
        ("orchid4", 0x8b4789),
        ("pale goldenrod", 0xeee8aa),
        ("pale green", 0x98fb98),
        ("pale turquoise", 0xafeeee),
        ("pale violet red", 0xdb7093),
        ("papaya whip", 0xffefd5),
        ("peach puff", 0xffdab9),
        ("peru", 0xcd853f),
        ("pink", 0xffc0cb),
        ("pink1", 0xffb5c5),
        ("pink2", 0xeea9b8),
        ("pink3", 0xcd919e),
        ("pink4", 0x8b636c),
        ("plum", 0xdda0dd),
        ("plum1", 0xffbbff),
        ("plum2", 0xeeaeee),
        ("plum3", 0xcd96cd),
        ("plum4", 0x8b668b),
        ("powder blue", 0xb0e0e6),
        ("purple", 0xa020f0),
        ("purple1", 0x9b30ff),
        ("purple2", 0x912cee),
        ("purple3", 0x7d26cd),
        ("purple4", 0x551a8b),
        ("rebecca purple", 0x663399),
        ("red", 0xff0000),
        ("red1", 0xff0000),
        ("red2", 0xee0000),
        ("red3", 0xcd0000),
        ("red4", 0x8b0000),
        ("rosy brown", 0xbc8f8f),
        ("royal blue", 0x4169e1),
        ("saddle brown", 0x8b4513),
        ("salmon", 0xfa8072),
        ("salmon1", 0xff8c69),
        ("salmon2", 0xee8262),
        ("salmon3", 0xcd7054),
        ("salmon4", 0x8b4c39),
        ("sandy brown", 0xf4a460),
        ("sea green", 0x2e8b57),
        ("seashell", 0xfff5ee),
        ("seashell1", 0xfff5ee),
        ("seashell2", 0xeee5de),
        ("seashell3", 0xcdc5bf),
        ("seashell4", 0x8b8682),
        ("sienna", 0xa0522d),
        ("sienna1", 0xff8247),
        ("sienna2", 0xee7942),
        ("sienna3", 0xcd6839),
        ("sienna4", 0x8b4726),
        ("silver", 0xc0c0c0),
        ("sky blue", 0x87ceeb),
        ("slate blue", 0x6a5acd),
        ("slate gray", 0x708090),
        ("slate grey", 0x708090),
        ("snow", 0xfffafa),
        ("snow1", 0xfffafa),
        ("snow2", 0xeee9e9),
        ("snow3", 0xcdc9c9),
        ("snow4", 0x8b8989),
        ("spring green", 0x00ff7f),
        ("steel blue", 0x4682b4),
        ("tan", 0xd2b48c),
        ("tan1", 0xffa54f),
        ("tan2", 0xee9a49),
        ("tan3", 0xcd853f),
        ("tan4", 0x8b5a2b),
        ("teal", 0x008080),
        ("thistle", 0xd8bfd8),
        ("thistle1", 0xffe1ff),
        ("thistle2", 0xeed2ee),
        ("thistle3", 0xcdb5cd),
        ("thistle4", 0x8b7b8b),
        ("tomato", 0xff6347),
        ("tomato1", 0xff6347),
        ("tomato2", 0xee5c42),
        ("tomato3", 0xcd4f39),
        ("tomato4", 0x8b3626),
        ("turquoise", 0x40e0d0),
        ("turquoise1", 0x00f5ff),
        ("turquoise2", 0x00e5ee),
        ("turquoise3", 0x00c5cd),
        ("turquoise4", 0x00868b),
        ("violet red", 0xd02090),
        ("violet", 0xee82ee),
        ("web gray", 0x808080),
        ("web green", 0x008000),
        ("web grey", 0x808080),
        ("web maroon", 0x800000),
        ("web purple", 0x800080),
        ("wheat", 0xf5deb3),
        ("wheat1", 0xffe7ba),
        ("wheat2", 0xeed8ae),
        ("wheat3", 0xcdba96),
        ("wheat4", 0x8b7e66),
        ("white smoke", 0xf5f5f5),
        ("white", 0xffffff),
        ("x11 gray", 0xbebebe),
        ("x11 green", 0x00ff00),
        ("x11 grey", 0xbebebe),
        ("x11 maroon", 0xb03060),
        ("x11 purple", 0xa020f0),
        ("yellow green", 0x9acd32),
        ("yellow", 0xffff00),
        ("yellow1", 0xffff00),
        ("yellow2", 0xeeee00),
        ("yellow3", 0xcdcd00),
        ("yellow4", 0x8b8b00),
    ];

    // C colour.c:1155: strncasecmp(name, "grey"/"gray", 4) — case-insensitive
    // prefix. A bare "grey"/"gray" (nothing after the prefix) is 0xbebebe; a
    // numeric suffix is a 0-100 percentage scaled by round(2.55 * c) in double
    // precision.
    if name.len() >= 4
        && (name.as_bytes()[..4].eq_ignore_ascii_case(b"grey")
            || name.as_bytes()[..4].eq_ignore_ascii_case(b"gray"))
    {
        if name.len() == 4 {
            return 0xbebebe | COLOUR_FLAG_RGB;
        }

        let Ok(c) = strtonum_(&name[4..], 0, 100) else {
            return -1;
        };
        let c = (2.55f64 * (c as f64)).round() as i32;

        if !(0..=255).contains(&c) {
            return -1;
        }

        let c = c as u8;
        return colour_join_rgb(c, c, c);
    }

    for (color_name, color_hex) in &COLOURS {
        if color_name.eq_ignore_ascii_case(name) {
            return color_hex | COLOUR_FLAG_RGB;
        }
    }

    -1
}

// Replacement palette.
#[repr(C)]
#[derive(Clone)]
pub(crate) struct colour_palette {
    pub(crate) fg: i32,
    pub(crate) bg: i32,

    pub(crate) palette: Option<Box<[i32]>>,
    pub(crate) default_palette: Option<Box<[i32]>>,
}

/// C `vendor/tmux/colour.c:1217`: `void colour_palette_init(struct colour_palette *p)`
pub fn colour_palette_init() -> colour_palette {
    colour_palette {
        fg: 8,
        bg: 8,
        palette: None,
        default_palette: None,
    }
}

/// Clear palette.
/// C `vendor/tmux/colour.c:1227`: `void colour_palette_clear(struct colour_palette *p)`
pub fn colour_palette_clear(p: Option<&mut colour_palette>) {
    if let Some(p) = p {
        p.fg = 8;
        p.bg = 8;
        p.palette.take();
    }
}

/// Free a palette
/// C `vendor/tmux/colour.c:1239`: `void colour_palette_free(struct colour_palette *p)`
pub fn colour_palette_free(p: Option<&mut colour_palette>) {
    if let Some(p) = p {
        p.palette.take();
        p.default_palette.take();
    }
}

/// Get a colour from a palette.
/// C `vendor/tmux/colour.c:1251`: `int colour_palette_get(struct colour_palette *p, int n)`
pub fn colour_palette_get(p: Option<&colour_palette>, mut c: i32) -> i32 {
    let Some(p) = p else {
        return -1;
    };

    if (90..=97).contains(&c) {
        c = 8 + c - 90;
    } else if c & COLOUR_FLAG_256 != 0 {
        c &= !COLOUR_FLAG_256;
    } else if c >= 8 {
        return -1;
    }

    let c = c as usize;

    if let Some(palette) = p.palette.as_ref()
        && palette[c] != -1
    {
        palette[c]
    } else if let Some(default_palette) = p.default_palette.as_ref()
        && default_palette[c] != -1
    {
        default_palette[c]
    } else {
        -1
    }
}

/// C `vendor/tmux/colour.c:1272`: `int colour_palette_set(struct colour_palette *p, int n, int c)`
pub fn colour_palette_set(p: Option<&mut colour_palette>, n: i32, c: i32) -> i32 {
    let Some(p) = p else {
        return 0;
    };
    if n > 255 {
        return 0;
    }

    if c == -1 && p.palette.is_none() {
        return 0;
    }

    if p.palette.is_none() {
        p.palette = Some(vec![-1; 256].into_boxed_slice());
    }
    (p.palette.as_mut().unwrap())[n as usize] = c;

    1
}

/// C `vendor/tmux/colour.c:1293`: `void colour_palette_from_option(struct colour_palette *p, struct options *oo)`
pub unsafe fn colour_palette_from_option(p: Option<&mut colour_palette>, oo: *mut options) {
    unsafe {
        let Some(p) = p else {
            return;
        };

        let o = options_get(&mut *oo, "pane-colours");

        let mut a = options_array_first(o);
        if a.is_null() {
            p.default_palette.take();
            return;
        }

        match &mut p.default_palette {
            None => p.default_palette = Some(vec![-1; 256].into_boxed_slice()),
            Some(palette) => palette.fill(-1),
        }

        while !a.is_null() {
            let n = options_array_item_index(a);
            if n < 256 {
                let c = (*options_array_item_value(a)).number as i32;
                (p.default_palette.as_mut().unwrap())[n as usize] = c;
            }
            a = options_array_next(a);
        }
    }
}

// below has the auto generated code I haven't bothered to translate yet
pub unsafe fn colour_parse_x11(mut p: *const u8) -> i32 {
    unsafe {
        let mut c: f64 = 0.0;
        let mut m: f64 = 0.0;
        let mut y: f64 = 0.0;
        let mut k: f64 = 0.0;

        let mut r: u32 = 0;
        let mut g: u32 = 0;
        let mut b: u32 = 0;

        let mut len = strlen(p);
        let colour: i32;
        let copy: *mut u8;
        if len == 12
            && sscanf(
                p.cast(),
                c"rgb:%02x/%02x/%02x".as_ptr(),
                &raw mut r,
                &raw mut g,
                &raw mut b,
            ) == 3
            || len == 7
                && sscanf(
                    p.cast(),
                    c"#%02x%02x%02x".as_ptr(),
                    &raw mut r,
                    &raw mut g,
                    &raw mut b,
                ) == 3
            || sscanf(
                p.cast(),
                c"%d,%d,%d".as_ptr(),
                &raw mut r,
                &raw mut g,
                &raw mut b,
            ) == 3
        {
            colour = colour_join_rgb(r as u8, g as u8, b as u8);
        } else if len == 18
            && sscanf(
                p.cast(),
                c"rgb:%04x/%04x/%04x".as_ptr(),
                &raw mut r,
                &raw mut g,
                &raw mut b,
            ) == 3
            || len == 13
                && sscanf(
                    p.cast(),
                    c"#%04x%04x%04x".as_ptr(),
                    &raw mut r,
                    &raw mut g,
                    &raw mut b,
                ) == 3
        {
            colour = colour_join_rgb((r >> 8) as u8, (g >> 8) as u8, (b >> 8) as u8);
        } else if (sscanf(
            p.cast(),
            c"cmyk:%lf/%lf/%lf/%lf".as_ptr(),
            &raw mut c,
            &raw mut m,
            &raw mut y,
            &raw mut k,
        ) == 4
            || sscanf(
                p.cast(),
                c"cmy:%lf/%lf/%lf".as_ptr(),
                &raw mut c,
                &raw mut m,
                &raw mut y,
            ) == 3)
            && (0.0..=1.0).contains(&c)
            && (0.0..=1.0).contains(&m)
            && (0.0..=1.0).contains(&y)
            && (0.0..=1.0).contains(&k)
        {
            colour = colour_join_rgb(
                ((1f64 - c) * (1f64 - k) * 255f64) as u8,
                ((1f64 - m) * (1f64 - k) * 255f64) as u8,
                ((1f64 - y) * (1f64 - k) * 255f64) as u8,
            );
        } else {
            while len != 0 && *p == b' ' {
                p = p.add(1);
                len = len.wrapping_sub(1);
            }
            while len != 0 && *p.add(len - 1) == b' ' {
                len = len.wrapping_sub(1);
            }
            copy = xstrndup(p, len).cast().as_ptr();
            colour = colour_byname(cstr_to_str(copy));
            free(copy as _);
        }
        log_debug!(
            "{}: {} = {}",
            "colour_parseX11",
            _s(p),
            colour_tostring(colour)
        );
        colour
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_colour_join_split_rgb_roundtrip() {
        for &(r, g, b) in &[(0xffu8, 0x00u8, 0x00u8), (0x12, 0x34, 0x56), (0xff, 0xff, 0xff)] {
            let c = colour_join_rgb(r, g, b);
            // RGB flag must be set.
            assert_ne!(c & COLOUR_FLAG_RGB, 0);
            assert_eq!(colour_split_rgb(c), (r, g, b));
        }
    }

    #[test]
    fn test_colour_find_rgb_exact() {
        // Pure black maps exactly to cube index 16.
        assert_eq!(colour_find_rgb(0, 0, 0), 16 | COLOUR_FLAG_256);
        // Pure white maps exactly to cube index 231.
        assert_eq!(colour_find_rgb(0xff, 0xff, 0xff), 231 | COLOUR_FLAG_256);
    }

    #[test]
    fn test_colour_force_rgb() {
        // Already-RGB colours are returned unchanged.
        let rgb = colour_join_rgb(1, 2, 3);
        assert_eq!(colour_force_rgb(rgb), rgb);

        // Basic colour 1 (red) -> palette entry 0x800000.
        assert_eq!(colour_force_rgb(1), 0x800000 | COLOUR_FLAG_RGB);

        // Bright colour 90 -> 256 palette entry 8 == 0x808080.
        assert_eq!(colour_force_rgb(90), 0x808080 | COLOUR_FLAG_RGB);

        // Colour 8 (default) cannot be forced to RGB.
        assert_eq!(colour_force_rgb(8), -1);
    }

    #[test]
    fn test_colour_256to16_known() {
        // The first sixteen entries map to themselves.
        for i in 0..16 {
            assert_eq!(colour_256to16(i), i);
        }
        // Cube start (16) folds back to black (0).
        assert_eq!(colour_256to16(16), 0);
    }

    #[test]
    fn test_colour_byname_known() {
        assert_eq!(colour_byname("black"), COLOUR_FLAG_RGB);
        assert_eq!(colour_byname("white"), 0xffffff | COLOUR_FLAG_RGB);
        // Case-insensitive lookup.
        assert_eq!(colour_byname("RED"), 0xff0000 | COLOUR_FLAG_RGB);
        // grey<n> is computed as a scaled grey ramp.
        assert_eq!(colour_byname("grey100"), colour_join_rgb(255, 255, 255));
        // Bare "gray"/"grey" map to 0xbebebe (colour.c:1157); unknown is invalid.
        assert_eq!(colour_byname("gray"), 0xbebebe | COLOUR_FLAG_RGB);
        assert_eq!(colour_byname("notacolour"), -1);
    }

    #[test]
    fn test_colour_fromstring_known() {
        assert_eq!(colour_fromstring("red"), 1);
        assert_eq!(colour_fromstring("brightred"), 91);
        assert_eq!(colour_fromstring("colour123"), 123 | COLOUR_FLAG_256);
        assert_eq!(colour_fromstring("#ff0000"), colour_join_rgb(0xff, 0x00, 0x00));
        assert_eq!(colour_fromstring("default"), 8);
        // "none" is not a named colour and falls through to -1.
        assert_eq!(colour_fromstring("none"), -1);
    }

    #[test]
    fn test_colour_tostring_fromstring_roundtrip() {
        // Named basic colour.
        assert_eq!(colour_tostring(1).as_ref(), "red");
        assert_eq!(colour_fromstring(&colour_tostring(1)), 1);

        // Bright colour.
        assert_eq!(colour_tostring(91).as_ref(), "brightred");
        assert_eq!(colour_fromstring(&colour_tostring(91)), 91);

        // 256-palette colour.
        let c256 = 123 | COLOUR_FLAG_256;
        assert_eq!(colour_tostring(c256).as_ref(), "colour123");
        assert_eq!(colour_fromstring(&colour_tostring(c256)), c256);

        // RGB colour.
        let rgb = colour_join_rgb(0xff, 0x00, 0x00);
        assert_eq!(colour_tostring(rgb).as_ref(), "#ff0000");
        assert_eq!(colour_fromstring(&colour_tostring(rgb)), rgb);

        // default / none.
        assert_eq!(colour_tostring(8).as_ref(), "default");
        assert_eq!(colour_fromstring("default"), 8);
        assert_eq!(colour_tostring(-1).as_ref(), "none");

        // Theme colour: name -> index|COLOUR_FLAG_THEME -> name (C colour.c:424/234).
        assert_eq!(colour_fromstring("themered"), 6 | COLOUR_FLAG_THEME);
        assert_eq!(colour_fromstring("themeblack"), COLOUR_FLAG_THEME);
        assert_eq!(colour_fromstring("thememagenta"), 9 | COLOUR_FLAG_THEME);
        assert_eq!(colour_tostring(6 | COLOUR_FLAG_THEME).as_ref(), "themered");
        assert_eq!(colour_tostring(COLOUR_FLAG_THEME).as_ref(), "themeblack");
        // Case-insensitive parse, like the ANSI names.
        assert_eq!(colour_fromstring("ThemeCyan"), 8 | COLOUR_FLAG_THEME);
        // Not a theme colour: falls through unchanged.
        assert_eq!(colour_fromstring("themebogus"), -1);
    }

    // colour_fromstring accepts both the "colour" and US "color" spellings for
    // 256-palette entries, case-insensitively.
    #[test]
    fn test_colour_fromstring_color_spellings() {
        assert_eq!(colour_fromstring("colour123"), 123 | COLOUR_FLAG_256);
        assert_eq!(colour_fromstring("color123"), 123 | COLOUR_FLAG_256);
        assert_eq!(colour_fromstring("COLOUR5"), 5 | COLOUR_FLAG_256);
        assert_eq!(colour_fromstring("Color0"), COLOUR_FLAG_256);
        // Boundary: 255 is valid, 256 is out of range.
        assert_eq!(colour_fromstring("colour255"), 255 | COLOUR_FLAG_256);
        assert_eq!(colour_fromstring("colour256"), -1);
    }

    // colour_fromstring returns -1 for malformed input, and resolves the bare
    // numeric and "terminal" named forms.
    #[test]
    fn test_colour_fromstring_invalid_and_edge() {
        // Malformed hex / wrong length.
        assert_eq!(colour_fromstring("#gggggg"), -1); // 7 chars, not hex
        assert_eq!(colour_fromstring("#12345"), -1); // 6 chars, too short
        assert_eq!(colour_fromstring("#1234567"), -1); // 8 chars, too long
        // Empty and unknown names.
        assert_eq!(colour_fromstring(""), -1);
        assert_eq!(colour_fromstring("colour"), -1); // prefix with no number
        assert_eq!(colour_fromstring("notacolour"), -1);
        // Bare numeric basic/bright colours.
        assert_eq!(colour_fromstring("0"), 0);
        assert_eq!(colour_fromstring("7"), 7);
        assert_eq!(colour_fromstring("97"), 97);
        // "terminal" is colour 9.
        assert_eq!(colour_fromstring("terminal"), 9);
    }

    // Multibyte input must be rejected with -1, never panic on a non-char-boundary
    // slice. C uses byte ops (strncasecmp / isxdigit / sscanf) that can't panic;
    // the port must match. Found by the fuzz harness: a 7-*byte* string can hold a
    // multibyte char (e.g. an invalid byte lossily mapped to U+FFFD), and both the
    // `#rrggbb` branch and the grey/colour prefix branches previously sliced at a
    // non-boundary.
    #[test]
    fn test_colour_multibyte_never_panics() {
        // 7 bytes where the tail is a 3-byte char → `#` branch must not slice it.
        assert_eq!(colour_fromstring("#72N\u{fffd}"), -1);
        assert_eq!(colour_fromstring("#12\u{20ac}"), -1); // '#','1','2',€ = 6 bytes
        // grey/gray prefix on a string whose byte 4 is inside a multibyte char.
        assert_eq!(colour_byname("gre\u{3d53}"), -1);
        assert_eq!(colour_byname("gra\u{3d53}0"), -1);
        // colour/color prefix likewise.
        assert_eq!(colour_fromstring("colou\u{3d53}"), -1);
        assert_eq!(colour_fromstring("colo\u{3d53}"), -1);
        // A few more arbitrary multibyte strings across the entry points.
        for probe in ["\u{1f600}xyz", "a\u{fffd}b", "#\u{fffd}\u{fffd}", "grey\u{fffd}"] {
            assert_eq!(colour_fromstring(probe), -1);
            let _ = colour_byname(probe); // must not panic
        }
    }

    // colour_256to16 always folds any 256-palette index into the 16-colour
    // range 0..=15 (the lookup table can never yield an out-of-range value).
    #[test]
    fn test_colour_256to16_range_invariant() {
        for c in 0..256 {
            let m = colour_256to16(c);
            assert!((0..=15).contains(&m), "colour_256to16({c}) = {m}");
        }
        // A couple of anchors: last greyscale entry folds to white (15).
        assert_eq!(colour_256to16(255), 15);
    }

    // colour_find_rgb always returns a 256-flagged index in the cube/greyscale
    // range 16..=255 (it never returns a basic 0..15 or an RGB value).
    #[test]
    fn test_colour_find_rgb_range_invariant() {
        for r in (0u16..=255).step_by(51) {
            for g in (0u16..=255).step_by(51) {
                for b in (0u16..=255).step_by(51) {
                    let c = colour_find_rgb(r as u8, g as u8, b as u8);
                    assert_ne!(c & COLOUR_FLAG_256, 0, "rgb {r},{g},{b} -> {c:#x}");
                    let idx = c & 0xff;
                    assert!(
                        (16..=255).contains(&idx),
                        "rgb {r},{g},{b} -> idx {idx}"
                    );
                }
            }
        }
    }

    // colour_to_6cube maps a channel value to one of six cube levels
    // (colour.c:113). The two thresholds (48, 114) and the (v-35)/40 formula
    // above 114 give non-uniform boundaries.
    #[test]
    fn test_colour_to_6cube_boundaries() {
        assert_eq!(colour_to_6cube(0), 0);
        assert_eq!(colour_to_6cube(47), 0);
        assert_eq!(colour_to_6cube(48), 1);
        assert_eq!(colour_to_6cube(113), 1);
        assert_eq!(colour_to_6cube(114), 1); // 114 !< 114, (114-35)/40 == 1
        assert_eq!(colour_to_6cube(115), 2);
        assert_eq!(colour_to_6cube(154), 2);
        assert_eq!(colour_to_6cube(155), 3);
        assert_eq!(colour_to_6cube(255), 5);
    }

    // colour_dist_sq is the squared Euclidean distance between two RGB points
    // (colour.c:108). Symmetric, zero on identity.
    #[test]
    fn test_colour_dist_sq() {
        assert_eq!(colour_dist_sq(0, 0, 0, 0, 0, 0), 0);
        assert_eq!(colour_dist_sq(255, 0, 0, 0, 0, 0), 255 * 255);
        // (3,4,0) distance from origin squared = 9 + 16 = 25.
        assert_eq!(colour_dist_sq(3, 4, 0, 0, 0, 0), 25);
        // Symmetric in the two triplets.
        assert_eq!(
            colour_dist_sq(10, 20, 30, 40, 50, 60),
            colour_dist_sq(40, 50, 60, 10, 20, 30)
        );
    }

    // colour_256_to_rgb resolves a 256-palette index to its RGB value with the
    // RGB flag set (colour.c:196 table). Anchors: basic 0/8/15, cube start/end,
    // greyscale ends.
    #[test]
    fn test_colour_256_to_rgb_anchors() {
        assert_eq!(colour_256_to_rgb(0), COLOUR_FLAG_RGB);
        assert_eq!(colour_256_to_rgb(7), 0xc0c0c0 | COLOUR_FLAG_RGB);
        assert_eq!(colour_256_to_rgb(15), 0xffffff | COLOUR_FLAG_RGB);
        assert_eq!(colour_256_to_rgb(16), COLOUR_FLAG_RGB); // cube start
        assert_eq!(colour_256_to_rgb(231), 0xffffff | COLOUR_FLAG_RGB); // cube end
        assert_eq!(colour_256_to_rgb(232), 0x080808 | COLOUR_FLAG_RGB); // grey start
        assert_eq!(colour_256_to_rgb(255), 0xeeeeee | COLOUR_FLAG_RGB); // grey end
    }

    // colour_find_rgb: exact hits on cube corners return the cube index early
    // (colour.c:150). Each of the six q2c levels along the diagonal is an exact
    // grey-of-cube point.
    #[test]
    fn test_colour_find_rgb_cube_diagonal() {
        // (0x5f,0x5f,0x5f): q = (1,1,1) -> 16+36+6+1 = 59.
        assert_eq!(colour_find_rgb(0x5f, 0x5f, 0x5f), 59 | COLOUR_FLAG_256);
        // (0x87,0x87,0x87): q = (2,2,2) -> 16+72+12+2 = 102.
        assert_eq!(colour_find_rgb(0x87, 0x87, 0x87), 102 | COLOUR_FLAG_256);
        // (0xd7,0x00,0x00): q = (4,0,0) -> 16+144 = 160.
        assert_eq!(colour_find_rgb(0xd7, 0x00, 0x00), 160 | COLOUR_FLAG_256);
    }

    // colour_find_rgb: a mid grey that is not a cube corner should snap to the
    // greyscale ramp (232..255) rather than the cube (colour.c:160). 128,128,128
    // -> grey_idx (128-3)/10 = 12 -> 232+12 = 244.
    #[test]
    fn test_colour_find_rgb_greyscale_branch() {
        assert_eq!(colour_find_rgb(128, 128, 128), 244 | COLOUR_FLAG_256);
    }

    // colour_theme_terminal_colour indexes the theme table's terminal_colour
    // column, returning 8 for out-of-range slots (colour.c:99).
    #[test]
    fn test_colour_theme_terminal_colour() {
        assert_eq!(colour_theme_terminal_colour(0), 0); // themeblack
        assert_eq!(colour_theme_terminal_colour(1), 7); // themewhite
        assert_eq!(colour_theme_terminal_colour(4), 2); // themegreen
        assert_eq!(colour_theme_terminal_colour(9), 5); // thememagenta
        // Out of range -> 8.
        assert_eq!(colour_theme_terminal_colour(10), 8);
        assert_eq!(colour_theme_terminal_colour(1000), 8);
    }

    // colour_palette_init sets fg/bg to 8 (default) and leaves both slices None
    // (colour.c:1215).
    #[test]
    fn test_colour_palette_init() {
        let p = colour_palette_init();
        assert_eq!(p.fg, 8);
        assert_eq!(p.bg, 8);
        assert!(p.palette.is_none());
        assert!(p.default_palette.is_none());
    }

    // colour_palette_set lazily allocates the 256-slot palette, stores the
    // colour, and colour_palette_get reads it back (colour.c:1249, 1272).
    #[test]
    fn test_colour_palette_set_get_roundtrip() {
        let mut p = colour_palette_init();
        // Setting a real colour into an unallocated palette returns 1 and
        // allocates it.
        assert_eq!(colour_palette_set(Some(&mut p), 5, 0xabcdef | COLOUR_FLAG_RGB), 1);
        assert!(p.palette.is_some());
        assert_eq!(colour_palette_get(Some(&p), 5), 0xabcdef | COLOUR_FLAG_RGB);
        // A 256-flagged query strips the flag before indexing, so it reads the
        // same slot 5.
        assert_eq!(
            colour_palette_get(Some(&p), 5 | COLOUR_FLAG_256),
            0xabcdef | COLOUR_FLAG_RGB
        );
        // An unset slot yields -1.
        assert_eq!(colour_palette_get(Some(&p), 6), -1);
    }

    // colour_palette_get remaps bright colours 90..97 to slots 8..15 before
    // lookup (colour.c:1255).
    #[test]
    fn test_colour_palette_get_bright_remap() {
        let mut p = colour_palette_init();
        // Bright black (90) reads slot 8.
        colour_palette_set(Some(&mut p), 8, 0x111111 | COLOUR_FLAG_RGB);
        assert_eq!(colour_palette_get(Some(&p), 90), 0x111111 | COLOUR_FLAG_RGB);
        // Bright white (97) reads slot 15.
        colour_palette_set(Some(&mut p), 15, 0x222222 | COLOUR_FLAG_RGB);
        assert_eq!(colour_palette_get(Some(&p), 97), 0x222222 | COLOUR_FLAG_RGB);
    }

    // colour_palette_get returns -1 for a plain colour >= 8 that is neither a
    // bright (90..97) nor 256-flagged value (colour.c:1259 `else if (n >= 8)`).
    // This includes the default colour 8 itself.
    #[test]
    fn test_colour_palette_get_high_plain_is_none() {
        let mut p = colour_palette_init();
        colour_palette_set(Some(&mut p), 8, 0x333333);
        // 8 hits the `n >= 8` early-out and never reaches the slice, so -1 even
        // though slot 8 was populated.
        assert_eq!(colour_palette_get(Some(&p), 8), -1);
        assert_eq!(colour_palette_get(Some(&p), 100), -1);
        // A NULL palette is always -1.
        assert_eq!(colour_palette_get(None, 5), -1);
    }

    // colour_palette_set guards: n > 255 is rejected, and clearing (c == -1)
    // an unallocated palette is a no-op returning 0 (colour.c:1276).
    #[test]
    fn test_colour_palette_set_guards() {
        let mut p = colour_palette_init();
        assert_eq!(colour_palette_set(Some(&mut p), 256, 1), 0);
        assert_eq!(colour_palette_set(Some(&mut p), 1000, 1), 0);
        // Clearing when no palette exists yet: nothing to do, returns 0, stays None.
        assert_eq!(colour_palette_set(Some(&mut p), 5, -1), 0);
        assert!(p.palette.is_none());
        // A NULL palette returns 0.
        assert_eq!(colour_palette_set(None, 5, 1), 0);
    }

    // colour_palette_clear resets fg/bg to 8 and drops the palette, leaving the
    // default_palette intact (colour.c:1225).
    #[test]
    fn test_colour_palette_clear() {
        let mut p = colour_palette_init();
        p.fg = 1;
        p.bg = 2;
        colour_palette_set(Some(&mut p), 3, 0x999999);
        p.default_palette = Some(vec![-1; 256].into_boxed_slice());
        colour_palette_clear(Some(&mut p));
        assert_eq!(p.fg, 8);
        assert_eq!(p.bg, 8);
        assert!(p.palette.is_none());
        // clear does not touch default_palette.
        assert!(p.default_palette.is_some());
    }

    // colour_join_rgb packs r,g,b into the low 24 bits and sets the RGB flag
    // (colour.c:169). Verify the exact bit layout, not just a round-trip.
    #[test]
    fn test_colour_join_rgb_bit_layout() {
        let c = colour_join_rgb(0x12, 0x34, 0x56);
        assert_eq!(c & 0xffffff, 0x123456);
        assert_ne!(c & COLOUR_FLAG_RGB, 0);
        assert_eq!(c & COLOUR_FLAG_256, 0);
        // Black is all-zero bits plus the flag.
        assert_eq!(colour_join_rgb(0, 0, 0), COLOUR_FLAG_RGB);
    }

    // colour_force_rgb resolves basic (0..7) and bright (90..97) colours via the
    // 256 table, and refuses the special colours 8/9 (colour.c:187).
    #[test]
    fn test_colour_force_rgb_ranges() {
        assert_eq!(colour_force_rgb(0), COLOUR_FLAG_RGB);
        assert_eq!(colour_force_rgb(7), 0xc0c0c0 | COLOUR_FLAG_RGB);
        // Bright white 97 -> table entry 8 + 97 - 90 = 15 -> 0xffffff.
        assert_eq!(colour_force_rgb(97), 0xffffff | COLOUR_FLAG_RGB);
        // A 256-flagged colour resolves through the table (flag already RGB'd).
        assert_ne!(colour_force_rgb(200 | COLOUR_FLAG_256) & COLOUR_FLAG_RGB, 0);
        // 9 (terminal) and any other out-of-range value cannot be forced.
        assert_eq!(colour_force_rgb(9), -1);
        assert_eq!(colour_force_rgb(100), -1);
    }

    // colour_256to16 anchors across cube and greyscale (colour.c:538 table).
    #[test]
    fn test_colour_256to16_more_anchors() {
        // Bright range 8..15 map to themselves.
        for i in 8..16 {
            assert_eq!(colour_256to16(i), i);
        }
        // Last cube entry (231, pure white) folds to bright white 15.
        assert_eq!(colour_256to16(231), 15);
        // First greyscale entry (232, near-black) folds to black 0.
        assert_eq!(colour_256to16(232), 0);
    }

    // colour_tostring names the special colours 8 (default) and 9 (terminal),
    // and returns "invalid" for an unflagged value with no name (colour.c:224).
    #[test]
    fn test_colour_tostring_special_and_invalid() {
        assert_eq!(colour_tostring(8).as_ref(), "default");
        assert_eq!(colour_tostring(9).as_ref(), "terminal");
        assert_eq!(colour_tostring(0).as_ref(), "black");
        assert_eq!(colour_tostring(96).as_ref(), "brightcyan");
        // Unflagged, unnamed value -> "invalid".
        assert_eq!(colour_tostring(100).as_ref(), "invalid");
        // 256-flagged low value renders as colourN.
        assert_eq!(colour_tostring(COLOUR_FLAG_256).as_ref(), "colour0");
    }

    // colour_fromstring parses uppercase hex and round-trips through tostring
    // (colour.c:385). from_str_radix is case-insensitive on the digits.
    #[test]
    fn test_colour_fromstring_hex_uppercase() {
        let c = colour_fromstring("#FF00AA");
        assert_eq!(c, colour_join_rgb(0xff, 0x00, 0xaa));
        // tostring always lowercases the hex.
        assert_eq!(colour_tostring(c).as_ref(), "#ff00aa");
        assert_eq!(colour_fromstring(&colour_tostring(c)), c);
    }

    // colour_byname grey ramp (colour.c:1155): "grey"/"gray" matched
    // case-insensitively; bare word -> 0xbebebe; numeric suffix scaled by
    // round(2.55 * c) in double precision.
    #[test]
    fn test_colour_byname_grey_ramp() {
        // grey0 -> black, grey100 -> white; both agree with C (no .5 rounding).
        assert_eq!(colour_byname("grey0"), colour_join_rgb(0, 0, 0));
        assert_eq!(colour_byname("gray100"), colour_join_rgb(255, 255, 255));
        // Grey ramp invariant: r == g == b for every valid grey<n>.
        for n in [0u32, 10, 20, 30, 40, 60, 80, 100] {
            let c = colour_byname(&format!("grey{n}"));
            let (r, g, b) = colour_split_rgb(c);
            assert_eq!((r, g), (g, b), "grey{n} not a true grey: {r},{g},{b}");
        }
        // Case-insensitive prefix: bare "grey"/"gray" -> 0xbebebe (colour.c:1157).
        assert_eq!(colour_byname("grey"), 0xbebebe | COLOUR_FLAG_RGB);
        assert_eq!(colour_byname("gray"), 0xbebebe | COLOUR_FLAG_RGB);
        // Uppercase prefix still matches; GREY50 -> round(2.55*50)=127.
        let g50 = (2.55f64 * 50.0).round() as u8;
        assert_eq!(colour_byname("GREY50"), colour_join_rgb(g50, g50, g50));
        // Out-of-range percentage (> 100) is rejected by strtonum bounds.
        assert_eq!(colour_byname("grey200"), -1);
    }

    // colour_byname resolves the X11/web colour table (colour.c:566) case
    // insensitively, always setting COLOUR_FLAG_RGB. Spot-check the common web
    // aliases that also appear as CSS keywords.
    #[test]
    fn test_colour_byname_web_colours() {
        for (name, hex) in [
            ("aqua", 0x00ffff),
            ("fuchsia", 0xff00ff),
            ("lime", 0x00ff00),
            ("navy", 0x000080),
            ("olive", 0x808000),
            ("teal", 0x008080),
            ("silver", 0xc0c0c0),
            ("maroon", 0xb03060),
        ] {
            assert_eq!(colour_byname(name), hex | COLOUR_FLAG_RGB, "name = {name}");
            // Case-insensitive (strcasecmp in C).
            assert_eq!(
                colour_byname(&name.to_uppercase()),
                hex | COLOUR_FLAG_RGB,
                "name = {name} (upper)"
            );
        }
        // A name with an embedded space is still an exact table key.
        assert_eq!(colour_byname("navy blue"), 0x000080 | COLOUR_FLAG_RGB);
    }

    // colour_split_rgb extracts the three channels from the low 24 bits and is
    // indifferent to the flag bits above them (colour.c:130). join then split is
    // the identity for arbitrary channel triples.
    #[test]
    fn test_colour_split_rgb_ignores_flags() {
        for &(r, g, b) in &[(0u8, 0u8, 0u8), (0x12, 0x34, 0x56), (0xde, 0xad, 0xbe), (0xff, 0xff, 0xff)] {
            let c = colour_join_rgb(r, g, b);
            assert_eq!(colour_split_rgb(c), (r, g, b));
            // Setting the 256 flag as well must not perturb the channel bytes.
            assert_eq!(colour_split_rgb(c | COLOUR_FLAG_256), (r, g, b));
        }
    }

    // colour_fromstring only accepts bare numerics 0..7 and 90..97 (colour.c:293
    // switch). A bare "8"/"9" or any two-digit value outside 90..97 is NOT a
    // named colour and falls through to colour_byname, which rejects it (-1).
    // Note 8/9 are reachable only via the words "default"/"terminal".
    #[test]
    fn test_colour_fromstring_bare_numeric_gaps() {
        assert_eq!(colour_fromstring("8"), -1);
        assert_eq!(colour_fromstring("9"), -1);
        assert_eq!(colour_fromstring("10"), -1);
        assert_eq!(colour_fromstring("89"), -1);
        assert_eq!(colour_fromstring("98"), -1);
        // The words still resolve to 8 and 9.
        assert_eq!(colour_fromstring("default"), 8);
        assert_eq!(colour_fromstring("terminal"), 9);
    }

    // Every basic (0..7) and bright (90..97) colour tostring-names must parse
    // back to the same code (colour.c:224 names, :262 fromstring). Cross-checks
    // the two hard-coded name tables against each other.
    #[test]
    fn test_colour_all_named_basic_bright_roundtrip() {
        for c in (0..=7).chain(90..=97) {
            let name = colour_tostring(c);
            assert_ne!(name.as_ref(), "invalid", "colour {c} unnamed");
            assert_eq!(colour_fromstring(&name), c, "roundtrip {c} via {name}");
        }
    }
}
