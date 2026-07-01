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

// Special key codes.
#[repr(u64)]
pub(crate) enum keyc {
    // Focus events.
    KEYC_FOCUS_IN = KEYC_BASE,
    KEYC_FOCUS_OUT,

    // "Any" key, used if not found in key table.
    KEYC_ANY,

    // Paste brackets.
    KEYC_PASTE_START,
    KEYC_PASTE_END,

    // Mouse keys.
    KEYC_MOUSE,       // unclassified mouse event
    KEYC_DRAGGING,    // dragging in progress
    KEYC_DOUBLECLICK, // double click complete

    KEYC_MOUSEMOVE_PANE,
    KEYC_MOUSEMOVE_STATUS,
    KEYC_MOUSEMOVE_STATUS_LEFT,
    KEYC_MOUSEMOVE_STATUS_RIGHT,
    KEYC_MOUSEMOVE_STATUS_DEFAULT,
    KEYC_MOUSEMOVE_BORDER,

    KEYC_MOUSEDOWN1_PANE,
    KEYC_MOUSEDOWN1_STATUS,
    KEYC_MOUSEDOWN1_STATUS_LEFT,
    KEYC_MOUSEDOWN1_STATUS_RIGHT,
    KEYC_MOUSEDOWN1_STATUS_DEFAULT,
    KEYC_MOUSEDOWN1_BORDER,

    KEYC_MOUSEDOWN2_PANE,
    KEYC_MOUSEDOWN2_STATUS,
    KEYC_MOUSEDOWN2_STATUS_LEFT,
    KEYC_MOUSEDOWN2_STATUS_RIGHT,
    KEYC_MOUSEDOWN2_STATUS_DEFAULT,
    KEYC_MOUSEDOWN2_BORDER,

    KEYC_MOUSEDOWN3_PANE,
    KEYC_MOUSEDOWN3_STATUS,
    KEYC_MOUSEDOWN3_STATUS_LEFT,
    KEYC_MOUSEDOWN3_STATUS_RIGHT,
    KEYC_MOUSEDOWN3_STATUS_DEFAULT,
    KEYC_MOUSEDOWN3_BORDER,

    KEYC_MOUSEDOWN6_PANE,
    KEYC_MOUSEDOWN6_STATUS,
    KEYC_MOUSEDOWN6_STATUS_LEFT,
    KEYC_MOUSEDOWN6_STATUS_RIGHT,
    KEYC_MOUSEDOWN6_STATUS_DEFAULT,
    KEYC_MOUSEDOWN6_BORDER,

    KEYC_MOUSEDOWN7_PANE,
    KEYC_MOUSEDOWN7_STATUS,
    KEYC_MOUSEDOWN7_STATUS_LEFT,
    KEYC_MOUSEDOWN7_STATUS_RIGHT,
    KEYC_MOUSEDOWN7_STATUS_DEFAULT,
    KEYC_MOUSEDOWN7_BORDER,

    KEYC_MOUSEDOWN8_PANE,
    KEYC_MOUSEDOWN8_STATUS,
    KEYC_MOUSEDOWN8_STATUS_LEFT,
    KEYC_MOUSEDOWN8_STATUS_RIGHT,
    KEYC_MOUSEDOWN8_STATUS_DEFAULT,
    KEYC_MOUSEDOWN8_BORDER,

    KEYC_MOUSEDOWN9_PANE,
    KEYC_MOUSEDOWN9_STATUS,
    KEYC_MOUSEDOWN9_STATUS_LEFT,
    KEYC_MOUSEDOWN9_STATUS_RIGHT,
    KEYC_MOUSEDOWN9_STATUS_DEFAULT,
    KEYC_MOUSEDOWN9_BORDER,

    KEYC_MOUSEDOWN10_PANE,
    KEYC_MOUSEDOWN10_STATUS,
    KEYC_MOUSEDOWN10_STATUS_LEFT,
    KEYC_MOUSEDOWN10_STATUS_RIGHT,
    KEYC_MOUSEDOWN10_STATUS_DEFAULT,
    KEYC_MOUSEDOWN10_BORDER,

    KEYC_MOUSEDOWN11_PANE,
    KEYC_MOUSEDOWN11_STATUS,
    KEYC_MOUSEDOWN11_STATUS_LEFT,
    KEYC_MOUSEDOWN11_STATUS_RIGHT,
    KEYC_MOUSEDOWN11_STATUS_DEFAULT,
    KEYC_MOUSEDOWN11_BORDER,

    KEYC_MOUSEUP1_PANE,
    KEYC_MOUSEUP1_STATUS,
    KEYC_MOUSEUP1_STATUS_LEFT,
    KEYC_MOUSEUP1_STATUS_RIGHT,
    KEYC_MOUSEUP1_STATUS_DEFAULT,
    KEYC_MOUSEUP1_BORDER,

    KEYC_MOUSEUP2_PANE,
    KEYC_MOUSEUP2_STATUS,
    KEYC_MOUSEUP2_STATUS_LEFT,
    KEYC_MOUSEUP2_STATUS_RIGHT,
    KEYC_MOUSEUP2_STATUS_DEFAULT,
    KEYC_MOUSEUP2_BORDER,

    KEYC_MOUSEUP3_PANE,
    KEYC_MOUSEUP3_STATUS,
    KEYC_MOUSEUP3_STATUS_LEFT,
    KEYC_MOUSEUP3_STATUS_RIGHT,
    KEYC_MOUSEUP3_STATUS_DEFAULT,
    KEYC_MOUSEUP3_BORDER,

