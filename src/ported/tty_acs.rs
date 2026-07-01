// Copyright (c) 2010 Nicholas Marriott <nicholas.marriott@gmail.com>
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

#[repr(C)]
pub struct tty_acs_entry {
    pub key: u8,
    pub string: &'static [u8; 4],
}
impl tty_acs_entry {
    #[expect(clippy::trivially_copy_pass_by_ref, reason = "false positive")]
    pub const fn new(key: u8, string: &'static [u8; 4]) -> Self {
        Self { key, string }
    }
}

static TTY_ACS_TABLE: [tty_acs_entry; 36] = [
    tty_acs_entry::new(b'+', &[0o342, 0o206, 0o222, 0o000]), // arrow pointing right
    tty_acs_entry::new(b',', &[0o342, 0o206, 0o220, 0o000]), // arrow pointing left
    tty_acs_entry::new(b'-', &[0o342, 0o206, 0o221, 0o000]), // arrow pointing up
    tty_acs_entry::new(b'.', &[0o342, 0o206, 0o223, 0o000]), // arrow pointing down
    tty_acs_entry::new(b'0', &[0o342, 0o226, 0o256, 0o000]), // solid square block
    tty_acs_entry::new(b'`', &[0o342, 0o227, 0o206, 0o000]), // diamond
    tty_acs_entry::new(b'a', &[0o342, 0o226, 0o222, 0o000]), // checker board (stipple)
    tty_acs_entry::new(b'b', &[0o342, 0o220, 0o211, 0o000]),
    tty_acs_entry::new(b'c', &[0o342, 0o220, 0o214, 0o000]),
    tty_acs_entry::new(b'd', &[0o342, 0o220, 0o215, 0o000]),
    tty_acs_entry::new(b'e', &[0o342, 0o220, 0o212, 0o000]),
    tty_acs_entry::new(b'f', &[0o302, 0o260, 0o000, 0o000]), // degree symbol
    tty_acs_entry::new(b'g', &[0o302, 0o261, 0o000, 0o000]), // plus/minus
    tty_acs_entry::new(b'h', &[0o342, 0o220, 0o244, 0o000]),
    tty_acs_entry::new(b'i', &[0o342, 0o220, 0o213, 0o000]),
    tty_acs_entry::new(b'j', &[0o342, 0o224, 0o230, 0o000]), // lower right corner
    tty_acs_entry::new(b'k', &[0o342, 0o224, 0o220, 0o000]), // upper right corner
    tty_acs_entry::new(b'l', &[0o342, 0o224, 0o214, 0o000]), // upper left corner
    tty_acs_entry::new(b'm', &[0o342, 0o224, 0o224, 0o000]), // lower left corner
    tty_acs_entry::new(b'n', &[0o342, 0o224, 0o274, 0o000]), // large plus or crossover
    tty_acs_entry::new(b'o', &[0o342, 0o216, 0o272, 0o000]), // scan line 1
    tty_acs_entry::new(b'p', &[0o342, 0o216, 0o273, 0o000]), // scan line 3
    tty_acs_entry::new(b'q', &[0o342, 0o224, 0o200, 0o000]), // horizontal line
    tty_acs_entry::new(b'r', &[0o342, 0o216, 0o274, 0o000]), // scan line 7
    tty_acs_entry::new(b's', &[0o342, 0o216, 0o275, 0o000]), // scan line 9
    tty_acs_entry::new(b't', &[0o342, 0o224, 0o234, 0o000]), // tee pointing right
    tty_acs_entry::new(b'u', &[0o342, 0o224, 0o244, 0o000]), // tee pointing left
    tty_acs_entry::new(b'v', &[0o342, 0o224, 0o264, 0o000]), // tee pointing up
    tty_acs_entry::new(b'w', &[0o342, 0o224, 0o254, 0o000]), // tee pointing down
    tty_acs_entry::new(b'x', &[0o342, 0o224, 0o202, 0o000]), // vertical line
    tty_acs_entry::new(b'y', &[0o342, 0o211, 0o244, 0o000]), // less-than-or-equal-to
    tty_acs_entry::new(b'z', &[0o342, 0o211, 0o245, 0o000]), // greater-than-or-equal-to
    tty_acs_entry::new(b'{', &[0o317, 0o200, 0o000, 0o000]), // greek pi
    tty_acs_entry::new(b'|', &[0o342, 0o211, 0o240, 0o000]), // not-equal
    tty_acs_entry::new(b'}', &[0o302, 0o243, 0o000, 0o000]), // UK pound sign
    tty_acs_entry::new(b'~', &[0o302, 0o267, 0o000, 0o000]), // bullet
];

