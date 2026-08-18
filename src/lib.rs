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
#![expect(rustdoc::broken_intra_doc_links, reason = "github markdown callout")]
// The README is included as crate docs; its ASCII banner and layout-tree fences
// are plain text, not Rust, so allow rustdoc's code-block parse lint (keeps the
// strict `RUSTDOCFLAGS=-D warnings` CI doc gate green).
#![allow(rustdoc::invalid_rust_codeblocks)]
#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![cfg_attr(
    fuzzing,
    allow(
        private_interfaces,
        reason = "we use the fuzzing config flag to mark modules public which otherwise wouldn't be in order to fuzz internal implementations"
    )
)]
#![allow(
    non_camel_case_types,
    reason = "this lint is here instead of in Cargo.toml because of a bug in rust analyzer"
)]

#[path = "ported/libc.rs"]
mod libc;
pub(crate) use crate::libc::errno;
pub(crate) use crate::libc::*;
pub(crate) use crate::libc::{free_, memcpy_, memcpy__, streq_};

// The event loop (a Rust replacement for libevent; see src/extensions/event_loop).
#[path = "extensions/event_loop/mod.rs"]
mod event_;
use terminfo_lean::expand::ExpandContext;

use crate::event_::*;

macro_rules! cfg_pub_mods {
    // Optional `#[path = "..."]` bridges a module whose file was renamed to the
    // tmux C-mirroring convention (e.g. file `grid.rs`) while keeping the module
    // identifier (`grid_`) that avoids clashing with the same-named transpiled
    // struct at the crate root. When those placeholder structs are replaced, the
    // `#[path]` + trailing underscore can drop and the module becomes `mod grid;`.
    //
    // The ported source lives under `src/ported/`, so every entry below carries
    // an explicit `#[path = "ported/…"]` (the module identifiers stay at the
    // crate root, so all `crate::…` paths in the ported code are unaffected).
    ($( $(#[path = $p:literal])? mod $mod_name:ident; )*) => {
        $(
            #[cfg(fuzzing)]
            $(#[path = $p])?
            pub mod $mod_name;

            #[cfg(not(fuzzing))]
            $(#[path = $p])?
            mod $mod_name;
        )*
    };
}

cfg_pub_mods! {
    #[path = "ported/alerts.rs"]
    mod alerts;
    #[path = "ported/arguments.rs"]
    mod arguments;
    #[path = "ported/attributes.rs"]
    mod attributes;
    #[path = "ported/bitstr.rs"]
    mod bitstr;
    #[path = "ported/cfg.rs"]
    mod cfg_;
    #[path = "ported/client.rs"]
    mod client_;
    #[path = "ported/cmd.rs"]
    mod cmd_;
    #[path = "ported/cmd_parse.rs"]
    mod cmd_parse;
    #[path = "ported/colour.rs"]
    mod colour;
    #[path = "ported/compat/mod.rs"]
    mod compat;
    #[path = "ported/control.rs"]
    mod control;
    #[path = "ported/control_notify.rs"]
    mod control_notify;
    #[path = "ported/environ.rs"]
    mod environ_;
    #[path = "ported/file.rs"]
    mod file;
    #[path = "ported/format.rs"]
    mod format;
    #[path = "ported/format_draw.rs"]
    mod format_draw_;
    #[path = "ported/fuzzy.rs"]
    mod fuzzy;
    #[path = "ported/grid.rs"]
    mod grid_;
    #[path = "ported/grid_reader.rs"]
    mod grid_reader_;
    #[path = "ported/grid_view.rs"]
    mod grid_view;
    #[path = "ported/hyperlinks.rs"]
    mod hyperlinks_;
    #[path = "ported/input.rs"]
    mod input;
    #[path = "ported/input_keys.rs"]
    mod input_keys;
    #[path = "ported/job.rs"]
    mod job_;
    #[path = "ported/key_bindings.rs"]
    mod key_bindings_;
    #[path = "ported/key_string.rs"]
    mod key_string;
    #[path = "ported/layout.rs"]
    mod layout;
    #[path = "ported/layout_custom.rs"]
    mod layout_custom;
    #[path = "ported/layout_set.rs"]
    mod layout_set;
    #[path = "ported/menu.rs"]
    mod menu_;
    #[path = "ported/mode_tree.rs"]
    mod mode_tree;
    #[path = "ported/names.rs"]
    mod names;
    #[path = "ported/notify.rs"]
    mod notify;
    #[path = "ported/options.rs"]
    mod options_;
    #[path = "ported/options_table.rs"]
    mod options_table;
    #[path = "ported/osdep.rs"]
    mod osdep;
    #[path = "ported/paste.rs"]
    mod paste;
    #[path = "ported/popup.rs"]
    mod popup;
    #[path = "ported/proc.rs"]
    mod proc;
    #[path = "ported/regsub.rs"]
    mod regsub;
    #[path = "ported/resize.rs"]
    mod resize;
    #[path = "ported/screen.rs"]
    mod screen_;
    #[path = "ported/screen_redraw.rs"]
    mod screen_redraw;
    #[path = "ported/screen_write.rs"]
    mod screen_write;
    #[path = "ported/server.rs"]
    mod server;
    #[path = "ported/server_acl.rs"]
    mod server_acl;
    #[path = "ported/server_client.rs"]
    mod server_client;
    #[path = "ported/server_fn.rs"]
    mod server_fn;
    #[path = "ported/prompt.rs"]
    mod prompt_;
    #[path = "ported/prompt_history.rs"]
    mod prompt_history;
    #[path = "ported/session.rs"]
    mod session_;
    #[path = "ported/sort.rs"]
    mod sort;
    #[path = "ported/spawn.rs"]
    mod spawn;
    #[path = "ported/status.rs"]
    mod status;
    #[path = "ported/style.rs"]
    mod style_;
    #[path = "ported/tmux.rs"]
    mod tmux;
    #[path = "ported/tmux_protocol_h.rs"]
    mod tmux_protocol;
    #[path = "ported/tty.rs"]
    mod tty_;
    #[path = "ported/tty_acs.rs"]
    mod tty_acs;
    #[path = "ported/tty_draw.rs"]
    mod tty_draw;
    #[path = "ported/tty_features.rs"]
    mod tty_features;
    #[path = "ported/tty_keys.rs"]
    mod tty_keys;
    #[path = "ported/tty_term.rs"]
    mod tty_term_;
    #[path = "ported/utf8.rs"]
    mod utf8;
    #[path = "ported/utf8_combined.rs"]
    mod utf8_combined;
    #[path = "ported/window.rs"]
    mod window_;
    #[path = "ported/window_border.rs"]
    mod window_border;
    #[path = "ported/window_buffer.rs"]
    mod window_buffer;
    #[path = "ported/window_client.rs"]
    mod window_client;
    #[path = "ported/window_clock.rs"]
    mod window_clock;
    #[path = "ported/window_copy.rs"]
    mod window_copy;
    #[path = "ported/window_customize.rs"]
    mod window_customize;
    #[path = "ported/window_switch.rs"]
    mod window_switch;
    #[path = "ported/window_tree.rs"]
    mod window_tree;
    #[path = "ported/window_visible.rs"]
    mod window_visible;
    #[path = "ported/xmalloc.rs"]
    mod xmalloc;
}

// Original ztmux extensions (dashboard, structured output, …) — not a tmux
// port; live under src/extensions/ and are exempt from the anti-drift gate.
mod extensions;

// In-process randomized fuzz harness for the pure parsers/decoders (test-only).
#[cfg(test)]
mod fuzz_smoke;
// The structured-output extension is consumed by ported list-* commands via
// `crate::structured::…`, so re-export it at the crate root.
pub(crate) use extensions::structured;

#[macro_use] // log_debug
#[path = "ported/log.rs"]
mod log;
use std::{
    borrow::Cow,
    cell::RefCell,
    cmp,
    collections::LinkedList,
    ffi::{
        CStr, CString, c_int, c_long, c_longlong, c_short, c_uchar, c_uint, c_ulonglong, c_void,
    },
    mem::{MaybeUninit, size_of, zeroed},
    ptr::{NonNull, addr_of, addr_of_mut, null, null_mut},
    rc::Rc,
    sync::{
        Mutex,
        atomic::{self, AtomicBool, AtomicU32, AtomicU64},
    },
};

use crate::log::*;
pub use crate::tmux::tmux_main;
use crate::{
    alerts::*,
    arguments::*,
    attributes::*,
    bitstr::*,
    cfg_::*,
    client_::*,
    cmd_::{
        cmd_attach_session::cmd_attach_session, cmd_find::*, cmd_log_argv, cmd_queue::*,
        cmd_wait_for::cmd_wait_for_flush, *,
    },
    cmd_parse::*,
    colour::*,
    compat::{imsg::imsg, queue::*, strtonum, tree::*, *}, /* strtonum need to disambiguate from libc on macos */
    control::{control_write, *},
    control_notify::*,
    environ_::*,
    file::*,
    format::*,
    format_draw_::*,
    grid_::*,
    grid_reader_::*,
    grid_view::*,
    hyperlinks_::*,
    input::*,
    input_keys::*,
    job_::*,
    key_bindings_::*,
    key_string::*,
    layout::*,
    layout_custom::*,
    layout_set::*,
    menu_::*,
    mode_tree::*,
    names::*,
    notify::*,
    options_::{options, options_array_item},
    options_table::*,
    osdep::*,
    paste::*,
    popup::*,
    proc::*,
    prompt_::*,
    prompt_history::*,
    regsub::regsub,
    resize::*,
    screen_::*,
    screen_redraw::*,
    screen_write::*,
    server::*,
    server_acl::*,
    server_client::*,
    server_fn::*,
    session_::*,
    sort::*,
    spawn::*,
    status::*,
    style_::*,
    tmux::*,
    tmux_protocol::*,
    tty_::*,
    tty_acs::*,
    tty_draw::*,
    tty_features::*,
    tty_keys::*,
    tty_term_::*,
    utf8::*,
    utf8_combined::*,
    window_::*,
    window_border::*,
    window_buffer::WINDOW_BUFFER_MODE,
    window_client::WINDOW_CLIENT_MODE,
    window_clock::{WINDOW_CLOCK_MODE, WINDOW_CLOCK_TABLE},
    window_copy::{window_copy_add, *},
    window_customize::WINDOW_CUSTOMIZE_MODE,
    window_switch::WINDOW_SWITCH_MODE,
    window_tree::WINDOW_TREE_MODE,
    window_visible::*,
    xmalloc::*,
};

#[cfg(feature = "sixel")]
#[path = "ported/image.rs"]
mod image_;
#[cfg(feature = "sixel")]
#[path = "ported/image_sixel.rs"]
mod image_sixel;
#[cfg(feature = "sixel")]
use image_sixel::sixel_image;

#[cfg(feature = "utempter")]
#[path = "ported/utempter.rs"]
mod utempter;

macro_rules! env_or {
    ($key:literal, $default:expr) => {
        match std::option_env!($key) {
            Some(value) => value,
            None => $default,
        }
    };
}
const TMUX_VERSION: &str = env_or!("TMUX_VERSION", env!("CARGO_PKG_VERSION"));
const TMUX_CONF: &str = env_or!(
    "TMUX_CONF",
    "/etc/tmux.conf:~/.tmux.conf:$XDG_CONFIG_HOME/tmux/tmux.conf:~/.config/tmux/tmux.conf"
);
const TMUX_SOCK: &str = env_or!("TMUX_SOCK", "$TMUX_TMPDIR:/tmp/");
// Matches tmux's build default: Makefile.am passes -DTMUX_TERM="@DEFAULT_TERM@"
// which configure resolves to "tmux-256color" (tmux.h's bare "screen" is only
// the no-configure fallback). Overridable at build time via the TMUX_TERM env.
const TMUX_TERM: &str = env_or!("TMUX_TERM", "tmux-256color");
const TMUX_LOCK_CMD: &str = env_or!("TMUX_LOCK_CMD", "lock -np");

// /usr/include/paths.h
const _PATH_TTY: *const u8 = c!("/dev/tty");
const _PATH_BSHELL: *const u8 = c!("/bin/sh");
const _PATH_BSHELL_STR: &str = "/bin/sh";
const _PATH_DEFPATH: *const u8 = c!("/usr/bin:/bin");
const _PATH_DEV: *const u8 = c!("/dev/");
const _PATH_DEVNULL: *const u8 = c!("/dev/null");
const _PATH_VI: &str = "/usr/bin/vi";
const SIZEOF_PATH_DEV: usize = 6;
const TTY_NAME_MAX: usize = 32;

#[inline]
const fn transmute_ptr<T>(value: Option<NonNull<T>>) -> *mut T {
    match value {
        Some(ptr) => ptr.as_ptr(),
        None => null_mut(),
    }
}

/// Convert an owned `String` into a `CString`, truncating at the first interior
/// NUL so it mirrors C storing a `char *` that ends at the first NUL byte.
/// Shared by the owned-`CString` struct fields that replaced raw `char *`.
pub(crate) fn cstring_truncating(s: String) -> std::ffi::CString {
    match std::ffi::CString::new(s) {
        Ok(c) => c,
        Err(e) => {
            let n = e.nul_position();
            let mut v = e.into_vec();
            v.truncate(n);
            // `v` now has no interior NUL, so this cannot fail.
            std::ffi::CString::new(v).unwrap()
        }
    }
}

#[inline]
const unsafe fn ptr_to_ref<'a, T>(value: *const T) -> Option<&'a T> {
    unsafe { if value.is_null() { None } else { Some(&*value) } }
}

#[inline]
const unsafe fn ptr_to_mut_ref<'a, T>(value: *mut T) -> Option<&'a mut T> {
    unsafe {
        if value.is_null() {
            None
        } else {
            Some(&mut *value)
        }
    }
}

// discriminant structs
struct discr_all_entry;
struct discr_by_uri_entry;
struct discr_by_inner_entry;
struct discr_entry;
struct discr_name_entry;
struct discr_pending_entry;
struct discr_sentry;
struct discr_time_entry;
struct discr_tree_entry;
struct discr_wentry;
struct discr_zentry;

/// Minimum layout cell size, NOT including border lines.
const PANE_MINIMUM: u32 = 1;

/// C `vendor/tmux/tmux.h:105`: `#define PANE_MAXIMUM 10000`.
const PANE_MAXIMUM: u32 = 10000;

/// Automatic name refresh interval, in microseconds. Must be < 1 second.
const NAME_INTERVAL: libc::suseconds_t = 500000;

/// Visual option values
#[repr(i32)]
#[derive(Copy, Clone, Eq, PartialEq, num_enum::TryFromPrimitive)]
enum visual_option {
    VISUAL_OFF,
    VISUAL_ON,
    VISUAL_BOTH,
}

// No key or unknown key.
const KEYC_NONE: c_ulonglong = 0x000ff000000000;
const KEYC_UNKNOWN: c_ulonglong = 0x000fe000000000;

// Base for special (that is, not Unicode) keys. An enum must be at most a
// signed int, so these are based in the highest Unicode PUA.
const KEYC_BASE: c_ulonglong = 0x0000000010e000;
const KEYC_USER: c_ulonglong = 0x0000000010f000;
const KEYC_USER_END: c_ulonglong = KEYC_USER + KEYC_NUSER;

// Key modifier bits
const KEYC_META: c_ulonglong = 0x00100000000000;
const KEYC_CTRL: c_ulonglong = 0x00200000000000;
const KEYC_SHIFT: c_ulonglong = 0x00400000000000;

// Key flag bits.
const KEYC_LITERAL: c_ulonglong = 0x01000000000000;
const KEYC_KEYPAD: c_ulonglong = 0x02000000000000;
const KEYC_CURSOR: c_ulonglong = 0x04000000000000;
const KEYC_IMPLIED_META: c_ulonglong = 0x08000000000000;
const KEYC_BUILD_MODIFIERS: c_ulonglong = 0x10000000000000;
const KEYC_VI: c_ulonglong = 0x20000000000000;
const KEYC_SENT: c_ulonglong = 0x40000000000000;

// Masks for key bits.
const KEYC_MASK_MODIFIERS: c_ulonglong = 0x00f00000000000;
const KEYC_MASK_FLAGS: c_ulonglong = 0xff000000000000;
const KEYC_MASK_KEY: c_ulonglong = 0x000fffffffffff;

const KEYC_NUSER: c_ulonglong = 1000;

#[expect(non_snake_case)]
#[inline]
fn KEYC_IS_MOUSE(key: key_code) -> bool {
    const KEYC_MOUSE: c_ulonglong = keyc::KEYC_MOUSE as c_ulonglong;
    const KEYC_BSPACE: c_ulonglong = keyc::KEYC_BSPACE as c_ulonglong;

    (key & KEYC_MASK_KEY) >= KEYC_MOUSE && (key & KEYC_MASK_KEY) < KEYC_BSPACE
}

#[expect(non_snake_case)]
#[inline]
fn KEYC_IS_UNICODE(key: key_code) -> bool {
    const KEYC_BASE_END: c_ulonglong = keyc::KEYC_BASE_END as c_ulonglong;

    let masked = key & KEYC_MASK_KEY;

    masked > 0x7f
        && !(KEYC_BASE..KEYC_BASE_END).contains(&masked)
        && !(KEYC_USER..KEYC_USER_END).contains(&masked)
}

const KEYC_CLICK_TIMEOUT: i32 = 300;

/// A single key. This can be ASCII or Unicode or one of the keys between
/// `KEYC_BASE` and `KEYC_BASE_END`.
type key_code = core::ffi::c_ulonglong;

// skipped C0 control characters

// C0 control characters
#[repr(u64)]
#[derive(Copy, Clone)]
enum c0 {
    C0_NUL,
    C0_SOH,
    C0_STX,
    C0_ETX,
    C0_EOT,
    C0_ENQ,
    C0_ASC,
    C0_BEL,
    C0_BS,
    C0_HT,
    C0_LF,
    C0_VT,
    C0_FF,
    C0_CR,
    C0_SO,
    C0_SI,
    C0_DLE,
    C0_DC1,
    C0_DC2,
    C0_DC3,
    C0_DC4,
    C0_NAK,
    C0_SYN,
    C0_ETB,
    C0_CAN,
    C0_EM,
    C0_SUB,
    C0_ESC,
    C0_FS,
    C0_GS,
    C0_RS,
    C0_US,
}

// idea write a custom top level macro
// which allows me to annotate a variant
// that should be converted to mouse key
// enum mouse_keys {
// KEYC_MOUSE,
//
// #[keyc_mouse_key]
// MOUSEMOVE,
// }
include!("ported/keyc_mouse_key.rs");

/// Termcap codes.
#[repr(u32)]
#[derive(Copy, Clone, num_enum::TryFromPrimitive)]
enum tty_code_code {
    TTYC_ACSC,
    TTYC_AM,
    TTYC_AX,
    TTYC_BCE,
    TTYC_BEL,
    TTYC_BIDI,
    TTYC_BLINK,
    TTYC_BOLD,
    TTYC_CIVIS,
    TTYC_CLEAR,
    TTYC_CLMG,
    TTYC_CMG,
    TTYC_CNORM,
    TTYC_COLORS,
    TTYC_CR,
    TTYC_CS,
    TTYC_CSR,
    TTYC_CUB,
    TTYC_CUB1,
    TTYC_CUD,
    TTYC_CUD1,
    TTYC_CUF,
    TTYC_CUF1,
    TTYC_CUP,
    TTYC_CUU,
    TTYC_CUU1,
    TTYC_CVVIS,
    TTYC_DCH,
    TTYC_DCH1,
    TTYC_DIM,
    TTYC_DL,
    TTYC_DL1,
    TTYC_DSBP,
    TTYC_DSEKS,
    TTYC_DSFCS,
    TTYC_DSMG,
    TTYC_E3,
    TTYC_ECH,
    TTYC_ED,
    TTYC_EL,
    TTYC_EL1,
    TTYC_ENACS,
    TTYC_ENBP,
    TTYC_ENEKS,
    TTYC_ENFCS,
    TTYC_ENMG,
    TTYC_FSL,
    TTYC_HLS,
    TTYC_HOME,
    TTYC_HPA,
    TTYC_ICH,
    TTYC_ICH1,
    TTYC_IL,
    TTYC_IL1,
    TTYC_INDN,
    TTYC_INVIS,
    TTYC_KCBT,
    TTYC_KCUB1,
    TTYC_KCUD1,
    TTYC_KCUF1,
    TTYC_KCUU1,
    TTYC_KDC2,
    TTYC_KDC3,
    TTYC_KDC4,
    TTYC_KDC5,
    TTYC_KDC6,
    TTYC_KDC7,
    TTYC_KDCH1,
    TTYC_KDN2,
    TTYC_KDN3,
    TTYC_KDN4,
    TTYC_KDN5,
    TTYC_KDN6,
    TTYC_KDN7,
    TTYC_KEND,
    TTYC_KEND2,
    TTYC_KEND3,
    TTYC_KEND4,
    TTYC_KEND5,
    TTYC_KEND6,
    TTYC_KEND7,
    TTYC_KF1,
    TTYC_KF10,
    TTYC_KF11,
    TTYC_KF12,
    TTYC_KF13,
    TTYC_KF14,
    TTYC_KF15,
    TTYC_KF16,
    TTYC_KF17,
    TTYC_KF18,
    TTYC_KF19,
    TTYC_KF2,
    TTYC_KF20,
    TTYC_KF21,
    TTYC_KF22,
    TTYC_KF23,
    TTYC_KF24,
    TTYC_KF25,
    TTYC_KF26,
    TTYC_KF27,
    TTYC_KF28,
    TTYC_KF29,
    TTYC_KF3,
    TTYC_KF30,
    TTYC_KF31,
    TTYC_KF32,
    TTYC_KF33,
    TTYC_KF34,
    TTYC_KF35,
    TTYC_KF36,
    TTYC_KF37,
    TTYC_KF38,
    TTYC_KF39,
    TTYC_KF4,
    TTYC_KF40,
    TTYC_KF41,
    TTYC_KF42,
    TTYC_KF43,
    TTYC_KF44,
    TTYC_KF45,
    TTYC_KF46,
    TTYC_KF47,
    TTYC_KF48,
    TTYC_KF49,
    TTYC_KF5,
    TTYC_KF50,
    TTYC_KF51,
    TTYC_KF52,
    TTYC_KF53,
    TTYC_KF54,
    TTYC_KF55,
    TTYC_KF56,
    TTYC_KF57,
    TTYC_KF58,
    TTYC_KF59,
    TTYC_KF6,
    TTYC_KF60,
    TTYC_KF61,
    TTYC_KF62,
    TTYC_KF63,
    TTYC_KF7,
    TTYC_KF8,
    TTYC_KF9,
    TTYC_KHOM2,
    TTYC_KHOM3,
    TTYC_KHOM4,
    TTYC_KHOM5,
    TTYC_KHOM6,
    TTYC_KHOM7,
    TTYC_KHOME,
    TTYC_KIC2,
    TTYC_KIC3,
    TTYC_KIC4,
    TTYC_KIC5,
    TTYC_KIC6,
    TTYC_KIC7,
    TTYC_KICH1,
    TTYC_KIND,
    TTYC_KLFT2,
    TTYC_KLFT3,
    TTYC_KLFT4,
    TTYC_KLFT5,
    TTYC_KLFT6,
    TTYC_KLFT7,
    TTYC_KMOUS,
    TTYC_KNP,
    TTYC_KNXT2,
    TTYC_KNXT3,
    TTYC_KNXT4,
    TTYC_KNXT5,
    TTYC_KNXT6,
    TTYC_KNXT7,
    TTYC_KPP,
    TTYC_KPRV2,
    TTYC_KPRV3,
    TTYC_KPRV4,
    TTYC_KPRV5,
    TTYC_KPRV6,
    TTYC_KPRV7,
    TTYC_KRI,
    TTYC_KRIT2,
    TTYC_KRIT3,
    TTYC_KRIT4,
    TTYC_KRIT5,
    TTYC_KRIT6,
    TTYC_KRIT7,
    TTYC_KUP2,
    TTYC_KUP3,
    TTYC_KUP4,
    TTYC_KUP5,
    TTYC_KUP6,
    TTYC_KUP7,
    TTYC_MS,
    TTYC_NOBR,
    TTYC_OL,
    TTYC_OP,
    TTYC_RECT,
    TTYC_REV,
    TTYC_RGB,
    TTYC_RI,
    TTYC_RIN,
    TTYC_RMACS,
    TTYC_RMCUP,
    TTYC_RMKX,
    TTYC_SE,
    TTYC_SETAB,
    TTYC_SETAF,
    TTYC_SETAL,
    TTYC_SETRGBB,
    TTYC_SETRGBF,
    TTYC_SETULC,
    TTYC_SETULC1,
    TTYC_SGR0,
    TTYC_SITM,
    TTYC_SMACS,
    TTYC_SMCUP,
    TTYC_SMKX,
    TTYC_SMOL,
    TTYC_SMSO,
    TTYC_SMUL,
    TTYC_SMULX,
    TTYC_SMXX,
    TTYC_SPB,
    TTYC_SXL,
    TTYC_SS,
    TTYC_SWD,
    TTYC_SYNC,
    TTYC_TC,
    TTYC_TSL,
    TTYC_U8,
    TTYC_VPA,
    TTYC_XT,
}

// C tmux.h:659 `#define WHITESPACE "\t "` — TAB is whitespace too; the port had
// dropped the tab, breaking vi word motion around tabs in copy mode.
const WHITESPACE: *const u8 = c!("\t ");

#[repr(i32)]
#[derive(Copy, Clone, Eq, PartialEq, num_enum::TryFromPrimitive)]
enum modekey {
    MODEKEY_EMACS = 0,
    MODEKEY_VI = 1,
}

bitflags::bitflags! {
    /// Grid flags.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq)]
    struct mode_flag : i32 {
        const MODE_CURSOR = 0x1;
        const MODE_INSERT = 0x2;
        const MODE_KCURSOR = 0x4;
        const MODE_KKEYPAD = 0x8;
        const MODE_WRAP = 0x10;
        const MODE_MOUSE_STANDARD = 0x20;
        const MODE_MOUSE_BUTTON = 0x40;
        const MODE_CURSOR_BLINKING = 0x80;
        const MODE_MOUSE_UTF8 = 0x100;
        const MODE_MOUSE_SGR = 0x200;
        const MODE_BRACKETPASTE = 0x400;
        const MODE_FOCUSON = 0x800;
        const MODE_MOUSE_ALL = 0x1000;
        const MODE_ORIGIN = 0x2000;
        const MODE_CRLF = 0x4000;
        const MODE_KEYS_EXTENDED = 0x8000;
        const MODE_CURSOR_VERY_VISIBLE = 0x10000;
        const MODE_CURSOR_BLINKING_SET = 0x20000;
        const MODE_KEYS_EXTENDED_2 = 0x40000;
        const MODE_THEME_UPDATES = 0x80000;
        const MODE_SYNC = 0x100000;
    }
}

#[expect(dead_code)]
const ALL_MODES: i32 = 0xffffff;
const ALL_MOUSE_MODES: mode_flag = mode_flag::MODE_MOUSE_STANDARD
    .union(mode_flag::MODE_MOUSE_BUTTON)
    .union(mode_flag::MODE_MOUSE_ALL);
const MOTION_MOUSE_MODES: mode_flag = mode_flag::MODE_MOUSE_BUTTON.union(mode_flag::MODE_MOUSE_ALL);
const CURSOR_MODES: mode_flag = mode_flag::MODE_CURSOR
    .union(mode_flag::MODE_CURSOR_BLINKING)
    .union(mode_flag::MODE_CURSOR_VERY_VISIBLE);
const EXTENDED_KEY_MODES: mode_flag =
    mode_flag::MODE_KEYS_EXTENDED.union(mode_flag::MODE_KEYS_EXTENDED_2);

// Mouse protocol constants.
const MOUSE_PARAM_MAX: u32 = 0xff;
const MOUSE_PARAM_UTF8_MAX: u32 = 0x7ff;
const MOUSE_PARAM_BTN_OFF: u32 = 0x20;
const MOUSE_PARAM_POS_OFF: u32 = 0x21;

// cmd_list_print flags (vendor/tmux/tmux.h:2996-2997).
const CMD_LIST_PRINT_ESCAPED: c_int = 0x1;
const CMD_LIST_PRINT_NO_GROUPS: c_int = 0x2;

/// C `vendor/tmux/tmux.h:1243`: `enum client_theme`
#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
#[repr(C)]
enum client_theme {
    #[default]
    THEME_UNKNOWN,
    THEME_LIGHT,
    THEME_DARK,
}

// Colour flags.
const COLOUR_FLAG_256: i32 = 0x01000000;
const COLOUR_FLAG_RGB: i32 = 0x02000000;
const COLOUR_FLAG_THEME: i32 = 0x04000000;
/// Theme colours. C `vendor/tmux/tmux.h:739`: `enum colour_theme`.
///
/// The index half of a `COLOUR_FLAG_THEME` colour: the slot a theme name
/// resolves to, matching the order of `COLOUR_THEME_TABLE` in `colour.rs`.
#[expect(dead_code)]
mod colour_theme_slot {
    pub const COLOUR_THEME_BLACK: i32 = 0;
    pub const COLOUR_THEME_WHITE: i32 = 1;
    pub const COLOUR_THEME_LIGHT_GREY: i32 = 2;
    pub const COLOUR_THEME_DARK_GREY: i32 = 3;
    pub const COLOUR_THEME_GREEN: i32 = 4;
    pub const COLOUR_THEME_YELLOW: i32 = 5;
    pub const COLOUR_THEME_RED: i32 = 6;
    pub const COLOUR_THEME_BLUE: i32 = 7;
    pub const COLOUR_THEME_CYAN: i32 = 8;
    pub const COLOUR_THEME_MAGENTA: i32 = 9;
}

// vendor/tmux/tmux.h:752 `#define COLOUR_THEME_COUNT 10` — number of theme
// colour slots, sized for the client's `theme_colours` array.
const COLOUR_THEME_COUNT: usize = 10;

/// Special colours.
#[expect(non_snake_case)]
#[inline]
fn COLOUR_DEFAULT(c: i32) -> bool {
    c == 8 || c == 9
}

// Grid attributes. Anything above 0xff is stored in an extended cell.
bitflags::bitflags! {
    /// Grid flags.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    struct grid_attr : u16 {
        const GRID_ATTR_BRIGHT = 0x1;
        const GRID_ATTR_DIM = 0x2;
        const GRID_ATTR_UNDERSCORE = 0x4;
        const GRID_ATTR_BLINK = 0x8;
        const GRID_ATTR_REVERSE = 0x10;
        const GRID_ATTR_HIDDEN = 0x20;
        const GRID_ATTR_ITALICS = 0x40;
        const GRID_ATTR_CHARSET = 0x80; // alternative character set
        const GRID_ATTR_STRIKETHROUGH = 0x100;
        const GRID_ATTR_UNDERSCORE_2 = 0x200;
        const GRID_ATTR_UNDERSCORE_3 = 0x400;
        const GRID_ATTR_UNDERSCORE_4 = 0x800;
        const GRID_ATTR_UNDERSCORE_5 = 0x1000;
        const GRID_ATTR_OVERLINE = 0x2000;
        /// C `vendor/tmux/tmux.h:778`: set by the style keyword `noattr`, which
        /// asks a selection not to inherit the attributes of the text under it.
        const GRID_ATTR_NOATTR = 0x4000;
    }
}

/// All underscore attributes.
const GRID_ATTR_ALL_UNDERSCORE: grid_attr = grid_attr::GRID_ATTR_UNDERSCORE
    .union(grid_attr::GRID_ATTR_UNDERSCORE_2)
    .union(grid_attr::GRID_ATTR_UNDERSCORE_3)
    .union(grid_attr::GRID_ATTR_UNDERSCORE_4)
    .union(grid_attr::GRID_ATTR_UNDERSCORE_5);

bitflags::bitflags! {
    /// Grid flags.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq)]
    struct grid_flag : u8 {
        const FG256 = 0x1;
        const BG256 = 0x2;
        const PADDING = 0x4;
        const EXTENDED = 0x8;
        const SELECTED = 0x10;
        const NOPALETTE = 0x20;
        const CLEARED = 0x40;
        const TAB = 0x80;
    }
}

bitflags::bitflags! {
    /// Grid line flags.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq)]
    struct grid_line_flag: i32 {
        const WRAPPED      = 1 << 0; // 0x1
        const EXTENDED     = 1 << 1; // 0x2
        const DEAD         = 1 << 2; // 0x4
        const START_PROMPT = 1 << 3; // 0x8
        const START_OUTPUT = 1 << 4; // 0x10
        const HYPERLINK    = 1 << 5; // 0x20
    }
}

bitflags::bitflags! {
    /// Grid string flags.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq)]
    struct grid_string_flags: i32 {
        const GRID_STRING_WITH_SEQUENCES = 0x1;
        const GRID_STRING_ESCAPE_SEQUENCES = 0x2;
        const GRID_STRING_TRIM_SPACES = 0x4;
        const GRID_STRING_USED_ONLY = 0x8;
        const GRID_STRING_EMPTY_CELLS = 0x10;
    }
}

/// Cell positions.
#[repr(i32)]
#[derive(Copy, Clone, Eq, PartialEq, Default)]
enum cell_type {
    #[default]
    CELL_INSIDE = 0,
    CELL_TOPBOTTOM = 1,
    CELL_LEFTRIGHT = 2,
    CELL_TOPLEFT = 3,
    CELL_TOPRIGHT = 4,
    CELL_BOTTOMLEFT = 5,
    CELL_BOTTOMRIGHT = 6,
    CELL_TOPJOIN = 7,
    CELL_BOTTOMJOIN = 8,
    CELL_LEFTJOIN = 9,
    CELL_RIGHTJOIN = 10,
    CELL_JOIN = 11,
    CELL_OUTSIDE = 12,
}

/// Cell borders.
const CELL_BORDERS: [u8; 13] = [
    b' ', b'x', b'q', b'l', b'k', b'm', b'j', b'w', b'v', b't', b'u', b'n', b'~',
];
const SIMPLE_BORDERS: [u8; 13] = [
    b' ', b'|', b'-', b'+', b'+', b'+', b'+', b'+', b'+', b'+', b'+', b'+', b'.',
];
const PADDED_BORDERS: [u8; 13] = [b' '; 13];

/// Grid cell data.
#[repr(C)]
#[derive(Copy, Clone)]
struct grid_cell {
    data: utf8_data,
    attr: grid_attr,
    flags: grid_flag,
    fg: i32,
    bg: i32,
    us: i32,
    link: u32,
}

impl grid_cell {
    const fn new(
        data: utf8_data,
        attr: grid_attr,
        flags: grid_flag,
        fg: i32,
        bg: i32,
        us: i32,
        link: u32,
    ) -> Self {
        Self {
            data,
            attr,
            flags,
            fg,
            bg,
            us,
            link,
        }
    }
}

/// Grid extended cell entry.
///
/// `__packed` in the C (tmux.h:857), so 23 bytes rather than the 24 natural
/// alignment would give. The grid stores these by the million, and
/// `#{history_all_bytes}` reports `extdsize * sizeof *gl->extddata`, so the
/// packing is both the real memory cost and an observable number.
#[repr(C, packed)]
struct grid_extd_entry {
    data: utf8_char,
    attr: u16,
    flags: u8,
    fg: i32,
    bg: i32,
    us: i32,
    link: u32,
}

// Four `u_char`s in the C, with no alignment attribute of its own — it takes
// the union's 4-byte alignment from the `u_int` arm beside it, which is why
// forcing align(4) here is both unnecessary and enough to block packing the
// enclosing entry.
#[derive(Copy, Clone)]
#[repr(C)]
struct grid_cell_entry_data {
    attr: u8,
    fg: u8,
    bg: u8,
    data: u8,
}

#[repr(C)]
union grid_cell_entry_union {
    offset: u32,
    data: grid_cell_entry_data,
}

/// Grid cell entry.
///
/// `__packed` in the C (tmux.h:871): a 4-byte union plus a 1-byte flags word
/// is 5 bytes, not the 8 that aligning the union to 4 would give.
#[repr(C, packed)]
struct grid_cell_entry {
    union_: grid_cell_entry_union,
    flags: grid_flag,
}

/// Grid line.
#[repr(C)]
struct grid_line {
    celldata: *mut grid_cell_entry,
    cellused: u32,
    cellsize: u32,

    extddata: *mut grid_extd_entry,
    extdsize: u32,

    flags: grid_line_flag,
    time: time_t,
}

const GRID_HISTORY: i32 = 0x1; // scroll lines into history

/// Entire grid of cells.
#[repr(C)]
struct grid {
    flags: i32,

    sx: u32,
    sy: u32,

    hscrolled: u32,
    hsize: u32,
    hlimit: u32,

    // Monotonic scroll counters. Copy mode snapshots them so an incremental
    // refresh can tell how much history scrolled in (`scroll_added`) or was
    // collected off the top (`scroll_collected`) since the snapshot, and
    // `scroll_generation` invalidates the whole snapshot when the grid is
    // cleared or reflowed.
    scroll_added: u32,
    scroll_collected: u32,
    scroll_generation: u32,

    linedata: *mut grid_line,
}

/// Virtual cursor in a grid.
#[repr(C)]
struct grid_reader {
    gd: *mut grid,
    cx: u32,
    cy: u32,
}

/// Style alignment.
#[repr(i32)]
#[derive(Copy, Clone, Eq, PartialEq)]
enum style_align {
    STYLE_ALIGN_DEFAULT,
    STYLE_ALIGN_LEFT,
    STYLE_ALIGN_CENTRE,
    STYLE_ALIGN_RIGHT,
    STYLE_ALIGN_ABSOLUTE_CENTRE,
}

/// Style list.
#[repr(i32)]
#[derive(Copy, Clone, Eq, PartialEq)]
enum style_list {
    STYLE_LIST_OFF,
    STYLE_LIST_ON,
    STYLE_LIST_FOCUS,
    STYLE_LIST_LEFT_MARKER,
    STYLE_LIST_RIGHT_MARKER,
}

/// Style range.
#[repr(i32)]
#[derive(Copy, Clone, Eq, PartialEq)]
enum style_range_type {
    STYLE_RANGE_NONE,
    STYLE_RANGE_LEFT,
    STYLE_RANGE_RIGHT,
    STYLE_RANGE_PANE,
    STYLE_RANGE_WINDOW,
    STYLE_RANGE_SESSION,
    STYLE_RANGE_USER,
    /// C `vendor/tmux/tmux.h:939`: `#[range=control|N]`, N in 0..=9 -- a click
    /// target inside a drawn format, used by the default pane-border-format.
    STYLE_RANGE_CONTROL,
}

impl_tailq_entry!(style_range, entry, tailq_entry<style_range>);
// #[derive(crate::compat::TailQEntry)]
#[repr(C)]
struct style_range {
    type_: style_range_type,
    argument: u32,
    string: [u8; 16],
    start: u32,
    /// not included
    end: u32,

    // #[entry]
    entry: tailq_entry<style_range>,
}
type style_ranges = tailq_head<style_range>;

/// Style default.
#[repr(i32)]
#[derive(Copy, Clone, Eq, PartialEq)]
enum style_default_type {
    STYLE_DEFAULT_BASE,
    STYLE_DEFAULT_PUSH,
    STYLE_DEFAULT_POP,
    STYLE_DEFAULT_SET,
}

/// C `vendor/tmux/tmux.h:1479`: `pane-scrollbars` values.
const PANE_SCROLLBARS_OFF: i32 = 0;
const PANE_SCROLLBARS_MODAL: i32 = 1;
const PANE_SCROLLBARS_ALWAYS: i32 = 2;
const PANE_SCROLLBARS_AUTOHIDE: i32 = 3;

/// C `vendor/tmux/tmux.h:1485`: `pane-scrollbars-position` values.
const PANE_SCROLLBARS_RIGHT: i32 = 0;
const PANE_SCROLLBARS_LEFT: i32 = 1;

/// C `vendor/tmux/tmux.h:1489`: fallbacks when the style gives no width or pad.
const PANE_SCROLLBARS_DEFAULT_PADDING: i32 = 0;
const PANE_SCROLLBARS_DEFAULT_WIDTH: i32 = 1;
/// The cell the scrollbar is drawn with — its colours carry the whole look.
const PANE_SCROLLBARS_CHARACTER: u8 = b' ';

/// Style option.
#[repr(C)]
#[derive(Copy, Clone)]
struct style {
    gc: grid_cell,
    ignore: i32,

    fill: i32,
    align: style_align,
    list: style_list,

    range_type: style_range_type,
    range_argument: u32,
    range_string: [u8; 16],

    /// C `vendor/tmux/tmux.h:985`: `width=` and `pad=` on a style, both
    /// `-1` when unset. `width_percentage` marks a `width=N%` form, which is
    /// resolved against the area the style is applied to rather than being a
    /// cell count.
    width: i32,
    width_percentage: i32,
    pad: i32,

    default_type: style_default_type,

    /// C `vendor/tmux/tmux.h:1005`: the id of this style's `link=` URI in the
    /// global hyperlink set, or 0 for no link. The URI itself lives in
    /// `style.rs`'s `STYLE_HYPERLINKS` so that repeated `link=` directives share
    /// one entry and the id stays stable across redraws.
    link: u32,
}

/// C `vendor/tmux/tmux.h:960`: no `width=` given.
const STYLE_WIDTH_DEFAULT: i32 = -1;
/// C `vendor/tmux/tmux.h:961`: no `pad=` given.
const STYLE_PAD_DEFAULT: i32 = -1;

#[cfg(feature = "sixel")]
impl crate::compat::queue::Entry<image, discr_all_entry> for image {
    unsafe fn entry(this: *mut Self) -> *mut tailq_entry<image> {
        unsafe { &raw mut (*this).all_entry }
    }
}
#[cfg(feature = "sixel")]
impl crate::compat::queue::Entry<image, discr_entry> for image {
    unsafe fn entry(this: *mut Self) -> *mut tailq_entry<image> {
        unsafe { &raw mut (*this).entry }
    }
}
#[cfg(feature = "sixel")]
#[repr(C)]
#[derive(Clone)]
struct image {
    s: *mut screen,
    data: *mut sixel_image,
    /// Owned text placeholder (`image_fallback`); `None` until `image_store`
    /// sets it. Drops with the boxed `image` — no manual `free()`.
    fallback: Option<std::ffi::CString>,
    px: u32,
    py: u32,
    sx: u32,
    sy: u32,

    all_entry: tailq_entry<image>,
    entry: tailq_entry<image>,
}

#[cfg(feature = "sixel")]
type images = tailq_head<image>;

/// Cursor style.
#[repr(i32)]
#[derive(Copy, Clone, Eq, PartialEq)]
enum screen_cursor_style {
    SCREEN_CURSOR_DEFAULT,
    SCREEN_CURSOR_BLOCK,
    SCREEN_CURSOR_UNDERLINE,
    SCREEN_CURSOR_BAR,
}

/// C `vendor/tmux/tmux.h:1024`: `enum progress_bar_state` — the OSC 9;4 states.
#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum progress_bar_state {
    #[default]
    PROGRESS_BAR_HIDDEN = 0,
    PROGRESS_BAR_NORMAL = 1,
    PROGRESS_BAR_ERROR = 2,
    PROGRESS_BAR_INDETERMINATE = 3,
    PROGRESS_BAR_PAUSED = 4,
}

impl progress_bar_state {
    /// The state for an OSC 9;4 digit, `None` for anything outside `0`-`4`
    /// (the C compares `*pb < '0' || *pb > '4'` before casting).
    fn from_digit(d: u8) -> Option<Self> {
        Some(match d {
            b'0' => Self::PROGRESS_BAR_HIDDEN,
            b'1' => Self::PROGRESS_BAR_NORMAL,
            b'2' => Self::PROGRESS_BAR_ERROR,
            b'3' => Self::PROGRESS_BAR_INDETERMINATE,
            b'4' => Self::PROGRESS_BAR_PAUSED,
            _ => return None,
        })
    }
}

/// C `vendor/tmux/tmux.h:1031`: `struct progress_bar` — OSC 9;4 progress bar.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct progress_bar {
    state: progress_bar_state,
    progress: i32,
}

/// Virtual screen.
#[repr(C)]
#[derive(Clone)]
struct screen {
    /// Owned screen title (`""` after init) / OSC 7 path (`None` until set);
    /// dropped in `screen_free`. Read via `title_ptr()`/`path_ptr()`.
    title: Option<std::ffi::CString>,
    path: Option<std::ffi::CString>,
    titles: *mut screen_titles,

    /// grid data
    grid: *mut grid,

    /// cursor x
    cx: u32,
    /// cursor y
    cy: u32,

    /// cursor style
    cstyle: screen_cursor_style,
    default_cstyle: screen_cursor_style,
    /// cursor colour
    ccolour: i32,
    /// default cursor colour
    default_ccolour: i32,

    /// scroll region top
    rupper: u32,
    /// scroll region bottom
    rlower: u32,

    mode: mode_flag,
    default_mode: mode_flag,

    saved_cx: u32,
    saved_cy: u32,
    saved_grid: *mut grid,
    saved_cell: grid_cell,
    saved_flags: i32,

    tabs: Option<Rc<RefCell<BitStr>>>,
    sel: *mut screen_sel,

    #[cfg(feature = "sixel")]
    images: images,

    write_list: *mut screen_write_cline,

    hyperlinks: *mut hyperlinks,

    progress_bar: progress_bar,
}

const SCREEN_WRITE_SYNC: i32 = 0x1;
/// C `vendor/tmux/tmux.h:1090`: a floating pane covers part of this one.
const SCREEN_WRITE_OBSCURED: i32 = 0x2;
/// C `vendor/tmux/tmux.h:1091`: the obscured test has already run for this ctx.
const SCREEN_WRITE_CHECKED_IF_OBSCURED: i32 = 0x4;

// Screen write context.
type screen_write_init_ctx_cb = Option<unsafe fn(*mut screen_write_ctx, *mut tty_ctx)>;
#[repr(C)]
struct screen_write_ctx {
    wp: *mut window_pane,
    s: *mut screen,

    flags: i32,

    init_ctx_cb: screen_write_init_ctx_cb,

    arg: *mut c_void,

    item: *mut screen_write_citem,
    scrolled: u32,
    bg: u32,
}

/// Box border lines option.
#[repr(i32)]
#[derive(Copy, Clone, Default, Eq, PartialEq, num_enum::TryFromPrimitive)]
enum box_lines {
    #[default]
    BOX_LINES_DEFAULT = -1,
    BOX_LINES_SINGLE,
    BOX_LINES_DOUBLE,
    BOX_LINES_HEAVY,
    BOX_LINES_SIMPLE,
    BOX_LINES_ROUNDED,
    BOX_LINES_PADDED,
    BOX_LINES_NONE,
}

/// Pane border lines option.
#[repr(i32)]
#[derive(Copy, Clone, Default, Eq, PartialEq, num_enum::TryFromPrimitive)]
enum pane_lines {
    #[default]
    PANE_LINES_SINGLE,
    PANE_LINES_DOUBLE,
    PANE_LINES_HEAVY,
    PANE_LINES_SIMPLE,
    PANE_LINES_NUMBER,
    PANE_LINES_SPACES,
    PANE_LINES_NONE,
}

#[repr(i32)]
#[derive(Copy, Clone, num_enum::TryFromPrimitive)]
enum pane_border_indicator {
    PANE_BORDER_OFF,
    PANE_BORDER_COLOUR,
    PANE_BORDER_ARROWS,
    PANE_BORDER_BOTH,
}

// Mode returned by window_pane_mode function.
const WINDOW_PANE_NO_MODE: i32 = 0;
const WINDOW_PANE_COPY_MODE: i32 = 1;
const WINDOW_PANE_VIEW_MODE: i32 = 2;

// Screen redraw context.
#[repr(C)]
struct screen_redraw_ctx {
    c: *mut client,

    statuslines: u32,
    statustop: i32,

    pane_status: pane_status,
    pane_lines: pane_lines,

    no_pane_gc: grid_cell,
    no_pane_gc_set: i32,

    sx: u32,
    sy: u32,
    ox: u32,
    oy: u32,
}

/// Type of span in the scene.
/// C `vendor/tmux/screen-redraw.c:64`: `enum redraw_span_type`
#[repr(i32)]
#[derive(Copy, Clone, Eq, PartialEq, Default)]
enum redraw_span_type {
    /// inside a pane
    #[default]
    REDRAW_SPAN_PANE,
    /// outside the window
    REDRAW_SPAN_OUTSIDE,
    /// inside the window but nothing visible
    REDRAW_SPAN_EMPTY,
    /// pane status line
    REDRAW_SPAN_STATUS,
    /// pane border
    REDRAW_SPAN_BORDER,
    /// pane scrollbar
    REDRAW_SPAN_SCROLLBAR,
}
/// C `vendor/tmux/screen-redraw.c:72`: `#define REDRAW_SPAN_TYPES 6`
const REDRAW_SPAN_TYPES: usize = 6;

// Border connections to adjacent cells.
/// C `vendor/tmux/screen-redraw.c:75`: `#define REDRAW_BORDER_L 0x1`
const REDRAW_BORDER_L: i32 = 0x1;
/// C `vendor/tmux/screen-redraw.c:76`: `#define REDRAW_BORDER_R 0x2`
const REDRAW_BORDER_R: i32 = 0x2;
/// C `vendor/tmux/screen-redraw.c:77`: `#define REDRAW_BORDER_U 0x4`
const REDRAW_BORDER_U: i32 = 0x4;
/// C `vendor/tmux/screen-redraw.c:78`: `#define REDRAW_BORDER_D 0x8`
const REDRAW_BORDER_D: i32 = 0x8;

// Span flags.
/// C `vendor/tmux/screen-redraw.c:81`: `#define REDRAW_BORDER_IS_ARROW 0x1`
const REDRAW_BORDER_IS_ARROW: i32 = 0x1;
/// C `vendor/tmux/screen-redraw.c:82`: `#define REDRAW_SCROLLBAR_LEFT 0x2`
const REDRAW_SCROLLBAR_LEFT: i32 = 0x2;
/// C `vendor/tmux/screen-redraw.c:83`: `#define REDRAW_SCROLLBAR_RIGHT 0x4`
const REDRAW_SCROLLBAR_RIGHT: i32 = 0x4;
/// C `vendor/tmux/screen-redraw.c:84`: `#define REDRAW_SCROLLBAR_OVERLAY 0x8`
const REDRAW_SCROLLBAR_OVERLAY: i32 = 0x8;

// Draw operations.
/// C `vendor/tmux/screen-redraw.c:88`: `#define REDRAW_PANE 0x1`
const REDRAW_PANE: i32 = 0x1;
/// C `vendor/tmux/screen-redraw.c:89`: `#define REDRAW_OUTSIDE 0x2`
const REDRAW_OUTSIDE: i32 = 0x2;
/// C `vendor/tmux/screen-redraw.c:90`: `#define REDRAW_EMPTY 0x4`
const REDRAW_EMPTY: i32 = 0x4;
/// C `vendor/tmux/screen-redraw.c:91`: `#define REDRAW_PANE_BORDER 0x8`
const REDRAW_PANE_BORDER: i32 = 0x8;
/// C `vendor/tmux/screen-redraw.c:92`: `#define REDRAW_PANE_STATUS 0x10`
const REDRAW_PANE_STATUS: i32 = 0x10;
/// C `vendor/tmux/screen-redraw.c:93`: `#define REDRAW_PANE_SCROLLBAR 0x20`
const REDRAW_PANE_SCROLLBAR: i32 = 0x20;
/// C `vendor/tmux/screen-redraw.c:94`: `#define REDRAW_STATUS 0x40`
const REDRAW_STATUS: i32 = 0x40;
/// C `vendor/tmux/screen-redraw.c:95`: `#define REDRAW_OVERLAY 0x80`
const REDRAW_OVERLAY: i32 = 0x80;
/// C `vendor/tmux/screen-redraw.c:98`: `#define REDRAW_ALL 0x7fffffff`
const REDRAW_ALL: i32 = 0x7fff_ffff;
// C `vendor/tmux/screen-redraw.c:99`'s `REDRAW_IS_ALL(flags)` macro is written
// out as `flags == REDRAW_ALL` at its call sites.

/// Data for a span.
/// C `vendor/tmux/screen-redraw.c:106`: `struct redraw_span_data`
///
/// The C original is a tagged union; the arms are flattened here so a build
/// cell can be compared and copied without reading an inactive union member.
/// Only the fields belonging to `type` are ever meaningful.
#[repr(C)]
#[derive(Copy, Clone, Default)]
struct redraw_span_data {
    type_: redraw_span_type,

    /// `p.wp`: pane this span belongs to.
    p_wp: *mut window_pane,
    /// `p.px`, `p.py`: position of the span inside the pane.
    p_px: u32,
    p_py: u32,

    /// `b.top_wp` .. `b.right_wp`: adjacent panes on each side.
    b_top_wp: *mut window_pane,
    b_bottom_wp: *mut window_pane,
    b_left_wp: *mut window_pane,
    b_right_wp: *mut window_pane,
    /// `b.style_wp`: pane owning the style, when known at build time. Used for
    /// the half-coloured active pane indicator.
    b_style_wp: *mut window_pane,
    /// `b.cell_type`, `b.cell_mask`: border shape and its connection mask.
    b_cell_type: cell_type,
    b_cell_mask: i32,
    /// `b.top_lines` .. `b.right_lines`: line style contributed by each side.
    b_top_lines: pane_lines,
    b_bottom_lines: pane_lines,
    b_left_lines: pane_lines,
    b_right_lines: pane_lines,
    /// `b.flags`: `REDRAW_BORDER_IS_ARROW`.
    b_flags: i32,

    /// `st.wp`, `st.offset`, `st.cell_type`: pane status line and the offset
    /// into it, plus the border shape underneath.
    st_wp: *mut window_pane,
    st_offset: u32,
    st_cell_type: cell_type,

    /// `sb.wp`: pane this scrollbar belongs to.
    sb_wp: *mut window_pane,
    /// `sb.y`: line within the scrollbar.
    sb_y: u32,
    /// `sb.height`: full height of the scrollbar.
    sb_height: u32,
    /// `sb.flags`: `REDRAW_SCROLLBAR_LEFT`, `REDRAW_SCROLLBAR_RIGHT` and
    /// `REDRAW_SCROLLBAR_OVERLAY`.
    sb_flags: i32,
}

/// A span of cells of the same type inside a line.
/// C `vendor/tmux/screen-redraw.c:167`: `struct redraw_span`
#[derive(Copy, Clone, Default)]
struct redraw_span {
    x: u32,
    width: u32,
    data: redraw_span_data,
}

/// A visible line on the client.
/// C `vendor/tmux/screen-redraw.c:177`: `struct redraw_line`
///
/// The C original keeps one TAILQ per span type; a `Vec` per type preserves
/// insertion order with the same effect and no per-span allocation.
#[derive(Default)]
struct redraw_line {
    spans: [Vec<redraw_span>; REDRAW_SPAN_TYPES],
}

/// A scene representing all the spans on the client.
/// C `vendor/tmux/screen-redraw.c:182`: `struct redraw_scene`
struct redraw_scene {
    c: *mut client,
    w: *mut window,
    lines: Vec<redraw_line>,

    generation: u64,
    sx: u32,
    sy: u32,
    ox: u32,
    oy: u32,
}

/// Cell for building the scene.
/// C `vendor/tmux/screen-redraw.c:194`: `struct redraw_build_cell`
#[derive(Copy, Clone, Default)]
struct redraw_build_cell {
    data: redraw_span_data,
}

/// Context for building the scene.
/// C `vendor/tmux/screen-redraw.c:201`: `struct redraw_build_ctx`
struct redraw_build_ctx {
    #[expect(dead_code)]
    c: *mut client,
    w: *mut window,

    ox: u32,
    oy: u32,
    sx: u32,
    sy: u32,

    ind: i32,

    cells: Vec<redraw_build_cell>,
}

// Draw context flags.
/// C `vendor/tmux/screen-redraw.c:230`: `#define REDRAW_ISOLATES 0x1`
const REDRAW_ISOLATES: i32 = 0x1;
/// C `vendor/tmux/screen-redraw.c:231`: `#define REDRAW_DEFAULT_SET 0x2`
const REDRAW_DEFAULT_SET: i32 = 0x2;
/// C `vendor/tmux/screen-redraw.c:232`: `#define REDRAW_STATUS_TOP 0x4`
const REDRAW_STATUS_TOP: i32 = 0x4;

/// Context for redrawing.
/// C `vendor/tmux/screen-redraw.c:216`: `struct redraw_draw_ctx`
struct redraw_draw_ctx {
    scene: *mut redraw_scene,

    active: *mut window_pane,
    marked: *mut window_pane,

    status_lines: u32,
    pane_lines: pane_lines,
    default_gc: grid_cell,

    flags: i32,
}

unsafe fn screen_size_x(s: *const screen) -> u32 {
    unsafe { (*(*s).grid).sx }
}
unsafe fn screen_size_y(s: *const screen) -> u32 {
    unsafe { (*(*s).grid).sy }
}
unsafe fn screen_hsize(s: *const screen) -> u32 {
    unsafe { (*(*s).grid).hsize }
}
unsafe fn screen_hlimit(s: *const screen) -> u32 {
    unsafe { (*(*s).grid).hlimit }
}

/// Menu.
#[repr(C)]
#[derive(Default)]
struct menu_item {
    name: Cow<'static, str>,
    key: key_code,
    command: SyncCharPtr,
}
impl menu_item {
    const fn new(name: &'static str, key: key_code, command: *const u8) -> Self {
        Self {
            name: Cow::Borrowed(name),
            key,
            command: SyncCharPtr(command),
        }
    }
}

#[repr(C)]
struct menu {
    title: String,
    items: Vec<menu_item>,
    width: u32,
}
type menu_choice_cb = Option<unsafe fn(*mut menu, u32, key_code, *mut c_void)>;

#[expect(clippy::type_complexity)]
/// Window mode. Windows can be in several modes and this is used to call the
/// right function to handle input and output.
#[repr(C)]
struct window_mode {
    name: &'static str,
    default_format: Option<&'static str>,

    init: unsafe fn(NonNull<window_mode_entry>, *mut cmd_find_state, *mut args) -> *mut screen,
    free: unsafe fn(NonNull<window_mode_entry>),
    resize: unsafe fn(NonNull<window_mode_entry>, u32, u32),
    update: Option<unsafe fn(NonNull<window_mode_entry>)>,
    key: Option<
        unsafe fn(
            NonNull<window_mode_entry>,
            *mut client,
            *mut session,
            *mut winlink,
            key_code,
            *mut mouse_event,
        ),
    >,

    key_table: Option<unsafe fn(*mut window_mode_entry) -> *const u8>,
    command: Option<
        unsafe fn(
            NonNull<window_mode_entry>,
            *mut client,
            *mut session,
            *mut winlink,
            *mut args,
            *mut mouse_event,
        ),
    >,
    formats: Option<unsafe fn(*mut window_mode_entry, *mut format_tree)>,
    get_screen: Option<unsafe fn(*mut window_mode_entry) -> *mut screen>,
}

// Active window mode.
impl_tailq_entry!(window_mode_entry, entry, tailq_entry<window_mode_entry>);
#[repr(C)]
struct window_mode_entry {
    wp: *mut window_pane,
    swp: *mut window_pane,

    mode: *const window_mode,
    data: *mut c_void,

    screen: *mut screen,
    prefix: u32,
    /// C `vendor/tmux/tmux.h:1194`: `int kill` — set from `-k` on the command that
    /// entered the mode (`window.c:1380`); `window_pane_reset_mode` kills the
    /// pane when the mode exits. The default `Tab`/`BTab` bindings rely on it
    /// to dispose of the scratch pane they open `switch-mode` in.
    kill: c_int,

    // #[entry]
    entry: tailq_entry<window_mode_entry>,
}

/// Offsets into pane buffer.
#[repr(C)]
#[derive(Copy, Clone)]
struct window_pane_offset {
    used: usize,
}

impl_tailq_entry!(window_pane_resize, entry, tailq_entry<window_pane_resize>);
/// Queued pane resize.
#[repr(C)]
struct window_pane_resize {
    sx: u32,
    sy: u32,

    osx: u32,
    osy: u32,

    entry: tailq_entry<window_pane_resize>,
}
type window_pane_resizes = tailq_head<window_pane_resize>;

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq)]
    struct window_pane_flags : i32 {
        const PANE_REDRAW = 0x1;
        const PANE_DROP = 0x2;
        const PANE_FOCUSED = 0x4;
        const PANE_VISITED = 0x8;
        const PANE_ZOOMED = 0x10;
        /// C `vendor/tmux/tmux.h:1285`: `#define PANE_NEWSTATUS 0x20`. Set when
        /// the pane's status line changed and its spans need redrawing.
        const PANE_NEWSTATUS = 0x20;
        const PANE_INPUTOFF = 0x40;
        const PANE_CHANGED = 0x80;
        const PANE_EXITED = 0x100;
        const PANE_STATUSREADY = 0x200;
        const PANE_STATUSDRAWN = 0x400;
        const PANE_EMPTY = 0x800;
        const PANE_STYLECHANGED = 0x1000;
        /// C `vendor/tmux/tmux.h:1293`. Set when the pane's theme changed and a
        /// theme update is owed to the program inside it.
        const PANE_THEMECHANGED = 0x2000;
        /// C `vendor/tmux/tmux.h:1294`. This sat at 0x2000 -- the bit the C
        /// gives PANE_THEMECHANGED -- because the port never had that flag.
        const PANE_UNSEENCHANGES = 0x4000;
        /// C `vendor/tmux/tmux.h:1295`: the pane's scrollbar needs redrawing on
        /// its own. A reserved scrollbar sits outside the pane's own area, so it
        /// can be repainted without repainting the pane; an overlay one cannot,
        /// and takes `PANE_REDRAW` instead.
        const PANE_REDRAWSCROLLBAR = 0x8000;
    }
}