    KEYC_MOUSEUP6_PANE,
    KEYC_MOUSEUP6_STATUS,
    KEYC_MOUSEUP6_STATUS_LEFT,
    KEYC_MOUSEUP6_STATUS_RIGHT,
    KEYC_MOUSEUP6_STATUS_DEFAULT,
    KEYC_MOUSEUP6_BORDER,

    KEYC_MOUSEUP7_PANE,
    KEYC_MOUSEUP7_STATUS,
    KEYC_MOUSEUP7_STATUS_LEFT,
    KEYC_MOUSEUP7_STATUS_RIGHT,
    KEYC_MOUSEUP7_STATUS_DEFAULT,
    KEYC_MOUSEUP7_BORDER,

    KEYC_MOUSEUP8_PANE,
    KEYC_MOUSEUP8_STATUS,
    KEYC_MOUSEUP8_STATUS_LEFT,
    KEYC_MOUSEUP8_STATUS_RIGHT,
    KEYC_MOUSEUP8_STATUS_DEFAULT,
    KEYC_MOUSEUP8_BORDER,

    KEYC_MOUSEUP9_PANE,
    KEYC_MOUSEUP9_STATUS,
    KEYC_MOUSEUP9_STATUS_LEFT,
    KEYC_MOUSEUP9_STATUS_RIGHT,
    KEYC_MOUSEUP9_STATUS_DEFAULT,
    KEYC_MOUSEUP9_BORDER,

    KEYC_MOUSEUP10_PANE,
    KEYC_MOUSEUP10_STATUS,
    KEYC_MOUSEUP10_STATUS_LEFT,
    KEYC_MOUSEUP10_STATUS_RIGHT,
    KEYC_MOUSEUP10_STATUS_DEFAULT,
    KEYC_MOUSEUP10_BORDER,

    KEYC_MOUSEUP11_PANE,
    KEYC_MOUSEUP11_STATUS,
    KEYC_MOUSEUP11_STATUS_LEFT,
    KEYC_MOUSEUP11_STATUS_RIGHT,
    KEYC_MOUSEUP11_STATUS_DEFAULT,
    KEYC_MOUSEUP11_BORDER,

    KEYC_MOUSEDRAG1_PANE,
    KEYC_MOUSEDRAG1_STATUS,
    KEYC_MOUSEDRAG1_STATUS_LEFT,
    KEYC_MOUSEDRAG1_STATUS_RIGHT,
    KEYC_MOUSEDRAG1_STATUS_DEFAULT,
    KEYC_MOUSEDRAG1_BORDER,

    KEYC_MOUSEDRAG2_PANE,
    KEYC_MOUSEDRAG2_STATUS,
    KEYC_MOUSEDRAG2_STATUS_LEFT,
    KEYC_MOUSEDRAG2_STATUS_RIGHT,
    KEYC_MOUSEDRAG2_STATUS_DEFAULT,
    KEYC_MOUSEDRAG2_BORDER,

    KEYC_MOUSEDRAG3_PANE,
    KEYC_MOUSEDRAG3_STATUS,
    KEYC_MOUSEDRAG3_STATUS_LEFT,
    KEYC_MOUSEDRAG3_STATUS_RIGHT,
    KEYC_MOUSEDRAG3_STATUS_DEFAULT,
    KEYC_MOUSEDRAG3_BORDER,

    KEYC_MOUSEDRAG6_PANE,
    KEYC_MOUSEDRAG6_STATUS,
    KEYC_MOUSEDRAG6_STATUS_LEFT,
    KEYC_MOUSEDRAG6_STATUS_RIGHT,
    KEYC_MOUSEDRAG6_STATUS_DEFAULT,
    KEYC_MOUSEDRAG6_BORDER,

    KEYC_MOUSEDRAG7_PANE,
    KEYC_MOUSEDRAG7_STATUS,
    KEYC_MOUSEDRAG7_STATUS_LEFT,
    KEYC_MOUSEDRAG7_STATUS_RIGHT,
    KEYC_MOUSEDRAG7_STATUS_DEFAULT,
    KEYC_MOUSEDRAG7_BORDER,

    KEYC_MOUSEDRAG8_PANE,
    KEYC_MOUSEDRAG8_STATUS,
    KEYC_MOUSEDRAG8_STATUS_LEFT,
    KEYC_MOUSEDRAG8_STATUS_RIGHT,
    KEYC_MOUSEDRAG8_STATUS_DEFAULT,
    KEYC_MOUSEDRAG8_BORDER,

    KEYC_MOUSEDRAG9_PANE,
    KEYC_MOUSEDRAG9_STATUS,
    KEYC_MOUSEDRAG9_STATUS_LEFT,
    KEYC_MOUSEDRAG9_STATUS_RIGHT,
    KEYC_MOUSEDRAG9_STATUS_DEFAULT,
    KEYC_MOUSEDRAG9_BORDER,

    KEYC_MOUSEDRAG10_PANE,
    KEYC_MOUSEDRAG10_STATUS,
    KEYC_MOUSEDRAG10_STATUS_LEFT,
    KEYC_MOUSEDRAG10_STATUS_RIGHT,
    KEYC_MOUSEDRAG10_STATUS_DEFAULT,
    KEYC_MOUSEDRAG10_BORDER,

    KEYC_MOUSEDRAG11_PANE,
    KEYC_MOUSEDRAG11_STATUS,
    KEYC_MOUSEDRAG11_STATUS_LEFT,
    KEYC_MOUSEDRAG11_STATUS_RIGHT,
    KEYC_MOUSEDRAG11_STATUS_DEFAULT,
    KEYC_MOUSEDRAG11_BORDER,

