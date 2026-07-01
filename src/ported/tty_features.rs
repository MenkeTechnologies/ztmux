// Copyright (c) 2020 Nicholas Marriott <nicholas.marriott@gmail.com>
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

unsafe impl Sync for tty_feature {}
#[repr(C)]
struct tty_feature {
    name: &'static str,
    capabilities: &'static [&'static str],
    flags: term_flags,
}
impl tty_feature {
    const fn new(
        name: &'static str,
        capabilities: &'static [&'static str],
        flags: term_flags,
    ) -> Self {
        Self {
            name,
            capabilities,
            flags,
        }
    }
}

static TTY_FEATURE_TITLE_CAPABILITIES: &[&str] = &[
    "tsl=\\E]0;", // should be using TS really
    "fsl=\\a",
];
static TTY_FEATURE_TITLE: tty_feature =
    tty_feature::new("title", TTY_FEATURE_TITLE_CAPABILITIES, term_flags::empty());

/// Terminal has OSC 7 working directory.
static TTY_FEATURE_OSC7_CAPABILITIES: &[&str] = &["Swd=\\E]7;", "fsl=\\a"];
static TTY_FEATURE_OSC7: tty_feature =
    tty_feature::new("osc7", TTY_FEATURE_OSC7_CAPABILITIES, term_flags::empty());

/// Terminal has mouse support.
static TTY_FEATURE_MOUSE_CAPABILITIES: &[&str] = &["kmous=\\E[M"];
static TTY_FEATURE_MOUSE: tty_feature =
    tty_feature::new("mouse", TTY_FEATURE_MOUSE_CAPABILITIES, term_flags::empty());

/// Terminal can set the clipboard with OSC 52.
static TTY_FEATURE_CLIPBOARD_CAPABILITIES: &[&str] = &["Ms=\\E]52;%p1%s;%p2%s\\a"];
static TTY_FEATURE_CLIPBOARD: tty_feature = tty_feature::new(
    "clipboard",
    TTY_FEATURE_CLIPBOARD_CAPABILITIES,
    term_flags::empty(),
);

// #if defined (__OpenBSD__) || (defined(NCURSES_VERSION_MAJOR) && (NCURSES_VERSION_MAJOR > 5 ||  (NCURSES_VERSION_MAJOR == 5 && NCURSES_VERSION_MINOR > 8)))

/// Terminal supports OSC 8 hyperlinks.
#[cfg(feature = "hyperlinks")]
static TTY_FEATURE_HYPERLINKS_CAPABILITIES: &[&str] =
    &["*:Hls=\\E]8;%?%p1%l%tid=%p1%s%;;%p2%s\\E\\\\"];
#[cfg(not(feature = "hyperlinks"))]
static TTY_FEATURE_HYPERLINKS_CAPABILITIES: &[&str] = &[];
static TTY_FEATURE_HYPERLINKS: tty_feature = tty_feature::new(
    "hyperlinks",
    TTY_FEATURE_HYPERLINKS_CAPABILITIES,
    term_flags::empty(),
);

/// Terminal supports RGB colour. This replaces setab and setaf also since
/// terminals with RGB have versions that do not allow setting colours from the
/// 256 palette.
static TTY_FEATURE_RGB_CAPABILITIES: &[&str] = &[
    "AX",
    "setrgbf=\\E[38;2;%p1%d;%p2%d;%p3%dm",
    "setrgbb=\\E[48;2;%p1%d;%p2%d;%p3%dm",
    "setab=\\E[%?%p1%{8}%<%t4%p1%d%e%p1%{16}%<%t10%p1%{8}%-%d%e48;5;%p1%d%;m",
    "setaf=\\E[%?%p1%{8}%<%t3%p1%d%e%p1%{16}%<%t9%p1%{8}%-%d%e38;5;%p1%d%;m",
];
static TTY_FEATURE_RGB: tty_feature = tty_feature::new(
    "RGB",
    TTY_FEATURE_RGB_CAPABILITIES,
    term_flags::TERM_256COLOURS.union(term_flags::TERM_RGBCOLOURS),
);