/// Child window structure.
#[repr(C)]
struct window_pane {
    id: u32,
    active_point: u32,

    window: *mut window,
    options: *mut options,

    layout_cell: *mut layout_cell,
    saved_layout_cell: *mut layout_cell,

    sx: u32,
    sy: u32,

    /// C `vendor/tmux/tmux.h:1518`: `int xoff` / `int yoff`.
    ///
    /// Signed: a floating pane can be positioned partly off an edge, so the
    /// offset legitimately goes negative. Stored unsigned, -1 became ~4.29e9
    /// and every downstream comparison overflowed. Read sites cast back to u32
    /// because the C promotes `int + u_int` to unsigned, so the arithmetic they
    /// do is unchanged; only the ability to hold and test a negative is new.
    xoff: i32,
    yoff: i32,

    flags: window_pane_flags,

    argc: i32,
    argv: *mut *mut u8,
    /// Owned shell path / working dir; `None` until set (`xcalloc` zeroes them
    /// to valid `None` via the null-pointer niche). Dropped in
    /// `window_pane_destroy` before the pane is freed. Read via
    /// `shell_ptr()`/`cwd_ptr()`.
    shell: Option<std::ffi::CString>,
    cwd: Option<std::ffi::CString>,

    pid: pid_t,
    tty: [u8; TTY_NAME_MAX],
    status: i32,
    dead_time: timeval,

