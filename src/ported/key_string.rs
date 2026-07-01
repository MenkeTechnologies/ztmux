// Copyright (c) 2007 Nicholas Marriott <nichu8ott@gmail.com>
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
use crate::libc::{memcpy, snprintf, sscanf};
use crate::*;

unsafe impl Sync for key_string_table_entry {}
#[repr(C)]
#[derive(Copy, Clone)]
struct key_string_table_entry {
    string: &'static str,
    key: key_code,
}

impl key_string_table_entry {
    const fn new(string: &'static str, key: key_code) -> Self {
        Self { string, key }
    }
}

// #define KEYC_MOUSE_KEY(name)
// 	KEYC_ ## name ## _PANE,
// 	KEYC_ ## name ## _STATUS,
// 	KEYC_ ## name ## _STATUS_LEFT,
// 	KEYC_ ## name ## _STATUS_RIGHT,
// 	KEYC_ ## name ## _STATUS_DEFAULT,
// 	KEYC_ ## name ## _BORDER
// #define KEYC_MOUSE_STRING(name, s)
// 	{ #s "Pane", KEYC_ ## name ## _PANE },
// 	{ #s "Status", KEYC_ ## name ## _STATUS },
// 	{ #s "StatusLeft", KEYC_ ## name ## _STATUS_LEFT },
// 	{ #s "StatusRight", KEYC_ ## name ## _STATUS_RIGHT },
// 	{ #s "StatusDefault", KEYC_ ## name ## _STATUS_DEFAULT },
// 	{ #s "Border", KEYC_ ## name ## _BORDER }
macro_rules! KEYC_MOUSE_STRING {
    ($name:ident, $s:literal) => {
        ::paste::paste! {
            [
                key_string_table_entry{string: concat!($s, "Pane"), key: keyc::[<KEYC_ $name _PANE>] as u64},
                key_string_table_entry{string: concat!($s, "Status"), key: keyc::[<KEYC_ $name _STATUS>] as u64 },
                key_string_table_entry{string: concat!($s, "StatusLeft"), key: keyc::[<KEYC_ $name _STATUS_LEFT>] as u64},
                key_string_table_entry{string: concat!($s, "StatusRight"), key: keyc::[<KEYC_ $name _STATUS_RIGHT>] as u64},
                key_string_table_entry{string: concat!($s, "StatusDefault"), key: keyc::[<KEYC_ $name _STATUS_DEFAULT>] as u64 },
                key_string_table_entry{string: concat!($s, "Border"), key: keyc::[<KEYC_ $name _BORDER>] as u64},
            ]
        }
    };
}

macro_rules! concat_array {
    ($out:ident, $out_i: ident, $in:expr) => {
        let tmp = $in;
        let mut tmp_i = 0usize;
        while tmp_i < tmp.len() {
            $out[$out_i] = tmp[tmp_i];
            $out_i += 1;
            tmp_i += 1;
        }
    };
}

// N. B. the order of the enum variants is incremental
// KEYC_MOUSEDOWN1_PANE,
// KEYC_MOUSEDOWN1_STATUS,
// KEYC_MOUSEDOWN1_STATUS_LEFT,
// KEYC_MOUSEDOWN1_STATUS_RIGHT,
// KEYC_MOUSEDOWN1_STATUS_DEFAULT,
// KEYC_MOUSEDOWN1_BORDER,
macro_rules! KEYC_MOUSE_STRING_I {
    ($name:ident, $s:literal, $i:literal) => {
        ::paste::paste! {
            [
                key_string_table_entry{string: concat!($s, $i, "Pane"), key: keyc::[<KEYC_ $name $i _PANE>] as u64},
                key_string_table_entry{string: concat!($s, $i, "Status"), key: keyc::[<KEYC_ $name $i _STATUS>] as u64 },
                key_string_table_entry{string: concat!($s, $i, "StatusLeft"), key: keyc::[<KEYC_ $name $i _STATUS_LEFT>] as u64},
                key_string_table_entry{string: concat!($s, $i, "StatusRight"), key: keyc::[<KEYC_ $name $i _STATUS_RIGHT>] as u64},
                key_string_table_entry{string: concat!($s, $i, "StatusDefault"), key: keyc::[<KEYC_ $name $i _STATUS_DEFAULT>] as u64 },
                key_string_table_entry{string: concat!($s, $i, "Border"), key: keyc::[<KEYC_ $name $i _BORDER>] as u64},
            ]
        }
    };
}

macro_rules! KEYC_MOUSE_STRING11 {
    ($out:ident, $out_i: ident, $name:ident, $s:literal) => {
        concat_array!($out, $out_i, KEYC_MOUSE_STRING_I!($name, $s, 1));
        concat_array!($out, $out_i, KEYC_MOUSE_STRING_I!($name, $s, 2));
        concat_array!($out, $out_i, KEYC_MOUSE_STRING_I!($name, $s, 3));
        // yes, there's no 4 or 5
        concat_array!($out, $out_i, KEYC_MOUSE_STRING_I!($name, $s, 6));
        concat_array!($out, $out_i, KEYC_MOUSE_STRING_I!($name, $s, 7));
        concat_array!($out, $out_i, KEYC_MOUSE_STRING_I!($name, $s, 8));
        concat_array!($out, $out_i, KEYC_MOUSE_STRING_I!($name, $s, 9));
        concat_array!($out, $out_i, KEYC_MOUSE_STRING_I!($name, $s, 10));
        concat_array!($out, $out_i, KEYC_MOUSE_STRING_I!($name, $s, 11));
    };
}

