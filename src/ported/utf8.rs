// Copyright (c) 2008 Nicholas Marriott <nicholas.marriott@gmail.com>
//
// Permission u8, copy, modify, and distribute this software for any
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
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fmt::{self, Display},
    slice,
};

use crate::compat::vis;
use crate::libc::{memcpy, memset};
use crate::options_::{
    options_array_first, options_array_item_value, options_array_next, options_get_only,
};
use crate::*;

#[cfg(feature = "utf8proc")]
unsafe extern "C" {
    /// C `vendor/tmux/compat/utf8proc.c:24`: `int utf8proc_wcwidth(wchar_t wc)`
    fn utf8proc_wcwidth(_: wchar_t) -> i32;
    /// C `vendor/tmux/compat/utf8proc.c:40`: `int utf8proc_mbtowc(wchar_t *pwc, const char *s, size_t n)`
    fn utf8proc_mbtowc(_: *mut wchar_t, _: *const u8, _: usize) -> i32;
    /// C `vendor/tmux/compat/utf8proc.c:58`: `int utf8proc_wctomb(char *s, wchar_t wc)`
    fn utf8proc_wctomb(_: *mut char, _: wchar_t) -> i32;
}

// A single UTF-8 character.
pub(crate) type utf8_char = c_uint;

// An expanded UTF-8 character. UTF8_SIZE must be big enough to hold combining
// characters as well. It can't be more than 32 bytes without changes to how
// characters are stored.
pub(crate) const UTF8_SIZE: usize = 21;

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct utf8_data {
    pub(crate) data: [u8; UTF8_SIZE], /* TODO if we make this private we can only expose the initialized part */

    pub(crate) have: u8,
    pub(crate) size: u8, /* TODO check the codebase for things checking if size == 0, which is the sentinal value */
    /// 0xff if invalid
    pub(crate) width: u8,
}

impl utf8_data {
    pub(crate) const fn new<const N: usize>(data: [u8; N], have: u8, size: u8, width: u8) -> Self {
        if N >= UTF8_SIZE {
            panic!("invalid size");
        }

        let mut padded_data = [0u8; UTF8_SIZE];
        let mut i = 0usize;
        while i < N {
            padded_data[i] = data[i];
            i += 1;
        }

        Self {
            data: padded_data,
            have,
            size,
            width,
        }
    }

    pub(crate) fn initialized_slice(&self) -> &[u8] {
        &self.data[..self.size as usize]
    }
}

#[repr(i32)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub(crate) enum utf8_state {
    UTF8_MORE,
    UTF8_DONE,
    UTF8_ERROR,
}

static UTF8_FORCE_WIDE: [wchar_t; 162] = [
    0x0261D, 0x026F9, 0x0270A, 0x0270B, 0x0270C, 0x0270D, 0x1F1E6, 0x1F1E7, 0x1F1E8, 0x1F1E9,
    0x1F1EA, 0x1F1EB, 0x1F1EC, 0x1F1ED, 0x1F1EE, 0x1F1EF, 0x1F1F0, 0x1F1F1, 0x1F1F2, 0x1F1F3,
    0x1F1F4, 0x1F1F5, 0x1F1F6, 0x1F1F7, 0x1F1F8, 0x1F1F9, 0x1F1FA, 0x1F1FB, 0x1F1FC, 0x1F1FD,
    0x1F1FE, 0x1F1FF, 0x1F385, 0x1F3C2, 0x1F3C3, 0x1F3C4, 0x1F3C7, 0x1F3CA, 0x1F3CB, 0x1F3CC,
    0x1F3FB, 0x1F3FC, 0x1F3FD, 0x1F3FE, 0x1F3FF, 0x1F442, 0x1F443, 0x1F446, 0x1F447, 0x1F448,
    0x1F449, 0x1F44A, 0x1F44B, 0x1F44C, 0x1F44D, 0x1F44E, 0x1F44F, 0x1F450, 0x1F466, 0x1F467,
    0x1F468, 0x1F469, 0x1F46B, 0x1F46C, 0x1F46D, 0x1F46E, 0x1F470, 0x1F471, 0x1F472, 0x1F473,
    0x1F474, 0x1F475, 0x1F476, 0x1F477, 0x1F478, 0x1F47C, 0x1F481, 0x1F482, 0x1F483, 0x1F485,
    0x1F486, 0x1F487, 0x1F48F, 0x1F491, 0x1F4AA, 0x1F574, 0x1F575, 0x1F57A, 0x1F590, 0x1F595,
    0x1F596, 0x1F645, 0x1F646, 0x1F647, 0x1F64B, 0x1F64C, 0x1F64D, 0x1F64E, 0x1F64F, 0x1F6A3,
    0x1F6B4, 0x1F6B5, 0x1F6B6, 0x1F6C0, 0x1F6CC, 0x1F90C, 0x1F90F, 0x1F918, 0x1F919, 0x1F91A,
    0x1F91B, 0x1F91C, 0x1F91D, 0x1F91E, 0x1F91F, 0x1F926, 0x1F930, 0x1F931, 0x1F932, 0x1F933,
    0x1F934, 0x1F935, 0x1F936, 0x1F937, 0x1F938, 0x1F939, 0x1F93D, 0x1F93E, 0x1F977, 0x1F9B5,
    0x1F9B6, 0x1F9B8, 0x1F9B9, 0x1F9BB, 0x1F9CD, 0x1F9CE, 0x1F9CF, 0x1F9D1, 0x1F9D2, 0x1F9D3,
    0x1F9D4, 0x1F9D5, 0x1F9D6, 0x1F9D7, 0x1F9D8, 0x1F9D9, 0x1F9DA, 0x1F9DB, 0x1F9DC, 0x1F9DD,
    0x1FAC3, 0x1FAC4, 0x1FAC5, 0x1FAF0, 0x1FAF1, 0x1FAF2, 0x1FAF3, 0x1FAF4, 0x1FAF5, 0x1FAF6,
    0x1FAF7, 0x1FAF8,
];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct utf8_item_index {
    pub index: u32,
}

#[derive(Clone, Copy)] // TODO investigate manual clone
pub struct utf8_item_data {
    data: [MaybeUninit<u8>; UTF8_SIZE],
    size: u8,
}

impl utf8_item_data {
    fn new(bytes: &[u8]) -> Self {
        assert!(bytes.len() <= UTF8_SIZE);

        let mut data = [MaybeUninit::new(0); UTF8_SIZE];
        for (i, ch) in bytes.iter().enumerate() {
            data[i] = MaybeUninit::new(*ch);
        }
        Self {
            data,
            size: bytes.len() as u8,
        }
    }
}

impl Display for utf8_item_data {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            std::str::from_utf8(self.initialized_slice())
                .unwrap_or("invalid utf8 in utf8_item_data"),
        )
    }
}

/// once stabilized use: <https://doc.rust-lang.org/std/primitive.slice.html#method.assume_init_ref>
unsafe fn assume_init_ref<T>(data: &[MaybeUninit<T>]) -> &[T] {
    unsafe { std::slice::from_raw_parts(data.as_ptr().cast(), data.len()) }
}
impl utf8_item_data {
    fn initialized_slice(&self) -> &[u8] {
        // SAFETY: type invariant utf8_item_data.data should be initialized until self.size
        unsafe { assume_init_ref(&self.data[..self.size as usize]) }
    }
}

impl_ord!(utf8_item_data as utf8_data_cmp);

/// C `vendor/tmux/utf8.c:228`: `static int utf8_data_cmp(struct utf8_item *ui1, struct utf8_item *ui2)`
fn utf8_data_cmp(ui1: &utf8_item_data, ui2: &utf8_item_data) -> std::cmp::Ordering {
    ui1.initialized_slice().cmp(ui2.initialized_slice())
}

thread_local! {
    static UTF8_DATA_TREE: RefCell<BTreeMap<utf8_item_data, utf8_item_index>> = const { RefCell::new(BTreeMap::new()) };
    static UTF8_INDEX_TREE: RefCell<BTreeMap<utf8_item_index, utf8_item_data>> = const { RefCell::new(BTreeMap::new()) };
}

static mut UTF8_NEXT_INDEX: u32 = 0;

fn utf8_get_size(uc: utf8_char) -> u8 {
    (((uc) >> 24) & 0x1f) as u8
}
fn utf8_get_width(uc: utf8_char) -> u8 {
    (((uc) >> 29) - 1) as u8
}
fn utf8_set_size(size: u8) -> utf8_char {
    (size as utf8_char) << 24
}
fn utf8_set_width(width: u8) -> utf8_char {
    (width as utf8_char + 1) << 29
}

/// C `vendor/tmux/utf8.c:264`: `static struct utf8_item *utf8_item_by_data(const u_char *data, size_t size)`
pub fn utf8_item_by_data(item: &utf8_item_data) -> Option<utf8_item_index> {
    UTF8_DATA_TREE.with(|tree| tree.borrow().get(item).copied())
}