#[repr(C)]
pub struct tty_acs_reverse_entry {
    pub string: &'static [u8; 4],
    pub key: u8,
}
impl tty_acs_reverse_entry {
    #[expect(clippy::trivially_copy_pass_by_ref, reason = "false positive")]
    const fn new(string: &'static [u8; 4], key: u8) -> Self {
        Self { string, key }
    }
}

static TTY_ACS_REVERSE2: [tty_acs_reverse_entry; 1] = [tty_acs_reverse_entry::new(
    &[0o302, 0o267, 0o000, 0o000],
    b'~',
)];

static TTY_ACS_REVERSE3: [tty_acs_reverse_entry; 32] = [
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o200, 0o000], b'q'),
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o201, 0o000], b'q'),
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o202, 0o000], b'x'),
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o203, 0o000], b'x'),
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o214, 0o000], b'l'),
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o217, 0o000], b'k'),
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o220, 0o000], b'k'),
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o223, 0o000], b'l'),
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o224, 0o000], b'm'),
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o227, 0o000], b'm'),
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o230, 0o000], b'j'),
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o233, 0o000], b'j'),
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o234, 0o000], b't'),
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o243, 0o000], b't'),
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o244, 0o000], b'u'),
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o253, 0o000], b'u'),
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o263, 0o000], b'w'),
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o264, 0o000], b'v'),
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o273, 0o000], b'v'),
    tty_acs_reverse_entry::new(&[0o342, 0o224, 0o274, 0o000], b'n'),
    tty_acs_reverse_entry::new(&[0o342, 0o225, 0o213, 0o000], b'n'),
    tty_acs_reverse_entry::new(&[0o342, 0o225, 0o220, 0o000], b'q'),
    tty_acs_reverse_entry::new(&[0o342, 0o225, 0o221, 0o000], b'x'),
    tty_acs_reverse_entry::new(&[0o342, 0o225, 0o224, 0o000], b'l'),
    tty_acs_reverse_entry::new(&[0o342, 0o225, 0o227, 0o000], b'k'),
    tty_acs_reverse_entry::new(&[0o342, 0o225, 0o232, 0o000], b'm'),
    tty_acs_reverse_entry::new(&[0o342, 0o225, 0o235, 0o000], b'j'),
    tty_acs_reverse_entry::new(&[0o342, 0o225, 0o240, 0o000], b't'),
    tty_acs_reverse_entry::new(&[0o342, 0o225, 0o243, 0o000], b'u'),
    tty_acs_reverse_entry::new(&[0o342, 0o225, 0o246, 0o000], b'w'),
    tty_acs_reverse_entry::new(&[0o342, 0o225, 0o251, 0o000], b'v'),
    tty_acs_reverse_entry::new(&[0o342, 0o225, 0o254, 0o000], b'n'),
];

/// UTF-8 double borders.
static TTY_ACS_DOUBLE_BORDERS_LIST: [utf8_data; 13] = [
    utf8_data::new([0o000, 0o000, 0o000, 0o000], 0, 0, 0),
    utf8_data::new([0o342, 0o225, 0o221, 0o000], 0, 3, 1), // U+2551
    utf8_data::new([0o342, 0o225, 0o220, 0o000], 0, 3, 1), // U+2550
    utf8_data::new([0o342, 0o225, 0o224, 0o000], 0, 3, 1), // U+2554
    utf8_data::new([0o342, 0o225, 0o227, 0o000], 0, 3, 1), // U+2557
    utf8_data::new([0o342, 0o225, 0o232, 0o000], 0, 3, 1), // U+255A
    utf8_data::new([0o342, 0o225, 0o235, 0o000], 0, 3, 1), // U+255D
    utf8_data::new([0o342, 0o225, 0o246, 0o000], 0, 3, 1), // U+2566
    utf8_data::new([0o342, 0o225, 0o251, 0o000], 0, 3, 1), // U+2569
    utf8_data::new([0o342, 0o225, 0o240, 0o000], 0, 3, 1), // U+2560
    utf8_data::new([0o342, 0o225, 0o243, 0o000], 0, 3, 1), // U+2563
    utf8_data::new([0o342, 0o225, 0o254, 0o000], 0, 3, 1), // U+256C
    utf8_data::new([0o302, 0o267, 0o000, 0o000], 0, 2, 1), // U+00B7
];