/// Terminal supports 256 colours.
static TTY_FEATURE_256_CAPABILITIES: &[&str] = &[
    "AX",
    "setab=\\E[%?%p1%{8}%<%t4%p1%d%e%p1%{16}%<%t10%p1%{8}%-%d%e48;5;%p1%d%;m",
    "setaf=\\E[%?%p1%{8}%<%t3%p1%d%e%p1%{16}%<%t9%p1%{8}%-%d%e38;5;%p1%d%;m",
];
static TTY_FEATURE_256: tty_feature = tty_feature::new(
    "256",
    TTY_FEATURE_256_CAPABILITIES,
    term_flags::TERM_256COLOURS,
);

/// Terminal supports overline.
static TTY_FEATURE_OVERLINE_CAPABILITIES: &[&str] = &["Smol=\\E[53m"];
static TTY_FEATURE_OVERLINE: tty_feature = tty_feature::new(
    "overline",
    TTY_FEATURE_OVERLINE_CAPABILITIES,
    term_flags::empty(),
);

/// Terminal supports underscore styles.
static TTY_FEATURE_USSTYLE_CAPABILITIES: &[&str] = &[
    "Smulx=\\E[4::%p1%dm",
    "Setulc=\\E[58::2::%p1%{65536}%/%d::%p1%{256}%/%{255}%&%d::%p1%{255}%&%d%;m",
    "Setulc1=\\E[58::5::%p1%dm",
    "ol=\\E[59m",
];
static TTY_FEATURE_USSTYLE: tty_feature = tty_feature::new(
    "usstyle",
    TTY_FEATURE_USSTYLE_CAPABILITIES,
    term_flags::empty(),
);

/// Terminal supports bracketed paste.
static TTY_FEATURE_BPASTE_CAPABILITIES: &[&str] = &["Enbp=\\E[?2004h", "Dsbp=\\E[?2004l"];
static TTY_FEATURE_BPASTE: tty_feature = tty_feature::new(
    "bpaste",
    TTY_FEATURE_BPASTE_CAPABILITIES,
    term_flags::empty(),
);

/// Terminal supports focus reporting.
static TTY_FEATURE_FOCUS_CAPABILITIES: &[&str] = &["Enfcs=\\E[?1004h", "Dsfcs=\\E[?1004l"];
static TTY_FEATURE_FOCUS: tty_feature =
    tty_feature::new("focus", TTY_FEATURE_FOCUS_CAPABILITIES, term_flags::empty());

/// Terminal supports cursor styles.
static TTY_FEATURE_CSTYLE_CAPABILITIES: &[&str] = &["Ss=\\E[%p1%d q", "Se=\\E[2 q"];
static TTY_FEATURE_CSTYLE: tty_feature = tty_feature::new(
    "cstyle",
    TTY_FEATURE_CSTYLE_CAPABILITIES,
    term_flags::empty(),
);

/// Terminal supports cursor colours.
static TTY_FEATURE_CCOLOUR_CAPABILITIES: &[&str] = &["Cs=\\E]12;%p1%s\\a", "Cr=\\E]112\\a"];
static TTY_FEATURE_CCOLOUR: tty_feature = tty_feature::new(
    "ccolour",
    TTY_FEATURE_CCOLOUR_CAPABILITIES,
    term_flags::empty(),
);

/// Terminal supports strikethrough.
static TTY_FEATURE_STRIKETHROUGH_CAPABILITIES: &[&str] = &["smxx=\\E[9m"];
static TTY_FEATURE_STRIKETHROUGH: tty_feature = tty_feature::new(
    "strikethrough",
    TTY_FEATURE_STRIKETHROUGH_CAPABILITIES,
    term_flags::empty(),
);

/// Terminal supports synchronized updates.
static TTY_FEATURE_SYNC_CAPABILITIES: &[&str] = &["Sync=\\E[?2026%?%p1%{1}%-%tl%eh%;"];
static TTY_FEATURE_SYNC: tty_feature =
    tty_feature::new("sync", TTY_FEATURE_SYNC_CAPABILITIES, term_flags::empty());