/// C `vendor/tmux/utf8.c:276`: `static struct utf8_item *utf8_item_by_index(u_int index)`
pub fn utf8_item_by_index(index: u32) -> Option<utf8_item_data> {
    let ui = utf8_item_index { index };

    UTF8_INDEX_TREE.with(|tree| tree.borrow().get(&ui).copied())
}

/// C `vendor/tmux/utf8.c:435`: `static int utf8_put_item(const u_char *data, size_t size, u_int *index)`
pub unsafe fn utf8_put_item(data: *const [u8; UTF8_SIZE], size: usize, index: *mut u32) -> i32 {
    unsafe {
        let ud = &utf8_item_data::new(slice::from_raw_parts(data.cast(), size));
        let ui = utf8_item_by_data(ud);
        if let Some(ui) = ui {
            *index = ui.index;
            log_debug!(
                "utf8_put_item: found {1:0$} = {2}",
                size,
                _s((&raw const data).cast::<u8>()),
                *index,
            );
            return 0;
        }

        if UTF8_NEXT_INDEX == 0xffffff + 1 {
            return -1;
        }

        let ui_index = utf8_item_index {
            index: UTF8_NEXT_INDEX,
        };
        UTF8_NEXT_INDEX += 1;

        let ui_data = *ud;
        UTF8_INDEX_TREE.with(|tree| tree.borrow_mut().insert(ui_index, ui_data));
        UTF8_DATA_TREE.with(|tree| tree.borrow_mut().insert(ui_data, ui_index));

        *index = ui_index.index;
        log_debug!(
            "utf8_put_item: added {1:0$} = {2}",
            size,
            _s((&raw const data).cast::<u8>()),
            *index,
        );
        0
    }
}

pub fn utf8_in_table(find: wchar_t, table: &[wchar_t]) -> bool {
    table.binary_search(&find).is_ok()
}

/// C `vendor/tmux/utf8.c:465`: `enum utf8_state utf8_from_data(const struct utf8_data *ud, utf8_char *uc)`
pub unsafe fn utf8_from_data(ud: *const utf8_data, uc: *mut utf8_char) -> utf8_state {
    unsafe {
        let mut index: u32 = 0;
        'fail: {
            if (*ud).width > 2 {
                fatalx_!("invalid UTF-8 width: {}", (*ud).width);
            }

            if (*ud).size > UTF8_SIZE as u8 {
                break 'fail;
            }
            if (*ud).size <= 3 {
                index = (((*ud).data[2] as u32) << 16)
                    | (((*ud).data[1] as u32) << 8)
                    | ((*ud).data[0] as u32);
            } else if utf8_put_item(
                (&raw const (*ud).data).cast(),
                (*ud).size as usize,
                &raw mut index,
            ) != 0
            {
                break 'fail;
            }
            *uc = utf8_set_size((*ud).size) | utf8_set_width((*ud).width) | index;
            log_debug!(
                "utf8_from_data: ({0} {1} {3:2$}) -> {4:08x}",
                (*ud).width,
                (*ud).size,
                (*ud).size as usize,
                _s((&raw const (*ud).data).cast::<u8>()),
                *uc,
            );
            return utf8_state::UTF8_DONE;
        }

        // fail:
        *uc = if (*ud).width == 0 {
            utf8_set_size(0) | utf8_set_width(0)
        } else if (*ud).width == 1 {
            utf8_set_size(1) | utf8_set_width(1) | 0x20
        } else {
            utf8_set_size(1) | utf8_set_width(1) | 0x2020
        };
        utf8_state::UTF8_ERROR
    }
}

/// C `vendor/tmux/utf8.c:497`: `void utf8_to_data(utf8_char uc, struct utf8_data *ud)`
pub fn utf8_to_data(uc: utf8_char) -> utf8_data {
    let mut ud = utf8_data {
        data: [0; UTF8_SIZE],
        size: utf8_get_size(uc),
        have: utf8_get_size(uc),
        width: utf8_get_width(uc),
    };

    if ud.size <= 3 {
        ud.data[2] = (uc >> 16) as u8;
        ud.data[1] = ((uc >> 8) & 0xff) as u8;
        ud.data[0] = (uc & 0xff) as u8;
    } else {
        let index = uc & 0xffffff;
        if let Some(ui) = utf8_item_by_index(index) {
            ud.data[..ud.size as usize].copy_from_slice(ui.initialized_slice());
        } else {
            ud.data[..ud.size as usize].fill(b' ');
        }
    }

    log_debug!(
        "utf8_to_data: {:08x} -> ({} {} {})",
        uc,
        ud.width,
        ud.size,
        String::from_utf8_lossy(ud.initialized_slice())
    );

    ud
}

/// C `vendor/tmux/utf8.c:524`: `u_int utf8_build_one(u_char ch)`
pub fn utf8_build_one(ch: c_uchar) -> u32 {
    utf8_set_size(1) | utf8_set_width(1) | ch as u32
}

/// C `vendor/tmux/utf8.c:531`: `void utf8_set(struct utf8_data *ud, u_char ch)`
pub unsafe fn utf8_set(ud: *mut utf8_data, ch: c_uchar) {
    static EMPTY: utf8_data = utf8_data {
        data: unsafe { zeroed() },
        have: 1,
        size: 1,
        width: 1,
    };

    unsafe {
        memcpy__(ud, &raw const EMPTY);
        (*ud).data[0] = ch;
    }
}

/// C `vendor/tmux/utf8.c:541`: `void utf8_copy(struct utf8_data *to, const struct utf8_data *from)`
pub unsafe fn utf8_copy(to: *mut utf8_data, from: *const utf8_data) {
    unsafe {
        memcpy__(to, from);

        for i in (*to).size..(UTF8_SIZE as u8) {
            (*to).data[i as usize] = b'\0';
        }
    }
}

thread_local! {
    /// Port of the user-configurable half of `utf8_width_cache` (utf8.c). ztmux
    /// keeps the built-in default widths in [`UTF8_FORCE_WIDE`] plus the
    /// regional-indicator case in [`utf8_width`]; this holds only the
    /// `codepoint-widths` overrides, which take precedence — behaviourally
    /// identical to C merging both into one RB tree and returning the per-entry
    /// width first.
    static UTF8_WIDTH_OVERRIDES: RefCell<BTreeMap<wchar_t, u32>> =
        const { RefCell::new(BTreeMap::new()) };
}

/// Look up a `codepoint-widths` override for `wc`. The default-width half lives
/// in [`utf8_width`] itself.
/// C `vendor/tmux/utf8.c:286`: `static struct utf8_width_item *utf8_find_in_width_cache(wchar_t wc)`
fn utf8_find_in_width_cache(wc: wchar_t) -> Option<u32> {
    UTF8_WIDTH_OVERRIDES.with(|m| m.borrow().get(&wc).copied())
}

/// C `vendor/tmux/utf8.c:297`: `static void utf8_insert_width_cache(wchar_t wc, u_int width)`
fn utf8_insert_width_cache(wc: wchar_t, width: u32) {
    log_debug!("Unicode width cache: {:08X}={}", wc as u32, width);
    UTF8_WIDTH_OVERRIDES.with(|m| {
        m.borrow_mut().insert(wc, width);
    });
}

/// Parse a single `codepoint-widths` entry — `U+XXXX=W`, `U+XXXX-U+YYYY=W`
/// (a range), or a literal `<char>=W`. Width is bounded 0..=2; malformed
/// entries are ignored, matching the C.
/// C `vendor/tmux/utf8.c:319`: `static void utf8_add_to_width_cache(const char *s)`
fn utf8_add_to_width_cache(s: &str) {
    let Some((key, wstr)) = s.split_once('=') else {
        return;
    };
    let Ok(width) = wstr.parse::<u32>() else {
        return;
    };
    if width > 2 {
        return;
    }
    if let Some(hex) = key.strip_prefix("U+") {
        let (start_s, end_s) = match hex.split_once("-U+") {
            Some((a, b)) => (a, b),
            None => (hex, hex),
        };
        let (Ok(start), Ok(end)) = (
            wchar_t::from_str_radix(start_s, 16),
            wchar_t::from_str_radix(end_s, 16),
        ) else {
            return;
        };
        if start == 0 || end == 0 || end < start {
            return;
        }
        for wc in start..=end {
            utf8_insert_width_cache(wc, width);
        }
    } else {
        // A single literal UTF-8 character (reject empty or multi-char).
        let mut chars = key.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            return;
        };
        utf8_insert_width_cache(c as wchar_t, width);
    }
}

/// Rebuild the `codepoint-widths` overrides from the global option. Called from
/// the options on-set path when `codepoint-widths` changes.
/// C `vendor/tmux/utf8.c:407`: `void utf8_update_width_cache(void)`
pub unsafe fn utf8_update_width_cache() {
    unsafe {
        UTF8_WIDTH_OVERRIDES.with(|m| m.borrow_mut().clear());
        let o = options_get_only(GLOBAL_OPTIONS, "codepoint-widths");
        if o.is_null() {
            return;
        }
        let mut a = options_array_first(o);
        while !a.is_null() {
            let val = options_array_item_value(a);
            if !val.is_null()
                && let Some(s) = cstr_to_str_((*val).string)
            {
                utf8_add_to_width_cache(s);
            }
            a = options_array_next(a);
        }
    }
}