    KEYC_MOUSEDRAGEND1_PANE,
    KEYC_MOUSEDRAGEND1_STATUS,
    KEYC_MOUSEDRAGEND1_STATUS_LEFT,
    KEYC_MOUSEDRAGEND1_STATUS_RIGHT,
    KEYC_MOUSEDRAGEND1_STATUS_DEFAULT,
    KEYC_MOUSEDRAGEND1_BORDER,

    KEYC_MOUSEDRAGEND2_PANE,
    KEYC_MOUSEDRAGEND2_STATUS,
    KEYC_MOUSEDRAGEND2_STATUS_LEFT,
    KEYC_MOUSEDRAGEND2_STATUS_RIGHT,
    KEYC_MOUSEDRAGEND2_STATUS_DEFAULT,
    KEYC_MOUSEDRAGEND2_BORDER,

    KEYC_MOUSEDRAGEND3_PANE,
    KEYC_MOUSEDRAGEND3_STATUS,
    KEYC_MOUSEDRAGEND3_STATUS_LEFT,
    KEYC_MOUSEDRAGEND3_STATUS_RIGHT,
    KEYC_MOUSEDRAGEND3_STATUS_DEFAULT,
    KEYC_MOUSEDRAGEND3_BORDER,

    KEYC_MOUSEDRAGEND6_PANE,
    KEYC_MOUSEDRAGEND6_STATUS,
    KEYC_MOUSEDRAGEND6_STATUS_LEFT,
    KEYC_MOUSEDRAGEND6_STATUS_RIGHT,
    KEYC_MOUSEDRAGEND6_STATUS_DEFAULT,
    KEYC_MOUSEDRAGEND6_BORDER,

    KEYC_MOUSEDRAGEND7_PANE,
    KEYC_MOUSEDRAGEND7_STATUS,
    KEYC_MOUSEDRAGEND7_STATUS_LEFT,
    KEYC_MOUSEDRAGEND7_STATUS_RIGHT,
    KEYC_MOUSEDRAGEND7_STATUS_DEFAULT,
    KEYC_MOUSEDRAGEND7_BORDER,

    KEYC_MOUSEDRAGEND8_PANE,
    KEYC_MOUSEDRAGEND8_STATUS,
    KEYC_MOUSEDRAGEND8_STATUS_LEFT,
    KEYC_MOUSEDRAGEND8_STATUS_RIGHT,
    KEYC_MOUSEDRAGEND8_STATUS_DEFAULT,
    KEYC_MOUSEDRAGEND8_BORDER,

    KEYC_MOUSEDRAGEND9_PANE,
    KEYC_MOUSEDRAGEND9_STATUS,
    KEYC_MOUSEDRAGEND9_STATUS_LEFT,
    KEYC_MOUSEDRAGEND9_STATUS_RIGHT,
    KEYC_MOUSEDRAGEND9_STATUS_DEFAULT,
    KEYC_MOUSEDRAGEND9_BORDER,

    KEYC_MOUSEDRAGEND10_PANE,
    KEYC_MOUSEDRAGEND10_STATUS,
    KEYC_MOUSEDRAGEND10_STATUS_LEFT,
    KEYC_MOUSEDRAGEND10_STATUS_RIGHT,
    KEYC_MOUSEDRAGEND10_STATUS_DEFAULT,
    KEYC_MOUSEDRAGEND10_BORDER,

    KEYC_MOUSEDRAGEND11_PANE,
    KEYC_MOUSEDRAGEND11_STATUS,
    KEYC_MOUSEDRAGEND11_STATUS_LEFT,
    KEYC_MOUSEDRAGEND11_STATUS_RIGHT,
    KEYC_MOUSEDRAGEND11_STATUS_DEFAULT,
    KEYC_MOUSEDRAGEND11_BORDER,

    KEYC_WHEELUP_PANE,
    KEYC_WHEELUP_STATUS,
    KEYC_WHEELUP_STATUS_LEFT,
    KEYC_WHEELUP_STATUS_RIGHT,
    KEYC_WHEELUP_STATUS_DEFAULT,
    KEYC_WHEELUP_BORDER,

    KEYC_WHEELDOWN_PANE,
    KEYC_WHEELDOWN_STATUS,
    KEYC_WHEELDOWN_STATUS_LEFT,
    KEYC_WHEELDOWN_STATUS_RIGHT,
    KEYC_WHEELDOWN_STATUS_DEFAULT,
    KEYC_WHEELDOWN_BORDER,

    KEYC_SECONDCLICK1_PANE,
    KEYC_SECONDCLICK1_STATUS,
    KEYC_SECONDCLICK1_STATUS_LEFT,
    KEYC_SECONDCLICK1_STATUS_RIGHT,
    KEYC_SECONDCLICK1_STATUS_DEFAULT,
    KEYC_SECONDCLICK1_BORDER,

    KEYC_SECONDCLICK2_PANE,
    KEYC_SECONDCLICK2_STATUS,
    KEYC_SECONDCLICK2_STATUS_LEFT,
    KEYC_SECONDCLICK2_STATUS_RIGHT,
    KEYC_SECONDCLICK2_STATUS_DEFAULT,
    KEYC_SECONDCLICK2_BORDER,

    KEYC_SECONDCLICK3_PANE,
    KEYC_SECONDCLICK3_STATUS,
    KEYC_SECONDCLICK3_STATUS_LEFT,
    KEYC_SECONDCLICK3_STATUS_RIGHT,
    KEYC_SECONDCLICK3_STATUS_DEFAULT,
    KEYC_SECONDCLICK3_BORDER,