/// UTF-8 heavy borders.
static TTY_ACS_HEAVY_BORDERS_LIST: [utf8_data; 13] = [
    utf8_data::new([0o000, 0o000, 0o000, 0o000], 0, 0, 0),
    utf8_data::new([0o342, 0o224, 0o203, 0o000], 0, 3, 1), // U+2503
    utf8_data::new([0o342, 0o224, 0o201, 0o000], 0, 3, 1), // U+2501
    utf8_data::new([0o342, 0o224, 0o217, 0o000], 0, 3, 1), // U+250F
    utf8_data::new([0o342, 0o224, 0o223, 0o000], 0, 3, 1), // U+2513
    utf8_data::new([0o342, 0o224, 0o227, 0o000], 0, 3, 1), // U+2517
    utf8_data::new([0o342, 0o224, 0o233, 0o000], 0, 3, 1), // U+251B
    utf8_data::new([0o342, 0o224, 0o263, 0o000], 0, 3, 1), // U+2533
    utf8_data::new([0o342, 0o224, 0o273, 0o000], 0, 3, 1), // U+253B
    utf8_data::new([0o342, 0o224, 0o243, 0o000], 0, 3, 1), // U+2523
    utf8_data::new([0o342, 0o224, 0o253, 0o000], 0, 3, 1), // U+252B
    utf8_data::new([0o342, 0o225, 0o213, 0o000], 0, 3, 1), // U+254B
    utf8_data::new([0o302, 0o267, 0o000, 0o000], 0, 2, 1), // U+00B7
];

/// UTF-8 rounded borders.
static TTY_ACS_ROUNDED_BORDERS_LIST: [utf8_data; 13] = [
    utf8_data::new([0o000, 0o000, 0o000, 0o000], 0, 0, 0),
    utf8_data::new([0o342, 0o224, 0o202, 0o000], 0, 3, 1), // U+2502
    utf8_data::new([0o342, 0o224, 0o200, 0o000], 0, 3, 1), // U+2500
    utf8_data::new([0o342, 0o225, 0o255, 0o000], 0, 3, 1), // U+256D
    utf8_data::new([0o342, 0o225, 0o256, 0o000], 0, 3, 1), // U+256E
    utf8_data::new([0o342, 0o225, 0o260, 0o000], 0, 3, 1), // U+2570
    utf8_data::new([0o342, 0o225, 0o257, 0o000], 0, 3, 1), // U+256F
    utf8_data::new([0o342, 0o224, 0o263, 0o000], 0, 3, 1), // U+2533
    utf8_data::new([0o342, 0o224, 0o273, 0o000], 0, 3, 1), // U+253B
    utf8_data::new([0o342, 0o224, 0o234, 0o000], 0, 3, 1), // U+2524
    utf8_data::new([0o342, 0o224, 0o244, 0o000], 0, 3, 1), // U+251C
    utf8_data::new([0o342, 0o225, 0o213, 0o000], 0, 3, 1), // U+254B
    utf8_data::new([0o302, 0o267, 0o000, 0o000], 0, 2, 1), // U+00B7
];

/// C `vendor/tmux/tty-acs.c:166`: `const struct utf8_data *tty_acs_double_borders(int cell_type)`
pub fn tty_acs_double_borders(cell_type: cell_type) -> &'static utf8_data {
    &TTY_ACS_DOUBLE_BORDERS_LIST[cell_type as usize]
}

/// C `vendor/tmux/tty-acs.c:173`: `const struct utf8_data *tty_acs_heavy_borders(int cell_type)`
pub fn tty_acs_heavy_borders(cell_type: cell_type) -> &'static utf8_data {
    &TTY_ACS_HEAVY_BORDERS_LIST[cell_type as usize]
}

/// Get cell border character for rounded style.
/// C `vendor/tmux/tty-acs.c:180`: `const struct utf8_data *tty_acs_rounded_borders(int cell_type)`
pub fn tty_acs_rounded_borders(cell_type: cell_type) -> &'static utf8_data {
    &TTY_ACS_ROUNDED_BORDERS_LIST[cell_type as usize]
}

/// C `vendor/tmux/tty-acs.c:186`: `static int tty_acs_cmp(const void *key, const void *value)`
pub fn tty_acs_cmp(test: u8, entry: &tty_acs_entry) -> std::cmp::Ordering {
    test.cmp(&entry.key)
}

/// C `vendor/tmux/tty-acs.c:195`: `static int tty_acs_reverse_cmp(const void *key, const void *value)`
pub unsafe fn tty_acs_reverse_cmp(
    key: *const u8,
    entry: *const tty_acs_reverse_entry,
) -> std::cmp::Ordering {
    unsafe { i32_to_ordering(libc::strcmp(key, (*entry).string.as_ptr().cast())) }
}