    fd: i32,
    event: *mut bufferevent,

    offset: window_pane_offset,
    base_offset: usize,

    resize_queue: window_pane_resizes,
    resize_timer: event,
    sync_timer: event,
    /// C `vendor/tmux/tmux.h`: `bitstr_t *sync_dirty` / `u_int sync_dirty_size`.
    ///
    /// Lines touched while synchronized-output mode is on. Drawing is deferred
    /// until the mode ends, so an app that repaints inside a sync block costs
    /// one flush rather than one draw per operation.
    sync_dirty: Option<Box<BitStr>>,

    /// C `vendor/tmux/tmux.h:1300`: where the scrollbar slider sits and how tall
    /// it is, in pane rows. Recomputed on redraw and read back when a drag
    /// starts, so the grab point stays on the slider.
    sb_slider_y: u32,
    sb_slider_h: u32,
    /// Whether an auto-hiding scrollbar is currently shown, whether the pointer
    /// is over it, and the timer that hides it again.
    sb_auto_visible: i32,
    sb_auto_hover: i32,
    sb_auto_timer: event,
    /// C `vendor/tmux/tmux.h:1365`: resolved from `pane-scrollbars-style`.
    scrollbar_style: style,

    ictx: *mut input_ctx,

    cached_gc: grid_cell,
    cached_active_gc: grid_cell,
    palette: colour_palette,