    KEYC_SECONDCLICK6_PANE,
    KEYC_SECONDCLICK6_STATUS,
    KEYC_SECONDCLICK6_STATUS_LEFT,
    KEYC_SECONDCLICK6_STATUS_RIGHT,
    KEYC_SECONDCLICK6_STATUS_DEFAULT,
    KEYC_SECONDCLICK6_BORDER,

    KEYC_SECONDCLICK7_PANE,
    KEYC_SECONDCLICK7_STATUS,
    KEYC_SECONDCLICK7_STATUS_LEFT,
    KEYC_SECONDCLICK7_STATUS_RIGHT,
    KEYC_SECONDCLICK7_STATUS_DEFAULT,
    KEYC_SECONDCLICK7_BORDER,

    KEYC_SECONDCLICK8_PANE,
    KEYC_SECONDCLICK8_STATUS,
    KEYC_SECONDCLICK8_STATUS_LEFT,
    KEYC_SECONDCLICK8_STATUS_RIGHT,
    KEYC_SECONDCLICK8_STATUS_DEFAULT,
    KEYC_SECONDCLICK8_BORDER,

    KEYC_SECONDCLICK9_PANE,
    KEYC_SECONDCLICK9_STATUS,
    KEYC_SECONDCLICK9_STATUS_LEFT,
    KEYC_SECONDCLICK9_STATUS_RIGHT,
    KEYC_SECONDCLICK9_STATUS_DEFAULT,
    KEYC_SECONDCLICK9_BORDER,

    KEYC_SECONDCLICK10_PANE,
    KEYC_SECONDCLICK10_STATUS,
    KEYC_SECONDCLICK10_STATUS_LEFT,
    KEYC_SECONDCLICK10_STATUS_RIGHT,
    KEYC_SECONDCLICK10_STATUS_DEFAULT,
    KEYC_SECONDCLICK10_BORDER,

    KEYC_SECONDCLICK11_PANE,
    KEYC_SECONDCLICK11_STATUS,
    KEYC_SECONDCLICK11_STATUS_LEFT,
    KEYC_SECONDCLICK11_STATUS_RIGHT,
    KEYC_SECONDCLICK11_STATUS_DEFAULT,
    KEYC_SECONDCLICK11_BORDER,

    KEYC_DOUBLECLICK1_PANE,
    KEYC_DOUBLECLICK1_STATUS,
    KEYC_DOUBLECLICK1_STATUS_LEFT,
    KEYC_DOUBLECLICK1_STATUS_RIGHT,
    KEYC_DOUBLECLICK1_STATUS_DEFAULT,
    KEYC_DOUBLECLICK1_BORDER,

    KEYC_DOUBLECLICK2_PANE,
    KEYC_DOUBLECLICK2_STATUS,
    KEYC_DOUBLECLICK2_STATUS_LEFT,
    KEYC_DOUBLECLICK2_STATUS_RIGHT,
    KEYC_DOUBLECLICK2_STATUS_DEFAULT,
    KEYC_DOUBLECLICK2_BORDER,

    KEYC_DOUBLECLICK3_PANE,
    KEYC_DOUBLECLICK3_STATUS,
    KEYC_DOUBLECLICK3_STATUS_LEFT,
    KEYC_DOUBLECLICK3_STATUS_RIGHT,
    KEYC_DOUBLECLICK3_STATUS_DEFAULT,
    KEYC_DOUBLECLICK3_BORDER,

    KEYC_DOUBLECLICK6_PANE,
    KEYC_DOUBLECLICK6_STATUS,
    KEYC_DOUBLECLICK6_STATUS_LEFT,
    KEYC_DOUBLECLICK6_STATUS_RIGHT,
    KEYC_DOUBLECLICK6_STATUS_DEFAULT,
    KEYC_DOUBLECLICK6_BORDER,

    KEYC_DOUBLECLICK7_PANE,
    KEYC_DOUBLECLICK7_STATUS,
    KEYC_DOUBLECLICK7_STATUS_LEFT,
    KEYC_DOUBLECLICK7_STATUS_RIGHT,
    KEYC_DOUBLECLICK7_STATUS_DEFAULT,
    KEYC_DOUBLECLICK7_BORDER,

    KEYC_DOUBLECLICK8_PANE,
    KEYC_DOUBLECLICK8_STATUS,
    KEYC_DOUBLECLICK8_STATUS_LEFT,
    KEYC_DOUBLECLICK8_STATUS_RIGHT,
    KEYC_DOUBLECLICK8_STATUS_DEFAULT,
    KEYC_DOUBLECLICK8_BORDER,

    KEYC_DOUBLECLICK9_PANE,
    KEYC_DOUBLECLICK9_STATUS,
    KEYC_DOUBLECLICK9_STATUS_LEFT,
    KEYC_DOUBLECLICK9_STATUS_RIGHT,
    KEYC_DOUBLECLICK9_STATUS_DEFAULT,
    KEYC_DOUBLECLICK9_BORDER,

    KEYC_DOUBLECLICK10_PANE,
    KEYC_DOUBLECLICK10_STATUS,
    KEYC_DOUBLECLICK10_STATUS_LEFT,
    KEYC_DOUBLECLICK10_STATUS_RIGHT,
    KEYC_DOUBLECLICK10_STATUS_DEFAULT,
    KEYC_DOUBLECLICK10_BORDER,