/// Terminal supports extended keys.
static TTY_FEATURE_EXTKEYS_CAPABILITIES: &[&str] = &["Eneks=\\E[>4;2m", "Dseks=\\E[>4m"];
static TTY_FEATURE_EXTKEYS: tty_feature = tty_feature::new(
    "extkeys",
    TTY_FEATURE_EXTKEYS_CAPABILITIES,
    term_flags::empty(),
);

/// Terminal supports DECSLRM margins.
static TTY_FEATURE_MARGINS_CAPABILITIES: &[&str] = &[
    "Enmg=\\E[?69h",
    "Dsmg=\\E[?69l",
    "Clmg=\\E[s",
    "Cmg=\\E[%i%p1%d;%p2%ds",
];
static TTY_FEATURE_MARGINS: tty_feature = tty_feature::new(
    "margins",
    TTY_FEATURE_MARGINS_CAPABILITIES,
    term_flags::TERM_DECSLRM,
);

/// Terminal supports DECFRA rectangle fill.
static TTY_FEATURE_RECTFILL_CAPABILITIES: &[&str] = &["Rect"];
static TTY_FEATURE_RECTFILL: tty_feature = tty_feature::new(
    "rectfill",
    TTY_FEATURE_RECTFILL_CAPABILITIES,
    term_flags::TERM_DECFRA,
);

/// Use builtin function keys only.
static TTY_FEATURE_IGNOREFKEYS_CAPABILITIES: &[&str] = &[
    "kf0@", "kf1@", "kf2@", "kf3@", "kf4@", "kf5@", "kf6@", "kf7@", "kf8@", "kf9@", "kf10@",
    "kf11@", "kf12@", "kf13@", "kf14@", "kf15@", "kf16@", "kf17@", "kf18@", "kf19@", "kf20@",
    "kf21@", "kf22@", "kf23@", "kf24@", "kf25@", "kf26@", "kf27@", "kf28@", "kf29@", "kf30@",
    "kf31@", "kf32@", "kf33@", "kf34@", "kf35@", "kf36@", "kf37@", "kf38@", "kf39@", "kf40@",
    "kf41@", "kf42@", "kf43@", "kf44@", "kf45@", "kf46@", "kf47@", "kf48@", "kf49@", "kf50@",
    "kf51@", "kf52@", "kf53@", "kf54@", "kf55@", "kf56@", "kf57@", "kf58@", "kf59@", "kf60@",
    "kf61@", "kf62@", "kf63@",
];

static TTY_FEATURE_IGNOREFKEYS: tty_feature = tty_feature::new(
    "ignorefkeys",
    TTY_FEATURE_IGNOREFKEYS_CAPABILITIES,
    term_flags::empty(),
);

/// Terminal has sixel capability.
static TTY_FEATURE_SIXEL_CAPABILITIES: &[&str] = &["Sxl"];
static TTY_FEATURE_SIXEL: tty_feature = tty_feature::new(
    "sixel",
    TTY_FEATURE_SIXEL_CAPABILITIES,
    term_flags::TERM_SIXEL,
);

/// Available terminal features.
static TTY_FEATURES: [&tty_feature; 20] = [
    &TTY_FEATURE_256,
    &TTY_FEATURE_BPASTE,
    &TTY_FEATURE_CCOLOUR,
    &TTY_FEATURE_CLIPBOARD,
    &TTY_FEATURE_HYPERLINKS,
    &TTY_FEATURE_CSTYLE,
    &TTY_FEATURE_EXTKEYS,
    &TTY_FEATURE_FOCUS,
    &TTY_FEATURE_IGNOREFKEYS,
    &TTY_FEATURE_MARGINS,
    &TTY_FEATURE_MOUSE,
    &TTY_FEATURE_OSC7,
    &TTY_FEATURE_OVERLINE,
    &TTY_FEATURE_RECTFILL,
    &TTY_FEATURE_RGB,
    &TTY_FEATURE_SIXEL,
    &TTY_FEATURE_STRIKETHROUGH,
    &TTY_FEATURE_SYNC,
    &TTY_FEATURE_TITLE,
    &TTY_FEATURE_USSTYLE,
];