    /// The theme this pane last told its program about (`tmux.h:1335`), so a
    /// DSR 996 query can be answered and repeat updates suppressed.
    last_theme: client_theme,
    /// pid of the `pipe-pane` child (`tmux.h:1339`), reported by
    /// `#{pane_pipe_pid}`. Meaningful only while `pipe_fd != -1`.
    pipe_pid: pid_t,
    pipe_fd: i32,
    pipe_event: *mut bufferevent,
    pipe_offset: window_pane_offset,

    screen: *mut screen,
    base: screen,

    status_screen: screen,
    status_size: usize,

    modes: tailq_head<window_mode_entry>,

    /// Owned last copy-mode search string; `None` until set. Read via
    /// `searchstr_ptr()`.
    searchstr: Option<std::ffi::CString>,
    searchregex: i32,

    /// C `vendor/tmux/tmux.h:1353`: `struct prompt *prompt` — an open prompt
    /// owned by this pane and drawn over it rather than on the status line
    /// (`command-prompt -P`). Null when the pane has no prompt.
    prompt: *mut prompt,
    /// C `vendor/tmux/tmux.h:1354`: `struct window_pane_prompt *prompt_data`
    prompt_data: *mut window_pane_prompt,
    /// C `vendor/tmux/tmux.h:1355`: column the prompt's cursor ended up at,
    /// written by `prompt_draw`.
    prompt_cx: c_uint,