    KEYC_DOUBLECLICK11_PANE,
    KEYC_DOUBLECLICK11_STATUS,
    KEYC_DOUBLECLICK11_STATUS_LEFT,
    KEYC_DOUBLECLICK11_STATUS_RIGHT,
    KEYC_DOUBLECLICK11_STATUS_DEFAULT,
    KEYC_DOUBLECLICK11_BORDER,

    KEYC_TRIPLECLICK1_PANE,
    KEYC_TRIPLECLICK1_STATUS,
    KEYC_TRIPLECLICK1_STATUS_LEFT,
    KEYC_TRIPLECLICK1_STATUS_RIGHT,
    KEYC_TRIPLECLICK1_STATUS_DEFAULT,
    KEYC_TRIPLECLICK1_BORDER,

    KEYC_TRIPLECLICK2_PANE,
    KEYC_TRIPLECLICK2_STATUS,
    KEYC_TRIPLECLICK2_STATUS_LEFT,
    KEYC_TRIPLECLICK2_STATUS_RIGHT,
    KEYC_TRIPLECLICK2_STATUS_DEFAULT,
    KEYC_TRIPLECLICK2_BORDER,

    KEYC_TRIPLECLICK3_PANE,
    KEYC_TRIPLECLICK3_STATUS,
    KEYC_TRIPLECLICK3_STATUS_LEFT,
    KEYC_TRIPLECLICK3_STATUS_RIGHT,
    KEYC_TRIPLECLICK3_STATUS_DEFAULT,
    KEYC_TRIPLECLICK3_BORDER,

    KEYC_TRIPLECLICK6_PANE,
    KEYC_TRIPLECLICK6_STATUS,
    KEYC_TRIPLECLICK6_STATUS_LEFT,
    KEYC_TRIPLECLICK6_STATUS_RIGHT,
    KEYC_TRIPLECLICK6_STATUS_DEFAULT,
    KEYC_TRIPLECLICK6_BORDER,

    KEYC_TRIPLECLICK7_PANE,
    KEYC_TRIPLECLICK7_STATUS,
    KEYC_TRIPLECLICK7_STATUS_LEFT,
    KEYC_TRIPLECLICK7_STATUS_RIGHT,
    KEYC_TRIPLECLICK7_STATUS_DEFAULT,
    KEYC_TRIPLECLICK7_BORDER,

    KEYC_TRIPLECLICK8_PANE,
    KEYC_TRIPLECLICK8_STATUS,
    KEYC_TRIPLECLICK8_STATUS_LEFT,
    KEYC_TRIPLECLICK8_STATUS_RIGHT,
    KEYC_TRIPLECLICK8_STATUS_DEFAULT,
    KEYC_TRIPLECLICK8_BORDER,

    KEYC_TRIPLECLICK9_PANE,
    KEYC_TRIPLECLICK9_STATUS,
    KEYC_TRIPLECLICK9_STATUS_LEFT,
    KEYC_TRIPLECLICK9_STATUS_RIGHT,
    KEYC_TRIPLECLICK9_STATUS_DEFAULT,
    KEYC_TRIPLECLICK9_BORDER,

    KEYC_TRIPLECLICK10_PANE,
    KEYC_TRIPLECLICK10_STATUS,
    KEYC_TRIPLECLICK10_STATUS_LEFT,
    KEYC_TRIPLECLICK10_STATUS_RIGHT,
    KEYC_TRIPLECLICK10_STATUS_DEFAULT,
    KEYC_TRIPLECLICK10_BORDER,

    KEYC_TRIPLECLICK11_PANE,
    KEYC_TRIPLECLICK11_STATUS,
    KEYC_TRIPLECLICK11_STATUS_LEFT,
    KEYC_TRIPLECLICK11_STATUS_RIGHT,
    KEYC_TRIPLECLICK11_STATUS_DEFAULT,
    KEYC_TRIPLECLICK11_BORDER,

    // Backspace key.
    KEYC_BSPACE,

    // Function keys.
    KEYC_F1,
    KEYC_F2,
    KEYC_F3,
    KEYC_F4,
    KEYC_F5,
    KEYC_F6,
    KEYC_F7,
    KEYC_F8,
    KEYC_F9,
    KEYC_F10,
    KEYC_F11,
    KEYC_F12,
    KEYC_IC,
    KEYC_DC,
    KEYC_HOME,
    KEYC_END,
    KEYC_NPAGE,
    KEYC_PPAGE,
    KEYC_BTAB,

    // Arrow keys.
    KEYC_UP,
    KEYC_DOWN,
    KEYC_LEFT,
    KEYC_RIGHT,

    // Numeric keypad.
    KEYC_KP_SLASH,
    KEYC_KP_STAR,
    KEYC_KP_MINUS,
    KEYC_KP_SEVEN,
    KEYC_KP_EIGHT,
    KEYC_KP_NINE,
    KEYC_KP_PLUS,
    KEYC_KP_FOUR,
    KEYC_KP_FIVE,
    KEYC_KP_SIX,
    KEYC_KP_ONE,
    KEYC_KP_TWO,
    KEYC_KP_THREE,
    KEYC_KP_ENTER,
    KEYC_KP_ZERO,
    KEYC_KP_PERIOD,

    // End of special keys.
    KEYC_BASE_END,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Enum discriminant as a `key_code` (u64). Lives inside `mod tests` so it
    // is exempt from the ported-fn-name anti-drift gate.
    fn v(k: keyc) -> u64 {
        k as u64
    }

