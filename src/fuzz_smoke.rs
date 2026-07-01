//! In-process randomized fuzz harness for the pure parsers/decoders.
//!
//! This is NOT coverage-guided libFuzzer — it is a deterministic, seeded random
//! hammer that (a) feeds arbitrary bytes into the string/byte parsers and
//! catches any panic (a panic in these unsafe C-ports means a buffer
//! over/underflow, bad slice index, or arithmetic overflow — i.e. a real bug),
//! and (b) checks round-trip invariants that must hold for every input
//! (`b64_ntop` then `b64_pton` == identity; `strvis` then `strunvis` ==
//! identity). A violated invariant or a panic is a confirmed bug; the harness
//! prints the exact triggering input (hex) so it can be turned into a fixed
//! regression test.
//!
//! Runs under `cargo test` like any other test, so it needs no nightly,
//! cargo-fuzz, sanitizer, or network. It cannot catch pure UB that neither
//! panics nor violates an invariant — the static Rust-vs-C audits cover that
//! angle. Together they are the two halves of "fuzz it and find bugs".

#![cfg(test)]

// Wrapped in `mod tests` so the ported-fn-names anti-drift gate skips these
// test-only helpers (it only scans free `fn`s outside `mod tests`/`mod test`).
mod tests {
    use std::ffi::CStr;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use crate::compat::b64::{b64_ntop, b64_pton};
    use crate::compat::{strnvis, strunvis, strvis, vis_flags};