    border_gc_set: i32,
    border_gc: grid_cell,

    /// C `vendor/tmux/tmux.h:1362`: `int active_border_gc_set`
    active_border_gc_set: i32,
    /// C `vendor/tmux/tmux.h:1363`: `struct grid_cell active_border_gc`
    ///
    /// Cached separately from `border_gc` because a scene redraw resolves both
    /// the active and inactive border style for the same pane in one pass.
    active_border_gc: grid_cell,

    control_bg: i32,
    control_fg: i32,

    /// link in list of all panes
    entry: tailq_entry<window_pane>,
    /// C `vendor/tmux/tmux.h:1367`: `struct visible_ranges r`
    ///
    /// Scratch buffer reused by `window_visible_ranges` for this pane, so
    /// clipping a line does not allocate on every write.
    r: visible_ranges,

    /// link in list of last visited
    sentry: tailq_entry<window_pane>,
    /// z-index link in list of all panes (`window.z_index`)
    zentry: tailq_entry<window_pane>,
    tree_entry: rb_entry<window_pane>,
}
type window_panes = tailq_head<window_pane>;
type window_pane_tree = rb_head<window_pane>;

impl Entry<window_pane, discr_entry> for window_pane {
    unsafe fn entry(this: *mut Self) -> *mut tailq_entry<window_pane> {
        unsafe { &raw mut (*this).entry }
    }
}
impl Entry<window_pane, discr_sentry> for window_pane {
    unsafe fn entry(this: *mut Self) -> *mut tailq_entry<window_pane> {
        unsafe { &raw mut (*this).sentry }
    }
}
impl Entry<window_pane, discr_zentry> for window_pane {
    unsafe fn entry(this: *mut Self) -> *mut tailq_entry<window_pane> {
        unsafe { &raw mut (*this).zentry }
    }
}

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq)]
    struct window_flag: i32 {
        const BELL = 0x1;
        const ACTIVITY = 0x2;
        const SILENCE = 0x4;
        const ZOOMED = 0x8;
        const WASZOOMED = 0x10;
        const RESIZE = 0x20;
    }
}
const WINDOW_ALERTFLAGS: window_flag = window_flag::BELL
    .union(window_flag::ACTIVITY)
    .union(window_flag::SILENCE);

/// Window structure.
#[repr(C)]
struct window {
    id: u32,
    latest: *mut c_void,

    /// Owned window name (always set — `""` at create); `None` only transiently
    /// during rename. Dropped in `window_destroy` before the struct is freed.
    /// Read via `name_ptr()`.
    name: Option<std::ffi::CString>,
    name_event: event,
    name_time: timeval,

    alerts_timer: event,
    offset_timer: event,

    activity_time: timeval,
    creation_time: timeval,

    active: *mut window_pane,
    last_panes: window_panes,
    /// C `vendor/tmux/tmux.h`: `struct window_panes z_index` — panes ordered by
    /// z-index (floating panes sit above tiled ones); linked via `zentry`.
    z_index: window_panes,
    panes: window_panes,

    lastlayout: i32,
    layout_root: *mut layout_cell,
    saved_layout_root: *mut layout_cell,
    /// Owned serialized layout saved for `select-layout -o` (undo); `None` when
    /// unset. Dropped in `window_destroy` before the struct is freed.
    old_layout: Option<std::ffi::CString>,

    sx: u32,
    sy: u32,
    manual_sx: u32,
    manual_sy: u32,
    xpixel: u32,
    ypixel: u32,

    new_sx: u32,
    new_sy: u32,
    new_xpixel: u32,
    new_ypixel: u32,

    /// Bumped whenever the redraw scene changes; floating resize/move invalidate
    /// via `redraw_invalidate_scene`.
    redraw_scene_generation: u64,
    /// Position of the last floating pane created, for cascading placement.
    last_new_pane_x: u32,
    last_new_pane_y: u32,

    /// C `vendor/tmux/tmux.h:1420`: `pane-scrollbars` and its position, cached
    /// on the window because the layout needs them on every fix.
    sb: i32,
    sb_pos: i32,

    fill_character: *mut utf8_data,
    flags: window_flag,

    alerts_queued: i32,

    options: *mut options,

    references: u32,
    winlinks: tailq_head<winlink>,
    entry: rb_entry<window>,
}
type windows = rb_head<window>;

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq)]
    struct winlink_flags: i32 {
        const WINLINK_BELL = 0x1;
        const WINLINK_ACTIVITY = 0x2;
        const WINLINK_SILENCE = 0x4;
        const WINLINK_VISITED = 0x8;
    }
}
const WINLINK_ALERTFLAGS: winlink_flags = winlink_flags::WINLINK_BELL
    .union(winlink_flags::WINLINK_ACTIVITY)
    .union(winlink_flags::WINLINK_SILENCE);

#[repr(C)]
#[derive(Copy, Clone)]
struct winlink {
    idx: i32,
    session: *mut session,
    window: *mut window,

    flags: winlink_flags,

    entry: rb_entry<winlink>,

    wentry: tailq_entry<winlink>,
    sentry: tailq_entry<winlink>,
}

impl crate::compat::queue::Entry<winlink, discr_wentry> for winlink {
    unsafe fn entry(this: *mut Self) -> *mut tailq_entry<winlink> {
        unsafe { &raw mut (*this).wentry }
    }
}

impl crate::compat::queue::Entry<winlink, discr_sentry> for winlink {
    unsafe fn entry(this: *mut Self) -> *mut tailq_entry<winlink> {
        unsafe { &raw mut (*this).sentry }
    }
}

type winlinks = rb_head<winlink>;
// crate::compat::impl_rb_tree_protos!(winlinks, winlink);
type winlink_stack = tailq_head<winlink>;
// crate::compat::impl_rb_tree_protos!(winlink_stack, winlink);

/// Window size option.
#[repr(i32)]
#[derive(Copy, Clone, Eq, PartialEq, num_enum::TryFromPrimitive)]
enum window_size_option {
    WINDOW_SIZE_LARGEST,
    WINDOW_SIZE_SMALLEST,
    WINDOW_SIZE_MANUAL,
    WINDOW_SIZE_LATEST,
}

/// Pane border status option.
#[repr(i32)]
#[derive(Copy, Clone, Eq, PartialEq, num_enum::TryFromPrimitive)]
enum pane_status {
    PANE_STATUS_OFF,
    PANE_STATUS_TOP,
    PANE_STATUS_BOTTOM,
    /// C `vendor/tmux/tmux.h:1474`: `PANE_STATUS_TOP_FLOATING`. Status line on
    /// floating panes only; tiled panes read it as `PANE_STATUS_OFF`.
    PANE_STATUS_TOP_FLOATING,
    /// C `vendor/tmux/tmux.h:1475`: `PANE_STATUS_BOTTOM_FLOATING`.
    PANE_STATUS_BOTTOM_FLOATING,
}

/// Layout direction.
#[repr(i32)]
#[derive(Copy, Clone, Eq, PartialEq, num_enum::TryFromPrimitive)]
enum layout_type {
    LAYOUT_LEFTRIGHT,
    LAYOUT_TOPBOTTOM,
    LAYOUT_WINDOWPANE,
}

/// Layout cells queue.
type layout_cells = tailq_head<layout_cell>;

impl_tailq_entry!(layout_cell, entry, tailq_entry<layout_cell>);

/// C `vendor/tmux/tmux.h:1510`: `#define LAYOUT_CELL_FLOATING 0x1`.
const LAYOUT_CELL_FLOATING: c_int = 0x1;

/// Layout cell.
#[repr(C)]
struct layout_cell {
    type_: layout_type,

    /// C `vendor/tmux/tmux.h:1510`: `int flags` — `LAYOUT_CELL_FLOATING` marks a
    /// floating (non-tiled) pane cell.
    flags: c_int,

    parent: *mut layout_cell,

    sx: u32,
    sy: u32,

    /// C `vendor/tmux/tmux.h:1276`: `int xoff` / `int yoff`. Signed for the
    /// same reason as `window_pane` — a floating cell can sit off an edge.
    xoff: i32,
    yoff: i32,

    /// C `vendor/tmux/tmux.h:1521`-`1525`: the tiled geometry a cell had before
    /// it was floated, so `break-pane -W` has a size and offset to start from.
    saved_sx: u32,
    saved_sy: u32,
    saved_xoff: i32,
    saved_yoff: i32,

    wp: *mut window_pane,
    cells: layout_cells,

    entry: tailq_entry<layout_cell>,
}

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Copy, Clone)]
    struct environ_flags: i32 {
        const ENVIRON_HIDDEN = 0x1;
    }
}
const ENVIRON_HIDDEN: environ_flags = environ_flags::ENVIRON_HIDDEN;

/// Environment variable.
///
/// `name`/`value` are Rust-owned (`CString`) rather than raw `char *`: the
/// entry owns its strings and frees them via `Drop` when the boxed node is
/// dropped, so there is no manual `free()` to double-free — including in the
/// fork child of `spawn`/`job`, which previously aborted libmalloc with
/// `POINTER_BEING_FREED_WAS_NOT_ALLOCATED`. `value` is `None` for a cleared
/// entry (C `NULL`). Read raw pointers via `name_ptr()`/`value_ptr()`.
#[repr(C)]
struct environ_entry {
    name: std::ffi::CString,
    value: Option<std::ffi::CString>,

    flags: environ_flags,
    entry: rb_entry<environ_entry>,
}