/// C `vendor/tmux/utf8.c:553`: `static enum utf8_state utf8_width(struct utf8_data *ud, int *width)`
pub unsafe fn utf8_width(ud: *mut utf8_data, width: *mut i32) -> utf8_state {
    unsafe {
        let mut wc: wchar_t = 0;

        if utf8_towc(ud, &raw mut wc) != utf8_state::UTF8_DONE {
            return utf8_state::UTF8_ERROR;
        }
        // A `codepoint-widths` override wins over the built-in defaults, exactly
        // as C's merged width cache returns the per-entry width first
        // (utf8.c:560-564).
        if let Some(w) = utf8_find_in_width_cache(wc) {
            *width = w as i32;
            log_debug!("cached width for {:08X} is {}", wc as u32, *width);
            return utf8_state::UTF8_DONE;
        }
        // The regional-indicator code points U+1F1E6..=U+1F1FF are listed in
        // UTF8_FORCE_WIDE, but C's utf8_default_width_cache (utf8.c:60-85) gives
        // them .width = 1 (every other cached entry is width 2). C's utf8_width
        // returns the per-entry cached width (utf8.c:560-564), so a flag emoji
        // (two regional indicators) is width 1+1=2, not 2+2=4. Match that by
        // yielding width 1 for this range before the width-2 table check.
        if (0x1F1E6..=0x1F1FF).contains(&wc) {
            *width = 1;
            return utf8_state::UTF8_DONE;
        }
        if utf8_in_table(wc, &UTF8_FORCE_WIDE) {
            *width = 2;
            return utf8_state::UTF8_DONE;
        }
        if cfg!(feature = "utf8proc") {
            #[cfg(feature = "utf8proc")]
            {
                *width = utf8proc_wcwidth(wc);
                log_debug!("utf8proc_wcwidth({:05X}) returned {}", wc, *width);
            }
        } else {
            *width = wcwidth(wc);
            log_debug!("wcwidth({:05X}) returned {}", wc, *width);
            #[expect(clippy::bool_to_int_with_if, reason = "more readable this way")]
            if *width < 0 {
                *width = if (0x80..=0x9f).contains(&wc) { 0 } else { 1 };
            }
        }
        if *width >= 0 && *width <= 0xff {
            return utf8_state::UTF8_DONE;
        }
        utf8_state::UTF8_ERROR
    }
}

/// C `vendor/tmux/utf8.c:587`: `enum utf8_state utf8_towc(const struct utf8_data *ud, wchar_t *wc)`
pub unsafe fn utf8_towc(ud: *const utf8_data, wc: *mut wchar_t) -> utf8_state {
    unsafe {
        #[cfg(feature = "utf8proc")]
        let value = utf8proc_mbtowc(wc, (*ud).data.as_ptr().cast(), (*ud).size as usize);
        #[cfg(not(feature = "utf8proc"))]
        let value = mbtowc(wc, (*ud).data.as_ptr().cast(), (*ud).size as usize);

        match value {
            -1 => {
                log_debug!(
                    "UTF-8 {}, mbtowc() {}",
                    String::from_utf8_lossy((*ud).initialized_slice()),
                    errno!(),
                );
                mbtowc(null_mut(), null(), MB_CUR_MAX());
                return utf8_state::UTF8_ERROR;
            }
            0 => return utf8_state::UTF8_ERROR,
            _ => (),
        }
        log_debug!(
            "UTF-8 {1:0$} is {2:5X}",
            (*ud).size as usize,
            _s((&raw const (*ud).data).cast::<u8>()),
            *wc as u32,
        );
    }

    utf8_state::UTF8_DONE
}

/// C `vendor/tmux/utf8.c:608`: `enum utf8_state utf8_fromwc(wchar_t wc, struct utf8_data *ud)`
pub unsafe fn utf8_fromwc(wc: wchar_t, ud: *mut utf8_data) -> utf8_state {
    unsafe {
        let mut width: i32 = 0;

        #[cfg(feature = "utf8proc")]
        let size = utf8proc_wctomb((*ud).data.as_mut_ptr().cast(), wc);
        #[cfg(not(feature = "utf8proc"))]
        let size = wctomb((*ud).data.as_mut_ptr().cast(), wc);

        if size < 0 {
            log_debug!("UTF-8 {}, wctomb() {}", wc, errno!());
            wctomb(null_mut(), 0);
            return utf8_state::UTF8_ERROR;
        }
        if size == 0 {
            return utf8_state::UTF8_ERROR;
        }
        (*ud).have = size as u8;
        (*ud).size = size as u8;
        if utf8_width(ud, &raw mut width) == utf8_state::UTF8_DONE {
            (*ud).width = width as u8;
            return utf8_state::UTF8_DONE;
        }
    }
    utf8_state::UTF8_ERROR
}

/// C `vendor/tmux/utf8.c:640`: `enum utf8_state utf8_open(struct utf8_data *ud, u_char ch)`
pub unsafe fn utf8_open(ud: *mut utf8_data, ch: c_uchar) -> utf8_state {
    unsafe {
        memset(ud.cast(), 0, size_of::<utf8_data>());

        (*ud).size = match ch {
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => return utf8_state::UTF8_ERROR,
        };

        utf8_append(ud, ch);
    }

    utf8_state::UTF8_MORE
}

/// C `vendor/tmux/utf8.c:657`: `enum utf8_state utf8_append(struct utf8_data *ud, u_char ch)`
pub unsafe fn utf8_append(ud: *mut utf8_data, ch: c_uchar) -> utf8_state {
    unsafe {
        let mut width: i32 = 0;

        if (*ud).have >= (*ud).size {
            fatalx("UTF-8 character overflow");
        }
        if (*ud).size > UTF8_SIZE as u8 {
            fatalx("UTF-8 character size too large");
        }

        if (*ud).have != 0 && (ch & 0xc0) != 0x80 {
            (*ud).width = 0xff;
        }

        (*ud).data[(*ud).have as usize] = ch;
        (*ud).have += 1;
        if (*ud).have != (*ud).size {
            return utf8_state::UTF8_MORE;
        }

        if (*ud).width == 0xff {
            return utf8_state::UTF8_ERROR;
        }
        if utf8_width(ud, &raw mut width) != utf8_state::UTF8_DONE {
            return utf8_state::UTF8_ERROR;
        }
        (*ud).width = width as u8;
    }
    utf8_state::UTF8_DONE
}

/// C `vendor/tmux/utf8.c:690`: `size_t utf8_strvis(char *dst, const char *src, size_t len, int flag)`
pub unsafe fn utf8_strvis(
    mut dst: *mut u8,
    mut src: *const u8,
    len: usize,
    flag: vis_flags,
) -> i32 {
    unsafe {
        let mut ud: utf8_data = zeroed();
        let start = dst;
        let end = src.add(len);
        let mut more: utf8_state;

        while src < end {
            more = utf8_open(&raw mut ud, *src);
            if more == utf8_state::UTF8_MORE {
                // C: while (++src < end && more == UTF8_MORE) — src must advance
                // BEFORE each append, else the same continuation byte is read
                // repeatedly and a valid multibyte char is mangled into octal.
                loop {
                    src = src.add(1);
                    if !(src < end && more == utf8_state::UTF8_MORE) {
                        break;
                    }
                    more = utf8_append(&raw mut ud, *src);
                }
                if more == utf8_state::UTF8_DONE {
                    // UTF-8 character finished.
                    for i in 0..ud.size {
                        *dst = ud.data[i as usize];
                        dst = dst.add(1);
                    }
                    continue;
                }
                // Not a complete, valid UTF-8 character.
                src = src.sub(ud.have as usize);
            }
            if flag.intersects(vis_flags::VIS_DQ) && *src == b'$' && src < end.sub(1) {
                if (*src.add(1)).is_ascii_alphabetic() || *src.add(1) == b'_' || *src.add(1) == b'{'
                {
                    *dst = b'\\';
                    dst = dst.add(1);
                }
                *dst = b'$';
                dst = dst.add(1);
            } else if src < end.sub(1) {
                dst = vis(dst, *src as i32, flag, *src.add(1) as i32);
            } else if src < end {
                dst = vis(dst, *src as i32, flag, b'\0' as i32);
            }
            src = src.add(1);
        }
        *dst = b'\0';
        (dst.addr() - start.addr()) as i32
    }
}