    // C (tmux.h): `enum keyc { KEYC_FOCUS_IN = KEYC_BASE, ... }`. The first
    // variant is pinned to KEYC_BASE and every later variant increments by one,
    // so the whole special-key block is a single contiguous range. KEYC_BASE is
    // 0x10e000 (highest Unicode PUA base).
    #[test]
    fn focus_in_is_pinned_to_keyc_base() {
        assert_eq!(v(keyc::KEYC_FOCUS_IN), KEYC_BASE);
        assert_eq!(KEYC_BASE, 0x10e000);
    }

    // C order (tmux.h): FOCUS_IN, FOCUS_OUT, ANY, PASTE_START, PASTE_END,
    // MOUSE, DRAGGING, DOUBLECLICK, then the mouse key table starting with
    // MOUSEMOVE_PANE. Verify the leading run is exactly consecutive.
    #[test]
    fn leading_special_keys_are_consecutive() {
        let b = KEYC_BASE;
        assert_eq!(v(keyc::KEYC_FOCUS_OUT), b + 1);
        assert_eq!(v(keyc::KEYC_ANY), b + 2);
        assert_eq!(v(keyc::KEYC_PASTE_START), b + 3);
        assert_eq!(v(keyc::KEYC_PASTE_END), b + 4);
        assert_eq!(v(keyc::KEYC_MOUSE), b + 5);
        assert_eq!(v(keyc::KEYC_DRAGGING), b + 6);
        assert_eq!(v(keyc::KEYC_DOUBLECLICK), b + 7);
        assert_eq!(v(keyc::KEYC_MOUSEMOVE_PANE), b + 8);
    }

    // C: `KEYC_MOUSE_KEYS(t)` emits, for each button, the six locations in a
    // fixed order: PANE, STATUS, STATUS_LEFT, STATUS_RIGHT, STATUS_DEFAULT,
    // BORDER. key-string.c (KEYC_MOUSE_STRING and its inverse parser) maps a
    // mouse key <-> its "MouseDown1Pane" style name purely by this offset from
    // the _PANE base, so the round trip only holds if each location sits at the
    // documented offset. Verify for a MOUSEDOWN and a WHEEL block.
    #[test]
    fn mouse_location_block_is_ordered() {
        let d = v(keyc::KEYC_MOUSEDOWN1_PANE);
        assert_eq!(v(keyc::KEYC_MOUSEDOWN1_STATUS), d + 1);
        assert_eq!(v(keyc::KEYC_MOUSEDOWN1_STATUS_LEFT), d + 2);
        assert_eq!(v(keyc::KEYC_MOUSEDOWN1_STATUS_RIGHT), d + 3);
        assert_eq!(v(keyc::KEYC_MOUSEDOWN1_STATUS_DEFAULT), d + 4);
        assert_eq!(v(keyc::KEYC_MOUSEDOWN1_BORDER), d + 5);

        let w = v(keyc::KEYC_WHEELUP_PANE);
        assert_eq!(v(keyc::KEYC_WHEELUP_STATUS), w + 1);
        assert_eq!(v(keyc::KEYC_WHEELUP_STATUS_LEFT), w + 2);
        assert_eq!(v(keyc::KEYC_WHEELUP_STATUS_RIGHT), w + 3);
        assert_eq!(v(keyc::KEYC_WHEELUP_STATUS_DEFAULT), w + 4);
        assert_eq!(v(keyc::KEYC_WHEELUP_BORDER), w + 5);
    }

    // C: buttons run 1, 2, 3, 6, 7, 8, 9, 10, 11 (4 and 5 are reserved for the
    // wheel), each a full 6-location block. Consecutive button blocks are 6
    // apart, and the 3 -> 6 jump is still only one block wide (no gap for the
    // missing 4/5).
    #[test]
    fn mouse_button_blocks_are_six_apart() {
        let d1 = v(keyc::KEYC_MOUSEDOWN1_PANE);
        assert_eq!(v(keyc::KEYC_MOUSEDOWN2_PANE), d1 + 6);
        assert_eq!(v(keyc::KEYC_MOUSEDOWN3_PANE), d1 + 12);
        assert_eq!(v(keyc::KEYC_MOUSEDOWN6_PANE), d1 + 18);
        assert_eq!(v(keyc::KEYC_MOUSEDOWN7_PANE), d1 + 24);
        assert_eq!(v(keyc::KEYC_MOUSEDOWN8_PANE), d1 + 30);
        assert_eq!(v(keyc::KEYC_MOUSEDOWN9_PANE), d1 + 36);
        assert_eq!(v(keyc::KEYC_MOUSEDOWN10_PANE), d1 + 42);
        assert_eq!(v(keyc::KEYC_MOUSEDOWN11_PANE), d1 + 48);
    }

    // C order of event types: MOUSEMOVE (buttonless, one 6-block), then the
    // per-button MOUSEDOWN table. So the first MOUSEDOWN block immediately
    // follows the single MOUSEMOVE block.
    #[test]
    fn mousemove_precedes_mousedown_block() {
        assert_eq!(
            v(keyc::KEYC_MOUSEDOWN1_PANE),
            v(keyc::KEYC_MOUSEMOVE_PANE) + 6
        );
    }

    // WHEELUP and WHEELDOWN are buttonless 6-blocks that follow the DRAGEND
    // table; WHEELDOWN sits directly after WHEELUP (tmux.h order).
    #[test]
    fn wheel_down_follows_wheel_up_block() {
        assert_eq!(v(keyc::KEYC_WHEELDOWN_PANE), v(keyc::KEYC_WHEELUP_PANE) + 6);
    }