/// Client session.
#[repr(C)]
struct session_group {
    name: Cow<'static, str>,
    sessions: tailq_head<session>,

    entry: rb_entry<session_group>,
}
type session_groups = rb_head<session_group>;

const SESSION_PASTING: i32 = 0x1;
const SESSION_ALERTED: i32 = 0x2;

#[repr(C)]
struct session {
    id: u32,
    name: Cow<'static, str>,
    /// Owned working directory; `None` only transiently (zeroed on create,
    /// set immediately). Dropped in session teardown before the struct is freed.
    cwd: Option<std::ffi::CString>,

    creation_time: timeval,
    last_attached_time: timeval,
    activity_time: timeval,
    last_activity_time: timeval,

    lock_timer: event,

    curw: *mut winlink,
    lastw: winlink_stack,
    windows: winlinks,

    statusat: i32,
    statuslines: u32,

    options: *mut options,

    flags: i32,

    attached: u32,

    tio: *mut termios,

    environ: *mut environ,

    references: i32,

    gentry: tailq_entry<session>,
    entry: rb_entry<session>,
}
type sessions = rb_head<session>;
impl_tailq_entry!(session, gentry, tailq_entry<session>);

const MOUSE_MASK_BUTTONS: u32 = 195;
const MOUSE_MASK_SHIFT: u32 = 4;
const MOUSE_MASK_META: u32 = 8;
const MOUSE_MASK_CTRL: u32 = 16;
const MOUSE_MASK_DRAG: u32 = 32;
const MOUSE_MASK_MODIFIERS: u32 = MOUSE_MASK_SHIFT | MOUSE_MASK_META | MOUSE_MASK_CTRL;

// Mouse wheel type.
const MOUSE_WHEEL_UP: u32 = 64;
const MOUSE_WHEEL_DOWN: u32 = 65;

// Mouse button type.
const MOUSE_BUTTON_1: u32 = 0;
const MOUSE_BUTTON_2: u32 = 1;
const MOUSE_BUTTON_3: u32 = 2;
const MOUSE_BUTTON_6: u32 = 66;
const MOUSE_BUTTON_7: u32 = 67;
const MOUSE_BUTTON_8: u32 = 128;
const MOUSE_BUTTON_9: u32 = 129;
const MOUSE_BUTTON_10: u32 = 130;
const MOUSE_BUTTON_11: u32 = 131;

// Mouse helpers.
#[expect(non_snake_case)]
#[inline]
fn MOUSE_BUTTONS(b: u32) -> u32 {
    b & MOUSE_MASK_BUTTONS
}
#[expect(non_snake_case)]
#[inline]
fn MOUSE_WHEEL(b: u32) -> bool {
    ((b) & MOUSE_MASK_BUTTONS) == MOUSE_WHEEL_UP || ((b) & MOUSE_MASK_BUTTONS) == MOUSE_WHEEL_DOWN
}
#[expect(non_snake_case)]
#[inline]
fn MOUSE_DRAG(b: u32) -> bool {
    b & MOUSE_MASK_DRAG != 0
}
#[expect(non_snake_case)]
#[inline]
fn MOUSE_RELEASE(b: u32) -> bool {
    b & MOUSE_MASK_BUTTONS == 3
}

/// Mouse input.
#[repr(C)]
#[derive(Copy, Clone)]
struct mouse_event {
    valid: bool,
    ignore: i32,

    key: key_code,

    statusat: i32,
    statuslines: u32,

    x: u32,
    y: u32,
    b: u32,

    lx: u32,
    ly: u32,
    lb: u32,

    ox: u32,
    oy: u32,

    s: i32,
    w: i32,
    wp: i32,

    sgr_type: u32,
    sgr_b: u32,
}

/// Key event.
#[repr(C)]
struct key_event {
    key: key_code,
    m: mouse_event,
}

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Copy, Clone)]
    struct term_flags: i32 {
        const TERM_256COLOURS = 0x1;
        const TERM_NOAM = 0x2;
        const TERM_DECSLRM = 0x4;
        const TERM_DECFRA = 0x8;
        const TERM_RGBCOLOURS = 0x10;
        const TERM_VT100LIKE = 0x20;
        const TERM_SIXEL = 0x40;
    }
}

/// Terminal definition.
#[repr(C)]
struct tty_term {
    /// Owned terminal name; dropped in `tty_term_free`. Read via `name_ptr()`.
    name: Option<std::ffi::CString>,
    tty: *mut tty,
    features: i32,

    acs: [[u8; 2]; c_uchar::MAX as usize + 1],

    codes: *mut tty_code,
    expand_context: ExpandContext,
    flags: term_flags,

    entry: list_entry<tty_term>,
}
type tty_terms = list_head<tty_term>;
impl ListEntry<tty_term, discr_entry> for tty_term {
    unsafe fn field(this: *mut Self) -> *mut list_entry<tty_term> {
        unsafe { &raw mut (*this).entry }
    }
}

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Copy, Clone)]
    struct tty_flags: i32 {
        const TTY_NOCURSOR = 0x1;
        const TTY_FREEZE = 0x2;
        const TTY_TIMER = 0x4;
        const TTY_NOBLOCK = 0x8;
        const TTY_STARTED = 0x10;
        const TTY_OPENED = 0x20;
        const TTY_OSC52QUERY = 0x40;
        const TTY_BLOCK = 0x80;
        const TTY_HAVEDA = 0x100; // Primary DA.
        const TTY_HAVEXDA = 0x200;
        const TTY_SYNCING = 0x400;
        const TTY_HAVEDA2 = 0x800; // Secondary DA.
    }
}
const TTY_ALL_REQUEST_FLAGS: tty_flags = tty_flags::TTY_HAVEDA
    .union(tty_flags::TTY_HAVEDA2)
    .union(tty_flags::TTY_HAVEXDA);

/// Client terminal.
#[repr(C)]
struct tty {
    client: *mut client,
    start_timer: event,
    clipboard_timer: event,
    last_requests: time_t,

    sx: u32,
    sy: u32,

    xpixel: u32,
    ypixel: u32,

    cx: u32,
    cy: u32,
    cstyle: screen_cursor_style,
    ccolour: i32,

    oflag: i32,
    oox: u32,
    ooy: u32,
    osx: u32,
    osy: u32,

    mode: mode_flag,
    fg: i32,
    bg: i32,

    rlower: u32,
    rupper: u32,

    rleft: u32,
    rright: u32,

    event_in: event,
    in_: *mut evbuffer,
    event_out: event,
    out: *mut evbuffer,
    timer: event,
    discarded: usize,

    tio: termios,

    cell: grid_cell,
    last_cell: grid_cell,

    flags: tty_flags,

    term: *mut tty_term,

    mouse_last_x: u32,
    mouse_last_y: u32,
    mouse_last_b: u32,
    mouse_drag_flag: i32,
    /// C `vendor/tmux/tmux.h:1769`: where within the scrollbar slider a drag
    /// was grabbed, or -1 when no slider drag is in progress. Read by
    /// `scroll-to-mouse` and `copy-mode -S`.
    /// C `vendor/tmux/tmux.h`: `int mouse_scrolling_flag` -- latched while a
    /// scrollbar slider drag is in progress, so motion outside the pane still
    /// routes to the slider.
    mouse_scrolling_flag: i32,
    mouse_slider_mpos: i32,
    /// C `vendor/tmux/tmux.h:1770`: `int mouse_last_pane`.
    ///
    /// Pane id the current drag was started on, or -1. Latching it keeps a
    /// border drag on the pane the user grabbed instead of re-resolving the
    /// pane under the cursor on every motion event.
    mouse_last_pane: i32,
    mouse_drag_update: Option<unsafe fn(*mut client, *mut mouse_event)>,
    mouse_drag_release: Option<unsafe fn(*mut client, *mut mouse_event)>,

    /// C `vendor/tmux/tmux.h:1737`: `struct visible_ranges r`
    ///
    /// Scratch buffer reused by the overlay-check callbacks.
    r: visible_ranges,

    key_timer: event,
    key_tree: *mut tty_key,
}

type tty_ctx_redraw_cb = Option<unsafe fn(*const tty_ctx)>;
type tty_ctx_set_client_cb = Option<unsafe fn(*mut tty_ctx, *mut client) -> i32>;

#[repr(C)]
struct tty_ctx {
    s: *mut screen,

    redraw_cb: tty_ctx_redraw_cb,
    set_client_cb: tty_ctx_set_client_cb,
    arg: *mut c_void,

    cell: *const grid_cell,
    wrapped: bool,

    num: u32,
    ptr: *mut c_void,
    ptr2: *mut c_void,

    allow_invisible_panes: i32,

    // Cursor and region position before the screen was updated - this is
    // where the command should be applied; the values in the screen have
    // already been updated.
    ocx: u32,
    ocy: u32,

    orupper: u32,
    orlower: u32,

    // Target region (usually pane) offset and size.
    xoff: u32,
    yoff: u32,
    rxoff: u32,
    ryoff: u32,
    sx: u32,
    sy: u32,

    // The background colour used for clearing (erasing).
    bg: u32,

    // The default colours and palette.
    defaults: grid_cell,
    palette: *const colour_palette,

    // Containing region (usually window) offset and size.
    bigger: i32,
    wox: u32,
    woy: u32,
    wsx: u32,
    wsy: u32,
}

// Saved message entry.
impl_tailq_entry!(message_entry, entry, tailq_entry<message_entry>);
#[repr(C)]
struct message_entry {
    /// Owned message text; drops with the boxed entry — no manual `free()`.
    msg: std::ffi::CString,
    msg_num: u32,
    msg_time: timeval,

    entry: tailq_entry<message_entry>,
}
type message_list = tailq_head<message_entry>;

/// Argument type.
#[repr(i32)]
#[derive(Copy, Clone, Eq, PartialEq)]
enum args_type {
    ARGS_NONE,
    ARGS_STRING,
    ARGS_COMMANDS,
}

#[repr(C)]
union args_value_union {
    string: *mut u8,
    cmdlist: *mut cmd_list,
}

impl_tailq_entry!(args_value, entry, tailq_entry<args_value>);
/// Argument value.
#[repr(C)]
struct args_value {
    type_: args_type,
    union_: args_value_union,
    /// Lazily-computed cache of the printed command list; `None` until first
    /// `args_value_as_string`. Dropped in `args_free_value`. `args_copy_value`
    /// leaves it `None` on the (zeroed) target. Read via `cached_ptr()`.
    cached: Option<std::ffi::CString>,
    // #[entry]
    entry: tailq_entry<args_value>,
}
type args_tree = rb_head<args_entry>;

/// Arguments parsing type.
#[repr(C)]
#[derive(Eq, PartialEq)]
enum args_parse_type {
    ARGS_PARSE_INVALID,
    ARGS_PARSE_STRING,
    ARGS_PARSE_COMMANDS_OR_STRING,
    #[expect(dead_code)]
    ARGS_PARSE_COMMANDS,
}

type args_parse_cb = Option<unsafe fn(*mut args, u32, *mut *mut u8) -> args_parse_type>;
#[repr(C)]
struct args_parse {
    template: &'static str,
    lower: i32,
    upper: i32,
    cb: args_parse_cb,
}

impl args_parse {
    const fn new(template: &'static str, lower: i32, upper: i32, cb: args_parse_cb) -> Self {
        Self {
            template,
            lower,
            upper,
            cb,
        }
    }
}

/// Command find structures.
#[repr(C)]
#[derive(Copy, Clone, Default)]
enum cmd_find_type {
    #[default]
    CMD_FIND_PANE,
    CMD_FIND_WINDOW,
    CMD_FIND_SESSION,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct cmd_find_state {
    flags: cmd_find_flags,
    current: *mut cmd_find_state,

    s: *mut session,
    wl: *mut winlink,
    w: *mut window,
    wp: *mut window_pane,
    idx: i32,
}

bitflags::bitflags! {
    // Command find flags.
    #[repr(transparent)]
    #[derive(Copy, Clone, Default, Eq, PartialEq)]
    struct cmd_find_flags: i32 {
        const CMD_FIND_PREFER_UNATTACHED = 0x1;
        const CMD_FIND_QUIET = 0x2;
        const CMD_FIND_WINDOW_INDEX = 0x4;
        const CMD_FIND_DEFAULT_MARKED = 0x8;
        const CMD_FIND_EXACT_SESSION = 0x10;
        const CMD_FIND_EXACT_WINDOW = 0x20;
        const CMD_FIND_CANFAIL = 0x40;
    }
}

/// List of commands.
#[repr(C)]
struct cmd_list {
    references: i32,
    group: u32,
    list: *mut cmds,
}

// Command return values.
#[repr(i32)]
#[derive(Copy, Clone, Eq, PartialEq)]
enum cmd_retval {
    CMD_RETURN_ERROR = -1,
    CMD_RETURN_NORMAL = 0,
    CMD_RETURN_WAIT,
    CMD_RETURN_STOP,
}

// Command parse result.
#[repr(i32)]
#[derive(Copy, Clone, Default, Eq, PartialEq)]
enum cmd_parse_status {
    #[default]
    CMD_PARSE_ERROR,
    CMD_PARSE_SUCCESS,
}

type cmd_parse_result = Result<*mut cmd_list /* cmdlist */, *mut u8 /* error */>;

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq)]
    struct cmd_parse_input_flags: i32 {
        const CMD_PARSE_QUIET = 0x1;
        const CMD_PARSE_PARSEONLY = 0x2;
        const CMD_PARSE_NOALIAS = 0x4;
        const CMD_PARSE_VERBOSE = 0x8;
        const CMD_PARSE_ONEGROUP = 0x10;
    }
}

#[repr(transparent)]
#[derive(Default)]
struct AtomicCmdParseInputFlags(std::sync::atomic::AtomicI32);
impl From<cmd_parse_input_flags> for AtomicCmdParseInputFlags {
    fn from(value: cmd_parse_input_flags) -> Self {
        Self(std::sync::atomic::AtomicI32::new(value.bits()))
    }
}
impl AtomicCmdParseInputFlags {
    fn intersects(&self, rhs: cmd_parse_input_flags) -> bool {
        cmd_parse_input_flags::from_bits(self.0.load(std::sync::atomic::Ordering::SeqCst))
            .unwrap()
            .intersects(rhs)
    }
}
impl std::ops::BitOrAssign<cmd_parse_input_flags> for &AtomicCmdParseInputFlags {
    fn bitor_assign(&mut self, rhs: cmd_parse_input_flags) {
        self.0
            .fetch_or(rhs.bits(), std::sync::atomic::Ordering::SeqCst);
    }
}
impl std::ops::BitAndAssign<cmd_parse_input_flags> for &AtomicCmdParseInputFlags {
    fn bitand_assign(&mut self, rhs: cmd_parse_input_flags) {
        self.0
            .fetch_and(rhs.bits(), std::sync::atomic::Ordering::SeqCst);
    }
}

#[repr(C)]
#[derive(Default)]
struct cmd_parse_input<'a> {
    flags: AtomicCmdParseInputFlags,

    file: Option<&'a str>,
    line: AtomicU32, // work around borrow checker

    item: *mut cmdq_item,
    c: *mut client,
    fs: cmd_find_state,
}