pub unsafe fn utf8_strvis_(dst: &mut Vec<u8>, mut src: *const u8, len: usize, flag: vis_flags) {
    unsafe {
        let mut ud: utf8_data = zeroed();
        let end = src.add(len);
        let mut more: utf8_state;

        while src < end {
            more = utf8_open(&raw mut ud, *src);
            if more == utf8_state::UTF8_MORE {
                // C: while (++src < end && more == UTF8_MORE) — src must advance
                // BEFORE each append, else the same continuation byte is read
                // repeatedly and a valid multibyte char is mangled into octal.
                loop {
                    src = src.add(1);
                    if !(src < end && more == utf8_state::UTF8_MORE) {
                        break;
                    }
                    more = utf8_append(&raw mut ud, *src);
                }
                if more == utf8_state::UTF8_DONE {
                    // UTF-8 character finished.
                    dst.extend(ud.initialized_slice());
                    continue;
                }
                // Not a complete, valid UTF-8 character.
                src = src.sub(ud.have as usize);
            }
            if flag.intersects(vis_flags::VIS_DQ) && *src == b'$' && src < end.sub(1) {
                if (*src.add(1)).is_ascii_alphabetic() || *src.add(1) == b'_' || *src.add(1) == b'{'
                {
                    dst.push(b'\\');
                }
                dst.push(b'$');
            } else if src < end.sub(1) {
                vis__(dst, *src as i32, flag, *src.add(1) as i32);
            } else if src < end {
                vis__(dst, *src as i32, flag, b'\0' as i32);
            }
            src = src.add(1);
        }
    }
}

/// C `vendor/tmux/utf8.c:728`: `size_t utf8_stravis(char **dst, const char *src, int flag)`
pub unsafe fn utf8_stravis(dst: *mut *mut u8, src: *const u8, flag: vis_flags) -> i32 {
    unsafe {
        let buf = xreallocarray(null_mut(), 4, strlen(src) + 1);
        let len = utf8_strvis(buf.as_ptr().cast(), src, strlen(src), flag);

        *dst = xrealloc(buf.as_ptr(), len as usize + 1).as_ptr().cast();
        len
    }
}

pub unsafe fn utf8_stravis_(src: *const u8, flag: vis_flags) -> Vec<u8> {
    unsafe {
        let mut buf: Vec<u8> = Vec::with_capacity(4 * (strlen(src) + 1));
        utf8_strvis_(&mut buf, src, strlen(src), flag);
        buf.shrink_to_fit();
        buf
    }
}

/// C `vendor/tmux/utf8.c:742`: `size_t utf8_stravisx(char **dst, const char *src, size_t srclen, int flag)`
pub unsafe fn utf8_stravisx(
    dst: *mut *mut u8,
    src: *const u8,
    srclen: usize,
    flag: vis_flags,
) -> i32 {
    unsafe {
        let buf = xreallocarray(null_mut(), 4, srclen + 1);
        let len = utf8_strvis(buf.as_ptr().cast(), src, srclen, flag);

        *dst = xrealloc(buf.as_ptr(), len as usize + 1).as_ptr().cast();
        len
    }
}

/// C `vendor/tmux/utf8.c:756`: `int utf8_isvalid(const char *s)`
pub unsafe fn utf8_isvalid(mut s: *const u8) -> bool {
    unsafe {
        let mut ud: utf8_data = zeroed();

        let end = s.add(strlen(s));
        while s < end {
            let mut more = utf8_open(&raw mut ud, *s);
            if more == utf8_state::UTF8_MORE {
                while {
                    s = s.add(1);
                    s < end && more == utf8_state::UTF8_MORE
                } {
                    more = utf8_append(&raw mut ud, *s);
                }
                if more == utf8_state::UTF8_DONE {
                    continue;
                }
                return false;
            }
            if *s < 0x20 || *s > 0x7e {
                return false;
            }
            s = s.add(1);
        }
    }

    true
}

/// C `vendor/tmux/utf8.c:784`: `char *utf8_sanitize(const char *src)`
pub unsafe fn utf8_sanitize(mut src: *const u8) -> *mut u8 {
    unsafe {
        let mut dst: *mut u8 = null_mut();
        let mut n: usize = 0;
        let mut ud: utf8_data = zeroed();

        while *src != b'\0' {
            dst = xreallocarray_(dst, n + 1).as_ptr();
            let mut more = utf8_open(&raw mut ud, *src);
            if more == utf8_state::UTF8_MORE {
                while {
                    src = src.add(1);
                    *src != b'\0' && more == utf8_state::UTF8_MORE
                } {
                    more = utf8_append(&raw mut ud, *src);
                }
                if more == utf8_state::UTF8_DONE {
                    dst = xreallocarray_(dst, n + ud.width as usize).as_ptr();
                    for _ in 0..ud.width {
                        *dst.add(n) = b'_';
                        n += 1;
                    }
                    continue;
                }
                src = src.sub(ud.have as usize);
            }
            if *src > 0x1f && *src < 0x7f {
                *dst.add(n) = *src;
                n += 1;
            } else {
                *dst.add(n) = b'_';
                n += 1;
            }
            src = src.add(1);
        }
        dst = xreallocarray_(dst, n + 1).as_ptr();
        *dst.add(n) = b'\0';
        dst
    }
}

/// C `vendor/tmux/utf8.c:819`: `size_t utf8_strlen(const struct utf8_data *s)`
pub unsafe fn utf8_strlen(s: *const utf8_data) -> usize {
    let mut i = 0;

    unsafe {
        while (*s.add(i)).size != 0 {
            i += 1;
        }
    }

    i
}

/// C `vendor/tmux/utf8.c:830`: `u_int utf8_strwidth(const struct utf8_data *s, ssize_t n)`
pub unsafe fn utf8_strwidth(s: *const utf8_data, n: isize) -> u32 {
    unsafe {
        let mut width: u32 = 0;

        let mut i: isize = 0;
        while (*s.add(i as usize)).size != 0 {
            if n != -1 && n == i {
                break;
            }
            width += (*s.add(i as usize)).width as u32;
            i += 1;
        }

        width
    }
}

/// C `vendor/tmux/utf8.c:848`: `struct utf8_data *utf8_fromcstr(const char *src)`
pub unsafe fn utf8_fromcstr(mut src: *const u8) -> *mut utf8_data {
    unsafe {
        let mut dst: *mut utf8_data = null_mut();
        let mut n = 0;

        while *src != b'\0' {
            dst = xreallocarray_(dst, n + 1).as_ptr();
            let mut more = utf8_open(dst.add(n), *src);
            if more == utf8_state::UTF8_MORE {
                while {
                    src = src.add(1);
                    *src != b'\0' && more == utf8_state::UTF8_MORE
                } {
                    more = utf8_append(dst.add(n), *src);
                }
                if more == utf8_state::UTF8_DONE {
                    n += 1;
                    continue;
                }
                src = src.sub((*dst.add(n)).have as usize);
            }
            utf8_set(dst.add(n), *src);
            n += 1;
            src = src.add(1);
        }
        dst = xreallocarray_(dst, n + 1).as_ptr();
        (*dst.add(n)).size = 0;

        dst
    }
}

/// C `vendor/tmux/utf8.c:876`: `char *utf8_tocstr(struct utf8_data *src)`
pub unsafe fn utf8_tocstr(mut src: *const utf8_data) -> *mut u8 {
    unsafe {
        let mut dst = null_mut::<u8>();
        let mut n: usize = 0;

        while (*src).size != 0 {
            dst = xreallocarray_(dst, n + (*src).size as usize).as_ptr();
            memcpy(
                dst.add(n).cast(),
                (*src).data.as_ptr().cast(),
                (*src).size as usize,
            );
            n += (*src).size as usize;
            src = src.add(1);
        }
        dst = xreallocarray_(dst, n + 1).as_ptr();
        *dst.add(n) = b'\0';
        dst
    }
}

// unlike utf8_tocstr, this can handle the empty vec case
// but perhaps an explicit check may speed up this common case
pub fn utf8_to_string(src: &[utf8_data]) -> String {
    let mut dst: Vec<u8> = Vec::new();

    for src in src {
        if src.size == 0 {
            // TODO evaluate if this is actually needed
            // before refactoring size == 0 is used as a sentinal value
            // after refactoring we keep length information with the slice
            // but some code may still set size to 0 in some place to truncate
            // or for other reasons
            break;
        }
        dst.extend(src.initialized_slice());
    }

    String::from_utf8(dst).unwrap()
}

/// C `vendor/tmux/utf8.c:893`: `u_int utf8_cstrwidth(const char *s)`
pub unsafe fn utf8_cstrwidth(mut s: *const u8) -> u32 {
    unsafe {
        let mut tmp: utf8_data = zeroed();

        let mut width: u32 = 0;
        while *s != b'\0' {
            let mut more = utf8_open(&raw mut tmp, *s);
            if more == utf8_state::UTF8_MORE {
                while {
                    s = s.add(1);
                    *s != b'\0' && more == utf8_state::UTF8_MORE
                } {
                    more = utf8_append(&raw mut tmp, *s);
                }
                if more == utf8_state::UTF8_DONE {
                    width += tmp.width as u32;
                    continue;
                }
                s = s.sub(tmp.have as usize);
            }
            if *s > 0x1f && *s != 0x7f {
                width += 1;
            }
            s = s.add(1);
        }
        width
    }
}