static KEY_STRING_TABLE: [key_string_table_entry; 469] = const {
    let mut out_i: usize = 0;
    let mut out: [key_string_table_entry; 469] =
        [key_string_table_entry { string: "", key: 0 }; 469];

    let function_keys = [
        key_string_table_entry::new("F1", keyc::KEYC_F1 as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("F2", keyc::KEYC_F2 as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("F3", keyc::KEYC_F3 as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("F4", keyc::KEYC_F4 as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("F5", keyc::KEYC_F5 as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("F6", keyc::KEYC_F6 as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("F7", keyc::KEYC_F7 as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("F8", keyc::KEYC_F8 as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("F9", keyc::KEYC_F9 as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("F10", keyc::KEYC_F10 as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("F11", keyc::KEYC_F11 as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("F12", keyc::KEYC_F12 as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("IC", keyc::KEYC_IC as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("Insert", keyc::KEYC_IC as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("DC", keyc::KEYC_DC as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("Delete", keyc::KEYC_DC as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("Home", keyc::KEYC_HOME as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("End", keyc::KEYC_END as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("NPage", keyc::KEYC_NPAGE as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("PageDown", keyc::KEYC_NPAGE as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("PgDn", keyc::KEYC_NPAGE as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("PPage", keyc::KEYC_PPAGE as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("PageUp", keyc::KEYC_PPAGE as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("PgUp", keyc::KEYC_PPAGE as u64 | KEYC_IMPLIED_META),
        key_string_table_entry::new("BTab", keyc::KEYC_BTAB as u64),
        key_string_table_entry::new("Space", ' ' as key_code),
        key_string_table_entry::new("BSpace", keyc::KEYC_BSPACE as u64),
        // C0 control characters, with the exception of Tab, Enter,
        // and Esc, should never appear as keys. We still render them,
        // so to be able to spot them in logs in case of an abnormality.
        key_string_table_entry::new("[NUL]", c0::C0_NUL as u64),
        key_string_table_entry::new("[SOH]", c0::C0_SOH as u64),
        key_string_table_entry::new("[STX]", c0::C0_STX as u64),
        key_string_table_entry::new("[ETX]", c0::C0_ETX as u64),
        key_string_table_entry::new("[EOT]", c0::C0_EOT as u64),
        key_string_table_entry::new("[ENQ]", c0::C0_ENQ as u64),
        key_string_table_entry::new("[ASC]", c0::C0_ASC as u64),
        key_string_table_entry::new("[BEL]", c0::C0_BEL as u64),
        key_string_table_entry::new("[BS]", c0::C0_BS as u64),
        key_string_table_entry::new("Tab", c0::C0_HT as u64),
        key_string_table_entry::new("[LF]", c0::C0_LF as u64),
        key_string_table_entry::new("[VT]", c0::C0_VT as u64),
        key_string_table_entry::new("[FF]", c0::C0_FF as u64),
        key_string_table_entry::new("Enter", c0::C0_CR as u64),
        key_string_table_entry::new("[SO]", c0::C0_SO as u64),
        key_string_table_entry::new("[SI]", c0::C0_SI as u64),
        key_string_table_entry::new("[DLE]", c0::C0_DLE as u64),
        key_string_table_entry::new("[DC1]", c0::C0_DC1 as u64),
        key_string_table_entry::new("[DC2]", c0::C0_DC2 as u64),
        key_string_table_entry::new("[DC3]", c0::C0_DC3 as u64),
        key_string_table_entry::new("[DC4]", c0::C0_DC4 as u64),
        key_string_table_entry::new("[NAK]", c0::C0_NAK as u64),
        key_string_table_entry::new("[SYN]", c0::C0_SYN as u64),
        key_string_table_entry::new("[ETB]", c0::C0_ETB as u64),
        key_string_table_entry::new("[CAN]", c0::C0_CAN as u64),
        key_string_table_entry::new("[EM]", c0::C0_EM as u64),
        key_string_table_entry::new("[SUB]", c0::C0_SUB as u64),
        key_string_table_entry::new("Escape", c0::C0_ESC as u64),
        key_string_table_entry::new("[FS]", c0::C0_FS as u64),
        key_string_table_entry::new("[GS]", c0::C0_GS as u64),
        key_string_table_entry::new("[RS]", c0::C0_RS as u64),
        key_string_table_entry::new("[US]", c0::C0_US as u64),
        // Arrow keys.
        key_string_table_entry::new("Up", keyc::KEYC_UP as u64 | KEYC_CURSOR | KEYC_IMPLIED_META),
        key_string_table_entry::new(
            "Down",
            keyc::KEYC_DOWN as u64 | KEYC_CURSOR | KEYC_IMPLIED_META,
        ),
        key_string_table_entry::new(
            "Left",
            keyc::KEYC_LEFT as u64 | KEYC_CURSOR | KEYC_IMPLIED_META,
        ),
        key_string_table_entry::new(
            "Right",
            keyc::KEYC_RIGHT as u64 | KEYC_CURSOR | KEYC_IMPLIED_META,
        ),
        // Numeric keypad.
        key_string_table_entry::new("KP/", keyc::KEYC_KP_SLASH as u64 | KEYC_KEYPAD),
        key_string_table_entry::new("KP*", keyc::KEYC_KP_STAR as u64 | KEYC_KEYPAD),
        key_string_table_entry::new("KP-", keyc::KEYC_KP_MINUS as u64 | KEYC_KEYPAD),
        key_string_table_entry::new("KP7", keyc::KEYC_KP_SEVEN as u64 | KEYC_KEYPAD),
        key_string_table_entry::new("KP8", keyc::KEYC_KP_EIGHT as u64 | KEYC_KEYPAD),
        key_string_table_entry::new("KP9", keyc::KEYC_KP_NINE as u64 | KEYC_KEYPAD),
        key_string_table_entry::new("KP+", keyc::KEYC_KP_PLUS as u64 | KEYC_KEYPAD),
        key_string_table_entry::new("KP4", keyc::KEYC_KP_FOUR as u64 | KEYC_KEYPAD),
        key_string_table_entry::new("KP5", keyc::KEYC_KP_FIVE as u64 | KEYC_KEYPAD),
        key_string_table_entry::new("KP6", keyc::KEYC_KP_SIX as u64 | KEYC_KEYPAD),
        key_string_table_entry::new("KP1", keyc::KEYC_KP_ONE as u64 | KEYC_KEYPAD),
        key_string_table_entry::new("KP2", keyc::KEYC_KP_TWO as u64 | KEYC_KEYPAD),
        key_string_table_entry::new("KP3", keyc::KEYC_KP_THREE as u64 | KEYC_KEYPAD),
        key_string_table_entry::new("KPEnter", keyc::KEYC_KP_ENTER as u64 | KEYC_KEYPAD),
        key_string_table_entry::new("KP0", keyc::KEYC_KP_ZERO as u64 | KEYC_KEYPAD),
        key_string_table_entry::new("KP.", keyc::KEYC_KP_PERIOD as u64 | KEYC_KEYPAD),
    ];

    concat_array!(out, out_i, function_keys);

    // Mouse keys.
    KEYC_MOUSE_STRING11!(out, out_i, MOUSEDOWN, "MouseDown");
    KEYC_MOUSE_STRING11!(out, out_i, MOUSEUP, "MouseUp");
    KEYC_MOUSE_STRING11!(out, out_i, MOUSEDRAG, "MouseDrag");
    KEYC_MOUSE_STRING11!(out, out_i, MOUSEDRAGEND, "MouseDragEnd");
    concat_array!(out, out_i, KEYC_MOUSE_STRING!(WHEELUP, "WheelUp"));
    concat_array!(out, out_i, KEYC_MOUSE_STRING!(WHEELDOWN, "WheelDown"));
    KEYC_MOUSE_STRING11!(out, out_i, SECONDCLICK, "SecondClick");
    KEYC_MOUSE_STRING11!(out, out_i, DOUBLECLICK, "DoubleClick");
    KEYC_MOUSE_STRING11!(out, out_i, TRIPLECLICK, "TripleClick");

    out
};

/// Find key string in table.
/// C `vendor/tmux/key-string.c:196`: `static key_code key_string_search_table(const char *string)`
pub unsafe fn key_string_search_table(string: *const u8) -> key_code {
    unsafe {
        for key_string in &KEY_STRING_TABLE {
            if strcaseeq_(string, key_string.string) {
                return key_string.key;
            }
        }

        let mut user = 0u32;
        if sscanf(string.cast(), c"User%u".as_ptr(), &raw mut user) == 1
            && user <= KEYC_NUSER as u32
        {
            return KEYC_USER + user as u64;
        }
    }

    KEYC_UNKNOWN
}

/// Find modifiers.
/// C `vendor/tmux/key-string.c:213`: `static key_code key_string_get_modifiers(const char **string)`
pub unsafe fn key_string_get_modifiers(string: *mut *const u8) -> key_code {
    unsafe {
        let mut modifiers: key_code = 0;

        while **string != b'\0' && *(*string).add(1) == b'-' {
            match **string {
                b'C' | b'c' => {
                    modifiers |= KEYC_CTRL;
                }
                b'M' | b'm' => {
                    modifiers |= KEYC_META;
                }
                b'S' | b's' => {
                    modifiers |= KEYC_SHIFT;
                }
                _ => {
                    *string = null_mut();
                    return 0;
                }
            }
            (*string) = (*string).add(2);
        }

        modifiers
    }
}

// TODO
const MB_LEN_MAX: usize = 16;

/// Lookup a string and convert to a key value.
/// C `vendor/tmux/key-string.c:243`: `key_code key_string_lookup_string(const char *string)`
pub unsafe fn key_string_lookup_string(mut string: *const u8) -> key_code {
    unsafe {
        let mut key: key_code;
        let mut modifiers: key_code = 0;
        let mut u: u32 = 0;
        let mut ud: utf8_data = zeroed();
        let mut uc: utf8_char = 0;

        let mut m = [MaybeUninit::<u8>::uninit(); MB_LEN_MAX + 1];

        // Is this no key or any key?
        if strcaseeq_(string, "None") {
            return KEYC_NONE;
        }
        if strcaseeq_(string, "Any") {
            return keyc::KEYC_ANY as key_code;
        }

        // Is this a hexadecimal value?
        if *string == b'0' && *string.add(1) == b'x' {
            if sscanf(string.add(2).cast(), c"%x".as_ptr(), &raw mut u) != 1 {
                return KEYC_UNKNOWN;
            }
            if u < 32 {
                return u as u64;
            }
            let mlen = wctomb(m.as_mut_slice().as_mut_ptr().cast(), u as i32);
            if mlen <= 0 || mlen > MB_LEN_MAX as i32 {
                return KEYC_UNKNOWN;
            }
            m[mlen as usize].write(b'\0');

            let udp: *mut utf8_data = utf8_fromcstr(m.as_slice().as_ptr().cast());
            if udp.is_null()
                || (*udp).size == 0
                || (*udp.add(1)).size != 0
                || utf8_from_data(udp, &raw mut uc) != utf8_state::UTF8_DONE
            {
                free_(udp);
                return KEYC_UNKNOWN;
            }
            free_(udp);
            return uc as u64;
        }

        // Check for short Ctrl key.
        if *string == b'^' && *string.add(1) != b'\0' {
            if *string.add(2) == b'\0' {
                return (*string.add(1)).to_ascii_lowercase() as u64 | KEYC_CTRL;
            }
            modifiers |= KEYC_CTRL;
            string = string.add(1);
        }

        // Check for modifiers.
        modifiers |= key_string_get_modifiers(&raw mut string);
        if string.is_null() || *string == b'\0' {
            return KEYC_UNKNOWN;
        }

        // Is this a standard ASCII key?
        if *string.add(1) == b'\0' && *string <= 127 {
            key = *string as u64;
            if key < 32 {
                return KEYC_UNKNOWN;
            }
        } else {
            // Try as a UTF-8 key.
            let mut more: utf8_state = utf8_open(&raw mut ud, *string);
            if more == utf8_state::UTF8_MORE {
                if strlen(string) != ud.size as usize {
                    return KEYC_UNKNOWN;
                }
                for i in 1..ud.size {
                    more = utf8_append(&raw mut ud, *string.add(i as usize));
                }
                if more != utf8_state::UTF8_DONE {
                    return KEYC_UNKNOWN;
                }
                if utf8_from_data(&raw const ud, &raw mut uc) != utf8_state::UTF8_DONE {
                    return KEYC_UNKNOWN;
                }
                return uc as u64 | modifiers;
            }

            // Otherwise look the key up in the table.
            key = key_string_search_table(string);
            if key == KEYC_UNKNOWN {
                return KEYC_UNKNOWN;
            }
            if !modifiers & KEYC_META != 0 {
                key &= !KEYC_IMPLIED_META;
            }
        }

        key | modifiers
    }
}

/// Convert a key code into string format, with prefix if necessary.
/// C `vendor/tmux/key-string.c:327`: `const char *key_string_lookup_key(key_code key, int with_flags)`
pub unsafe fn key_string_lookup_key(mut key: key_code, with_flags: i32) -> *const u8 {
    let sizeof_out: usize = 64;
    static mut OUT: [u8; 64] = [0; 64];
    unsafe {
        let saved = key;
        let sizeof_tmp: usize = 8;
        let mut tmp: [u8; 8] = [0; 8];
        let s: *const u8;

        OUT[0] = b'\0';

        'out: {
            'append: {
                // Literal keys are themselves.
                if key & KEYC_LITERAL != 0 {
                    snprintf(
                        (&raw mut OUT).cast(),
                        sizeof_out,
                        c"%c".as_ptr(),
                        (key & 0xff) as i32,
                    );
                    break 'out;
                }

                // Fill in the modifiers.
                if key & KEYC_CTRL != 0 {
                    strlcat(&raw mut OUT as *mut u8, c!("C-"), sizeof_out);
                }
                if key & KEYC_META != 0 {
                    strlcat(&raw mut OUT as *mut u8, c!("M-"), sizeof_out);
                }
                if key & KEYC_SHIFT != 0 {
                    strlcat(&raw mut OUT as *mut u8, c!("S-"), sizeof_out);
                }
                key &= KEYC_MASK_KEY;

                // Handle no key.
                if key == KEYC_NONE {
                    s = c!("None");
                    break 'append;
                }

                // Handle special keys.
                if key == KEYC_UNKNOWN {
                    s = c!("Unknown");
                    break 'append;
                }
                if key == keyc::KEYC_ANY as u64 {
                    s = c!("Any");
                    break 'append;
                }
                if key == keyc::KEYC_FOCUS_IN as u64 {
                    s = c!("FocusIn");
                    break 'append;
                }
                if key == keyc::KEYC_FOCUS_OUT as u64 {
                    s = c!("FocusOut");
                    break 'append;
                }
                if key == keyc::KEYC_PASTE_START as u64 {
                    s = c!("PasteStart");
                    break 'append;
                }
                if key == keyc::KEYC_PASTE_END as u64 {
                    s = c!("PasteEnd");
                    break 'append;
                }
                if key == keyc::KEYC_MOUSE as u64 {
                    s = c!("Mouse");
                    break 'append;
                }
                if key == keyc::KEYC_DRAGGING as u64 {
                    s = c!("Dragging");
                    break 'append;
                }
                if key == keyc::KEYC_MOUSEMOVE_PANE as u64 {
                    s = c!("MouseMovePane");
                    break 'append;
                }
                if key == keyc::KEYC_MOUSEMOVE_STATUS as u64 {
                    s = c!("MouseMoveStatus");
                    break 'append;
                }
                if key == keyc::KEYC_MOUSEMOVE_STATUS_LEFT as u64 {
                    s = c!("MouseMoveStatusLeft");
                    break 'append;
                }
                if key == keyc::KEYC_MOUSEMOVE_STATUS_RIGHT as u64 {
                    s = c!("MouseMoveStatusRight");
                    break 'append;
                }
                if key == keyc::KEYC_MOUSEMOVE_BORDER as u64 {
                    s = c!("MouseMoveBorder");
                    break 'append;
                }
                if (KEYC_USER..KEYC_USER_END).contains(&key) {
                    snprintf(
                        (&raw mut tmp).cast(),
                        sizeof_tmp,
                        c"User%u".as_ptr(),
                        (key - KEYC_USER) as u8 as u32,
                    );
                    strlcat(
                        &raw mut OUT as *mut u8,
                        &raw const tmp as *const u8,
                        sizeof_out,
                    );
                    break 'out;
                }

                // Try the key against the string table.
                if let Some(i) = KEY_STRING_TABLE
                    .iter()
                    .position(|e| key == e.key & KEYC_MASK_KEY)
                {
                    strlcat_(
                        &raw mut OUT as *mut u8,
                        KEY_STRING_TABLE[i].string,
                        sizeof_out,
                    );
                    break 'out;
                }

                // Is this a Unicode key?
                if KEYC_IS_UNICODE(key) {
                    let ud = utf8_to_data(key as u32);
                    let off = strlen(&raw const OUT as *const u8);
                    memcpy(
                        &raw mut OUT[off] as *mut c_void,
                        &raw const ud.data as *const c_void,
                        ud.size as usize,
                    );
                    OUT[off + ud.size as usize] = b'\0';
                    break 'out;
                }

                // Invalid keys are errors.
                if key > 255 {
                    snprintf(
                        (&raw mut OUT).cast(),
                        sizeof_out,
                        c"Invalid#%llx".as_ptr(),
                        saved,
                    );
                    break 'out;
                }

                // Printable ASCII keys.
                if key > 32 && key <= 126 {
                    tmp[0] = key as u8;
                    tmp[1] = b'\0';
                } else if key == 127 {
                    _ = xsnprintf_!(&raw mut tmp as _, sizeof_tmp, "C-?");
                } else if key >= 128 {
                    _ = xsnprintf_!(&raw mut tmp as _, sizeof_tmp, "\\{:o}", key,);
                }

                strlcat(
                    &raw mut OUT as *mut u8,
                    &raw const tmp as *const u8,
                    sizeof_out,
                );
                break 'out;
            }
            // append:
            strlcat(&raw mut OUT as *mut u8, s, sizeof_out);
        }
        // out:
        if with_flags != 0 && (saved & KEYC_MASK_FLAGS) != 0 {
            strlcat(&raw mut OUT as *mut u8, c!("["), sizeof_out);
            if saved & KEYC_LITERAL != 0 {
                strlcat(&raw mut OUT as *mut u8, c!("L"), sizeof_out);
            }
            if saved & KEYC_KEYPAD != 0 {
                strlcat(&raw mut OUT as *mut u8, c!("K"), sizeof_out);
            }
            if saved & KEYC_CURSOR != 0 {
                strlcat(&raw mut OUT as *mut u8, c!("C"), sizeof_out);
            }
            if saved & KEYC_IMPLIED_META != 0 {
                strlcat(&raw mut OUT as *mut u8, c!("I"), sizeof_out);
            }
            if saved & KEYC_BUILD_MODIFIERS != 0 {
                strlcat(&raw mut OUT as *mut u8, c!("B"), sizeof_out);
            }
            if saved & KEYC_SENT != 0 {
                strlcat(&raw mut OUT as *mut u8, c!("S"), sizeof_out);
            }
            strlcat(&raw mut OUT as *mut u8, c!("]"), sizeof_out);
        }
        &raw const OUT as *const u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    // Set a UTF-8 locale once, mirroring src/utf8.rs tests. Only needed for
    // the hexadecimal >= 32 path which routes through wctomb/utf8_fromcstr.
    fn setup_locale() {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| unsafe {
            if crate::libc::setlocale(::libc::LC_CTYPE, crate::c!("en_US.UTF-8")).is_null()
                && crate::libc::setlocale(::libc::LC_CTYPE, crate::c!("C.UTF-8")).is_null()
            {
                crate::libc::setlocale(::libc::LC_CTYPE, crate::c!(""));
            }
        });
    }

    // Look up a NUL-terminated byte string.
    unsafe fn lookup(s: &[u8]) -> key_code {
        debug_assert_eq!(*s.last().unwrap(), 0, "test input must be NUL-terminated");
        unsafe { key_string_lookup_string(s.as_ptr()) }
    }

    // Render a key_code back to a Rust String via key_string_lookup_key.
    // key_string_lookup_key writes into a shared `static mut OUT`, so calls must
    // be serialized against each other while the result is copied out.
    unsafe fn name(key: key_code, with_flags: i32) -> String {
        use std::sync::Mutex;
        static OUT_LOCK: Mutex<()> = Mutex::new(());
        let _guard = OUT_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        unsafe {
            let p = key_string_lookup_key(key, with_flags);
            let bytes = std::slice::from_raw_parts(p, strlen(p));
            String::from_utf8_lossy(bytes).into_owned()
        }
    }

    // --- key_string_lookup_string ------------------------------------------

    #[test]
    fn test_lookup_string_none_and_any() {
        // C key-string.c:254-257: "None"/"Any" are matched case-insensitively.
        unsafe {
            assert_eq!(lookup(b"None\0"), KEYC_NONE);
            assert_eq!(lookup(b"none\0"), KEYC_NONE);
            assert_eq!(lookup(b"NONE\0"), KEYC_NONE);
            assert_eq!(lookup(b"Any\0"), keyc::KEYC_ANY as key_code);
            assert_eq!(lookup(b"any\0"), keyc::KEYC_ANY as key_code);
        }
    }

    #[test]
    fn test_lookup_string_plain_ascii() {
        // C key-string.c:296-299: a single ASCII char >= 32 is its own value.
        unsafe {
            assert_eq!(lookup(b"a\0"), b'a' as key_code);
            assert_eq!(lookup(b"Z\0"), b'Z' as key_code);
            assert_eq!(lookup(b"0\0"), b'0' as key_code);
            // '^' with no following char is a plain key, not a Ctrl prefix.
            assert_eq!(lookup(b"^\0"), b'^' as key_code);
        }
    }

    #[test]
    fn test_lookup_string_control_char_rejected() {
        // C key-string.c:298-299: single key < 32 -> KEYC_UNKNOWN.
        unsafe {
            assert_eq!(lookup(b"\x01\0"), KEYC_UNKNOWN);
            assert_eq!(lookup(b"\x1f\0"), KEYC_UNKNOWN);
        }
    }

    #[test]
    fn test_lookup_string_named_keys() {
        // C key-string.c:314-319: table hit; KEYC_IMPLIED_META is stripped when
        // no explicit meta modifier is present.
        unsafe {
            assert_eq!(lookup(b"Enter\0"), c0::C0_CR as key_code); // 13
            assert_eq!(lookup(b"Tab\0"), c0::C0_HT as key_code); // 9
            assert_eq!(lookup(b"Escape\0"), c0::C0_ESC as key_code); // 27
            assert_eq!(lookup(b"Space\0"), b' ' as key_code); // 32
            assert_eq!(lookup(b"BTab\0"), keyc::KEYC_BTAB as key_code);
            // F1's table entry carries KEYC_IMPLIED_META, stripped here.
            assert_eq!(lookup(b"F1\0"), keyc::KEYC_F1 as key_code);
            assert_eq!(lookup(b"F12\0"), keyc::KEYC_F12 as key_code);
            // Arrow keys keep KEYC_CURSOR but lose KEYC_IMPLIED_META.
            assert_eq!(lookup(b"Up\0"), keyc::KEYC_UP as key_code | KEYC_CURSOR);
            assert_eq!(lookup(b"Left\0"), keyc::KEYC_LEFT as key_code | KEYC_CURSOR);
        }
    }

    #[test]
    fn test_lookup_string_named_case_insensitive() {
        // C uses strcasecmp for the table lookup.
        unsafe {
            assert_eq!(lookup(b"enter\0"), c0::C0_CR as key_code);
            assert_eq!(lookup(b"TAB\0"), c0::C0_HT as key_code);
        }
    }

    #[test]
    fn test_lookup_string_modifiers() {
        // C key-string.c:290-322: modifiers combine with the base key.
        unsafe {
            assert_eq!(lookup(b"C-a\0"), b'a' as key_code | KEYC_CTRL);
            assert_eq!(lookup(b"S-a\0"), b'a' as key_code | KEYC_SHIFT);
            assert_eq!(lookup(b"M-a\0"), b'a' as key_code | KEYC_META);
            // lowercase modifier prefixes are accepted too.
            assert_eq!(lookup(b"c-a\0"), b'a' as key_code | KEYC_CTRL);
            // C-M-x combines both modifiers.
            assert_eq!(
                lookup(b"C-M-x\0"),
                b'x' as key_code | KEYC_CTRL | KEYC_META
            );
            // With an explicit meta modifier the KEYC_IMPLIED_META bit survives.
            assert_eq!(
                lookup(b"M-Left\0"),
                keyc::KEYC_LEFT as key_code | KEYC_CURSOR | KEYC_IMPLIED_META | KEYC_META
            );
        }
    }

    #[test]
    fn test_lookup_string_short_ctrl() {
        // C key-string.c:283-288: "^X" is Ctrl+lowercased X.
        unsafe {
            assert_eq!(lookup(b"^A\0"), b'a' as key_code | KEYC_CTRL);
            assert_eq!(lookup(b"^a\0"), b'a' as key_code | KEYC_CTRL);
            assert_eq!(lookup(b"^Z\0"), b'z' as key_code | KEYC_CTRL);
        }
    }

    #[test]
    fn test_lookup_string_invalid() {
        // Unknown names, invalid modifiers, and dangling modifiers all fail.
        unsafe {
            assert_eq!(lookup(b"Bogus\0"), KEYC_UNKNOWN);
            assert_eq!(lookup(b"NotAKey\0"), KEYC_UNKNOWN);
            // Invalid modifier letter: key_string_get_modifiers nulls the string.
            assert_eq!(lookup(b"X-a\0"), KEYC_UNKNOWN);
            // A modifier with no following key -> empty string -> KEYC_UNKNOWN.
            assert_eq!(lookup(b"C-\0"), KEYC_UNKNOWN);
        }
    }

    #[test]
    fn test_lookup_string_hex_below_32() {
        // C key-string.c:260-264: 0x<hex>; values < 32 returned verbatim.
        unsafe {
            assert_eq!(lookup(b"0x1b\0"), 0x1b); // 27
            assert_eq!(lookup(b"0x07\0"), 0x07);
            assert_eq!(lookup(b"0x00\0"), 0x00);
            // Malformed hex fails the sscanf.
            assert_eq!(lookup(b"0xzz\0"), KEYC_UNKNOWN);
        }
    }

    #[test]
    fn test_lookup_string_hex_ascii() {
        // C key-string.c:265-279: values >= 32 go through wctomb + utf8 decode
        // and return a packed utf8_char (not the raw codepoint). Rather than
        // pin the packed encoding, verify it decodes back to the ASCII glyph.
        setup_locale();
        unsafe {
            let a = lookup(b"0x41\0");
            assert_ne!(a, KEYC_UNKNOWN);
            assert_eq!(name(a, 0), "A");
            let tilde = lookup(b"0x7e\0");
            assert_ne!(tilde, KEYC_UNKNOWN);
            assert_eq!(name(tilde, 0), "~");
        }
    }

    // --- key_string_lookup_key ---------------------------------------------

    #[test]
    fn test_lookup_key_plain_ascii() {
        // C key-string.c:453-455: printable ASCII rendered as itself.
        unsafe {
            assert_eq!(name(b'a' as key_code, 0), "a");
            assert_eq!(name(b'Z' as key_code, 0), "Z");
            assert_eq!(name(b'~' as key_code, 0), "~");
            // key == 127 renders as C-?.
            assert_eq!(name(127, 0), "C-?");
        }
    }

    #[test]
    fn test_lookup_key_special() {
        // C key-string.c:355-368.
        unsafe {
            assert_eq!(name(KEYC_NONE, 0), "None");
            assert_eq!(name(KEYC_UNKNOWN, 0), "Unknown");
            assert_eq!(name(keyc::KEYC_ANY as key_code, 0), "Any");
        }
    }

    #[test]
    fn test_lookup_key_named() {
        // C key-string.c:428-434: table lookup uses key & KEYC_MASK_KEY.
        unsafe {
            assert_eq!(name(keyc::KEYC_F1 as key_code, 0), "F1");
            assert_eq!(name(keyc::KEYC_F12 as key_code, 0), "F12");
            assert_eq!(name(keyc::KEYC_UP as key_code | KEYC_CURSOR, 0), "Up");
            assert_eq!(name(c0::C0_CR as key_code, 0), "Enter");
            assert_eq!(name(c0::C0_HT as key_code, 0), "Tab");
            assert_eq!(name(b' ' as key_code, 0), "Space");
        }
    }

    #[test]
    fn test_lookup_key_modifiers() {
        // C key-string.c:346-351: modifiers prefix in C-, M-, S- order.
        unsafe {
            assert_eq!(name(b'a' as key_code | KEYC_CTRL, 0), "C-a");
            assert_eq!(name(b'a' as key_code | KEYC_META, 0), "M-a");
            assert_eq!(name(b'a' as key_code | KEYC_SHIFT, 0), "S-a");
            assert_eq!(
                name(b'x' as key_code | KEYC_CTRL | KEYC_META, 0),
                "C-M-x"
            );
            assert_eq!(
                name(
                    keyc::KEYC_LEFT as key_code | KEYC_CURSOR | KEYC_IMPLIED_META | KEYC_META,
                    0
                ),
                "M-Left"
            );
        }
    }

    #[test]
    fn test_lookup_key_user() {
        // C key-string.c:421-425.
        unsafe {
            assert_eq!(name(KEYC_USER, 0), "User0");
            assert_eq!(name(KEYC_USER + 5, 0), "User5");
        }
    }

    #[test]
    fn test_lookup_key_invalid() {
        // C key-string.c:447-449: values > 255 that are inside the special-key
        // range (so not Unicode) but have no table/special/user entry render as
        // Invalid#<hex>. KEYC_DOUBLECLICK is such an unmapped special key.
        unsafe {
            let key = keyc::KEYC_DOUBLECLICK as key_code;
            assert_eq!(name(key, 0), format!("Invalid#{key:x}"));
        }
    }

    #[test]
    fn test_lookup_key_with_flags() {
        // C key-string.c:468-483: with_flags appends a [..] suffix of flag letters.
        unsafe {
            // KEYC_CURSOR -> "C".
            assert_eq!(name(keyc::KEYC_UP as key_code | KEYC_CURSOR, 1), "Up[C]");
            // KEYC_IMPLIED_META -> "I".
            assert_eq!(
                name(keyc::KEYC_F1 as key_code | KEYC_IMPLIED_META, 1),
                "F1[I]"
            );
            // No flags present -> no suffix even with with_flags set.
            assert_eq!(name(b'a' as key_code, 1), "a");
        }
    }

    // --- round trips -------------------------------------------------------

    #[test]
    fn test_round_trip_name_to_key_to_name() {
        // Canonical strings must survive lookup_string -> lookup_key.
        let cases: &[&[u8]] = &[
            b"a\0",
            b"C-a\0",
            b"S-a\0",
            b"M-a\0",
            b"C-M-x\0",
            b"Up\0",
            b"M-Left\0",
            b"F1\0",
            b"F12\0",
            b"Enter\0",
            b"Tab\0",
            b"Escape\0",
            b"Space\0",
        ];
        unsafe {
            for c in cases {
                let key = lookup(c);
                assert_ne!(key, KEYC_UNKNOWN, "case {c:?} failed to parse");
                let rendered = name(key, 0);
                let expected = std::str::from_utf8(&c[..c.len() - 1]).unwrap();
                assert_eq!(rendered, expected, "round trip mismatch for {c:?}");
            }
        }
    }

    // --- key_string_get_modifiers ------------------------------------------

    #[test]
    fn test_get_modifiers() {
        // C key-string.c:213-239.
        unsafe {
            let s = b"C-M-x\0";
            let mut p: *const u8 = s.as_ptr();
            let m = key_string_get_modifiers(&raw mut p);
            assert_eq!(m, KEYC_CTRL | KEYC_META);
            // The pointer is advanced past the consumed modifiers.
            assert_eq!(*p, b'x');

            // Lowercase spellings are accepted.
            let s = b"c-s-a\0";
            let mut p: *const u8 = s.as_ptr();
            assert_eq!(
                key_string_get_modifiers(&raw mut p),
                KEYC_CTRL | KEYC_SHIFT
            );
            assert_eq!(*p, b'a');

            // No modifiers: pointer unchanged, zero modifiers.
            let s = b"abc\0";
            let mut p: *const u8 = s.as_ptr();
            assert_eq!(key_string_get_modifiers(&raw mut p), 0);
            assert_eq!(*p, b'a');

            // Invalid modifier letter: string set to NULL and 0 returned.
            let s = b"X-a\0";
            let mut p: *const u8 = s.as_ptr();
            assert_eq!(key_string_get_modifiers(&raw mut p), 0);
            assert!(p.is_null());
        }
    }

    // --- key_string_search_table -------------------------------------------

    #[test]
    fn test_search_table() {
        // C key-string.c:195-209: raw table values (no KEYC_IMPLIED_META strip),
        // "User%u" handling, and KEYC_UNKNOWN fallback.
        unsafe {
            assert_eq!(key_string_search_table(c!("Enter")), c0::C0_CR as key_code);
            // strcasecmp: case-insensitive.
            assert_eq!(key_string_search_table(c!("enter")), c0::C0_CR as key_code);
            // The raw entry keeps KEYC_IMPLIED_META (unlike lookup_string).
            assert_eq!(
                key_string_search_table(c!("F1")),
                keyc::KEYC_F1 as key_code | KEYC_IMPLIED_META
            );
            assert_eq!(key_string_search_table(c!("User7")), KEYC_USER + 7);
            assert_eq!(key_string_search_table(c!("User0")), KEYC_USER);
            assert_eq!(key_string_search_table(c!("Bogus")), KEYC_UNKNOWN);
        }
    }

    // --- Known ztmux port divergence (ignored until fixed) -----------------

    // ztmux BUG: key_string_lookup_string rejects User<KEYC_NUSER>. tmux accepts
    // it — C uses `user <= KEYC_NUSER` (vendor/tmux/key-string.c:205) while ztmux
    // uses `user < KEYC_NUSER` (key_string.rs:237), so the top User index is off
    // by one. Remove #[ignore] once the bound matches C.
    #[test]
    fn bug_user_key_boundary_inclusive() {
        unsafe {
            let s = format!("User{KEYC_NUSER}\0");
            assert_eq!(lookup(s.as_bytes()), KEYC_USER + KEYC_NUSER);
        }
    }

    // Named-key aliases (key-string.c:53-90) collapse onto the same key_code:
    // Insert==IC, Delete==DC, PageUp==PPage==PgUp, PageDown==NPage==PgDn.
    #[test]
    fn test_lookup_string_named_aliases() {
        unsafe {
            assert_eq!(lookup(b"Insert\0"), lookup(b"IC\0"));
            assert_eq!(lookup(b"Insert\0"), keyc::KEYC_IC as key_code);
            assert_eq!(lookup(b"Delete\0"), lookup(b"DC\0"));
            assert_eq!(lookup(b"Delete\0"), keyc::KEYC_DC as key_code);
            assert_eq!(lookup(b"PageUp\0"), lookup(b"PPage\0"));
            assert_eq!(lookup(b"PgUp\0"), lookup(b"PPage\0"));
            assert_eq!(lookup(b"PageUp\0"), keyc::KEYC_PPAGE as key_code);
            assert_eq!(lookup(b"PageDown\0"), lookup(b"NPage\0"));
            assert_eq!(lookup(b"PgDn\0"), lookup(b"NPage\0"));
            assert_eq!(lookup(b"PageDown\0"), keyc::KEYC_NPAGE as key_code);
            // Home/End keep only KEYC_IMPLIED_META, stripped without explicit meta.
            assert_eq!(lookup(b"Home\0"), keyc::KEYC_HOME as key_code);
            assert_eq!(lookup(b"End\0"), keyc::KEYC_END as key_code);
        }
    }

    // The remaining function keys F5..F11 all resolve to their KEYC_F* code with
    // KEYC_IMPLIED_META stripped (key-string.c:314-319).
    #[test]
    fn test_lookup_string_all_function_keys() {
        unsafe {
            assert_eq!(lookup(b"F5\0"), keyc::KEYC_F5 as key_code);
            assert_eq!(lookup(b"F6\0"), keyc::KEYC_F6 as key_code);
            assert_eq!(lookup(b"F7\0"), keyc::KEYC_F7 as key_code);
            assert_eq!(lookup(b"F8\0"), keyc::KEYC_F8 as key_code);
            assert_eq!(lookup(b"F9\0"), keyc::KEYC_F9 as key_code);
            assert_eq!(lookup(b"F10\0"), keyc::KEYC_F10 as key_code);
            assert_eq!(lookup(b"F11\0"), keyc::KEYC_F11 as key_code);
        }
    }

    // Keypad names carry KEYC_KEYPAD (not KEYC_IMPLIED_META), so they survive
    // lookup_string unchanged and round-trip back to their canonical spelling.
    #[test]
    fn test_keypad_names_round_trip() {
        unsafe {
            for &(s, k) in &[
                (b"KP/\0".as_slice(), keyc::KEYC_KP_SLASH as key_code),
                (b"KP*\0".as_slice(), keyc::KEYC_KP_STAR as key_code),
                (b"KP7\0".as_slice(), keyc::KEYC_KP_SEVEN as key_code),
                (b"KP0\0".as_slice(), keyc::KEYC_KP_ZERO as key_code),
                (b"KP.\0".as_slice(), keyc::KEYC_KP_PERIOD as key_code),
                (b"KPEnter\0".as_slice(), keyc::KEYC_KP_ENTER as key_code),
            ] {
                let key = lookup(s);
                assert_eq!(key, k | KEYC_KEYPAD, "parse {s:?}");
                let expect = std::str::from_utf8(&s[..s.len() - 1]).unwrap();
                assert_eq!(name(key, 0), expect, "render {s:?}");
            }
        }
    }

    // Modifier prefixes commute: whatever order they are written, the parsed
    // key_code carries the same modifier bits, and lookup_key always renders
    // them back in the canonical C-, M-, S- order (key-string.c:346-351).
    #[test]
    fn test_modifier_order_is_canonicalized() {
        unsafe {
            let a = lookup(b"C-M-S-a\0");
            let b = lookup(b"S-M-C-a\0");
            let c = lookup(b"M-C-S-a\0");
            assert_eq!(a, b);
            assert_eq!(a, c);
            assert_eq!(a, b'a' as key_code | KEYC_CTRL | KEYC_META | KEYC_SHIFT);
            // Canonical render order regardless of input order.
            assert_eq!(name(a, 0), "C-M-S-a");
        }
    }

    // lookup_key renders the special (non-table) key names (key-string.c:355-416).
    #[test]
    fn test_lookup_key_special_names() {
        unsafe {
            assert_eq!(name(keyc::KEYC_MOUSE as key_code, 0), "Mouse");
            assert_eq!(name(keyc::KEYC_DRAGGING as key_code, 0), "Dragging");
            assert_eq!(name(keyc::KEYC_PASTE_START as key_code, 0), "PasteStart");
            assert_eq!(name(keyc::KEYC_PASTE_END as key_code, 0), "PasteEnd");
            assert_eq!(name(keyc::KEYC_FOCUS_IN as key_code, 0), "FocusIn");
            assert_eq!(name(keyc::KEYC_FOCUS_OUT as key_code, 0), "FocusOut");
            assert_eq!(name(keyc::KEYC_MOUSEMOVE_PANE as key_code, 0), "MouseMovePane");
            assert_eq!(
                name(keyc::KEYC_MOUSEMOVE_BORDER as key_code, 0),
                "MouseMoveBorder"
            );
        }
    }

    // C0 control codes render as their bracketed mnemonic via the string table
    // (key-string.c:145-176 entries), and key 0 -> "[NUL]".
    #[test]
    fn test_lookup_key_control_names() {
        unsafe {
            assert_eq!(name(0, 0), "[NUL]");
            assert_eq!(name(c0::C0_SOH as key_code, 0), "[SOH]");
            assert_eq!(name(c0::C0_ESC as key_code, 0), "Escape");
            assert_eq!(name(c0::C0_BEL as key_code, 0), "[BEL]");
        }
    }

    // Ctrl+Space is a valid combination: Space is a table entry (value 0x20), and
    // C-Space round-trips through both directions.
    #[test]
    fn test_ctrl_space_round_trip() {
        unsafe {
            let key = lookup(b"C-Space\0");
            assert_eq!(key, b' ' as key_code | KEYC_CTRL);
            assert_eq!(name(key, 0), "C-Space");
        }
    }

    // key_string_search_table returns the RAW table value (KEYC_IMPLIED_META and
    // KEYC_CURSOR/KEYC_KEYPAD intact), unlike lookup_string which strips implied
    // meta. Cross-check arrow, keypad, and space entries.
    #[test]
    fn test_search_table_raw_flags() {
        unsafe {
            assert_eq!(
                key_string_search_table(c!("Up")),
                keyc::KEYC_UP as key_code | KEYC_CURSOR | KEYC_IMPLIED_META
            );
            assert_eq!(
                key_string_search_table(c!("KP/")),
                keyc::KEYC_KP_SLASH as key_code | KEYC_KEYPAD
            );
            assert_eq!(key_string_search_table(c!("Space")), b' ' as key_code);
            assert_eq!(key_string_search_table(c!("BSpace")), keyc::KEYC_BSPACE as key_code);
        }
    }

    // Extended round-trip battery: named keys with modifiers survive
    // lookup_string -> lookup_key back to their canonical spelling.
    #[test]
    fn test_round_trip_modified_named_keys() {
        let cases: &[&[u8]] = &[
            b"C-Up\0",
            b"S-Down\0",
            b"M-Right\0",
            b"C-M-Left\0",
            b"C-F5\0",
            b"S-F12\0",
            b"C-Home\0",
            b"M-End\0",
            b"C-M-S-Up\0",
        ];
        unsafe {
            for c in cases {
                let key = lookup(c);
                assert_ne!(key, KEYC_UNKNOWN, "parse {c:?}");
                let rendered = name(key, 0);
                let expected = std::str::from_utf8(&c[..c.len() - 1]).unwrap();
                assert_eq!(rendered, expected, "round trip {c:?}");
            }
        }
    }

    // DEL (0x7f) is asymmetric by design: key_string_lookup_key renders raw 0x7f
    // as "C-?" (key-string.c:456, `key == 0x7f` special case), and re-parsing
    // "C-?" yields KEYC_CTRL | '?' (0x3f) — NOT 0x7f. This is the documented
    // C behavior, so we assert each direction independently rather than a full
    // round trip.
    #[test]
    fn test_del_ctrl_question_asymmetry() {
        unsafe {
            // Render direction: raw DEL -> "C-?".
            assert_eq!(name(127, 0), "C-?");
            // Parse direction: "C-?" -> Ctrl + '?' (0x3f), the C-? re-parse target.
            assert_eq!(lookup(b"C-?\0"), b'?' as key_code | KEYC_CTRL);
            // The two are deliberately different keys.
            assert_ne!(lookup(b"C-?\0"), 127);
        }
    }

    // The 0x<hex> form with a value >= 32 routes through wctomb + utf8 decode
    // (key-string.c:265-279) and returns a packed utf8_char. For multibyte code
    // points (é U+00E9, € U+20AC) the packed key must render back to the glyph,
    // exercising the non-ASCII branch that test_lookup_string_hex_ascii skips.
    #[test]
    fn test_lookup_string_hex_multibyte() {
        setup_locale();
        unsafe {
            let e = lookup(b"0xe9\0");
            assert_ne!(e, KEYC_UNKNOWN);
            assert_eq!(name(e, 0), "é");
            let euro = lookup(b"0x20ac\0");
            assert_ne!(euro, KEYC_UNKNOWN);
            assert_eq!(name(euro, 0), "€");
        }
    }

    // A literal multibyte UTF-8 character passed to lookup_string is decoded via
    // utf8_open/utf8_append into a packed utf8_char (key-string.c:302-315) and
    // must render back to the same glyph. Distinct code path from the 0x<hex>
    // form above.
    #[test]
    fn test_lookup_string_literal_unicode() {
        setup_locale();
        unsafe {
            // é = C3 A9.
            let e = lookup(b"\xc3\xa9\0");
            assert_ne!(e, KEYC_UNKNOWN);
            assert_eq!(name(e, 0), "é");
            // € = E2 82 AC.
            let euro = lookup(b"\xe2\x82\xac\0");
            assert_ne!(euro, KEYC_UNKNOWN);
            assert_eq!(name(euro, 0), "€");
        }
    }

    // A meta-modified literal unicode key keeps its KEYC_META bit and renders
    // with the "M-" prefix (key-string.c:346). M-é must parse and render back.
    #[test]
    fn test_lookup_string_meta_unicode() {
        setup_locale();
        unsafe {
            let key = lookup(b"M-\xc3\xa9\0");
            assert_ne!(key, KEYC_UNKNOWN);
            assert_ne!(key & KEYC_META, 0);
            assert_eq!(name(key, 0), "M-é");
        }
    }
}