bitflags::bitflags! {
    /// Command queue flags.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq)]
    struct cmdq_state_flags: i32 {
        const CMDQ_STATE_REPEAT = 0x1;
        const CMDQ_STATE_CONTROL = 0x2;
        const CMDQ_STATE_NOHOOKS = 0x4;
    }
}

// Command queue callback.
type cmdq_cb = Option<unsafe fn(*mut cmdq_item, *mut c_void) -> cmd_retval>;

// Command definition flag.
#[repr(C)]
#[derive(Copy, Clone, Default)]
struct cmd_entry_flag {
    flag: u8,
    type_: cmd_find_type,
    flags: cmd_find_flags,
}

impl cmd_entry_flag {
    const fn new(flag: u8, type_: cmd_find_type, flags: cmd_find_flags) -> Self {
        Self { flag, type_, flags }
    }

    const fn zeroed() -> Self {
        Self {
            flag: b'\0',
            type_: cmd_find_type::CMD_FIND_PANE,
            flags: cmd_find_flags::empty(),
        }
    }
}

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq)]
    struct cmd_flag: i32 {
        const CMD_STARTSERVER = 0x1;
        const CMD_READONLY = 0x2;
        const CMD_AFTERHOOK = 0x4;
        const CMD_CLIENT_CFLAG = 0x8;
        const CMD_CLIENT_TFLAG = 0x10;
        const CMD_CLIENT_CANFAIL = 0x20;
    }
}

// Command definition.
#[repr(C)]
struct cmd_entry {
    name: &'static str,
    alias: Option<&'static str>,

    args: args_parse,
    usage: &'static str,

    source: cmd_entry_flag,
    target: cmd_entry_flag,

    flags: cmd_flag,

    exec: unsafe fn(*mut cmd, *mut cmdq_item) -> cmd_retval,
}

// Status line.
const STATUS_LINES_LIMIT: usize = 5;
#[repr(C)]
struct status_line_entry {
    /// Owned cached expansion of this status line; `None` until drawn. Dropped
    /// in `status_free`. Read via `expanded_ptr()`.
    expanded: Option<std::ffi::CString>,
    ranges: style_ranges,
}

impl status_line_entry {
    /// Borrowed `char *` to the cached expansion, or NULL when unset.
    #[inline]
    pub(crate) fn expanded_ptr(&self) -> *const u8 {
        match &self.expanded {
            Some(c) => c.as_ptr().cast(),
            None => std::ptr::null(),
        }
    }
}
#[repr(C)]
struct status_line {
    timer: event,

    screen: screen,
    active: *mut screen,
    references: c_int,

    /// C `vendor/tmux/tmux.h:2014`: column the prompt's cursor ended up at,
    /// written by `prompt_draw` and read back by `status_prompt_cursor`.
    prompt_cx: c_uint,

    style: grid_cell,
    entries: [status_line_entry; STATUS_LINES_LIMIT],
}

/// Prompt type. C `vendor/tmux/tmux.h:2061`: next-3.7 cut this to two when the
/// prompt moved into prompt.c — `target` and `window-target` are gone, and with
/// them the target/session/window-menu completion they selected.
const PROMPT_NTYPES: u32 = 2;
#[repr(u32)]
#[derive(Copy, Clone, Default, Eq, PartialEq, num_enum::TryFromPrimitive)]
enum prompt_type {
    #[default]
    PROMPT_TYPE_COMMAND = 0,
    PROMPT_TYPE_SEARCH,
    PROMPT_TYPE_INVALID = 0xff,
}

/// Prompt result. C `vendor/tmux/tmux.h:2069`: what an input callback says the
/// prompt should do next.
#[repr(u32)]
#[derive(Copy, Clone, Default, Eq, PartialEq)]
enum prompt_result {
    #[default]
    PROMPT_CONTINUE,
    PROMPT_CLOSE,
}

/// Prompt key result. C `vendor/tmux/tmux.h:2075`: what the prompt did with a
/// key, which is also what the input callback is told about the key that fired
/// it.
#[repr(u32)]
#[derive(Copy, Clone, Default, Eq, PartialEq, Debug)]
enum prompt_key_result {
    #[default]
    PROMPT_KEY_NOT_HANDLED,
    PROMPT_KEY_HANDLED,
    PROMPT_KEY_CLOSE,
    PROMPT_KEY_MOVE,
}

// File in client.
type client_file_cb = Option<unsafe fn(*mut client, *mut u8, i32, i32, *mut evbuffer, *mut c_void)>;
#[repr(C)]
struct client_file {
    c: *mut client,
    peer: *mut tmuxpeer,
    tree: *mut client_files,

    references: i32,
    stream: i32,

    /// Owned file path, `None` until set. `xcalloc` zeroes it to a valid `None`
    /// (null-pointer niche); dropped in `file_free` before the struct is freed.
    path: Option<std::ffi::CString>,
    buffer: *mut evbuffer,
    event: *mut bufferevent,

    fd: i32,
    error: i32,
    closed: i32,

    cb: client_file_cb,
    data: *mut c_void,

    entry: rb_entry<client_file>,
}
type client_files = rb_head<client_file>;
RB_GENERATE!(client_files, client_file, entry, discr_entry, file_cmp);

// Client window.
#[repr(C)]
struct client_window {
    window: u32,
    pane: *mut window_pane,

    sx: u32,
    sy: u32,

    entry: rb_entry<client_window>,
}
type client_windows = rb_head<client_window>;
RB_GENERATE!(
    client_windows,
    client_window,
    entry,
    discr_entry,
    server_client_window_cmp
);

/// One unobstructed span on a line.
/// C `vendor/tmux/tmux.h:1250`: `struct visible_range`
#[repr(C)]
#[derive(Copy, Clone, Default)]
struct visible_range {
    /// Start column.
    px: u32,
    /// Length; 0 means the span was fully covered.
    nx: u32,
}

/// Visible areas not obstructed by an overlay or a floating pane.
/// C `vendor/tmux/tmux.h:1256`: `struct visible_ranges`
///
/// A growable array rather than a fixed set of slots: each floating pane
/// stacked over a line can split one span into two, so N floats need up to
/// N+1 spans. Zero-initialises correctly, which matters because the copies
/// living on `window_pane` and `tty` come from `xcalloc`.
#[repr(C)]
struct visible_ranges {
    /// Dynamically allocated array of `size` entries.
    ranges: *mut visible_range,
    /// Number of entries in use.
    used: u32,
    /// Allocated capacity.
    size: u32,
}

/// C `vendor/tmux/tmux.h:2083`: `typedef enum prompt_result (*prompt_input_cb)(void *, const char *, enum prompt_key_result)`
type prompt_input_cb =
    Option<unsafe fn(NonNull<c_void>, *const u8, prompt_key_result) -> prompt_result>;
/// C `vendor/tmux/tmux.h:2085`: `typedef enum prompt_result (*status_prompt_input_cb)(struct client *, void *, const char *, enum prompt_key_result)`
type status_prompt_input_cb =
    Option<unsafe fn(*mut client, NonNull<c_void>, *const u8, prompt_key_result) -> prompt_result>;
/// C `vendor/tmux/tmux.h:2087`: `typedef enum prompt_result (*mode_tree_prompt_input_cb)(struct client *, void *, const char *, enum prompt_key_result)`
type mode_tree_prompt_input_cb =
    Option<unsafe fn(*mut client, NonNull<c_void>, *const u8, prompt_key_result) -> prompt_result>;
/// C `vendor/tmux/tmux.h:2089`: `typedef void (*prompt_free_cb)(void *)`
type prompt_free_cb = Option<unsafe fn(NonNull<c_void>)>;

/// A prompt owned by a pane rather than by the status line.
/// C `vendor/tmux/window.c:82`: `struct window_pane_prompt`.
///
/// The pane is held by id, not by pointer: the input callback can destroy the
/// pane, so the free callback has to re-find it to know whether the pane it was
/// attached to still exists.
#[repr(C)]
struct window_pane_prompt {
    wp_id: c_uint,
    c: *mut client,
    inputcb: status_prompt_input_cb,
    freecb: prompt_free_cb,
    data: *mut c_void,
}

type overlay_check_cb =
    Option<unsafe fn(*mut client, *mut c_void, u32, u32, u32) -> *mut visible_ranges>;
type overlay_mode_cb =
    Option<unsafe fn(*mut client, *mut c_void, *mut u32, *mut u32) -> *mut screen>;
type overlay_draw_cb = Option<unsafe fn(*mut client, *mut c_void, *mut screen_redraw_ctx)>;
type overlay_key_cb = Option<unsafe fn(*mut client, *mut c_void, *mut key_event) -> i32>;
type overlay_free_cb = Option<unsafe fn(*mut client, *mut c_void)>;
type overlay_resize_cb = Option<unsafe fn(*mut client, *mut c_void)>;

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq)]
    struct client_flag: u64 {
        const TERMINAL           = 0x0000000001u64;
        const LOGIN              = 0x0000000002u64;
        const EXIT               = 0x0000000004u64;
        const REDRAWWINDOW       = 0x0000000008u64;
        const REDRAWSTATUS       = 0x0000000010u64;
        const REPEAT             = 0x0000000020u64;
        const SUSPENDED          = 0x0000000040u64;
        const ATTACHED           = 0x0000000080u64;
        const EXITED             = 0x0000000100u64;
        const DEAD               = 0x0000000200u64;
        const REDRAWBORDERS      = 0x0000000400u64;
        const READONLY           = 0x0000000800u64;
        const NOSTARTSERVER      = 0x0000001000u64;
        const CONTROL            = 0x0000002000u64;
        const CONTROLCONTROL     = 0x0000004000u64;
        const FOCUSED            = 0x0000008000u64;
        const UTF8               = 0x0000010000u64;
        const IGNORESIZE         = 0x0000020000u64;
        const IDENTIFIED         = 0x0000040000u64;
        const STATUSFORCE        = 0x0000080000u64;
        const DOUBLECLICK        = 0x0000100000u64;
        const TRIPLECLICK        = 0x0000200000u64;
        const SIZECHANGED        = 0x0000400000u64;
        const STATUSOFF          = 0x0000800000u64;
        const REDRAWSTATUSALWAYS = 0x0001000000u64;
        const REDRAWOVERLAY      = 0x0002000000u64;
        const CONTROL_NOOUTPUT   = 0x0004000000u64;
        const DEFAULTSOCKET      = 0x0008000000u64;
        const STARTSERVER        = 0x0010000000u64;
        const REDRAWPANES        = 0x0020000000u64;
        const NOFORK             = 0x0040000000u64;
        const ACTIVEPANE         = 0x0080000000u64;
        const CONTROL_PAUSEAFTER = 0x0100000000u64;
        const CONTROL_WAITEXIT   = 0x0200000000u64;
        const WINDOWSIZECHANGED  = 0x0400000000u64;
        const CLIPBOARDBUFFER    = 0x0800000000u64;
        const BRACKETPASTING     = 0x1000000000u64;
    }
}

const CLIENT_ALLREDRAWFLAGS: client_flag = client_flag::REDRAWWINDOW
    .union(client_flag::REDRAWSTATUS)
    .union(client_flag::REDRAWSTATUSALWAYS)
    .union(client_flag::REDRAWBORDERS)
    .union(client_flag::REDRAWOVERLAY)
    .union(client_flag::REDRAWPANES);
const CLIENT_UNATTACHEDFLAGS: client_flag = client_flag::DEAD
    .union(client_flag::SUSPENDED)
    .union(client_flag::EXIT);
const CLIENT_NODETACHFLAGS: client_flag = client_flag::DEAD.union(client_flag::EXIT);
const CLIENT_NOSIZEFLAGS: client_flag = client_flag::DEAD
    .union(client_flag::SUSPENDED)
    .union(client_flag::EXIT);

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Copy, Clone, Default, Eq, PartialEq)]
    /// C `vendor/tmux/tmux.h:2092`.
    struct prompt_flags: u32 {
        const PROMPT_SINGLE = 0x1;
        const PROMPT_NUMERIC = 0x2;
        const PROMPT_INCREMENTAL = 0x4;
        const PROMPT_NOFORMAT = 0x8;
        const PROMPT_KEY = 0x10;
        const PROMPT_ACCEPT = 0x20;
        const PROMPT_QUOTENEXT = 0x40;
        const PROMPT_BSPACE_EXIT = 0x80;
        const PROMPT_NOFREEZE = 0x100;
        const PROMPT_COMMANDMODE = 0x200;
        const PROMPT_ISPANE = 0x400;
        const PROMPT_ISMODE = 0x800;
        const PROMPT_EDITARROWS = 0x1000;
    }
}

/// Prompt create data. C `vendor/tmux/tmux.h:2107`: `struct prompt_create_data`.
///
/// The argument block `prompt_create` reads: everything the caller resolves up
/// front (styles, cursors, key mode, word separators) so a later `set-option`
/// cannot change a prompt that is already open.
#[repr(C)]
struct prompt_create_data {
    fs: *mut cmd_find_state,
    prompt: *const u8,
    input: *const u8,
    type_: prompt_type,
    flags: prompt_flags,

    style: grid_cell,
    command_style: grid_cell,
    cstyle: screen_cursor_style,
    command_cstyle: screen_cursor_style,
    ccolour: c_int,
    command_ccolour: c_int,
    cmode: mode_flag,
    command_cmode: mode_flag,
    message_format: *const u8,
    keys: c_int,
    word_separators: *const u8,

    inputcb: prompt_input_cb,
    freecb: prompt_free_cb,
    data: *mut c_void,
}

/// Prompt draw data. C `vendor/tmux/tmux.h:2132`: `struct prompt_draw_data`.
///
/// Where the host wants the prompt drawn (a row and an x range inside some
/// screen) and where it wants the resulting cursor column written back.
#[repr(C)]
struct prompt_draw_data {
    ctx: *mut screen_write_ctx,
    cursor_x: *mut c_uint,

    area_x: c_uint,
    area_width: c_uint,
    prompt_line: c_uint,
}

impl_tailq_entry!(client, entry, tailq_entry<client>);
#[repr(C)]
struct client {
    name: *const u8,
    peer: *mut tmuxpeer,
    queue: *mut cmdq_list,

    windows: client_windows,

    control_state: *mut control_state,
    pause_age: c_uint,

    pid: pid_t,
    fd: c_int,
    out_fd: c_int,
    event: event,
    retval: c_int,

    creation_time: timeval,
    activity_time: timeval,

    environ: *mut environ,
    jobs: *mut format_job_tree,

    title: Option<std::ffi::CString>,
    path: Option<std::ffi::CString>,
    cwd: Option<std::ffi::CString>,
    /// C `vendor/tmux/tmux.h:2179`: the progress bar last written to this
    /// client's terminal, so a redraw only re-emits `Spb` when it changed.
    progress_bar: progress_bar,

    term_name: Option<std::ffi::CString>,
    term_features: c_int,
    term_type: Option<std::ffi::CString>,
    term_caps: *mut *mut u8,
    term_ncaps: c_uint,

    ttyname: Option<std::ffi::CString>,
    tty: tty,

    written: usize,
    discarded: usize,
    redraw: usize,

    repeat_timer: event,

    click_timer: event,
    click_button: c_uint,
    click_event: mouse_event,

    status: status_line,