/// C `vendor/tmux/utf8.c:919`: `char *utf8_padcstr(const char *s, u_int width)`
pub unsafe fn utf8_padcstr(s: *const u8, width: u32) -> *mut u8 {
    unsafe {
        let n = utf8_cstrwidth(s);
        if n >= width {
            return xstrdup(s).as_ptr();
        }

        let mut slen = strlen(s);
        let out: *mut u8 = xmalloc(slen + 1 + (width - n) as usize).as_ptr().cast();
        memcpy(out.cast(), s.cast(), slen);
        let mut i = n;
        while i < width {
            *out.add(slen) = b' ';
            slen += 1;
            i += 1;
        }
        *out.add(slen) = b'\0';
        out
    }
}

/// C `vendor/tmux/utf8.c:940`: `char *utf8_rpadcstr(const char *s, u_int width)`
pub unsafe fn utf8_rpadcstr(s: *const u8, width: u32) -> *mut u8 {
    unsafe {
        let n = utf8_cstrwidth(s);
        if n >= width {
            return xstrdup(s).as_ptr();
        }

        let slen = strlen(s);
        let out: *mut u8 = xmalloc(slen + 1 + (width - n) as usize).as_ptr().cast();
        let mut i = 0;
        // C: for (i = 0; i < width - n; i++) — pad to the *total* field width,
        // not `width` extra spaces (which also overran the width-n allocation).
        while i < width - n {
            *out.add(i as usize) = b' ';
            i += 1;
        }
        memcpy(out.add(i as usize).cast(), s.cast(), slen);
        *out.add(i as usize + slen) = b'\0';
        out
    }
}