/// C `vendor/tmux/tty-features.c:397`: `void tty_add_features(int *feat, const char *s, const char *separators)`
pub unsafe fn tty_add_features(feat: *mut i32, s: &str, separators: *const u8) {
    unsafe {
        log_debug!("adding terminal features {}", s);

        let copy = xstrdup__(s);
        let mut loop_ = copy;
        let mut next;

        while {
            next = strsep(&raw mut loop_, separators);
            !next.is_null()
        } {
            let Some(i) = TTY_FEATURES
                .iter()
                .position(|tf| libc::strcaseeq_(next, tf.name))
            else {
                log_debug!("unknown terminal feature: {}", _s(next));
                break;
            };

            let tf = TTY_FEATURES[i];
            if !(*feat) & (1 << i) != 0 {
                log_debug!("adding terminal feature: {}", tf.name);
                (*feat) |= 1 << i;
            }
        }
        free_(copy);
    }
}

/// C `vendor/tmux/tty-features.c:425`: `const char *tty_get_features(int feat)`
pub unsafe fn tty_get_features(feat: i32) -> *const u8 {
    static mut S_BUF: [MaybeUninit<u8>; 512] = [MaybeUninit::uninit(); 512];
    unsafe {
        let s: *mut u8 = (&raw mut S_BUF).cast();
        // const struct tty_feature *tf;

        *s = b'\0';
        for (i, tf) in TTY_FEATURES.iter().copied().enumerate() {
            if (!feat & (1 << i)) != 0 {
                continue;
            }

            strlcat_(s, tf.name, 512);
            strlcat_(s, ",", 512);
        }
        if *s != b'\0' {
            *s.add(strlen(s) - 1) = b'\0';
        }

        s
    }
}

/// C `vendor/tmux/tty-features.c:485`: `int tty_apply_features(struct tty_term *term, int feat)`
pub unsafe fn tty_apply_features(term: *mut tty_term, feat: i32) -> bool {
    if feat == 0 {
        return false;
    }

    unsafe {
        log_debug!("applying terminal features: {}", _s(tty_get_features(feat)));

        for (i, tf) in TTY_FEATURES.iter().copied().enumerate() {
            if ((*term).features & (1 << i) != 0) || (!feat & (1 << i)) != 0 {
                continue;
            }

            log_debug!("applying terminal feature: {}", tf.name);
            for capability in tf.capabilities {
                log_debug!("adding capability: {}", capability);
                tty_term_apply(term, capability, 1);
            }
            (*term).flags |= tf.flags;
        }
        if ((*term).features | feat) == (*term).features {
            return false;
        }
        (*term).features |= feat;
    }

    true
}