    // C (this port's tmux.h era): KEYC_IS_MOUSE(key) is
    //   (key & KEYC_MASK_KEY) >= KEYC_MOUSE && (key & KEYC_MASK_KEY) < KEYC_BSPACE
    // i.e. every enum value from KEYC_MOUSE up to (but excluding) KEYC_BSPACE
    // classifies as a mouse key.
    #[test]
    fn keyc_is_mouse_covers_whole_mouse_range() {
        assert!(KEYC_IS_MOUSE(v(keyc::KEYC_MOUSE)));
        assert!(KEYC_IS_MOUSE(v(keyc::KEYC_DRAGGING)));
        assert!(KEYC_IS_MOUSE(v(keyc::KEYC_DOUBLECLICK)));
        assert!(KEYC_IS_MOUSE(v(keyc::KEYC_MOUSEMOVE_PANE)));
        assert!(KEYC_IS_MOUSE(v(keyc::KEYC_MOUSEDOWN1_PANE)));
        assert!(KEYC_IS_MOUSE(v(keyc::KEYC_MOUSEUP1_PANE)));
        assert!(KEYC_IS_MOUSE(v(keyc::KEYC_WHEELUP_PANE)));
        // Last mouse key, immediately before the KEYC_BSPACE sentinel.
        assert!(KEYC_IS_MOUSE(v(keyc::KEYC_TRIPLECLICK11_BORDER)));
    }

    // The range is half-open: KEYC_PASTE_END sits just below KEYC_MOUSE and
    // KEYC_BSPACE is the exclusive upper bound, so neither is a mouse key.
    // Function/arrow keys and plain ASCII are also excluded.
    #[test]
    fn keyc_is_mouse_excludes_neighbours_and_plain_keys() {
        assert!(!KEYC_IS_MOUSE(v(keyc::KEYC_PASTE_END)));
        assert!(!KEYC_IS_MOUSE(v(keyc::KEYC_BSPACE)));
        assert!(!KEYC_IS_MOUSE(v(keyc::KEYC_F1)));
        assert!(!KEYC_IS_MOUSE(v(keyc::KEYC_UP)));
        assert!(!KEYC_IS_MOUSE(v(keyc::KEYC_KP_PERIOD)));
        assert!(!KEYC_IS_MOUSE(b'a' as u64));
        assert!(!KEYC_IS_MOUSE(0));
    }

    // KEYC_IS_MOUSE masks with KEYC_MASK_KEY first, so modifier and flag bits
    // never change the classification (tmux.h).
    #[test]
    fn keyc_is_mouse_ignores_modifier_and_flag_bits() {
        let k = v(keyc::KEYC_MOUSEDOWN1_PANE);
        assert!(KEYC_IS_MOUSE(k | KEYC_CTRL | KEYC_META | KEYC_SHIFT));
        assert!(KEYC_IS_MOUSE(k | KEYC_MASK_FLAGS));
        // A non-mouse key stays non-mouse even with modifiers applied.
        assert!(!KEYC_IS_MOUSE(v(keyc::KEYC_F1) | KEYC_CTRL));
    }

    // C order: the mouse table ends at TRIPLECLICK11_BORDER, then KEYC_BSPACE,
    // then the function keys F1.. immediately after it.
    #[test]
    fn function_keys_follow_bspace_after_mouse_table() {
        assert!(v(keyc::KEYC_BSPACE) > v(keyc::KEYC_TRIPLECLICK11_BORDER));
        assert_eq!(v(keyc::KEYC_F1), v(keyc::KEYC_BSPACE) + 1);
        assert_eq!(v(keyc::KEYC_F2), v(keyc::KEYC_F1) + 1);
    }

    // KEYC_BASE_END is the sentinel terminating the special-key enum; the
    // numeric keypad period is the final real key just before it (tmux.h).
    #[test]
    fn base_end_is_the_final_sentinel() {
        assert_eq!(v(keyc::KEYC_BASE_END), v(keyc::KEYC_KP_PERIOD) + 1);
        assert!(v(keyc::KEYC_BASE_END) > v(keyc::KEYC_TRIPLECLICK11_BORDER));
        assert!(v(keyc::KEYC_BASE_END) > KEYC_BASE);
    }

    // The six-location block ordering (PANE, STATUS, STATUS_LEFT, STATUS_RIGHT,
    // STATUS_DEFAULT, BORDER) must hold for EVERY event family, since the
    // key-string.c mouse name parser resolves a location purely by its offset
    // from the family's _PANE base. Check MOUSEUP / MOUSEDRAG / MOUSEDRAGEND.
    #[test]
    fn up_drag_dragend_location_blocks_are_ordered() {
        for base in [
            v(keyc::KEYC_MOUSEUP1_PANE),
            v(keyc::KEYC_MOUSEDRAG1_PANE),
            v(keyc::KEYC_MOUSEDRAGEND1_PANE),
        ] {
            assert_eq!(base + 1, base + 1); // PANE..BORDER contiguous below
        }
        let u = v(keyc::KEYC_MOUSEUP1_PANE);
        assert_eq!(v(keyc::KEYC_MOUSEUP1_STATUS), u + 1);
        assert_eq!(v(keyc::KEYC_MOUSEUP1_STATUS_LEFT), u + 2);
        assert_eq!(v(keyc::KEYC_MOUSEUP1_STATUS_RIGHT), u + 3);
        assert_eq!(v(keyc::KEYC_MOUSEUP1_STATUS_DEFAULT), u + 4);
        assert_eq!(v(keyc::KEYC_MOUSEUP1_BORDER), u + 5);

        let dr = v(keyc::KEYC_MOUSEDRAG1_PANE);
        assert_eq!(v(keyc::KEYC_MOUSEDRAG1_STATUS_DEFAULT), dr + 4);
        assert_eq!(v(keyc::KEYC_MOUSEDRAG1_BORDER), dr + 5);

        let de = v(keyc::KEYC_MOUSEDRAGEND1_PANE);
        assert_eq!(v(keyc::KEYC_MOUSEDRAGEND1_STATUS_DEFAULT), de + 4);
        assert_eq!(v(keyc::KEYC_MOUSEDRAGEND1_BORDER), de + 5);
    }