/// Should this terminal use ACS instead of UTF-8 line drawing?
/// C `vendor/tmux/tty-acs.c:205`: `int tty_acs_needed(struct tty *tty)`
pub unsafe fn tty_acs_needed(tty: *const tty) -> bool {
    unsafe {
        if tty.is_null() {
            return false;
        }

        if tty_term_has((*tty).term, tty_code_code::TTYC_U8)
            && tty_term_number((*tty).term, tty_code_code::TTYC_U8) == 0
        {
            return true;
        }

        if (*(*tty).client).flags.intersects(client_flag::UTF8) {
            return false;
        }
        true
    }
}

/// Retrieve ACS to output as UTF-8.
/// C `vendor/tmux/tty-acs.c:231`: `const char *tty_acs_get(struct tty *tty, u_char ch)`
pub unsafe fn tty_acs_get(tty: *mut tty, ch: u8) -> *const u8 {
    unsafe {
        // Use the ACS set instead of UTF-8 if needed.
        if tty_acs_needed(tty) {
            if (*(*tty).term).acs[ch as usize][0] == b'\0' {
                return null();
            }
            return &raw const (*(*tty).term).acs[ch as usize][0];
        }

        let Ok(entry) = TTY_ACS_TABLE.binary_search_by(|e| tty_acs_cmp(ch, e).reverse()) else {
            return null_mut();
        };

        TTY_ACS_TABLE[entry].string.as_ptr().cast()
    }
}