/// C `vendor/tmux/tty-features.c:518`: `void tty_default_features(int *feat, const char *name, u_int version)`
pub unsafe fn tty_default_features(feat: *mut i32, name: *const u8, version: u32) {
    struct entry {
        name: &'static CStr,
        version: u32,
        features: &'static str,
    }
    macro_rules! TTY_FEATURES_BASE_MODERN_XTERM {
        () => {
            "256,RGB,bpaste,clipboard,mouse,strikethrough,title"
        };
    }

    // TODO note version isn't init in the C code
    #[rustfmt::skip]
    static TABLE: &[entry] = &[
        entry { name: c"mintty", features: concat!( TTY_FEATURES_BASE_MODERN_XTERM!(), ",ccolour,cstyle,extkeys,margins,overline,usstyle"), version: 0, },
        entry { name: c"tmux", features: concat!( TTY_FEATURES_BASE_MODERN_XTERM!(), ",ccolour,cstyle,focus,overline,usstyle,hyperlinks"), version: 0, },
        entry { name: c"rxvt-unicode", features: "256,bpaste,ccolour,cstyle,mouse,title,ignorefkeys", version: 0, },
        entry { name: c"iTerm2", features: concat!( TTY_FEATURES_BASE_MODERN_XTERM!(), ",cstyle,extkeys,margins,usstyle,sync,osc7,hyperlinks"), version: 0, },
        // xterm also supports DECSLRM and DECFRA, but they can be
        // disabled so not set it here - they will be added if
        // secondary DA shows VT420.
        entry { name: c"XTerm", features: concat!(TTY_FEATURES_BASE_MODERN_XTERM!(), ",ccolour,cstyle,extkeys,focus"), version: 0, },
    ];

    unsafe {
        for e in TABLE {
            if libc::strcmp(e.name.as_ptr().cast(), name) != 0 {
                continue;
            }
            if version != 0 && version < e.version {
                continue;
            }
            tty_add_features(feat, e.features, c!(","));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // `tty_get_features` returns a pointer into a single process-global static
    // buffer (`S_BUF`), so concurrent test threads must serialize their calls
    // and copy the result out while holding the lock.
    static FEATURES_LOCK: Mutex<()> = Mutex::new(());

    // Read the rendered comma-separated feature list into an owned String.
    unsafe fn get_features_string(feat: i32) -> String {
        unsafe {
            let _guard = FEATURES_LOCK.lock().unwrap();
            let ptr = tty_get_features(feat);
            std::ffi::CStr::from_ptr(ptr.cast())
                .to_str()
                .unwrap()
                .to_string()
        }
    }

    // Add features from a &str using "," as the separator, returning the bits.
    unsafe fn add(s: &str) -> i32 {
        unsafe {
            let mut feat: i32 = 0;
            tty_add_features(&raw mut feat, s, c!(","));
            feat
        }
    }

    // The 20 feature names in `TTY_FEATURES` array order (bit index == position).
    // Mirrors vendor/tmux/tty-features.c:372 `tty_features[]` but WITHOUT the
    // "progressbar" entry, which this port omits (see TTY_FEATURES, 20 entries).
    const NAMES: [&str; 20] = [
        "256",           // 0
        "bpaste",        // 1
        "ccolour",       // 2
        "clipboard",     // 3
        "hyperlinks",    // 4
        "cstyle",        // 5
        "extkeys",       // 6
        "focus",         // 7
        "ignorefkeys",   // 8
        "margins",       // 9
        "mouse",         // 10
        "osc7",          // 11
        "overline",      // 12
        "rectfill",      // 13
        "RGB",           // 14
        "sixel",         // 15
        "strikethrough", // 16
        "sync",          // 17
        "title",         // 18
        "usstyle",       // 19
    ];

    // C `tty_get_features`, vendor/tmux/tty-features.c:431: an all-clear feat
    // yields an empty string (`*s = '\0'` and nothing appended).
    #[test]
    fn get_features_empty() {
        unsafe {
            assert_eq!(get_features_string(0), "");
        }
    }

    // Every named feature round-trips: add a single name to a clear feat and
    // `tty_get_features` renders exactly that name back (single set bit).
    #[test]
    fn single_named_feature_roundtrip() {
        unsafe {
            for name in NAMES {
                let feat = add(name);
                assert_ne!(feat, 0, "feature {name} was not recognized");
                assert_eq!(get_features_string(feat), name, "round-trip for {name}");
            }
        }
    }

    // Each name maps to its documented bit index (1 << position in TTY_FEATURES).
    // Cross-checks vendor/tmux/tty-features.c:407-419 which sets `1 << i`.
    #[test]
    fn feature_bit_indices() {
        unsafe {
            for (i, name) in NAMES.iter().enumerate() {
                assert_eq!(add(name), 1 << i, "bit index for {name}");
            }
        }
    }

    // `tty_get_features` renders set bits in TTY_FEATURES array order, NOT the
    // order the caller passed them (vendor/tmux/tty-features.c:432 loops i=0..).
    #[test]
    fn get_features_renders_in_array_order() {
        unsafe {
            // Passed reversed / shuffled; expect array order 256(0),mouse(10),title(18).
            let feat = add("title,mouse,256");
            assert_eq!(get_features_string(feat), "256,mouse,title");
        }
    }

    // Combining several features accumulates the bits.
    #[test]
    fn combining_features() {
        unsafe {
            let feat = add("256,RGB,mouse");
            // array order: 256(0), mouse(10), RGB(14).
            assert_eq!(get_features_string(feat), "256,mouse,RGB");
            assert_eq!(feat, (1 << 0) | (1 << 10) | (1 << 14));
        }
    }

    // All 20 features at once -> the full array-order list; and a fully-set int
    // (bits 0..19) renders the same. Boundary on the top valid bit (19).
    #[test]
    fn all_features_roundtrip() {
        unsafe {
            let full = NAMES.join(",");
            let feat = add(&full);
            assert_eq!(feat, (1 << 20) - 1);

            let expected = NAMES.join(",");
            assert_eq!(get_features_string(feat), expected);
            // Rendering directly from the fully-set bitmask matches too.
            assert_eq!(get_features_string((1 << 20) - 1), expected);
        }
    }

    // Feature matching is case-insensitive (C uses strcasecmp,
    // vendor/tmux/tty-features.c:409); the canonical name is rendered back.
    #[test]
    fn case_insensitive_match() {
        unsafe {
            // Lower-case input matches the "RGB" feature and renders canonically.
            assert_eq!(get_features_string(add("rgb")), "RGB");
            assert_eq!(get_features_string(add("RGB")), "RGB");
            // Upper-case input matches the "mouse" feature.
            assert_eq!(get_features_string(add("MOUSE")), "mouse");
            assert_eq!(get_features_string(add("MoUsE")), "mouse");
        }
    }

    // Adding the same feature twice is idempotent (the `~(*feat) & (1<<i)` guard
    // at vendor/tmux/tty-features.c:416 only sets an unset bit).
    #[test]
    fn adding_is_idempotent() {
        unsafe {
            let mut feat: i32 = 0;
            tty_add_features(&raw mut feat, "mouse", c!(","));
            let after_first = feat;
            tty_add_features(&raw mut feat, "mouse", c!(","));
            assert_eq!(feat, after_first);
            assert_eq!(get_features_string(feat), "mouse");
        }
    }

    // Pre-existing bits are preserved when adding more features.
    #[test]
    fn add_preserves_existing_bits() {
        unsafe {
            let mut feat: i32 = 0;
            tty_add_features(&raw mut feat, "mouse", c!(","));
            tty_add_features(&raw mut feat, "title", c!(","));
            // array order: mouse(10), title(18).
            assert_eq!(get_features_string(feat), "mouse,title");
        }
    }

    // An unknown feature stops the loop: vendor/tmux/tty-features.c:412-414
    // `break`s on the first unrecognized token, so tokens AFTER it are dropped
    // (features BEFORE it are kept).
    #[test]
    fn unknown_feature_breaks_loop() {
        unsafe {
            // "256" is added, then "bogus" breaks, so "mouse" is never reached.
            let feat = add("256,bogus,mouse");
            assert_eq!(get_features_string(feat), "256");
            assert_eq!(feat, 1 << 0);

            // A lone unknown feature yields nothing.
            assert_eq!(add("nosuchfeature"), 0);
            assert_eq!(get_features_string(add("nosuchfeature")), "");
        }
    }

    // A non-"," separator is honoured (strsep on the passed separators),
    // vendor/tmux/tty-features.c:406.
    #[test]
    fn custom_separator() {
        unsafe {
            let mut feat: i32 = 0;
            tty_add_features(&raw mut feat, "mouse:title:256", c!(":"));
            // array order: 256(0), mouse(10), title(18).
            assert_eq!(get_features_string(feat), "256,mouse,title");
        }
    }

    // `tty_default_features` looks up a terminal name in its table and adds that
    // terminal's feature string. XTerm entry (vendor/tmux/tty-features.c:599,
    // port XTerm = base + ",ccolour,cstyle,extkeys,focus").
    #[test]
    fn default_features_xterm() {
        unsafe {
            let mut feat: i32 = 0;
            tty_default_features(&raw mut feat, c!("XTerm"), 0);
            // base "256,RGB,bpaste,clipboard,mouse,strikethrough,title" plus
            // ccolour,cstyle,extkeys,focus -> rendered in array order:
            assert_eq!(
                get_features_string(feat),
                "256,bpaste,ccolour,clipboard,cstyle,extkeys,focus,mouse,RGB,strikethrough,title"
            );
        }
    }

    // rxvt-unicode entry exercises a distinct (non-base) feature list including
    // "ignorefkeys" (vendor/tmux/tty-features.c:547).
    #[test]
    fn default_features_rxvt_unicode() {
        unsafe {
            let mut feat: i32 = 0;
            tty_default_features(&raw mut feat, c!("rxvt-unicode"), 0);
            // "256,bpaste,ccolour,cstyle,mouse,title,ignorefkeys" in array order:
            assert_eq!(
                get_features_string(feat),
                "256,bpaste,ccolour,cstyle,ignorefkeys,mouse,title"
            );
        }
    }

    // An unknown terminal name adds nothing (no table entry matches).
    #[test]
    fn default_features_unknown_terminal() {
        unsafe {
            let mut feat: i32 = 0;
            tty_default_features(&raw mut feat, c!("NoSuchTerminal"), 0);
            assert_eq!(feat, 0);
            assert_eq!(get_features_string(feat), "");
        }
    }

    // mintty entry (vendor/tmux/tty-features.c): base modern xterm features plus
    // ccolour,cstyle,extkeys,margins,overline,usstyle — rendered in array order.
    #[test]
    fn default_features_mintty() {
        unsafe {
            let mut feat: i32 = 0;
            tty_default_features(&raw mut feat, c!("mintty"), 0);
            assert_eq!(
                get_features_string(feat),
                "256,bpaste,ccolour,clipboard,cstyle,extkeys,margins,mouse,overline,RGB,strikethrough,title,usstyle"
            );
        }
    }

    // "tmux" terminal entry: base plus ccolour,cstyle,focus,overline,usstyle,
    // hyperlinks. The hyperlinks NAME is registered regardless of whether the
    // hyperlinks build feature populates its capability list.
    #[test]
    fn default_features_tmux_terminal() {
        unsafe {
            let mut feat: i32 = 0;
            tty_default_features(&raw mut feat, c!("tmux"), 0);
            assert_eq!(
                get_features_string(feat),
                "256,bpaste,ccolour,clipboard,hyperlinks,cstyle,focus,mouse,overline,RGB,strikethrough,title,usstyle"
            );
        }
    }

    // iTerm2 entry: base plus cstyle,extkeys,margins,usstyle,sync,osc7,hyperlinks
    // (note: NO ccolour, unlike mintty/tmux).
    #[test]
    fn default_features_iterm2() {
        unsafe {
            let mut feat: i32 = 0;
            tty_default_features(&raw mut feat, c!("iTerm2"), 0);
            assert_eq!(
                get_features_string(feat),
                "256,bpaste,clipboard,hyperlinks,cstyle,extkeys,margins,mouse,osc7,RGB,strikethrough,sync,title,usstyle"
            );
        }
    }

    // vendor/tmux/tty-features.c:526 the version gate is
    //   `if (version != 0 && version < e->version) continue;`
    // Every table entry has version 0, so `version < 0` is never true and any
    // non-zero client version still applies the terminal's features.
    #[test]
    fn default_features_version_does_not_gate() {
        unsafe {
            let mut a: i32 = 0;
            tty_default_features(&raw mut a, c!("XTerm"), 0);
            let mut b: i32 = 0;
            tty_default_features(&raw mut b, c!("XTerm"), 500);
            assert_eq!(a, b);
            assert_ne!(a, 0);
        }
    }

    // An empty token between separators matches no feature name and therefore
    // breaks the loop (strcaseeq_ of "" against every name fails). So a trailing
    // or doubled separator drops everything after it, keeping earlier features.
    #[test]
    fn empty_token_breaks_loop() {
        unsafe {
            // "256,,mouse": 256 added, empty token breaks, mouse never reached.
            assert_eq!(get_features_string(add("256,,mouse")), "256");
            // A leading empty token drops everything.
            assert_eq!(add(",256"), 0);
        }
    }

    // The top valid bit (usstyle, index 19) renders alone, and rendering a
    // bitmask with an out-of-range bit set (bit 20+) simply ignores it — the
    // loop only walks the 20 known features.
    #[test]
    fn top_bit_and_out_of_range_bits() {
        unsafe {
            assert_eq!(get_features_string(1 << 19), "usstyle");
            // Bit 20 has no feature; it contributes nothing to the rendering.
            assert_eq!(get_features_string((1 << 19) | (1 << 20)), "usstyle");
            assert_eq!(get_features_string(1 << 20), "");
        }
    }
}