    // splitmix64 — tiny, fast, deterministic PRNG (no rand crate, no Math.random).
    struct Rng(u64);
    impl Rng {
        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next_u64() % n as u64) as usize
        }
        fn byte(&mut self) -> u8 {
            (self.next_u64() & 0xff) as u8
        }
    }

    // Byte fragments that steer inputs into deep code paths (grey ramps, format
    // operators, regex backrefs, key names, multibyte UTF-8, invalid lead bytes).
    const FRAGS: &[&[u8]] = &[
        b"grey",
        b"gray50",
        b"colour123",
        b"#00ff00",
        b"bright",
        b"underscore",
        b",",
        b"|",
        b"-",
        b"=",
        b"\\0",
        b"\\1\\2",
        b"C-M-x",
        b"F10",
        b"MouseDown1",
        b"\xe2\x82\xac",     // € valid 3-byte
        b"\xf0\x9f\x98\x80", // 😀 valid 4-byte
        b"\xff\xfe",         // invalid lead bytes
        b"\xc3\x28",         // invalid continuation
        b"0123456789",
        b"99999999999999999999",
        // Format mini-language constructs — drive the arithmetic / conditional /
        // string-op / modifier / loop parsers in format_expand (otherwise a random
        // string has no `#{` and is just copied through).
        b"#{",
        b"}",
        b"#{?",
        b"#{e|+|:",
        b"#{e|*|f|2:",
        b"#{e|/|:",
        b"#{e|%|:",
        b"#{s/a/b/:",
        b"#{s/a/b/g:",
        b"#{=5:",
        b"#{=-5:",
        b"#{p10:",
        b"#{p-10:",
        b"#{==:",
        b"#{||:",
        b"#{&&:",
        b"#{!:",
        b"#{!!:",
        b"#{m:",
        b"#{c/f:",
        b"#{t:",
        b"#{b:",
        b"#{n:",
        b"#{l:",
        b"#{a:",
        b"#{S:",
        b"#{W:",
        b"#{P:",
        b"#{#{",
        b"#{session_name}",
        b"#{host}",
        b"1,2",
        b":",
        b"##",
        b"#H",
        b"#[fg=red]",
    ];

    // Build one random buffer: a mix of fragments and raw bytes.
    fn gen_input(rng: &mut Rng) -> Vec<u8> {
        let mut v = Vec::new();
        let parts = rng.below(6);
        for _ in 0..=parts {
            if rng.below(2) == 0 {
                v.extend_from_slice(FRAGS[rng.below(FRAGS.len())]);
            } else {
                let n = rng.below(12);
                for _ in 0..n {
                    v.push(rng.byte());
                }
            }
        }
        v
    }

    // NUL-terminated copy for the `*const u8` C-string callees (they stop at the
    // first NUL, which is a legitimate fuzz input; interior NULs just shorten it).
    fn cstr(bytes: &[u8]) -> Vec<u8> {
        let mut c = bytes.to_vec();
        c.push(0);
        c
    }

    // Bytes up to (not including) the first NUL — what a C string callee "sees".
    fn upto_nul(bytes: &[u8]) -> &[u8] {
        match bytes.iter().position(|&b| b == 0) {
            Some(i) => &bytes[..i],
            None => bytes,
        }
    }

    struct Found {
        target: &'static str,
        detail: String,
        input: Vec<u8>,
    }

    fn record(out: &mut Vec<Found>, seen: &mut std::collections::HashSet<String>, f: Found) {
        let key = format!("{}:{}", f.target, f.detail);
        if seen.insert(key) {
            out.push(f);
        }
    }

    // Optional single-target filter for diagnosis: FUZZ_ONLY=<target> runs just that
    // guard (so an aborting target can be isolated from the rest).
    fn want(target: &str) -> bool {
        use std::sync::OnceLock;
        static ONLY: OnceLock<Option<String>> = OnceLock::new();
        match ONLY.get_or_init(|| std::env::var("FUZZ_ONLY").ok()) {
            Some(o) => o == target,
            None => true,
        }
    }

    // Run `body` under catch_unwind; on panic, record the message + input.
    fn guard<F: FnOnce()>(
        out: &mut Vec<Found>,
        seen: &mut std::collections::HashSet<String>,
        target: &'static str,
        input: &[u8],
        body: F,
    ) {
        if !want(target) {
            return;
        }
        if let Err(e) = catch_unwind(AssertUnwindSafe(body)) {
            let msg = e
                .downcast_ref::<&str>()
                .map(ToString::to_string)
                .or_else(|| e.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "<non-string panic>".to_string());
            record(
                out,
                seen,
                Found {
                    target,
                    detail: format!("panic: {msg}"),
                    input: input.to_vec(),
                },
            );
        }
    }

    #[test]
    #[ignore = "fuzz/stress harness — mutates process globals; opt-in to avoid racing the parallel suite. Run: FUZZ_ITERS=N cargo test --lib -- --ignored fuzz_smoke"]
    fn fuzz_pure_parsers() {
        let iters: u64 = std::env::var("FUZZ_ITERS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(200_000);

        // Suppress the default panic hook's stderr spam during the run; we collect
        // and print our own de-duplicated report.
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        let mut rng = Rng(0x0123_4567_89AB_CDEF);
        let mut found: Vec<Found> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        // The flag combos tmux actually round-trips through (args_escape uses
        // OCTAL|CSTYLE|TAB|NL, paste/vis use OCTAL|CSTYLE). VIS_DQ is intentionally
        // excluded: tmux never strvis→strunvis round-trips with it, and its `\"`
        // escaping is not a round-trip-safe invariant here.
        let flag_sets = [
            vis_flags::VIS_OCTAL | vis_flags::VIS_CSTYLE,
            vis_flags::VIS_OCTAL | vis_flags::VIS_CSTYLE | vis_flags::VIS_TAB | vis_flags::VIS_NL,
        ];

        for i in 0..iters {
            let buf = gen_input(&mut rng);
            let s = String::from_utf8_lossy(&buf).into_owned();
            let c = cstr(&buf);
            let visible = upto_nul(&buf); // what C-string callees actually process

            // --- &str parsers: must never panic ---
            guard(&mut found, &mut seen, "colour_byname", &buf, || {
                let _ = crate::colour::colour_byname(&s);
            });
            guard(&mut found, &mut seen, "colour_fromstring", &buf, || {
                let _ = crate::colour::colour_fromstring(&s);
            });
            guard(&mut found, &mut seen, "attributes_fromstring", &buf, || {
                let _ = crate::attributes::attributes_fromstring(&s);
            });
            guard(&mut found, &mut seen, "strtonum_i32", &buf, || {
                let _ = crate::compat::strtonum_(&s, 0i32, 100i32);
            });
            guard(&mut found, &mut seen, "strtonum_i64", &buf, || {
                let _ = crate::compat::strtonum_(&s, i64::MIN, i64::MAX);
            });

            // --- C-string callees: must never panic ---
            guard(
                &mut found,
                &mut seen,
                "key_string_lookup_string",
                &buf,
                || unsafe {
                    let _ = crate::key_string::key_string_lookup_string(c.as_ptr());
                },
            );
            guard(&mut found, &mut seen, "utf8_fromcstr", &buf, || unsafe {
                let p = crate::utf8::utf8_fromcstr(c.as_ptr());
                if !p.is_null() {
                    crate::libc::free_(p.cast::<u8>());
                }
            });
            guard(&mut found, &mut seen, "utf8_cstrwidth", &buf, || unsafe {
                let _ = crate::utf8::utf8_cstrwidth(c.as_ptr());
            });

            // --- utf8_open/append state machine over raw bytes ---
            guard(&mut found, &mut seen, "utf8_open_append", &buf, || unsafe {
                let mut ud = std::mem::MaybeUninit::<crate::utf8_data>::zeroed().assume_init();
                for &b in &buf {
                    if crate::utf8::utf8_open(&mut ud, b) == crate::utf8_state::UTF8_MORE {
                        let mut more = crate::utf8_state::UTF8_MORE;
                        let mut j = 0;
                        while more == crate::utf8_state::UTF8_MORE && j < 8 {
                            more = crate::utf8::utf8_append(&mut ud, b);
                            j += 1;
                        }
                    }
                }
            });

            // --- regsub: arbitrary pattern/replacement/text (regcomp may reject) ---
            guard(&mut found, &mut seen, "regsub", &buf, || unsafe {
                let r =
                    crate::regsub::regsub(c.as_ptr(), c.as_ptr(), c.as_ptr(), ::libc::REG_EXTENDED);
                if !r.is_null() {
                    crate::libc::free_(r.cast::<u8>());
                }
            });

            // --- b64 roundtrip invariant: ntop(x) then pton == x ---
            guard(&mut found, &mut seen, "b64_roundtrip", &buf, || unsafe {
                let src = visible;
                let mut enc = vec![0u8; src.len() / 3 * 4 + 8];
                let n = b64_ntop(src.as_ptr(), src.len(), enc.as_mut_ptr(), enc.len());
                assert!(n >= 0, "b64_ntop failed on {src:02x?}");
                // enc is the encoding; it is NUL-terminated by b64_ntop.
                let mut dec = vec![0u8; src.len() + 8];
                let m = b64_pton(enc.as_ptr(), dec.as_mut_ptr(), dec.len());
                assert!(
                    m >= 0,
                    "b64_pton REJECTED our own b64_ntop output: src={:02x?} enc={:?}",
                    src,
                    CStr::from_ptr(enc.as_ptr().cast()),
                );
                let m = m as usize;
                assert_eq!(
                    &dec[..m],
                    src,
                    "b64 roundtrip mismatch: src={:02x?} enc={:?} dec={:02x?}",
                    src,
                    CStr::from_ptr(enc.as_ptr().cast()),
                    &dec[..m],
                );
            });

            // --- vis roundtrip invariant: strvis(x) then strunvis == x (no NULs) ---
            guard(&mut found, &mut seen, "vis_roundtrip", &buf, || unsafe {
                let src = visible;
                let sc = cstr(src);
                for &flag in &flag_sets {
                    let mut enc = vec![0u8; src.len() * 4 + 1];
                    let elen = strvis(enc.as_mut_ptr(), sc.as_ptr(), flag);
                    assert!(elen >= 0, "strvis failed");
                    let mut dec = vec![0u8; enc.len() + 1];
                    let dlen = strunvis(dec.as_mut_ptr(), enc.as_ptr());
                    assert!(
                        dlen >= 0,
                        "strunvis rejected strvis output: src={:02x?} flag={:#x}",
                        src,
                        flag.bits(),
                    );
                    assert_eq!(
                        &dec[..dlen as usize],
                        src,
                        "vis roundtrip mismatch: src={:02x?} flag={:#x} enc={:02x?}",
                        src,
                        flag.bits(),
                        &enc[..elen as usize],
                    );
                }
            });

            // --- strnvis must never panic / overrun for any dlen ---
            guard(&mut found, &mut seen, "strnvis", &buf, || unsafe {
                let src = cstr(visible);
                let dlen = (i as usize % (visible.len() + 3)) + 1;
                let mut dst = vec![0u8; dlen + 1];
                let _ = strnvis(dst.as_mut_ptr(), src.as_ptr(), dlen, flag_sets[0]);
            });
        }

        std::panic::set_hook(prev);

        if !found.is_empty() {
            eprintln!("\n==== FUZZ FOUND {} DISTINCT ISSUE(S) ====", found.len());
            for f in &found {
                eprintln!("[{}] {}\n  input = {:02x?}\n", f.target, f.detail, f.input);
            }
            panic!(
                "fuzz_pure_parsers found {} distinct issue(s) (see above)",
                found.len()
            );
        }
    }

    // Second harness: higher-level parsers + roundtrip invariants that must hold for
    // every input. A roundtrip mismatch or panic is a confirmed bug.
    #[test]
    #[ignore = "fuzz/stress harness — mutates process globals; opt-in to avoid racing the parallel suite. Run: FUZZ_ITERS=N cargo test --lib -- --ignored fuzz_smoke"]
    fn fuzz_roundtrips() {
        let iters: u64 = std::env::var("FUZZ_ITERS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(150_000);

        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        let mut rng = Rng(0xDEAD_BEEF_CAFE_F00D);
        let mut found: Vec<Found> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for _ in 0..iters {
            let buf = gen_input(&mut rng);
            let s = String::from_utf8_lossy(&buf).into_owned();
            let c = cstr(&buf);

            // --- command parser: arbitrary command strings must not panic ---
            // CMD_PARSE_NOALIAS skips the command-alias lookup, which would otherwise
            // read GLOBAL_OPTIONS (null in a bare test binary) — the grammar, AST
            // build, and per-command resolution are still fully exercised.
            // NOTE: cmd_parse_from_string is intentionally NOT fuzzed here. Its
            // lexer/parser touch server globals (GLOBAL_OPTIONS for command-alias,
            // the global environ for ~ tilde expansion) that main() initializes but
            // a bare test binary leaves null, so arbitrary input null-derefs on those
            // uninitialized trees — a harness artifact, not a port bug. The existing
            // cmd_parse unit tests cover it with the proper NOALIAS/among setup.

            // NOTE: a string→key→string→key roundtrip is intentionally NOT asserted —
            // it does not hold in tmux itself (e.g. raw DEL 0x7f and C-? both render
            // "C-?" but parse to different key codes; key-string.c:456). And feeding
            // arbitrary 64-bit codes to key_string_lookup_key is invalid input (C
            // indexes the same fixed tables unchecked). Both were harness over-reach.

            // --- colour roundtrip: fromstring→tostring→fromstring must be stable ---
            guard(&mut found, &mut seen, "colour_roundtrip", &buf, || {
                let c1 = crate::colour::colour_fromstring(&s);
                if c1 != -1 {
                    let ts = crate::colour::colour_tostring(c1);
                    let c2 = crate::colour::colour_fromstring(&ts);
                    assert_eq!(
                        c2, c1,
                        "colour roundtrip: {s:?} -> {c1:#x} -> {ts:?} -> {c2:#x}"
                    );
                }
            });

            // --- colour_tostring must not panic for any i32 colour code ---
            guard(&mut found, &mut seen, "colour_tostring", &buf, || {
                let code = (buf
                    .iter()
                    .fold(0i64, |a, &b| a.wrapping_mul(31).wrapping_add(b as i64))
                    & 0xffff_ffff) as i32;
                let _ = crate::colour::colour_tostring(code);
                let _ = crate::colour::colour_tostring(-code);
            });

            // --- args_escape must not panic for arbitrary bytes ---
            guard(&mut found, &mut seen, "args_escape", &buf, || unsafe {
                let p = crate::arguments::args_escape(c.as_ptr());
                if !p.is_null() {
                    crate::libc::free_(p);
                }
            });

            // --- utf8_sanitize / utf8_stravis must not panic ---
            guard(&mut found, &mut seen, "utf8_sanitize", &buf, || unsafe {
                let p = crate::utf8::utf8_sanitize(c.as_ptr());
                if !p.is_null() {
                    crate::libc::free_(p);
                }
            });
            guard(&mut found, &mut seen, "utf8_stravis", &buf, || unsafe {
                let mut out: *mut u8 = std::ptr::null_mut();
                let _ = crate::utf8::utf8_stravis(&mut out, c.as_ptr(), flag_octal_cstyle());
                if !out.is_null() {
                    crate::libc::free_(out);
                }
            });
        }

        std::panic::set_hook(prev);

        if !found.is_empty() {
            eprintln!(
                "\n==== FUZZ (roundtrips) FOUND {} DISTINCT ISSUE(S) ====",
                found.len()
            );
            for f in &found {
                eprintln!("[{}] {}\n  input = {:02x?}\n", f.target, f.detail, f.input);
            }
            panic!(
                "fuzz_roundtrips found {} distinct issue(s) (see above)",
                found.len()
            );
        }
    }

    fn flag_octal_cstyle() -> vis_flags {
        vis_flags::VIS_OCTAL | vis_flags::VIS_CSTYLE
    }

    // Third harness: the format mini-language engine and grid operations. Format is
    // designed to run with a null client/session (display-message before attach,
    // tests), so a null-deref under a bare-but-options-initialised tree is a real
    // missing-guard bug, not a setup artifact. FORMAT_NOJOBS prevents `#(...)` from
    // spawning shell processes during fuzzing.
    #[test]
    #[ignore = "fuzz/stress harness — mutates process globals; opt-in to avoid racing the parallel suite. Run: FUZZ_ITERS=N cargo test --lib -- --ignored fuzz_smoke"]
    fn fuzz_format_grid() {
        let iters: u64 = std::env::var("FUZZ_ITERS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(120_000);

        unsafe { ensure_globals() };

        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        let mut rng = Rng(0xF00D_1234_5678_ABCDu64);
        let mut found: Vec<Found> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for _ in 0..iters {
            let buf = gen_input(&mut rng);
            let c = cstr(&buf);

            // --- format expansion: arbitrary format strings must not panic ---
            guard(&mut found, &mut seen, "format_expand", &buf, || unsafe {
                let ft = crate::format::format_create(
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    crate::format::FORMAT_NONE,
                    crate::format::format_flags::FORMAT_NOJOBS,
                );
                if !ft.is_null() {
                    let p = crate::format::format_expand(ft, c.as_ptr());
                    if !p.is_null() {
                        crate::libc::free_(p);
                    }
                    crate::format::format_free(ft);
                }
            });

            // --- grid create / set / get / clear / reflow / destroy ---
            guard(&mut found, &mut seen, "grid_ops", &buf, || unsafe {
                let sx = 1 + (rng_of(&buf, 0) % 40) as u32;
                let sy = 1 + (rng_of(&buf, 1) % 20) as u32;
                let hlimit = (rng_of(&buf, 2) % 50) as u32;
                let gd = crate::grid_::grid_create(sx, sy, hlimit);
                if gd.is_null() {
                    return;
                }
                // Set a few in-bounds cells (callers guarantee px<sx, py<sy).
                for k in 0..6usize {
                    let px = (rng_of(&buf, 3 + k) % sx as u64) as u32;
                    let py = (rng_of(&buf, 9 + k) % sy as u64) as u32;
                    crate::grid_::grid_set_cell(
                        gd,
                        px,
                        py,
                        &raw const crate::grid_::GRID_DEFAULT_CELL,
                    );
                    let mut gc: crate::grid_cell = std::mem::zeroed();
                    crate::grid_::grid_get_cell(gd, px, py, &mut gc);
                }
                // Reflow to an arbitrary width (the real, complex scenario).
                let new_sx = 1 + (rng_of(&buf, 20) % 120) as u32;
                crate::grid_::grid_reflow(gd, new_sx);
                crate::grid_::grid_destroy(gd);
            });
        }

        std::panic::set_hook(prev);

        if !found.is_empty() {
            eprintln!(
                "\n==== FUZZ (format/grid) FOUND {} DISTINCT ISSUE(S) ====",
                found.len()
            );
            for f in &found {
                eprintln!("[{}] {}\n  input = {:02x?}\n", f.target, f.detail, f.input);
            }
            panic!(
                "fuzz_format_grid found {} distinct issue(s) (see above)",
                found.len()
            );
        }
    }

    // A varied 64-bit value derived from the buffer + a salt (for grid dims/coords).
    fn rng_of(buf: &[u8], salt: usize) -> u64 {
        buf.iter()
            .fold(0x9E37u64.wrapping_add(salt as u64 * 0x100_0193), |a, &b| {
                a.rotate_left(5) ^ (b as u64).wrapping_add(0x9E37_79B9)
            })
            | 1
    }

    // Initialise the three global option trees with their table defaults, the way
    // tmux's main() does at startup, so format lookups that query options do not
    // hit a null tree (which would be a setup artifact, not a port bug).
    unsafe fn ensure_globals() {
        unsafe {
            use crate::tmux::{GLOBAL_ENVIRON, GLOBAL_OPTIONS, GLOBAL_S_OPTIONS, GLOBAL_W_OPTIONS};
            if GLOBAL_OPTIONS.is_null() {
                GLOBAL_OPTIONS = crate::options_::options_create(std::ptr::null_mut());
                GLOBAL_S_OPTIONS = crate::options_::options_create(std::ptr::null_mut());
                GLOBAL_W_OPTIONS = crate::options_::options_create(std::ptr::null_mut());
                // format_find falls back to environ_find(GLOBAL_ENVIRON, ...); real
                // tmux always initialises this, so create it to avoid a null-tree
                // artifact (environ_find over an existing tree just returns None).
                GLOBAL_ENVIRON = crate::environ_::environ_create().as_ptr();
                for oe in &crate::options_table::OPTIONS_TABLE {
                    if oe.scope & crate::OPTIONS_TABLE_SERVER != 0 {
                        crate::options_::options_default(GLOBAL_OPTIONS, oe);
                    }
                    if oe.scope & crate::OPTIONS_TABLE_SESSION != 0 {
                        crate::options_::options_default(GLOBAL_S_OPTIONS, oe);
                    }
                    if oe.scope & crate::OPTIONS_TABLE_WINDOW != 0 {
                        crate::options_::options_default(GLOBAL_W_OPTIONS, oe);
                    }
                }
            }
        }
    }
}