/// Reverse UTF-8 into ACS.
/// C `vendor/tmux/tty-acs.c:252`: `int tty_acs_reverse_get(__unused struct tty *tty, const char *s, size_t slen)`
pub unsafe fn tty_acs_reverse_get(_tty: *const tty, s: *const u8, slen: usize) -> i32 {
    unsafe {
        let table = if slen == 2 {
            TTY_ACS_REVERSE2.as_slice()
        } else if slen == 3 {
            TTY_ACS_REVERSE3.as_slice()
        } else {
            return -1;
        };
        let Ok(entry) = table.binary_search_by(|e| tty_acs_reverse_cmp(s, e).reverse()) else {
            return -1;
        };
        table[entry].key as _
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    // Read a NUL-terminated C string (as returned by tty_acs_get) into a Vec.
    unsafe fn cstr_bytes(p: *const u8) -> Vec<u8> {
        assert!(!p.is_null(), "unexpected NULL C string");
        let mut v = Vec::new();
        let mut i = 0usize;
        unsafe {
            while *p.add(i) != 0 {
                v.push(*p.add(i));
                i += 1;
            }
        }
        v
    }

    // With a NULL tty, tty_acs_needed() returns 0, so tty_acs_get() takes the
    // pure UTF-8 table (bsearch) path. This lets us drive the table directly.
    // C `vendor/tmux/tty-acs.c:231` tty_acs_get + tty_acs_table.
    #[test]
    fn get_known_line_drawing_mappings() {
        unsafe {
            // 'q' -> horizontal line U+2500 "\342\224\200"
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'q')), vec![0o342, 0o224, 0o200]);
            // 'x' -> vertical line U+2502 "\342\224\202"
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'x')), vec![0o342, 0o224, 0o202]);
            // Corners: j lower-right, k upper-right, l upper-left, m lower-left.
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'j')), vec![0o342, 0o224, 0o230]);
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'k')), vec![0o342, 0o224, 0o220]);
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'l')), vec![0o342, 0o224, 0o214]);
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'm')), vec![0o342, 0o224, 0o224]);
            // 'n' -> large plus / crossover U+253C.
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'n')), vec![0o342, 0o224, 0o274]);
        }
    }

    // Two-byte UTF-8 entries and the table boundaries.
    #[test]
    fn get_two_byte_and_boundary_entries() {
        unsafe {
            // 'f' -> degree symbol U+00B0 "\302\260" (2 bytes).
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'f')), vec![0o302, 0o260]);
            // 'g' -> plus/minus U+00B1 "\302\261".
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'g')), vec![0o302, 0o261]);
            // First table entry '+' -> arrow right U+2192 "\342\206\222".
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'+')), vec![0o342, 0o206, 0o222]);
            // Last table entry '~' -> bullet U+00B7 "\302\267".
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'~')), vec![0o302, 0o267]);
        }
    }

    // Characters not in tty_acs_table return NULL (bsearch miss).
    #[test]
    fn get_out_of_table_returns_null() {
        unsafe {
            // Just below the first key '+' (0x2b): '*' (0x2a).
            assert!(tty_acs_get(null_mut(), b'*').is_null());
            // Just above the last key '~' (0x7e): 0x7f (DEL).
            assert!(tty_acs_get(null_mut(), 0x7f).is_null());
            // 0x00 well below the table.
            assert!(tty_acs_get(null_mut(), 0).is_null());
            // '0' is present but the other digits are not.
            assert!(tty_acs_get(null_mut(), b'1').is_null());
            assert!(tty_acs_get(null_mut(), b'9').is_null());
            // Uppercase letters and other gaps are absent.
            assert!(tty_acs_get(null_mut(), b'A').is_null());
            assert!(tty_acs_get(null_mut(), b'Z').is_null());
            // '_' (0x5f) sits between 'Z' (0x5a) and '`' (0x60): absent.
            assert!(tty_acs_get(null_mut(), b'_').is_null());
            // A space is not in the table.
            assert!(tty_acs_get(null_mut(), b' ').is_null());
        }
    }

    // Reverse lookup of 3-byte UTF-8 sequences -> ACS key.
    // C `vendor/tmux/tty-acs.c:252` tty_acs_reverse_get + tty_acs_reverse3.
    #[test]
    fn reverse_get_three_byte() {
        unsafe {
            // U+2500 "\342\224\200" -> 'q'.
            let s = [0o342u8, 0o224, 0o200, 0];
            assert_eq!(tty_acs_reverse_get(null(), s.as_ptr(), 3), b'q' as i32);
            // U+2502 "\342\224\202" -> 'x'.
            let s = [0o342u8, 0o224, 0o202, 0];
            assert_eq!(tty_acs_reverse_get(null(), s.as_ptr(), 3), b'x' as i32);
            // U+2501 (heavy horizontal) "\342\224\201" -> 'q'.
            let s = [0o342u8, 0o224, 0o201, 0];
            assert_eq!(tty_acs_reverse_get(null(), s.as_ptr(), 3), b'q' as i32);
            // U+2554 double upper-left "\342\225\224" -> 'l'.
            let s = [0o342u8, 0o225, 0o224, 0];
            assert_eq!(tty_acs_reverse_get(null(), s.as_ptr(), 3), b'l' as i32);
            // Last reverse3 entry U+256C "\342\225\254" -> 'n'.
            let s = [0o342u8, 0o225, 0o254, 0];
            assert_eq!(tty_acs_reverse_get(null(), s.as_ptr(), 3), b'n' as i32);
            // First reverse3 entry U+2500 already covered; check U+254B "\342\225\213" -> 'n'.
            let s = [0o342u8, 0o225, 0o213, 0];
            assert_eq!(tty_acs_reverse_get(null(), s.as_ptr(), 3), b'n' as i32);
        }
    }

    // Reverse lookup of the sole 2-byte entry (bullet).
    #[test]
    fn reverse_get_two_byte() {
        unsafe {
            // U+00B7 "\302\267" -> '~' (only entry in tty_acs_reverse2).
            let s = [0o302u8, 0o267, 0];
            assert_eq!(tty_acs_reverse_get(null(), s.as_ptr(), 2), b'~' as i32);
        }
    }

    // Unknown sequences and invalid lengths return -1.
    #[test]
    fn reverse_get_misses_and_bad_lengths() {
        unsafe {
            // A 3-byte sequence not in reverse3 -> -1 (U+2504 "\342\224\204").
            let s = [0o342u8, 0o224, 0o204, 0];
            assert_eq!(tty_acs_reverse_get(null(), s.as_ptr(), 3), -1);
            // A 2-byte sequence that is not the bullet -> -1 (U+00B0 "\302\260").
            let s = [0o302u8, 0o260, 0];
            assert_eq!(tty_acs_reverse_get(null(), s.as_ptr(), 2), -1);
            // slen == 1 and slen == 4 are unsupported -> -1.
            let s = [0o342u8, 0o224, 0o200, 0];
            assert_eq!(tty_acs_reverse_get(null(), s.as_ptr(), 1), -1);
            assert_eq!(tty_acs_reverse_get(null(), s.as_ptr(), 4), -1);
            assert_eq!(tty_acs_reverse_get(null(), s.as_ptr(), 0), -1);
        }
    }

    // Round-trip: get(key) yields a UTF-8 string, and reverse of that string
    // (slen == 3) yields the same key for the plain box-drawing entries.
    #[test]
    fn get_reverse_round_trip() {
        unsafe {
            // Only the keys whose forward UTF-8 also appears in tty_acs_reverse3.
            // (e.g. 'w' -> U+252C is intentionally absent from reverse3, so it is
            // not reversible; see vendor/tmux/tty-acs.c tty_acs_reverse3.)
            for &key in b"qxjklmtuvn" {
                let bytes = cstr_bytes(tty_acs_get(null_mut(), key));
                assert_eq!(bytes.len(), 3, "expected 3-byte UTF-8 for {}", key as char);
                let mut buf = [0u8; 4];
                buf[..3].copy_from_slice(&bytes);
                assert_eq!(
                    tty_acs_reverse_get(null(), buf.as_ptr(), 3),
                    key as i32,
                    "round trip failed for {}",
                    key as char
                );
            }
            // Bullet round-trips through the 2-byte table.
            let bytes = cstr_bytes(tty_acs_get(null_mut(), b'~'));
            assert_eq!(bytes, vec![0o302, 0o267]);
            let mut buf = [0u8; 3];
            buf[..2].copy_from_slice(&bytes);
            assert_eq!(tty_acs_reverse_get(null(), buf.as_ptr(), 2), b'~' as i32);
        }
    }

    // tty_acs_cmp is C's (test - entry->key) reduced to an Ordering.
    // C `vendor/tmux/tty-acs.c:186`.
    #[test]
    fn cmp_orders_by_key() {
        let entry = tty_acs_entry::new(b'q', &[0o342, 0o224, 0o200, 0o000]);
        assert_eq!(tty_acs_cmp(b'q', &entry), Ordering::Equal);
        assert_eq!(tty_acs_cmp(b'p', &entry), Ordering::Less);
        assert_eq!(tty_acs_cmp(b'r', &entry), Ordering::Greater);
    }

    // The main table is sorted ascending by key (bsearch precondition).
    #[test]
    fn table_is_sorted_by_key() {
        assert!(TTY_ACS_TABLE.windows(2).all(|w| w[0].key < w[1].key));
        assert_eq!(TTY_ACS_TABLE.len(), 36);
    }

    // The reverse tables are sorted ascending by their UTF-8 string (bsearch
    // precondition, ordering matches C strcmp / lexicographic byte order).
    #[test]
    fn reverse_tables_are_sorted() {
        assert!(TTY_ACS_REVERSE3.windows(2).all(|w| w[0].string < w[1].string));
        assert_eq!(TTY_ACS_REVERSE3.len(), 32);
        assert_eq!(TTY_ACS_REVERSE2.len(), 1);
    }

    // Border style lookups index the list by cell_type.
    // C `vendor/tmux/tty-acs.c:166/173/180`.
    #[test]
    fn border_style_lookups() {
        // Index 0 (CELL_INSIDE) is the empty sentinel entry in every list.
        let d = tty_acs_double_borders(cell_type::CELL_INSIDE);
        assert_eq!(d.size, 0);
        assert_eq!(d.width, 0);

        // Double vertical (CELL_TOPBOTTOM=1) -> U+2551 "\342\225\221".
        let d = tty_acs_double_borders(cell_type::CELL_TOPBOTTOM);
        assert_eq!(&d.data[..3], &[0o342, 0o225, 0o221]);
        assert_eq!(d.size, 3);
        assert_eq!(d.width, 1);

        // Heavy vertical (CELL_TOPBOTTOM=1) -> U+2503 "\342\224\203".
        let h = tty_acs_heavy_borders(cell_type::CELL_TOPBOTTOM);
        assert_eq!(&h.data[..3], &[0o342, 0o224, 0o203]);

        // Rounded horizontal (CELL_LEFTRIGHT=2) -> U+2500 "\342\224\200".
        let r = tty_acs_rounded_borders(cell_type::CELL_LEFTRIGHT);
        assert_eq!(&r.data[..3], &[0o342, 0o224, 0o200]);

        // Rounded top-left corner (CELL_TOPLEFT=3) -> U+256D "\342\225\255".
        let r = tty_acs_rounded_borders(cell_type::CELL_TOPLEFT);
        assert_eq!(&r.data[..3], &[0o342, 0o225, 0o255]);

        // Last cell_type CELL_OUTSIDE=12 -> bullet U+00B7 "\302\267" in each list.
        let d = tty_acs_double_borders(cell_type::CELL_OUTSIDE);
        assert_eq!(&d.data[..2], &[0o302, 0o267]);
        assert_eq!(d.size, 2);
    }

    // Remaining single-byte / multi-byte forward mappings from tty_acs_table
    // (vendor/tmux/tty-acs.c:56-101): tees, scan lines, comparison glyphs and
    // symbol keys. Each is a distinct 2- or 3-byte UTF-8 sequence.
    #[test]
    fn get_tees_scanlines_and_symbols() {
        unsafe {
            // Tees: t right, u left, v up, w down (U+251C/2524/2534/252C).
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b't')), vec![0o342, 0o224, 0o234]);
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'u')), vec![0o342, 0o224, 0o244]);
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'v')), vec![0o342, 0o224, 0o264]);
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'w')), vec![0o342, 0o224, 0o254]);
            // Scan lines: o (1), p (3), r (7), s (9).
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'o')), vec![0o342, 0o216, 0o272]);
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'p')), vec![0o342, 0o216, 0o273]);
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'r')), vec![0o342, 0o216, 0o274]);
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b's')), vec![0o342, 0o216, 0o275]);
            // Comparisons: y <=, z >=, | !=.
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'y')), vec![0o342, 0o211, 0o244]);
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'z')), vec![0o342, 0o211, 0o245]);
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'|')), vec![0o342, 0o211, 0o240]);
            // Symbols: ` diamond, a stipple, 0 solid block, { greek pi.
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'`')), vec![0o342, 0o227, 0o206]);
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'a')), vec![0o342, 0o226, 0o222]);
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'0')), vec![0o342, 0o226, 0o256]);
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'{')), vec![0o317, 0o200]);
            // 2-byte: } UK pound U+00A3.
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'}')), vec![0o302, 0o243]);
        }
    }

    // The four arrow keys at the very start of the table (vendor/tmux/tty-acs.c:56):
    // + right, , left, - up, . down (U+2192/2190/2191/2193).
    #[test]
    fn get_arrow_glyphs() {
        unsafe {
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'+')), vec![0o342, 0o206, 0o222]);
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b',')), vec![0o342, 0o206, 0o220]);
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'-')), vec![0o342, 0o206, 0o221]);
            assert_eq!(cstr_bytes(tty_acs_get(null_mut(), b'.')), vec![0o342, 0o206, 0o223]);
        }
    }

    // Reverse lookups over the heavy box-drawing block of tty_acs_reverse3
    // (vendor/tmux/tty-acs.c): heavy horizontal/vertical fold onto the light
    // 'q'/'x', and the heavy corners onto j/k/l/m.
    #[test]
    fn reverse_get_heavy_box_drawing() {
        unsafe {
            let rev = |b: [u8; 4]| tty_acs_reverse_get(null(), b.as_ptr(), 3);
            // U+2501 heavy horizontal -> 'q'; U+2503 heavy vertical -> 'x'.
            assert_eq!(rev([0o342, 0o224, 0o201, 0]), b'q' as i32);
            assert_eq!(rev([0o342, 0o224, 0o203, 0]), b'x' as i32);
            // Light corners: U+250C -> l, U+2510 -> k, U+2514 -> m, U+2518 -> j.
            assert_eq!(rev([0o342, 0o224, 0o214, 0]), b'l' as i32);
            assert_eq!(rev([0o342, 0o224, 0o220, 0]), b'k' as i32);
            assert_eq!(rev([0o342, 0o224, 0o224, 0]), b'm' as i32);
            assert_eq!(rev([0o342, 0o224, 0o230, 0]), b'j' as i32);
            // Heavy corners: U+250F -> k, U+2513 -> l, U+2517 -> m, U+251B -> j.
            assert_eq!(rev([0o342, 0o224, 0o217, 0]), b'k' as i32);
            assert_eq!(rev([0o342, 0o224, 0o223, 0]), b'l' as i32);
            assert_eq!(rev([0o342, 0o224, 0o227, 0]), b'm' as i32);
            assert_eq!(rev([0o342, 0o224, 0o233, 0]), b'j' as i32);
        }
    }

    // Reverse lookups over the double box-drawing block (U+2550 range): these
    // fold onto the same ACS keys as their single-line counterparts.
    #[test]
    fn reverse_get_double_box_drawing() {
        unsafe {
            let rev = |b: [u8; 4]| tty_acs_reverse_get(null(), b.as_ptr(), 3);
            // U+2550 double horizontal -> q, U+2551 double vertical -> x.
            assert_eq!(rev([0o342, 0o225, 0o220, 0]), b'q' as i32);
            assert_eq!(rev([0o342, 0o225, 0o221, 0]), b'x' as i32);
            // Double corners: U+2554 -> l, U+2557 -> k, U+255A -> m, U+255D -> j.
            assert_eq!(rev([0o342, 0o225, 0o224, 0]), b'l' as i32);
            assert_eq!(rev([0o342, 0o225, 0o227, 0]), b'k' as i32);
            assert_eq!(rev([0o342, 0o225, 0o232, 0]), b'm' as i32);
            assert_eq!(rev([0o342, 0o225, 0o235, 0]), b'j' as i32);
            // Double tees: U+2560 -> t, U+2563 -> u, U+2566 -> w, U+2569 -> v.
            assert_eq!(rev([0o342, 0o225, 0o240, 0]), b't' as i32);
            assert_eq!(rev([0o342, 0o225, 0o243, 0]), b'u' as i32);
            assert_eq!(rev([0o342, 0o225, 0o246, 0]), b'w' as i32);
            assert_eq!(rev([0o342, 0o225, 0o251, 0]), b'v' as i32);
        }
    }

    // tty_acs_cmp is a signed (test - key) reduced to Ordering; sweep the whole
    // table so a mis-signed comparator (which would break the bsearch in
    // tty_acs_get) is caught. C `vendor/tmux/tty-acs.c:186`.
    #[test]
    fn cmp_matches_key_ordering_across_table() {
        for e in &TTY_ACS_TABLE {
            assert_eq!(tty_acs_cmp(e.key, e), Ordering::Equal);
            if e.key > 0 {
                assert_eq!(tty_acs_cmp(e.key - 1, e), Ordering::Less);
            }
            if e.key < 0xff {
                assert_eq!(tty_acs_cmp(e.key + 1, e), Ordering::Greater);
            }
        }
    }

    // Every cell_type (0..=12) indexes each border list without panicking, and
    // the size/width fields stay self-consistent: index 0 is the empty sentinel
    // and every later index is a printable 2- or 3-byte glyph of width 1.
    // C `vendor/tmux/tty-acs.c:166/173/180`.
    #[test]
    fn all_border_cell_types_are_consistent() {
        let types = [
            cell_type::CELL_INSIDE,
            cell_type::CELL_TOPBOTTOM,
            cell_type::CELL_LEFTRIGHT,
            cell_type::CELL_TOPLEFT,
            cell_type::CELL_TOPRIGHT,
            cell_type::CELL_BOTTOMLEFT,
            cell_type::CELL_BOTTOMRIGHT,
            cell_type::CELL_TOPJOIN,
            cell_type::CELL_BOTTOMJOIN,
            cell_type::CELL_LEFTJOIN,
            cell_type::CELL_RIGHTJOIN,
            cell_type::CELL_JOIN,
            cell_type::CELL_OUTSIDE,
        ];
        for (i, &t) in types.iter().enumerate() {
            for d in [
                tty_acs_double_borders(t),
                tty_acs_heavy_borders(t),
                tty_acs_rounded_borders(t),
            ] {
                if i == 0 {
                    assert_eq!(d.size, 0, "sentinel size at index 0");
                    assert_eq!(d.width, 0);
                } else {
                    assert!(d.size == 2 || d.size == 3, "glyph size 2 or 3");
                    assert_eq!(d.width, 1, "border glyph width is 1");
                }
            }
        }
    }

    // Forward-then-reverse over the full set of round-trippable table keys: for
    // every 3-byte forward glyph that also appears in tty_acs_reverse3, get()
    // then reverse_get() must return the original key. Guards against a table
    // that drifts out of forward/reverse agreement. C `vendor/tmux/tty-acs.c`.
    #[test]
    fn full_forward_reverse_agreement() {
        unsafe {
            let mut round_tripped = 0;
            for e in &TTY_ACS_TABLE {
                let p = tty_acs_get(null_mut(), e.key);
                if p.is_null() {
                    continue;
                }
                let bytes = cstr_bytes(p);
                if bytes.len() != 3 {
                    continue;
                }
                let mut buf = [0u8; 4];
                buf[..3].copy_from_slice(&bytes);
                let back = tty_acs_reverse_get(null(), buf.as_ptr(), 3);
                if back != -1 {
                    assert_eq!(
                        back, e.key as i32,
                        "reverse of {:?} should map back to {}",
                        bytes, e.key as char
                    );
                    round_tripped += 1;
                }
            }
            // Sanity: the box-drawing keys (a healthy chunk of the table) did
            // round-trip, so the loop wasn't silently a no-op.
            assert!(round_tripped >= 10, "expected many round-trips, got {round_tripped}");
        }
    }
}
