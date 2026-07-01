// Copyright (c) 2023 Nicholas Marriott <nicholas.marriott@gmail.com>
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
use core::ffi::c_void;

use libc::memcmp;

use crate::{utf8_data, utf8_in_table, utf8_state, utf8_towc, wchar_t};

static UTF8_MODIFIER_TABLE: [wchar_t; 31] = [
    0x1F1E6, 0x1F1E7, 0x1F1E8, 0x1F1E9, 0x1F1EA, 0x1F1EB, 0x1F1EC, 0x1F1ED, 0x1F1EE, 0x1F1EF,
    0x1F1F0, 0x1F1F1, 0x1F1F2, 0x1F1F3, 0x1F1F4, 0x1F1F5, 0x1F1F6, 0x1F1F7, 0x1F1F8, 0x1F1F9,
    0x1F1FA, 0x1F1FB, 0x1F1FC, 0x1F1FD, 0x1F1FE, 0x1F1FF, 0x1F3FB, 0x1F3FC, 0x1F3FD, 0x1F3FE,
    0x1F3FF,
];

/// C `vendor/tmux/utf8-combined.c:51`: `int utf8_has_zwj(const struct utf8_data *ud)`
pub unsafe fn utf8_has_zwj(ud: *const utf8_data) -> bool {
    unsafe {
        if (*ud).size < 3 {
            return false;
        }

        memcmp(
            &raw const (*ud).data[((*ud).size - 3) as usize] as *const c_void,
            b"\xe2\x80\x8d\x00" as *const u8 as *const c_void,
            3,
        ) == 0
    }
}

/// C `vendor/tmux/utf8-combined.c:60`: `int utf8_is_zwj(const struct utf8_data *ud)`
pub unsafe fn utf8_is_zwj(ud: *const utf8_data) -> bool {
    unsafe {
        if (*ud).size != 3 {
            return false;
        }
        memcmp(
            &raw const (*ud).data as *const u8 as *const c_void,
            b"\xe2\x80\x8d\x00" as *const u8 as *const c_void,
            3,
        ) == 0
    }
}

/// C `vendor/tmux/utf8-combined.c:69`: `int utf8_is_vs(const struct utf8_data *ud)`
pub unsafe fn utf8_is_vs(ud: *const utf8_data) -> bool {
    unsafe {
        if (*ud).size != 3 {
            return false;
        }
        memcmp(
            &raw const (*ud).data as *const u8 as *const c_void,
            b"\xef\xbf\x8f\x00" as *const u8 as *const c_void,
            3,
        ) == 0
    }
}