    /// C `vendor/tmux/tmux.h:1855`: `struct redraw_scene *redraw_scene`
    ///
    /// Cached composite of the window's visible cells. Rebuilt when the
    /// window, its generation, or the visible offset/size changes; freed with
    /// `redraw_free_scene`.
    redraw_scene: *mut redraw_scene,

    flags: client_flag,

    exit_type: exit_type,
    exit_msgtype: msgtype,
    exit_session: Option<std::ffi::CString>,
    exit_message: Option<std::ffi::CString>,

    keytable: *mut key_table,

    redraw_panes: u64,

    message_ignore_keys: c_int,
    message_ignore_styles: c_int,
    message_string: Option<std::ffi::CString>,
    message_timer: event,

    /// C `vendor/tmux/tmux.h:1353`: `struct prompt *prompt`.
    ///
    /// next-3.7 split the prompt out of the client into its own object
    /// (`prompt.c`), so the client keeps a single pointer instead of the
    /// nineteen `prompt_*` fields the pre-split design spread across it. NULL
    /// when no prompt is open; owned by the client and freed in
    /// `status_prompt_clear`.
    prompt: *mut prompt,

    session: *mut session,
    last_session: *mut session,

    references: c_int,

    /// What the terminal's background says it is (`tmux.h:2205`), learned from
    /// the OSC 11 reply. Drives `#{client_theme}`, the client-light-theme /
    /// client-dark-theme hooks, and which half of the palette options is read.
    theme: client_theme,
    theme_colours: [c_int; COLOUR_THEME_COUNT],

    pan_window: *mut c_void,
    pan_ox: c_uint,
    pan_oy: c_uint,

    overlay_check: overlay_check_cb,
    overlay_mode: overlay_mode_cb,
    overlay_draw: overlay_draw_cb,
    overlay_key: overlay_key_cb,
    overlay_free: overlay_free_cb,
    overlay_resize: overlay_resize_cb,
    overlay_data: *mut c_void,
    overlay_timer: event,

    files: client_files,

    clipboard_panes: *mut c_uint,
    clipboard_npanes: c_uint,

    // #[entry]
    entry: tailq_entry<client>,
}
type clients = tailq_head<client>;

/// Control mode subscription type.
#[repr(i32)]
enum control_sub_type {
    CONTROL_SUB_SESSION,
    CONTROL_SUB_PANE,
    CONTROL_SUB_ALL_PANES,
    CONTROL_SUB_WINDOW,
    CONTROL_SUB_ALL_WINDOWS,
}

const KEY_BINDING_REPEAT: i32 = 0x1;

/// Key binding and key table.
#[repr(C)]
struct key_binding {
    key: key_code,
    cmdlist: *mut cmd_list,
    /// Owned note text, `None` when unset; drops with the boxed binding.
    note: Option<std::ffi::CString>,
    /// The name of the table this binding lives in (`tmux.h:2332`). The C
    /// borrows `table->name`, which outlives every binding in it; `list-keys`
    /// reads it for `#{key_table}` and the table-column width, and
    /// `sort_key_binding_cmp` sorts on it.
    tablename: *const u8,

    flags: i32,

    entry: rb_entry<key_binding>,
}
type key_bindings = rb_head<key_binding>;

#[repr(C)]
struct key_table {
    /// Owned table name; drops with the boxed table — no manual `free()`.
    name: std::ffi::CString,
    activity_time: timeval,
    key_bindings: key_bindings,
    default_key_bindings: key_bindings,

    references: u32,

    entry: rb_entry<key_table>,
}
type key_tables = rb_head<key_table>;

// Option data.
type options_array = rb_head<options_array_item>;

#[repr(C)]
#[derive(Copy, Clone)]
union options_value {
    string: *mut u8,
    number: c_longlong,
    style: style,
    array: options_array,
    cmdlist: *mut cmd_list,
}

// Option table entries.
#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum options_table_type {
    OPTIONS_TABLE_STRING,
    OPTIONS_TABLE_NUMBER,
    OPTIONS_TABLE_KEY,
    OPTIONS_TABLE_COLOUR,
    OPTIONS_TABLE_FLAG,
    OPTIONS_TABLE_CHOICE,
    OPTIONS_TABLE_COMMAND,
}

const OPTIONS_TABLE_NONE: i32 = 0;
const OPTIONS_TABLE_SERVER: i32 = 0x1;
const OPTIONS_TABLE_SESSION: i32 = 0x2;
const OPTIONS_TABLE_WINDOW: i32 = 0x4;
const OPTIONS_TABLE_PANE: i32 = 0x8;

const OPTIONS_TABLE_IS_ARRAY: i32 = 0x1;
const OPTIONS_TABLE_IS_HOOK: i32 = 0x2;
const OPTIONS_TABLE_IS_STYLE: i32 = 0x4;
const OPTIONS_TABLE_IS_COLOUR: i32 = 0x8;

unsafe impl Sync for options_table_entry {}

#[repr(C)]
struct options_table_entry {
    name: &'static str,
    alternative_name: *mut u8,
    type_: options_table_type,
    scope: i32,
    flags: i32,
    minimum: u32,
    maximum: u32,

    choices: &'static [&'static str],

    default_str: Option<&'static str>,
    default_num: c_longlong,
    default_arr: *const *const u8,

    separator: *const u8,
    pattern: *const u8,

    text: *const u8,
    unit: *const u8,
}

impl options_table_entry {
    pub const fn const_default() -> Self {
        Self {
            name: "",
            alternative_name: null_mut(),
            type_: options_table_type::OPTIONS_TABLE_STRING,
            scope: 0,
            flags: 0,
            minimum: 0,
            maximum: 0,
            choices: &[],
            default_str: None,
            default_num: 0,
            default_arr: null(),
            separator: null(),
            pattern: null(),
            text: null(),
            unit: null(),
        }
    }
}

#[repr(C)]
struct options_name_map_str {
    from: &'static str,
    to: &'static str,
}
impl options_name_map_str {
    const fn new(from: &'static str, to: &'static str) -> Self {
        Self { from, to }
    }
}

#[repr(C)]
struct options_name_map {
    from: &'static str,
    to: &'static str,
}
impl options_name_map {
    const fn new(from: &'static str, to: &'static str) -> Self {
        Self { from, to }
    }
}

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq)]
    struct spawn_flags: i32 {
        const SPAWN_KILL = 0x1;
        const SPAWN_DETACHED = 0x2;
        const SPAWN_RESPAWN = 0x4;
        const SPAWN_BEFORE = 0x8;
        const SPAWN_NONOTIFY = 0x10;
        const SPAWN_FULLSIZE = 0x20;
        const SPAWN_EMPTY = 0x40;
        const SPAWN_ZOOM = 0x80;
        /// C `vendor/tmux/tmux.h:2451`: `#define SPAWN_FLOATING 0x100`.
        const SPAWN_FLOATING = 0x100;
    }
}

// TODO inline these and remove the definitions
const SPAWN_KILL: spawn_flags = spawn_flags::SPAWN_KILL;
const SPAWN_DETACHED: spawn_flags = spawn_flags::SPAWN_DETACHED;
const SPAWN_RESPAWN: spawn_flags = spawn_flags::SPAWN_RESPAWN;
const SPAWN_BEFORE: spawn_flags = spawn_flags::SPAWN_BEFORE;
const SPAWN_NONOTIFY: spawn_flags = spawn_flags::SPAWN_NONOTIFY;
const SPAWN_FULLSIZE: spawn_flags = spawn_flags::SPAWN_FULLSIZE;
const SPAWN_EMPTY: spawn_flags = spawn_flags::SPAWN_EMPTY;
const SPAWN_ZOOM: spawn_flags = spawn_flags::SPAWN_ZOOM;
const SPAWN_FLOATING: spawn_flags = spawn_flags::SPAWN_FLOATING;

/// Spawn common context.
#[repr(C)]
struct spawn_context {
    item: *mut cmdq_item,

    s: *mut session,
    wl: *mut winlink,
    tc: *mut client,

    wp0: *mut window_pane,
    lc: *mut layout_cell,

    name: *const u8,
    argv: *mut *mut u8,
    argc: i32,
    environ: *mut environ,

    idx: i32,
    cwd: *const u8,

    flags: spawn_flags,
}

/// Mode tree sort order.
#[repr(C)]
#[derive(Default)]
struct mode_tree_sort_criteria {
    field: u32,
    reversed: bool,
}

const WINDOW_MINIMUM: u32 = PANE_MINIMUM;
const WINDOW_MAXIMUM: u32 = 10_000;

#[repr(i32)]
enum exit_type {
    #[expect(dead_code)]
    CLIENT_EXIT_RETURN,
    CLIENT_EXIT_SHUTDOWN,
    CLIENT_EXIT_DETACH,
}

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Copy, Clone, Default, Eq, PartialEq)]
    struct job_flag: i32 {
        const JOB_NOWAIT = 1;
        const JOB_KEEPWRITE = 2;
        const JOB_PTY = 4;
        const JOB_DEFAULTSHELL = 8;
        // vendor/tmux/tmux.h:2728. Set by `run-shell -E`; makes the child's
        // stderr go to the job socket with its stdout instead of /dev/null.
        const JOB_SHOWSTDERR = 0x10;
    }
}

// unsafe fn args_get(_: *mut args, _: c_uchar) -> *const c_char;
unsafe fn args_get_(args: *mut args, flag: char) -> *const u8 {
    debug_assert!(flag.is_ascii());
    unsafe { args_get(args, flag as u8) }
}

unsafe impl Sync for SyncCharPtr {}
#[repr(transparent)]
#[derive(Copy, Clone, Default)]
struct SyncCharPtr(*const u8);
impl SyncCharPtr {
    const fn new(value: &'static CStr) -> Self {
        Self(value.as_ptr().cast())
    }
    const fn from_ptr(value: *const u8) -> Self {
        Self(value)
    }
    const fn null() -> Self {
        Self(null())
    }
    const fn as_ptr(&self) -> *const u8 {
        self.0
    }
}

unsafe fn _s(ptr: impl ToU8Ptr) -> DisplayCStrPtr {
    DisplayCStrPtr(ptr.to_u8_ptr())
}
trait ToU8Ptr {
    fn to_u8_ptr(self) -> *const u8;
}
impl ToU8Ptr for *mut u8 {
    fn to_u8_ptr(self) -> *const u8 {
        self.cast()
    }
}
impl ToU8Ptr for *const u8 {
    fn to_u8_ptr(self) -> *const u8 {
        self
    }
}
impl ToU8Ptr for *mut i8 {
    fn to_u8_ptr(self) -> *const u8 {
        self.cast()
    }
}
impl ToU8Ptr for *const i8 {
    fn to_u8_ptr(self) -> *const u8 {
        self.cast()
    }
}
impl ToU8Ptr for SyncCharPtr {
    fn to_u8_ptr(self) -> *const u8 {
        self.as_ptr()
    }
}
// TODO struct should have some sort of lifetime
/// Display wrapper for a *`c_char` pointer
#[repr(transparent)]
struct DisplayCStrPtr(*const u8);
impl std::fmt::Display for DisplayCStrPtr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_null() {
            return f.write_str("(null)");
        }

        // TODO alignment

        let len = if let Some(width) = f.precision() {
            unsafe { libc::strnlen(self.0, width) }
        } else if let Some(width) = f.width() {
            unsafe { libc::strnlen(self.0, width) }
        } else {
            unsafe { libc::strlen(self.0) }
        };

        let s: &[u8] = unsafe { std::slice::from_raw_parts(self.0, len) };
        let s = std::str::from_utf8(s).unwrap_or("%s-invalid-utf8");
        f.write_str(s)
    }
}

// TOOD make usable in const context
// https://stackoverflow.com/a/63904992
macro_rules! function_name {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);

        // Find and cut the rest of the path
        match &name[..name.len() - 3].rfind(':') {
            Some(pos) => &name[pos + 1..name.len() - 3],
            None => &name[..name.len() - 3],
        }
    }};
}
pub(crate) use function_name;

const fn concat_array<const N: usize, const M: usize, const O: usize, T: Copy>(
    a1: [T; N],
    a2: [T; M],
) -> [T; O] {
    let mut out: [MaybeUninit<T>; O] = [MaybeUninit::uninit(); O];

    let mut i: usize = 0;
    while i < a1.len() {
        out[i].write(a1[i]);
        i += 1;
    }
    while i < a1.len() + a2.len() {
        out[i].write(a2[i - a1.len()]);
        i += 1;
    }

    assert!(a1.len() + a2.len() == out.len());
    assert!(i == out.len());

    unsafe { std::mem::transmute_copy(&out) }
    // TODO once stabilized switch to:
    // unsafe { MaybeUninit::array_assume_init(out) }
}

pub(crate) fn i32_to_ordering(value: i32) -> std::cmp::Ordering {
    match value {
        ..0 => std::cmp::Ordering::Less,
        0 => std::cmp::Ordering::Equal,
        1.. => std::cmp::Ordering::Greater,
    }
}

pub(crate) unsafe fn cstr_to_str<'a>(ptr: *const u8) -> &'a str {
    unsafe { cstr_to_str_(ptr).unwrap() }
}

pub(crate) unsafe fn cstr_to_str_<'a>(ptr: *const u8) -> Option<&'a str> {
    unsafe {
        if ptr.is_null() {
            return None;
        }
        let len = libc::strlen(ptr);

        let bytes = std::slice::from_raw_parts(ptr.cast::<u8>(), len);

        // A fallible conversion must return None on invalid UTF-8, not panic —
        // callers use this variant precisely to handle non-UTF-8 C strings (e.g.
        // format keys / values that carry arbitrary bytes) gracefully.
        std::str::from_utf8(bytes).ok()
    }
}

// ideally we could just use c string literal until we transition to &str everywhere
// unfortunately, some platforms people use have
macro_rules! c {
    ($s:literal) => {{
        const S: &str = concat!($s, "\0");
        #[allow(clippy::allow_attributes)]
        #[allow(
            unused_unsafe,
            reason = "this macro should work in safe and unsafe blocks"
        )]
        unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(S.as_bytes()) }
            .as_ptr()
            .cast::<u8>()
    }};
}
pub(crate) use c;

macro_rules! impl_ord {
    ($ty:ty as $func:ident) => {
        impl Ord for $ty {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                $func(&self, &other)
            }
        }
        impl PartialEq for $ty {
            fn eq(&self, other: &Self) -> bool {
                self.cmp(other).is_eq()
            }
        }
        impl Eq for $ty {}
        impl PartialOrd for $ty {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
    };
}
pub(crate) use impl_ord;

macro_rules! const_unwrap_result {
    ($e:expr) => {
        match $e {
            Ok(value) => value,
            _ => panic!("const_unwrap_result"),
        }
    };
}
pub(crate) use const_unwrap_result;

macro_rules! cstring_concat {
    ($($e:expr),* $(,)?) => {
        const_unwrap_result!(::core::ffi::CStr::from_bytes_with_nul(concat!($($e),*, "\0").as_bytes()))
    };
}
pub(crate) use cstring_concat;

trait Reverseable {
    fn maybe_reverse(self, reversed: bool) -> Self;
}
impl Reverseable for cmp::Ordering {
    fn maybe_reverse(self, reversed: bool) -> Self {
        if reversed { self.reverse() } else { self }
    }
}