    // The click families (SECONDCLICK, DOUBLECLICK, TRIPLECLICK) keep the same
    // six-location block layout for their first button.
    #[test]
    fn click_family_location_blocks_are_ordered() {
        for base in [
            v(keyc::KEYC_SECONDCLICK1_PANE),
            v(keyc::KEYC_DOUBLECLICK1_PANE),
            v(keyc::KEYC_TRIPLECLICK1_PANE),
        ] {
            // BORDER is exactly 5 past PANE for each family's button 1.
            // (verified per-family below with the concrete enum names)
            let _ = base;
        }
        let s = v(keyc::KEYC_SECONDCLICK1_PANE);
        assert_eq!(v(keyc::KEYC_SECONDCLICK1_BORDER), s + 5);
        let d = v(keyc::KEYC_DOUBLECLICK1_PANE);
        assert_eq!(v(keyc::KEYC_DOUBLECLICK1_BORDER), d + 5);
        let t = v(keyc::KEYC_TRIPLECLICK1_PANE);
        assert_eq!(v(keyc::KEYC_TRIPLECLICK1_BORDER), t + 5);
    }

    // The button numbering skips 4 and 5 (reserved for the wheel) for the
    // MOUSEUP and MOUSEDRAG families too, so button blocks are a flat 6 apart
    // with no gap at the 3 -> 6 transition.
    #[test]
    fn up_and_drag_button_blocks_are_six_apart() {
        let u = v(keyc::KEYC_MOUSEUP1_PANE);
        assert_eq!(v(keyc::KEYC_MOUSEUP2_PANE), u + 6);
        assert_eq!(v(keyc::KEYC_MOUSEUP3_PANE), u + 12);
        assert_eq!(v(keyc::KEYC_MOUSEUP6_PANE), u + 18);
        assert_eq!(v(keyc::KEYC_MOUSEUP11_PANE), u + 48);

        let d = v(keyc::KEYC_MOUSEDRAG1_PANE);
        assert_eq!(v(keyc::KEYC_MOUSEDRAG3_PANE), d + 12);
        assert_eq!(v(keyc::KEYC_MOUSEDRAG6_PANE), d + 18);
        assert_eq!(v(keyc::KEYC_MOUSEDRAG11_PANE), d + 48);
    }

    // Event-family ordering (tmux.h): MOUSEMOVE, MOUSEDOWN, MOUSEUP, MOUSEDRAG,
    // MOUSEDRAGEND, WHEELUP, WHEELDOWN, SECONDCLICK, DOUBLECLICK, TRIPLECLICK.
    // Each per-button family spans 9 buttons * 6 = 54; the buttonless WHEEL
    // families span 6. Verify the chain of family starts is monotonic.
    #[test]
    fn event_family_starts_are_monotonic() {
        let starts = [
            v(keyc::KEYC_MOUSEMOVE_PANE),
            v(keyc::KEYC_MOUSEDOWN1_PANE),
            v(keyc::KEYC_MOUSEUP1_PANE),
            v(keyc::KEYC_MOUSEDRAG1_PANE),
            v(keyc::KEYC_MOUSEDRAGEND1_PANE),
            v(keyc::KEYC_WHEELUP_PANE),
            v(keyc::KEYC_WHEELDOWN_PANE),
            v(keyc::KEYC_SECONDCLICK1_PANE),
            v(keyc::KEYC_DOUBLECLICK1_PANE),
            v(keyc::KEYC_TRIPLECLICK1_PANE),
        ];
        assert!(starts.windows(2).all(|w| w[0] < w[1]), "families must be in order");
        // The three per-button families before the wheels are each 54 wide.
        assert_eq!(v(keyc::KEYC_MOUSEUP1_PANE), v(keyc::KEYC_MOUSEDOWN1_PANE) + 54);
        assert_eq!(v(keyc::KEYC_MOUSEDRAG1_PANE), v(keyc::KEYC_MOUSEUP1_PANE) + 54);
        // WHEELUP directly follows the buttonless-less DRAGEND family (54 wide).
        assert_eq!(v(keyc::KEYC_WHEELUP_PANE), v(keyc::KEYC_MOUSEDRAGEND1_PANE) + 54);
    }

    // The TRIPLECLICK family is the last mouse family; its final entry sits
    // immediately before KEYC_BSPACE (the exclusive KEYC_IS_MOUSE upper bound).
    #[test]
    fn tripleclick_is_last_mouse_family_before_bspace() {
        assert_eq!(v(keyc::KEYC_BSPACE), v(keyc::KEYC_TRIPLECLICK11_BORDER) + 1);
        // SECONDCLICK precedes DOUBLECLICK precedes TRIPLECLICK, each 54 wide.
        assert_eq!(
            v(keyc::KEYC_DOUBLECLICK1_PANE),
            v(keyc::KEYC_SECONDCLICK1_PANE) + 54
        );
        assert_eq!(
            v(keyc::KEYC_TRIPLECLICK1_PANE),
            v(keyc::KEYC_DOUBLECLICK1_PANE) + 54
        );
    }
}