/// C `vendor/tmux/utf8.c:960`: `int utf8_cstrhas(const char *s, const struct utf8_data *ud)`
pub unsafe fn utf8_cstrhas(s: *const u8, ud: *const utf8_data) -> bool {
    let mut found = false;

    unsafe {
        let copy = utf8_fromcstr(s);
        let mut loop_ = copy;
        while (*loop_).size != 0 {
            if (*loop_).size != (*ud).size {
                loop_ = loop_.add(1);
                continue;
            }
            if memcmp(
                (*loop_).data.as_ptr().cast(),
                (*ud).data.as_ptr().cast(),
                (*loop_).size as usize,
            ) == 0
            {
                found = true;
                break;
            }
            loop_ = loop_.add(1);
        }

        free_(copy);

        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Run a byte string (NUL-terminated internally) through utf8_stravis_ with
    // the same flags args_escape uses.
    unsafe fn stravis(bytes: &[u8]) -> Vec<u8> {
        let mut c = bytes.to_vec();
        c.push(0);
        let flags = vis_flags::VIS_OCTAL
            | vis_flags::VIS_CSTYLE
            | vis_flags::VIS_TAB
            | vis_flags::VIS_NL;
        unsafe { utf8_stravis_(c.as_ptr(), flags) }
    }

    // Decode a vis(3)-escaped string back to raw bytes via the ported strunvis.
    unsafe fn unvis(escaped: &[u8]) -> Vec<u8> {
        let mut src = escaped.to_vec();
        src.push(0);
        let mut dst = vec![0u8; src.len()];
        let n = unsafe { crate::compat::strunvis(dst.as_mut_ptr(), src.as_ptr()) };
        assert!(n >= 0, "strunvis failed on {escaped:02x?}");
        dst.truncate(n as usize);
        dst
    }

    // Regression for the utf8_strvis inner-loop bug: it advanced `src` once
    // before the append loop but not inside it (C uses `while (++src < end ...)`),
    // so every continuation byte after the first was re-read. A valid multibyte
    // char like € (E2 82 AC) came out as `E2 82 82 \202 \254` — raw garbage plus
    // octal escapes — which is what showed up as `\202\202\202` in the status bar.
    //
    // The assertion is round-trip (vis then unvis == identity) rather than raw
    // passthrough, because passthrough is locale-dependent: in a UTF-8 locale a
    // valid char is emitted raw, but under the C locale glibc can't compute its
    // width so it's octal-escaped instead (macOS libc is permissive, so a plain
    // passthrough check is green on macOS but red on Linux CI). Round-trip holds
    // in BOTH cases — and it's exactly the invariant the buggy duplication broke.
    #[test]
    fn utf8_strvis_roundtrips_through_unvis() {
        let cases: &[&[u8]] = &[
            b"plain ascii",
            &[0xc3, 0xa9],                   // é         U+00E9  (2 bytes)
            &[0xe2, 0x82, 0xac],             // €         U+20AC  (3 bytes)
            &[0xee, 0x82, 0xb0],             // powerline U+E0B0  (3 bytes)
            &[0xf0, 0x9f, 0x98, 0x80],       // 😀        U+1F600 (4 bytes)
            b"A\xe2\x82\xacB\xee\x82\xb0C",  // mixed ascii + multibyte
        ];
        for c in cases {
            let escaped = unsafe { stravis(c) };
            let decoded = unsafe { unvis(&escaped) };
            assert_eq!(
                decoded.as_slice(),
                *c,
                "vis/unvis must round-trip for {c:02x?}; escaped={escaped:02x?}"
            );
        }
    }

    // A lone continuation byte is not valid UTF-8 and must still be octal-escaped.
    #[test]
    fn invalid_byte_is_octal_escaped() {
        let out = unsafe { stravis(&[0x82]) };
        assert_eq!(out.as_slice(), b"\\202");
    }

    // Several helpers below decode multibyte characters through mbtowc()/wcwidth(),
    // which are locale-sensitive. The binary sets a UTF-8 LC_CTYPE in main(), but
    // unit tests run without that, so establish it once (idempotent, process-global).
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

    // Read a NUL-terminated C string produced by a ported function into a Vec.
    unsafe fn cstr_bytes(p: *const u8) -> Vec<u8> {
        unsafe { std::slice::from_raw_parts(p, strlen(p)).to_vec() }
    }

    // utf8_open primes the state from a lead byte, then each utf8_append feeds a
    // continuation byte until `have == size`; the final append computes the width.
    // Build € (U+20AC, E2 82 AC) byte by byte.
    #[test]
    fn utf8_open_append_builds_multibyte() {
        ensure_utf8_locale();
        unsafe {
            let mut ud: utf8_data = zeroed();

            assert_eq!(utf8_open(&raw mut ud, 0xe2), utf8_state::UTF8_MORE);
            assert_eq!(ud.size, 3);
            assert_eq!(ud.have, 1);

            assert_eq!(utf8_append(&raw mut ud, 0x82), utf8_state::UTF8_MORE);
            assert_eq!(ud.have, 2);

            assert_eq!(utf8_append(&raw mut ud, 0xac), utf8_state::UTF8_DONE);
            assert_eq!(ud.have, 3);
            assert_eq!(ud.size, 3);
            assert_eq!(&ud.data[..3], &[0xe2, 0x82, 0xac]);
            assert_eq!(ud.width, 1); // € occupies one column
        }
    }

    // utf8_width fills *width from wcwidth(): ASCII is 1 column, a CJK ideograph 2.
    #[test]
    fn utf8_width_ascii_and_wide() {
        ensure_utf8_locale();
        unsafe {
            let mut ascii = utf8_data::new([b'A'], 1, 1, 0);
            let mut w: i32 = -1;
            assert_eq!(utf8_width(&raw mut ascii, &raw mut w), utf8_state::UTF8_DONE);
            assert_eq!(w, 1);

            // U+4E00 一 (E4 B8 80) is a wide (double-width) CJK ideograph.
            let mut cjk = utf8_data::new([0xe4, 0xb8, 0x80], 3, 3, 0);
            let mut w2: i32 = -1;
            assert_eq!(utf8_width(&raw mut cjk, &raw mut w2), utf8_state::UTF8_DONE);
            assert_eq!(w2, 2);
        }
    }

    // Regional-indicator code points (U+1F1E6..=U+1F1FF) are in UTF8_FORCE_WIDE
    // but have .width = 1 in C's utf8_default_width_cache (utf8.c:60-85). A single
    // indicator must report width 1 (so a two-indicator flag is width 2, not 4),
    // while a genuinely-wide entry still in the force-wide table reports width 2.
    #[test]
    fn utf8_width_regional_indicator_is_one() {
        ensure_utf8_locale();
        unsafe {
            // U+1F1FA (regional indicator "U", part of flags) = F0 9F 87 BA.
            let mut ri = utf8_data::new([0xf0, 0x9f, 0x87, 0xba], 4, 4, 0);
            let mut w: i32 = -1;
            assert_eq!(utf8_width(&raw mut ri, &raw mut w), utf8_state::UTF8_DONE);
            assert_eq!(w, 1, "regional indicator U+1F1FA must be width 1");

            // U+1F385 (Santa Claus) = F0 9F 8E 85: a force-wide entry with width 2.
            let mut wide = utf8_data::new([0xf0, 0x9f, 0x8e, 0x85], 4, 4, 0);
            let mut w2: i32 = -1;
            assert_eq!(utf8_width(&raw mut wide, &raw mut w2), utf8_state::UTF8_DONE);
            assert_eq!(w2, 2, "force-wide entry U+1F385 must stay width 2");
        }
    }

    // utf8_fromwc encodes a wide char into utf8_data; utf8_towc decodes it back.
    // The pair must round-trip for ASCII and multibyte code points.
    #[test]
    fn utf8_towc_fromwc_roundtrip() {
        ensure_utf8_locale();
        for &wc in &[0x41 as wchar_t, 0x20ac, 0x4e00] {
            unsafe {
                let mut ud: utf8_data = zeroed();
                assert_eq!(utf8_fromwc(wc, &raw mut ud), utf8_state::UTF8_DONE);

                let mut back: wchar_t = 0;
                assert_eq!(utf8_towc(&raw const ud, &raw mut back), utf8_state::UTF8_DONE);
                assert_eq!(back, wc);
            }
        }
    }

    // utf8_strlen counts entries up to the size==0 terminator; utf8_strwidth sums
    // their widths (stopping at index n when n != -1).
    #[test]
    fn utf8_strlen_and_strwidth() {
        // Widths are stored directly on the structs, so no locale is needed here.
        let arr = [
            utf8_data::new([b'A'], 1, 1, 1),          // width 1
            utf8_data::new([0xe4, 0xb8, 0x80], 3, 3, 2), // 一 width 2
            utf8_data::new([0xe2, 0x82, 0xac], 3, 3, 1), // € width 1
            utf8_data::new([0u8], 0, 0, 0),           // terminator (size == 0)
        ];
        unsafe {
            assert_eq!(utf8_strlen(arr.as_ptr()), 3);
            assert_eq!(utf8_strwidth(arr.as_ptr(), -1), 4); // 1 + 2 + 1
            assert_eq!(utf8_strwidth(arr.as_ptr(), 2), 3); // first two: 1 + 2
        }
    }

    // utf8_cstrwidth measures the display width of a NUL-terminated C string that
    // mixes ASCII and multibyte characters.
    #[test]
    fn utf8_cstrwidth_mixed() {
        ensure_utf8_locale();
        // "A€B一": A=1, €=1, B=1, 一=2 => 5 columns.
        let s = b"A\xe2\x82\xacB\xe4\xb8\x80\0";
        let w = unsafe { utf8_cstrwidth(s.as_ptr()) };
        assert_eq!(w, 5);
    }

    // utf8_isvalid accepts well-formed UTF-8 (ASCII and multibyte) and rejects a
    // lone continuation byte.
    #[test]
    #[expect(
        clippy::manual_c_str_literals,
        reason = "raw byte pointers, incl. invalid UTF-8, are the point of these FFI tests"
    )]
    fn utf8_isvalid_true_and_false() {
        ensure_utf8_locale();
        unsafe {
            assert!(utf8_isvalid(b"hello\0".as_ptr()));
            assert!(utf8_isvalid(b"A\xe2\x82\xac\0".as_ptr())); // "A€"
            assert!(!utf8_isvalid(b"\x82\0".as_ptr())); // lone continuation byte
        }
    }

    // utf8_sanitize rewrites bytes that aren't printable ASCII (and invalid UTF-8)
    // into '_' underscores.
    #[test]
    #[expect(
        clippy::manual_c_str_literals,
        reason = "raw byte pointers, incl. invalid UTF-8, are the point of these FFI tests"
    )]
    fn utf8_sanitize_replaces_invalid() {
        unsafe {
            // 'a', lone-continuation 0x82, 'b', control 0x01 -> "a_b_".
            let out = utf8_sanitize(b"a\x82b\x01\0".as_ptr());
            assert_eq!(cstr_bytes(out).as_slice(), b"a_b_");
            free_(out);
        }
    }

    // utf8_set (utf8.c:527) fills a utf8_data with a single one-column,
    // one-byte ASCII cell.
    #[test]
    fn utf8_set_single_ascii() {
        unsafe {
            let mut ud: utf8_data = zeroed();
            utf8_set(&mut ud, b'Q');
            assert_eq!(ud.data[0], b'Q');
            assert_eq!(ud.size, 1);
            assert_eq!(ud.width, 1);
            assert_eq!(ud.have, 1);
        }
    }

    // utf8_padcstr (utf8.c:919) left-justifies: it appends trailing spaces until
    // the display width reaches `width`, and returns the string unchanged when it
    // already meets or exceeds it.
    #[test]
    fn utf8_padcstr_left_justifies() {
        unsafe {
            let out = utf8_padcstr(c"ab".as_ptr().cast(), 5);
            assert_eq!(cstr_bytes(out).as_slice(), b"ab   ");
            free_(out);

            // Exact width: unchanged.
            let out = utf8_padcstr(c"abcde".as_ptr().cast(), 5);
            assert_eq!(cstr_bytes(out).as_slice(), b"abcde");
            free_(out);

            // Wider than the field: also unchanged (never truncates).
            let out = utf8_padcstr(c"abcdef".as_ptr().cast(), 3);
            assert_eq!(cstr_bytes(out).as_slice(), b"abcdef");
            free_(out);
        }
    }

    // utf8_rpadcstr (utf8.c:940) right-justifies: it prepends leading spaces to
    // pad the field to `width`.
    #[test]
    fn utf8_rpadcstr_right_justifies() {
        unsafe {
            let out = utf8_rpadcstr(c"ab".as_ptr().cast(), 5);
            assert_eq!(cstr_bytes(out).as_slice(), b"   ab");
            free_(out);

            // Already wide enough: unchanged.
            let out = utf8_rpadcstr(c"abcde".as_ptr().cast(), 2);
            assert_eq!(cstr_bytes(out).as_slice(), b"abcde");
            free_(out);
        }
    }

    // utf8_cstrhas (utf8.c:960) reports whether a string contains a given cell.
    #[test]
    fn utf8_cstrhas_finds_char() {
        unsafe {
            let mut ud: utf8_data = zeroed();
            utf8_set(&mut ud, b'c');
            assert!(utf8_cstrhas(c"abc".as_ptr().cast(), &ud));

            utf8_set(&mut ud, b'z');
            assert!(!utf8_cstrhas(c"abc".as_ptr().cast(), &ud));

            // Empty haystack contains nothing.
            utf8_set(&mut ud, b'a');
            assert!(!utf8_cstrhas(c"".as_ptr().cast(), &ud));
        }
    }

    // Build a utf8_data from a runtime byte slice (utf8_data::new needs a const
    // array length). `have == size` so the character is treated as complete.
    fn ud_from(bytes: &[u8], width: u8) -> utf8_data {
        assert!(bytes.len() < UTF8_SIZE);
        let mut data = [0u8; UTF8_SIZE];
        data[..bytes.len()].copy_from_slice(bytes);
        utf8_data {
            data,
            have: bytes.len() as u8,
            size: bytes.len() as u8,
            width,
        }
    }

    // Like `stravis` above but with VIS_DQ so `$` handling in utf8_strvis_ is
    // exercised (utf8.c:710-715).
    unsafe fn stravis_dq(bytes: &[u8]) -> Vec<u8> {
        let mut c = bytes.to_vec();
        c.push(0);
        let flags = vis_flags::VIS_OCTAL
            | vis_flags::VIS_CSTYLE
            | vis_flags::VIS_DQ;
        unsafe { utf8_stravis_(c.as_ptr(), flags) }
    }

    // utf8_from_data (utf8.c:465) packs size/width/bytes into a utf8_char;
    // utf8_to_data (utf8.c:497) unpacks it. Characters up to 3 bytes are stored
    // inline in the 24-bit index field; the pair must round-trip losslessly.
    #[test]
    fn from_data_to_data_roundtrip_inline() {
        unsafe {
            let cases: &[(&[u8], u8)] = &[
                (b"A", 1),                // ASCII, width 1
                (&[0xc3, 0xa9], 1),       // é  U+00E9 (2 bytes) width 1
                (&[0xe2, 0x82, 0xac], 1), // €  U+20AC (3 bytes) width 1
            ];
            for &(bytes, width) in cases {
                let ud = ud_from(bytes, width);
                let mut uc: utf8_char = 0;
                assert_eq!(
                    utf8_from_data(&raw const ud, &raw mut uc),
                    utf8_state::UTF8_DONE
                );
                assert_eq!(utf8_get_size(uc), bytes.len() as u8);
                assert_eq!(utf8_get_width(uc), width);

                let back = utf8_to_data(uc);
                assert_eq!(back.size, bytes.len() as u8);
                assert_eq!(back.width, width);
                assert_eq!(&back.data[..bytes.len()], bytes);
            }
        }
    }

    // A 4-byte character does not fit inline, so utf8_from_data interns it in the
    // per-thread index tree (utf8.c:478 utf8_put_item) and utf8_to_data reads it
    // back (utf8.c:512 utf8_item_by_index). Each #[test] runs on its own thread,
    // so the tree starts empty here.
    #[test]
    fn from_data_to_data_roundtrip_interned_4byte() {
        unsafe {
            // 😀 U+1F600 = F0 9F 98 80, width 2.
            let ud = ud_from(&[0xf0, 0x9f, 0x98, 0x80], 2);
            let mut uc: utf8_char = 0;
            assert_eq!(
                utf8_from_data(&raw const ud, &raw mut uc),
                utf8_state::UTF8_DONE
            );
            assert_eq!(utf8_get_size(uc), 4);
            assert_eq!(utf8_get_width(uc), 2);

            let back = utf8_to_data(uc);
            assert_eq!(back.size, 4);
            assert_eq!(back.width, 2);
            assert_eq!(&back.data[..4], &[0xf0, 0x9f, 0x98, 0x80]);
        }
    }

    // utf8_to_data for an interned (size 4) character whose index is absent from
    // the tree fills the data with spaces (utf8.c:513). On a fresh test thread the
    // tree is empty, so any 4-byte uc decodes to spaces.
    #[test]
    fn to_data_missing_index_fills_spaces() {
        // size 4, width 2, index 5 — never interned.
        let uc = utf8_set_size(4) | utf8_set_width(2) | 5;
        let ud = utf8_to_data(uc);
        assert_eq!(ud.size, 4);
        assert_eq!(&ud.data[..4], b"    ");
    }

    // utf8_from_data's fail branch (utf8.c:485): when size exceeds UTF8_SIZE the
    // returned uc is a width-dependent placeholder and the state is ERROR. The
    // three placeholders are: width 0 -> empty cell; width 1 -> a single space
    // (0x20); width 2 -> two spaces (0x2020).
    #[test]
    fn from_data_oversize_returns_placeholder() {
        unsafe {
            for &(width, expect) in &[
                (0u8, utf8_set_size(0) | utf8_set_width(0)),
                (1u8, utf8_set_size(1) | utf8_set_width(1) | 0x20),
                (2u8, utf8_set_size(1) | utf8_set_width(1) | 0x2020),
            ] {
                // size 22 > UTF8_SIZE (21) triggers the fail path.
                let ud = utf8_data {
                    data: [0u8; UTF8_SIZE],
                    have: 0,
                    size: 22,
                    width,
                };
                let mut uc: utf8_char = 0xdead_beef;
                assert_eq!(
                    utf8_from_data(&raw const ud, &raw mut uc),
                    utf8_state::UTF8_ERROR
                );
                assert_eq!(uc, expect);
            }
        }
    }

    // utf8_build_one (utf8.c:524) makes a one-byte, one-column cell from an ASCII
    // byte; it must decode via utf8_to_data to exactly that byte.
    #[test]
    fn build_one_is_single_ascii_cell() {
        let uc = utf8_build_one(b'x');
        assert_eq!(utf8_get_size(uc), 1);
        assert_eq!(utf8_get_width(uc), 1);
        let ud = utf8_to_data(uc);
        assert_eq!(ud.size, 1);
        assert_eq!(ud.width, 1);
        assert_eq!(ud.data[0], b'x');
    }

    // utf8_copy (utf8.c:541) memcpys the struct then zeroes data bytes from `size`
    // to the end, so stale tail bytes never leak into a shorter character.
    #[test]
    fn copy_zeroes_tail_bytes() {
        unsafe {
            let from = ud_from(&[0xe2, 0x82, 0xac], 1); // € size 3
            let mut to = utf8_data {
                data: [0xff; UTF8_SIZE],
                have: 9,
                size: 9,
                width: 9,
            };
            utf8_copy(&raw mut to, &raw const from);
            assert_eq!(to.size, 3);
            assert_eq!(&to.data[..3], &[0xe2, 0x82, 0xac]);
            // Everything past `size` must be zeroed.
            assert!(to.data[3..].iter().all(|&b| b == 0));
        }
    }

    // utf8_open (utf8.c:640) only accepts lead bytes C2-DF, E0-EF, F0-F4. ASCII,
    // lone continuation bytes, the overlong-encoding leads C0/C1, and F5+ are
    // rejected with UTF8_ERROR.
    #[test]
    fn open_rejects_invalid_lead_bytes() {
        unsafe {
            for &ch in &[0x00u8, b'A', 0x7f, 0x80, 0xbf, 0xc0, 0xc1, 0xf5, 0xff] {
                let mut ud: utf8_data = zeroed();
                assert_eq!(
                    utf8_open(&raw mut ud, ch),
                    utf8_state::UTF8_ERROR,
                    "lead byte {ch:#04x} must be rejected"
                );
            }
        }
    }

    // utf8_open sets `size` from the lead byte's range and primes the first byte
    // (have == 1), returning UTF8_MORE at each range boundary.
    #[test]
    fn open_sets_size_at_range_boundaries() {
        unsafe {
            for &(ch, size) in &[
                (0xc2u8, 2u8),
                (0xdf, 2),
                (0xe0, 3),
                (0xef, 3),
                (0xf0, 4),
                (0xf4, 4),
            ] {
                let mut ud: utf8_data = zeroed();
                assert_eq!(utf8_open(&raw mut ud, ch), utf8_state::UTF8_MORE);
                assert_eq!(ud.size, size);
                assert_eq!(ud.have, 1);
                assert_eq!(ud.data[0], ch);
            }
        }
    }

    // utf8_append (utf8.c:666): once a non-continuation byte (top bits != 10)
    // arrives mid-sequence, width is poisoned to 0xff and the completed character
    // is reported as UTF8_ERROR. Here 0xC2 opens a 2-byte sequence and 0x41 ('A')
    // is an illegal continuation.
    #[test]
    fn append_bad_continuation_errors() {
        unsafe {
            let mut ud: utf8_data = zeroed();
            assert_eq!(utf8_open(&raw mut ud, 0xc2), utf8_state::UTF8_MORE);
            assert_eq!(utf8_append(&raw mut ud, 0x41), utf8_state::UTF8_ERROR);
            assert_eq!(ud.width, 0xff);
        }
    }

    // utf8_towc (utf8.c:587) fails on a lone continuation byte: mbtowc cannot
    // decode it, so the state is UTF8_ERROR.
    #[test]
    fn towc_invalid_sequence_is_error() {
        ensure_utf8_locale();
        unsafe {
            let ud = ud_from(&[0x82], 1);
            let mut wc: wchar_t = 0;
            assert_eq!(utf8_towc(&raw const ud, &raw mut wc), utf8_state::UTF8_ERROR);
        }
    }

    // utf8_fromwc (utf8.c:608) encodes a wide char, filling size and width from
    // wctomb()/wcwidth(). ASCII is 1 byte / 1 column; a CJK ideograph is 3 bytes /
    // 2 columns.
    #[test]
    fn fromwc_sets_size_and_width() {
        ensure_utf8_locale();
        unsafe {
            let mut a: utf8_data = zeroed();
            assert_eq!(utf8_fromwc(0x41, &raw mut a), utf8_state::UTF8_DONE);
            assert_eq!(a.size, 1);
            assert_eq!(a.width, 1);
            assert_eq!(a.data[0], b'A');

            // U+4E00 一 = E4 B8 80, a double-width ideograph.
            let mut cjk: utf8_data = zeroed();
            assert_eq!(utf8_fromwc(0x4e00, &raw mut cjk), utf8_state::UTF8_DONE);
            assert_eq!(cjk.size, 3);
            assert_eq!(cjk.width, 2);
            assert_eq!(&cjk.data[..3], &[0xe4, 0xb8, 0x80]);
        }
    }

    // utf8_fromcstr (utf8.c:848) parses a C string into an array of utf8_data
    // (size==0 terminated); utf8_tocstr (utf8.c:876) reverses it. The pair must
    // round-trip a mix of ASCII and multibyte, and strlen/strwidth agree.
    #[test]
    fn fromcstr_tocstr_roundtrip() {
        ensure_utf8_locale();
        unsafe {
            // "A€B一": 4 characters, widths 1 + 1 + 1 + 2 = 5.
            let src = b"A\xe2\x82\xacB\xe4\xb8\x80\0";
            let arr = utf8_fromcstr(src.as_ptr());
            assert_eq!(utf8_strlen(arr), 4);
            assert_eq!(utf8_strwidth(arr, -1), 5);

            let back = utf8_tocstr(arr);
            assert_eq!(cstr_bytes(back).as_slice(), &src[..src.len() - 1]);
            free_(back);
            free_(arr);
        }
    }

    // utf8_to_string (Rust helper) concatenates the initialized bytes and stops at
    // the first size==0 terminator, ignoring anything after it.
    #[test]
    fn to_string_stops_at_terminator() {
        let arr = [
            utf8_data::new([b'A'], 1, 1, 1),
            utf8_data::new([0xe2, 0x82, 0xac], 3, 3, 1), // €
            utf8_data::new([0u8], 0, 0, 0),              // terminator
            utf8_data::new([b'Z'], 1, 1, 1),             // past terminator, ignored
        ];
        assert_eq!(utf8_to_string(&arr), "A\u{20ac}");
    }

    // utf8_strvis_ with VIS_DQ (utf8.c:710) backslash-escapes a `$` only when the
    // next byte is a letter, `_`, or `{` — the shell-variable lead-ins. A `$`
    // before a digit or at end of string is passed through bare.
    #[test]
    fn strvis_dq_escapes_shell_variable_dollar() {
        unsafe {
            assert_eq!(stravis_dq(b"$x").as_slice(), b"\\$x");
            assert_eq!(stravis_dq(b"$_").as_slice(), b"\\$_");
            assert_eq!(stravis_dq(b"${").as_slice(), b"\\${");
            // Digit after `$` is not a variable start: no escape.
            assert_eq!(stravis_dq(b"$1").as_slice(), b"$1");
            // Trailing `$` (src == end-1) skips the DQ branch entirely.
            assert_eq!(stravis_dq(b"a$").as_slice(), b"a$");
        }
    }

    // utf8_isvalid (utf8.c:756) rejects any single byte outside printable ASCII
    // 0x20..=0x7e (so controls and DEL are invalid), and accepts the boundaries.
    #[test]
    #[expect(
        clippy::manual_c_str_literals,
        reason = "raw byte pointers with control bytes are the point of this FFI test"
    )]
    fn isvalid_ascii_printable_boundaries() {
        ensure_utf8_locale();
        unsafe {
            assert!(utf8_isvalid(b" \0".as_ptr())); // 0x20 space, lowest valid
            assert!(utf8_isvalid(b"~\0".as_ptr())); // 0x7e tilde, highest valid
            assert!(!utf8_isvalid(b"\x1f\0".as_ptr())); // below 0x20
            assert!(!utf8_isvalid(b"\x7f\0".as_ptr())); // DEL, above 0x7e
            assert!(!utf8_isvalid(b"\t\0".as_ptr())); // tab is a control byte
        }
    }

    // utf8_cstrwidth (utf8.c:893) counts control bytes and DEL as zero width
    // (`*s > 0x1f && *s != 0x7f`), so only the printable ASCII contributes.
    #[test]
    fn cstrwidth_controls_and_del_are_zero_width() {
        ensure_utf8_locale();
        // "a\tb\x7fc": a=1, tab=0, b=1, DEL=0, c=1 => 3.
        let s = b"a\tb\x7fc\0";
        let w = unsafe { utf8_cstrwidth(s.as_ptr()) };
        assert_eq!(w, 3);
    }

    // utf8_sanitize (utf8.c:784) replaces each multibyte character with `width`
    // underscores: a single-width € becomes one `_`, a double-width ideograph two.
    #[test]
    #[expect(
        clippy::manual_c_str_literals,
        reason = "raw byte pointers with multibyte sequences are the point of this FFI test"
    )]
    fn sanitize_multibyte_becomes_width_underscores() {
        ensure_utf8_locale();
        unsafe {
            // € (width 1) -> one underscore.
            let out = utf8_sanitize(b"a\xe2\x82\xacb\0".as_ptr());
            assert_eq!(cstr_bytes(out).as_slice(), b"a_b");
            free_(out);
            // 一 (width 2) -> two underscores.
            let out = utf8_sanitize(b"a\xe4\xb8\x80b\0".as_ptr());
            assert_eq!(cstr_bytes(out).as_slice(), b"a__b");
            free_(out);
        }
    }

    // utf8_padcstr measures display width (not byte length): "€" is one column, so
    // padding to width 3 appends two trailing spaces after the 3 UTF-8 bytes.
    #[test]
    fn padcstr_pads_by_display_width_not_bytes() {
        ensure_utf8_locale();
        unsafe {
            let out = utf8_padcstr(c"\xe2\x82\xac".as_ptr().cast::<u8>(), 3);
            assert_eq!(cstr_bytes(out).as_slice(), b"\xe2\x82\xac  ");
            free_(out);
        }
    }

    // utf8_in_table (utf8.c) binary-searches a sorted wchar_t table. Verify it
    // finds the first, a middle, and the last entry of UTF8_FORCE_WIDE, and
    // rejects a code point that is not present.
    #[test]
    fn in_table_hits_and_misses() {
        assert!(utf8_in_table(0x0261D, &UTF8_FORCE_WIDE)); // first entry
        assert!(utf8_in_table(0x1F385, &UTF8_FORCE_WIDE)); // 🎅 middle entry
        assert!(utf8_in_table(0x1FAF8, &UTF8_FORCE_WIDE)); // last entry
        assert!(!utf8_in_table(0x0041, &UTF8_FORCE_WIDE)); // 'A' not present
        assert!(!utf8_in_table(0x1F384, &UTF8_FORCE_WIDE)); // one below 0x1F385
    }

    // utf8_open primes a 2-byte lead byte (C2..DF, utf8.c:645), then a single
    // utf8_append completes it. Build é (U+00E9, C3 A9): one continuation byte,
    // width 1. This is the 2-byte analogue of utf8_open_append_builds_multibyte.
    #[test]
    fn utf8_open_append_builds_2byte() {
        ensure_utf8_locale();
        unsafe {
            let mut ud: utf8_data = zeroed();

            assert_eq!(utf8_open(&raw mut ud, 0xc3), utf8_state::UTF8_MORE);
            assert_eq!(ud.size, 2);
            assert_eq!(ud.have, 1);

            assert_eq!(utf8_append(&raw mut ud, 0xa9), utf8_state::UTF8_DONE);
            assert_eq!(ud.have, 2);
            assert_eq!(&ud.data[..2], &[0xc3, 0xa9]);
            assert_eq!(ud.width, 1); // é occupies one column
        }
    }

    // A combining mark has zero display width (wcwidth == 0). utf8_width routes
    // through wcwidth (utf8.c:571) for a code point that is neither in the
    // force-wide nor combining default caches; U+0301 COMBINING ACUTE ACCENT
    // (CC 81) must report width 0 so it never advances the cursor.
    #[test]
    fn utf8_width_combining_mark_is_zero() {
        ensure_utf8_locale();
        unsafe {
            let mut cc = utf8_data::new([0xcc, 0x81], 2, 2, 0);
            let mut w: i32 = -1;
            assert_eq!(utf8_width(&raw mut cc, &raw mut w), utf8_state::UTF8_DONE);
            assert_eq!(w, 0, "combining acute accent must be width 0");
        }
    }

    // A two-codepoint regional-indicator flag (🇺🇸 = U+1F1FA U+1F1F8) is two
    // width-1 cells, so utf8_cstrwidth must return 2 — the session fix that gave
    // regional indicators width 1 each (utf8.c:60-85) instead of 2. If either
    // indicator regressed to width 2 this would read 4.
    #[test]
    fn cstrwidth_regional_indicator_flag_is_two() {
        ensure_utf8_locale();
        // U+1F1FA = F0 9F 87 BA, U+1F1F8 = F0 9F 87 B8.
        let s = b"\xf0\x9f\x87\xba\xf0\x9f\x87\xb8\0";
        let w = unsafe { utf8_cstrwidth(s.as_ptr()) };
        assert_eq!(w, 2);
    }

    // utf8_fromwc/utf8_towc must round-trip a 2-byte (é U+00E9) and a 4-byte
    // (😀 U+1F600) code point, complementing the 1/3-byte cases in
    // utf8_towc_fromwc_roundtrip (utf8.c:587, 608).
    #[test]
    fn towc_fromwc_roundtrip_2byte_and_4byte() {
        ensure_utf8_locale();
        for &wc in &[0x00e9 as wchar_t, 0x1f600] {
            unsafe {
                let mut ud: utf8_data = zeroed();
                assert_eq!(utf8_fromwc(wc, &raw mut ud), utf8_state::UTF8_DONE);
                let mut back: wchar_t = 0;
                assert_eq!(utf8_towc(&raw const ud, &raw mut back), utf8_state::UTF8_DONE);
                assert_eq!(back, wc);
            }
        }
    }

    // utf8_stravis_ then strunvis must round-trip a string mixing a combining
    // mark (e + U+0301) and a regional indicator, not just standalone multibyte
    // characters — the escape/unescape pair is lossless regardless of width.
    #[test]
    fn utf8_stravis_roundtrips_combining_and_regional() {
        let cases: &[&[u8]] = &[
            b"e\xcc\x81",                     // e + combining acute (é decomposed)
            &[0xf0, 0x9f, 0x87, 0xba],       // regional indicator U
            b"a\xcc\x81\xf0\x9f\x87\xbaz",   // mixed
        ];
        for c in cases {
            let escaped = unsafe { stravis(c) };
            let decoded = unsafe { unvis(&escaped) };
            assert_eq!(
                decoded.as_slice(),
                *c,
                "vis/unvis must round-trip {c:02x?}; escaped={escaped:02x?}"
            );
        }
    }
}