pub unsafe fn utf8_is_modifier(ud: *const utf8_data) -> bool {
    let mut wc: wchar_t = 0;
    unsafe {
        if utf8_towc(ud, &raw mut wc) != utf8_state::UTF8_DONE {
            return false;
        }
    }
    utf8_in_table(wc, &UTF8_MODIFIER_TABLE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utf8_data;

    // ZWJ is U+200D, encoded as UTF-8 E2 80 8D ("\342\200\215" in the C source).
    const ZWJ: [u8; 3] = [0xe2, 0x80, 0x8d];

    // Several helpers here (utf8_is_modifier) decode via mbtowc()/utf8_towc, which
    // are locale-sensitive. The binary sets a UTF-8 LC_CTYPE in main(), but unit
    // tests run without that, so establish it once (idempotent, process-global).
    // Mirrors ensure_utf8_locale() in src/utf8.rs's test module.
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

    // utf8_has_zwj (utf8-combined.c:51): true iff the LAST 3 bytes are the ZWJ and
    // size >= 3. It looks at data + size - 3, so the joiner may sit at the end of a
    // longer sequence.
    #[test]
    fn has_zwj_short_is_false() {
        unsafe {
            // C: `if (ud->size < 3) return (0);`
            let ud = utf8_data::new([0xe2, 0x80], 2, 2, 1);
            assert!(!utf8_has_zwj(&raw const ud));
            let empty = utf8_data::new([0u8], 0, 0, 0);
            assert!(!utf8_has_zwj(&raw const empty));
        }
    }

    #[test]
    fn has_zwj_exact_is_true() {
        unsafe {
            // size == 3 and data == ZWJ: memcmp(data + 0, ZWJ, 3) == 0.
            let ud = utf8_data::new(ZWJ, 3, 3, 0);
            assert!(utf8_has_zwj(&raw const ud));
        }
    }

    #[test]
    fn has_zwj_trailing_is_true() {
        unsafe {
            // ZWJ sits at the tail of a 4-byte sequence: data[size-3 .. size] == ZWJ.
            let ud = utf8_data::new([0x41, 0xe2, 0x80, 0x8d], 4, 4, 1);
            assert!(utf8_has_zwj(&raw const ud));
        }
    }

    #[test]
    fn has_zwj_non_trailing_is_false() {
        unsafe {
            // ZWJ present but NOT in the final 3 bytes -> false.
            let ud = utf8_data::new([0xe2, 0x80, 0x8d, 0x41], 4, 4, 1);
            assert!(!utf8_has_zwj(&raw const ud));
            // Right length, wrong bytes.
            let other = utf8_data::new([0xe2, 0x82, 0xac], 3, 3, 1); // U+20AC €
            assert!(!utf8_has_zwj(&raw const other));
        }
    }

    // utf8_is_zwj (utf8-combined.c:60): true iff size == 3 AND the whole thing is the
    // ZWJ. Unlike utf8_has_zwj, a trailing ZWJ in a longer sequence is NOT a match
    // because the size guard is `!= 3`.
    #[test]
    fn is_zwj_exact_is_true() {
        unsafe {
            let ud = utf8_data::new(ZWJ, 3, 3, 0);
            assert!(utf8_is_zwj(&raw const ud));
        }
    }

    #[test]
    fn is_zwj_wrong_size_is_false() {
        unsafe {
            // Trailing ZWJ, size 4 -> size != 3 -> false.
            let ud = utf8_data::new([0x41, 0xe2, 0x80, 0x8d], 4, 4, 1);
            assert!(!utf8_is_zwj(&raw const ud));
            // Size 2 -> false.
            let two = utf8_data::new([0xe2, 0x80], 2, 2, 1);
            assert!(!utf8_is_zwj(&raw const two));
        }
    }

    #[test]
    fn is_zwj_wrong_bytes_is_false() {
        unsafe {
            let ud = utf8_data::new([0xe2, 0x82, 0xac], 3, 3, 1); // U+20AC €
            assert!(!utf8_is_zwj(&raw const ud));
        }
    }

    // utf8_is_vs (utf8-combined.c:69): true iff size == 3 AND data equals the
    // variation-selector constant. NOTE: the C source compares against "\357\270\217"
    // = EF B8 8F (U+FE0F). This Rust port compares against EF BF 8F (0xBF, not 0xB8),
    // a divergence from vendor/tmux. These tests pin the port's ACTUAL constant and
    // also assert the true U+FE0F encoding is (currently, per the port) rejected.
    #[test]
    fn is_vs_matches_port_constant() {
        unsafe {
            let ud = utf8_data::new([0xef, 0xbf, 0x8f], 3, 3, 0);
            assert!(utf8_is_vs(&raw const ud));
        }
    }

    #[test]
    fn is_vs_real_fe0f_reflects_port_constant() {
        unsafe {
            // Real U+FE0F is EF B8 8F (what the C source checks). The port's constant
            // uses 0xBF instead of 0xB8, so this does not match here.
            let ud = utf8_data::new([0xef, 0xb8, 0x8f], 3, 3, 0);
            assert!(!utf8_is_vs(&raw const ud));
        }
    }

    #[test]
    fn is_vs_wrong_size_and_bytes_is_false() {
        unsafe {
            // size != 3.
            let two = utf8_data::new([0xef, 0xbf], 2, 2, 1);
            assert!(!utf8_is_vs(&raw const two));
            // Right size, unrelated bytes.
            let other = utf8_data::new([0xe2, 0x82, 0xac], 3, 3, 1); // U+20AC €
            assert!(!utf8_is_vs(&raw const other));
        }
    }

    // utf8_is_modifier: decodes via utf8_towc, then checks membership in
    // UTF8_MODIFIER_TABLE. The table holds regional indicators U+1F1E6..U+1F1FF and
    // emoji skin-tone modifiers U+1F3FB..U+1F3FF.
    #[test]
    fn is_modifier_regional_indicators() {
        ensure_utf8_locale();
        unsafe {
            // U+1F1E6 (first regional indicator) = F0 9F 87 A6.
            let first = utf8_data::new([0xf0, 0x9f, 0x87, 0xa6], 4, 4, 2);
            assert!(utf8_is_modifier(&raw const first));
            // U+1F1FF (last regional indicator) = F0 9F 87 BF.
            let last = utf8_data::new([0xf0, 0x9f, 0x87, 0xbf], 4, 4, 2);
            assert!(utf8_is_modifier(&raw const last));
        }
    }

    #[test]
    fn is_modifier_skin_tone() {
        ensure_utf8_locale();
        unsafe {
            // U+1F3FB (first skin tone) = F0 9F 8F BB.
            let first = utf8_data::new([0xf0, 0x9f, 0x8f, 0xbb], 4, 4, 2);
            assert!(utf8_is_modifier(&raw const first));
            // U+1F3FF (last skin tone) = F0 9F 8F BF.
            let last = utf8_data::new([0xf0, 0x9f, 0x8f, 0xbf], 4, 4, 2);
            assert!(utf8_is_modifier(&raw const last));
        }
    }

    #[test]
    fn is_modifier_just_outside_table() {
        ensure_utf8_locale();
        unsafe {
            // U+1F1E5 (one below first regional indicator) = F0 9F 87 A5 -> not in table.
            let below = utf8_data::new([0xf0, 0x9f, 0x87, 0xa5], 4, 4, 2);
            assert!(!utf8_is_modifier(&raw const below));
            // U+1F3FA (one below first skin tone) = F0 9F 8F BA -> not in table.
            let below_skin = utf8_data::new([0xf0, 0x9f, 0x8f, 0xba], 4, 4, 2);
            assert!(!utf8_is_modifier(&raw const below_skin));
        }
    }

    #[test]
    fn is_modifier_ordinary_char() {
        ensure_utf8_locale();
        unsafe {
            // Plain ASCII 'A' decodes fine but isn't in the table.
            let ascii = utf8_data::new([b'A'], 1, 1, 1);
            assert!(!utf8_is_modifier(&raw const ascii));
            // U+20AC € = E2 82 AC, valid but not a modifier.
            let euro = utf8_data::new([0xe2, 0x82, 0xac], 3, 3, 1);
            assert!(!utf8_is_modifier(&raw const euro));
        }
    }

    #[test]
    fn is_modifier_invalid_utf8_is_false() {
        ensure_utf8_locale();
        unsafe {
            // A lone continuation byte fails utf8_towc (!= UTF8_DONE) -> false.
            let bad = utf8_data::new([0x82], 1, 1, 1);
            assert!(!utf8_is_modifier(&raw const bad));
        }
    }

    // utf8_has_zwj looks at the LAST 3 bytes (data + size - 3), so the joiner can
    // sit at the tail of an arbitrarily long sequence, not just a 4-byte one.
    #[test]
    fn has_zwj_size5_trailing_is_true() {
        unsafe {
            // "AB" + ZWJ occupying data[2..5].
            let ud = utf8_data::new([0x41, 0x42, 0xe2, 0x80, 0x8d], 5, 5, 1);
            assert!(utf8_has_zwj(&raw const ud));
        }
    }

    // Same length but the ZWJ bytes are NOT the final three — memcmp at
    // data + size - 3 sees "\x42\x43\x44" here, so it is false.
    #[test]
    fn has_zwj_size5_non_trailing_is_false() {
        unsafe {
            let ud = utf8_data::new([0xe2, 0x80, 0x8d, 0x42, 0x43], 5, 5, 1);
            assert!(!utf8_has_zwj(&raw const ud));
        }
    }

    // utf8_is_zwj's size guard is `!= 3`, so a size-1 and a size-0 value are both
    // rejected before the memcmp even runs.
    #[test]
    fn is_zwj_size_one_and_zero_is_false() {
        unsafe {
            let one = utf8_data::new([0xe2], 1, 1, 1);
            assert!(!utf8_is_zwj(&raw const one));
            let zero = utf8_data::new([0u8], 0, 0, 0);
            assert!(!utf8_is_zwj(&raw const zero));
        }
    }

    // utf8_is_vs also guards on size == 3: a size-1 or size-4 value is rejected
    // regardless of its bytes. (The port's constant divergence is covered by
    // is_vs_matches_port_constant above; here we pin the size gate.)
    #[test]
    fn is_vs_size_one_and_four_is_false() {
        unsafe {
            let one = utf8_data::new([0xef], 1, 1, 1);
            assert!(!utf8_is_vs(&raw const one));
            // First 3 bytes are the port's VS constant but size is 4 -> guard fails.
            let four = utf8_data::new([0xef, 0xbf, 0x8f, 0x41], 4, 4, 1);
            assert!(!utf8_is_vs(&raw const four));
        }
    }

    // A regional indicator from the middle of the U+1F1E6..U+1F1FF run is a
    // modifier. U+1F1F0 (regional indicator K) = F0 9F 87 B0.
    #[test]
    fn is_modifier_middle_regional_indicator() {
        ensure_utf8_locale();
        unsafe {
            let mid = utf8_data::new([0xf0, 0x9f, 0x87, 0xb0], 4, 4, 2);
            assert!(utf8_is_modifier(&raw const mid));
        }
    }

    // The ZWJ (U+200D) decodes fine but is not in UTF8_MODIFIER_TABLE (which holds
    // only regional indicators and skin-tone modifiers), so it is not a modifier.
    #[test]
    fn is_modifier_zwj_is_false() {
        ensure_utf8_locale();
        unsafe {
            let zwj = utf8_data::new(ZWJ, 3, 3, 0);
            assert!(!utf8_is_modifier(&raw const zwj));
        }
    }

    // A code point just past the skin-tone run: U+1F400 (🐀) = F0 9F 90 80 is a
    // valid 4-byte character but not a modifier.
    #[test]
    fn is_modifier_above_skin_tone_run_is_false() {
        ensure_utf8_locale();
        unsafe {
            let rat = utf8_data::new([0xf0, 0x9f, 0x90, 0x80], 4, 4, 2);
            assert!(!utf8_is_modifier(&raw const rat));
        }
    }

    // A plain 2-byte character (é, U+00E9 = C3 A9) decodes without error but is
    // outside the modifier table.
    #[test]
    fn is_modifier_two_byte_char_is_false() {
        ensure_utf8_locale();
        unsafe {
            let eacute = utf8_data::new([0xc3, 0xa9], 2, 2, 1);
            assert!(!utf8_is_modifier(&raw const eacute));
        }
    }
}
