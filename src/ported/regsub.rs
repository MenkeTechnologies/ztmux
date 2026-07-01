// Copyright (c) 2019 Nicholas Marriott <nicholas.marriott@gmail.com>
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
use core::ffi::c_int;

use xmalloc::xrealloc_;

use crate::libc::{memcpy, regcomp, regex_t, regexec, regfree, regmatch_t, strlen};
use crate::*;

/// C `vendor/tmux/regsub.c:27`: `static void regsub_copy(char **buf, ssize_t *len, const char *text, size_t start, size_t end)`
unsafe fn regsub_copy(
    buf: *mut *mut u8,
    len: *mut isize,
    text: *const u8,
    start: usize,
    end: usize,
) {
    let add: usize = end - start;
    unsafe {
        *buf = xrealloc_(*buf, (*len) as usize + add + 1).as_ptr();
        memcpy((*buf).add(*len as usize) as _, text.add(start) as _, add);
        (*len) += add as isize;
    }
}

/// C `vendor/tmux/regsub.c:38`: `static void regsub_expand(char **buf, ssize_t *len, const char *with, const char *text, regmatch_t *m, u_int n)`
pub unsafe fn regsub_expand(
    buf: *mut *mut u8,
    len: *mut isize,
    with: *const u8,
    text: *const u8,
    m: *mut regmatch_t,
    n: c_uint,
) {
    unsafe {
        // Faithful port of C `for (cp = with; *cp; cp++)`. The backslash branch
        // needs a following char (`cp[1] != '\0'`), and the `continue` (skip the
        // literal append) fires ONLY when the group actually matched. An
        // unmatched or out-of-range backref (`\2` on an unmatched optional group,
        // `\3` with 2 groups) copies nothing and falls through to append the
        // DIGIT literally — matching tmux (`\2` -> "2"). The `cp += 1` at the
        // bottom mirrors the for-loop's `cp++` in every path.
        let mut cp = with;
        while *cp != b'\0' {
            let mut copied = false;
            if *cp == b'\\' && *cp.add(1) != b'\0' {
                cp = cp.add(1);
                if *cp >= b'0' as _ && *cp <= b'9' as _ {
                    let i = (*cp - b'0') as u32;
                    if i < n && (*m.add(i as _)).rm_so != (*m.add(i as _)).rm_eo {
                        regsub_copy(
                            buf,
                            len,
                            text,
                            (*m.add(i as _)).rm_so as usize,
                            (*m.add(i as _)).rm_eo as usize,
                        );
                        copied = true;
                    }
                }
            }
            if !copied {
                *buf = xrealloc_(*buf, (*len) as usize + 2).as_ptr();
                *(*buf).add((*len) as usize) = *cp;
                (*len) += 1;
            }
            cp = cp.add(1);
        }
    }
}

/// C `vendor/tmux/regsub.c:62`: `char *regsub(const char *pattern, const char *with, const char *text, int flags)`
pub unsafe fn regsub(
    pattern: *const u8,
    with: *const u8,
    text: *const u8,
    flags: c_int,
) -> *mut u8 {
    unsafe {
        let mut r: regex_t = zeroed();
        let mut m: [regmatch_t; 10] = zeroed(); // TODO can use uninit
        let mut len: isize = 0;
        let mut empty = 0;
        let mut buf = null_mut();

        if *text == b'\0' {
            return xstrdup(c!("")).cast().as_ptr();
        }
        // C regsub.c:73 — an empty pattern matches at every position with regexec;
        // tmux short-circuits and returns the text unchanged.
        if *pattern == b'\0' {
            return xstrdup(text).cast().as_ptr();
        }
        if regcomp(&raw mut r, pattern, flags) != 0 {
            return null_mut();
        }

        let mut start: isize = 0;
        let mut last: isize = 0;
        let end: isize = strlen(text) as _;

        while start <= end {
            if regexec(
                &raw mut r,
                text.add(start as _) as _,
                m.len(),
                m.as_mut_ptr(),
                0,
            ) != 0
            {
                regsub_copy(
                    &raw mut buf,
                    &raw mut len,
                    text,
                    start as usize,
                    end as usize,
                );
                break;
            }

            // Append any text not part of this match (from the end of the
            // last match).
            regsub_copy(
                &raw mut buf,
                &raw mut len,
                text,
                last as usize,
                (m[0].rm_so as isize + start) as usize,
            );

            // If the last match was empty and this one isn't (it is either
            // later or has matched text), expand this match. If it is
            // empty, move on one character and try again from there.
            if empty != 0 || start + m[0].rm_so as isize != last || m[0].rm_so != m[0].rm_eo {
                regsub_expand(
                    &raw mut buf,
                    &raw mut len,
                    with,
                    text.offset(start),
                    m.as_mut_ptr(),
                    m.len() as u32,
                );

                last = start + m[0].rm_eo as isize;
                start += m[0].rm_eo as isize;
                empty = 0;
            } else {
                last = start + m[0].rm_eo as isize;
                start += (m[0].rm_eo + 1) as isize;
                empty = 1;
            }

            // Stop now if anchored to start.
            if *pattern == b'^' {
                regsub_copy(
                    &raw mut buf,
                    &raw mut len,
                    text,
                    start as usize,
                    end as usize,
                );
                break;
            }
        }
        *buf.offset(len) = b'\0' as _;

        regfree(&raw mut r);
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ffi::c_int;
    use std::ffi::{CStr, CString};

    // REG_EXTENDED so that `(...)` are capture groups and `\1` backrefs work
    // the way the tmux copy-mode / format regex helpers use them.
    const EXT: c_int = ::libc::REG_EXTENDED;

    /// Invoke `regsub` with owned C strings and return the produced string as
    /// bytes (None when `regsub` returns NULL, i.e. `regcomp` failed).
    unsafe fn run(pattern: &str, with: &str, text: &str, flags: c_int) -> Option<Vec<u8>> {
        unsafe {
            let p = CString::new(pattern).unwrap();
            let w = CString::new(with).unwrap();
            let t = CString::new(text).unwrap();
            let r = regsub(
                p.as_ptr().cast(),
                w.as_ptr().cast(),
                t.as_ptr().cast(),
                flags,
            );
            if r.is_null() {
                return None;
            }
            let out = CStr::from_ptr(r.cast()).to_bytes().to_vec();
            crate::libc::free_(r);
            Some(out)
        }
    }

    /// Convenience wrapper asserting a successful (non-NULL) result as &str.
    unsafe fn subst(pattern: &str, with: &str, text: &str, flags: c_int) -> String {
        unsafe { String::from_utf8(run(pattern, with, text, flags).unwrap()).unwrap() }
    }

    // C `regsub.c:70`: empty `text` returns xstrdup("") regardless of pattern.
    #[test]
    fn empty_text_returns_empty() {
        unsafe {
            assert_eq!(subst("anything", "X", "", EXT), "");
            // Even an invalid pattern is never compiled because the empty-text
            // check comes first, so this must not return NULL.
            assert_eq!(subst("(", "X", "", EXT), "");
        }
    }

    // C `regsub.c:74`: a pattern that fails to compile makes regsub return NULL.
    #[test]
    fn bad_pattern_returns_null() {
        unsafe {
            // Unbalanced paren is a REG_EXTENDED compile error.
            assert!(run("(", "X", "nonempty", EXT).is_none());
        }
    }

    // No match anywhere: the whole input is copied through verbatim
    // (C `regsub.c:82-84`, the regexec-failed branch on the first iteration).
    #[test]
    fn no_match_passthrough() {
        unsafe {
            assert_eq!(subst("z", "Q", "abc", EXT), "abc");
            assert_eq!(subst("xyz", "Q", "abcdef", EXT), "abcdef");
        }
    }

    // Simple literal replacement, applied globally to every occurrence
    // (regsub loops over the whole string; there is no first-only flag).
    #[test]
    fn literal_replace_global() {
        unsafe {
            assert_eq!(subst("o", "0", "foo bar", EXT), "f00 bar");
            assert_eq!(subst("a", "X", "banana", EXT), "bXnXnX");
        }
    }

    // Empty replacement deletes every match.
    #[test]
    fn empty_with_deletes_matches() {
        unsafe {
            assert_eq!(subst("o", "", "foo", EXT), "f");
            assert_eq!(subst("[0-9]", "", "a1b2c3", EXT), "abc");
        }
    }

    // Capture group backreference `\1` in the replacement
    // (C `regsub_expand`, regsub.c:47-53).
    #[test]
    fn capture_group_backref() {
        unsafe {
            assert_eq!(subst("(foo)", "[\\1]", "foo bar foo", EXT), "[foo] bar [foo]");
            // Reorder two captures: this is the exact regression documented in
            // the port comment — `\2\1` on "ab" must yield "ba" (no stray digits).
            assert_eq!(subst("(a)(b)", "\\2\\1", "ab", EXT), "ba");
        }
    }

    // A backref to a group that did not participate in the match expands to
    // nothing AND the digit is consumed (not emitted literally). For pattern
    // "(a)" there is no group 2, so `\2` produces the empty string.
    // C `regsub.c:49`: guarded by `m[i].rm_so != m[i].rm_eo`; the unmatched
    // slot has rm_so == rm_eo == -1, so nothing is copied.
    #[test]
    fn backref_to_unmatched_group_keeps_digit() {
        unsafe {
            // C only substitutes (and skips the digit) when the group matched;
            // an out-of-range/unmatched backref falls through and appends the
            // literal digit (regsub.c:38 — `continue` is inside the matched arm).
            assert_eq!(subst("(a)", "\\2", "a", EXT), "2");
            // `\0` is the whole match, so this echoes the matched text.
            assert_eq!(subst("(a)", "\\0", "a", EXT), "a");
        }
    }

    // A backslash before a non-digit drops the backslash and keeps the
    // following character (C `regsub_expand` falls through to the literal
    // append with cp already advanced past the backslash).
    #[test]
    fn backslash_before_nondigit() {
        unsafe {
            assert_eq!(subst("x", "\\n", "x", EXT), "n");
        }
    }

    // `^` anchor: regsub stops after the first (start-anchored) match and
    // copies the remainder verbatim (C `regsub.c:114-117`).
    #[test]
    fn anchored_start_replaces_once() {
        unsafe {
            assert_eq!(subst("^f", "X", "foo", EXT), "Xoo");
            // Without the anchor, the same single-char pattern is still only
            // matched where it occurs; "^o" never matches "foo" at start.
            assert_eq!(subst("^o", "X", "foo", EXT), "foo");
        }
    }

    // A greedy whole-string match followed by the trailing empty match must
    // NOT emit the replacement twice: `.*` -> "X" on "abc" yields just "X".
    // This exercises the empty-match bookkeeping (empty/last/start) in the
    // loop, C `regsub.c:98-111`.
    #[test]
    fn greedy_whole_match_no_trailing_dup() {
        unsafe {
            assert_eq!(subst(".*", "X", "abc", EXT), "X");
        }
    }

    // Flags are forwarded to regcomp: REG_ICASE makes matching case-insensitive.
    #[test]
    fn flags_are_forwarded_icase() {
        unsafe {
            assert_eq!(subst("abc", "x", "abcABC", EXT), "xABC");
            assert_eq!(subst("abc", "x", "abcABC", EXT | ::libc::REG_ICASE), "xx");
        }
    }

    // --- Known ztmux port divergence (ignored until fixed) -----------------

    // ztmux BUG: regsub is missing the empty-pattern early return that tmux has
    // (vendor/tmux/regsub.c:73 `if (*pattern == '\0') return xstrdup(text);`).
    // tmux returns the text unchanged for an empty pattern; ztmux instead runs
    // regexec with an empty regex, which matches at every position and injects
    // `with`. Remove #[ignore] once the guard is ported.
    #[test]
    fn bug_empty_pattern_returns_text_unchanged() {
        unsafe {
            assert_eq!(run("", "X", "ab", EXT).as_deref(), Some(&b"ab"[..]));
        }
    }

    // `\\` in the replacement: regsub_expand sees cp[0]='\\', cp[1]='\\' (non-NUL),
    // advances past the first backslash, finds a non-digit, so the branch falls
    // through and appends the second backslash literally (C regsub.c:44-58).
    // Two backslashes collapse to one.
    #[test]
    fn double_backslash_collapses_to_one() {
        unsafe {
            assert_eq!(subst("x", r"\\", "x", EXT), "\\");
            assert_eq!(subst("o", r"\\", "foo", EXT), "f\\\\");
        }
    }

    // A trailing backslash (cp[1] == '\0') fails the `cp[1] != '\0'` guard at
    // regsub.c:45, so the backslash itself is appended verbatim.
    #[test]
    fn trailing_backslash_is_literal() {
        unsafe {
            assert_eq!(subst("x", "\\", "x", EXT), "\\");
            assert_eq!(subst("x", "a\\", "x", EXT), "a\\");
        }
    }

    // `\0` is the whole match; used globally it wraps every occurrence
    // (regsub.c:48-51 with i==0 -> m[0], the full match span).
    #[test]
    fn whole_match_backref_global_wrap() {
        unsafe {
            assert_eq!(subst("[a-z]", r"<\0>", "ab", EXT), "<a><b>");
            assert_eq!(subst("[0-9]+", r"[\0]", "a12b345", EXT), "a[12]b[345]");
        }
    }

    // Three capture groups reordered by `\3\2\1` (regsub_expand copies each
    // matched span in the order the backrefs appear).
    #[test]
    fn three_capture_reorder() {
        unsafe {
            assert_eq!(subst("(a)(b)(c)", r"\3\2\1", "abc", EXT), "cba");
        }
    }

    // A backref whose group index is valid (< n == 10) but did not participate
    // in the match has rm_so == rm_eo == -1, so regsub.c:49 copies nothing and
    // the digit is emitted literally: `\9` on a single-group pattern -> "9".
    #[test]
    fn high_unmatched_backref_emits_digit() {
        unsafe {
            assert_eq!(subst("(a)", r"\9", "a", EXT), "9");
            assert_eq!(subst("(a)", r"\5x", "a", EXT), "5x");
        }
    }

    // Alternation matches either branch at each position; every match is
    // replaced (regsub loops the whole string).
    #[test]
    fn alternation_replaces_each_branch() {
        unsafe {
            assert_eq!(subst("a|b", "X", "cabc", EXT), "cXXc");
        }
    }

    // `.` matches every single character, so each is replaced independently.
    #[test]
    fn dot_replaces_every_char() {
        unsafe {
            assert_eq!(subst(".", "X", "abc", EXT), "XXX");
        }
    }

    // A quantified character class collapses each maximal run to one replacement.
    #[test]
    fn quantified_class_collapses_runs() {
        unsafe {
            assert_eq!(subst("[0-9]+", "N", "a12b345c", EXT), "aNbNc");
        }
    }

    // `^`-anchored pattern with a capture: the single start match is expanded
    // (backref filled) and the remainder copied verbatim (regsub.c:114-117).
    #[test]
    fn anchored_capture_then_remainder() {
        unsafe {
            assert_eq!(subst("^(f)", r"[\1]", "foo", EXT), "[f]oo");
        }
    }

    // Newlines in the text are ordinary bytes: matches around them are replaced
    // and the newlines pass through untouched.
    #[test]
    fn newlines_pass_through() {
        unsafe {
            assert_eq!(subst("a", "X", "a\nb\na", EXT), "X\nb\nX");
        }
    }

    // ICASE is forwarded to regcomp and also governs which text a capture spans:
    // "(a)b" matches "AB", and `\1` yields the actually-matched "A", not "a".
    #[test]
    fn icase_capture_uses_matched_text() {
        unsafe {
            assert_eq!(subst("(a)b", r"\1", "AB", EXT | ::libc::REG_ICASE), "A");
        }
    }

    // Unlike sed, regsub gives `&` no special meaning in the replacement — it is
    // a plain literal (only `\<digit>` is a backref, regsub.c:44-54).
    #[test]
    fn ampersand_is_literal_not_whole_match() {
        unsafe {
            assert_eq!(subst("a", "&", "a", EXT), "&");
            assert_eq!(subst("a", "x&y", "a", EXT), "x&y");
        }
    }

    // Adjacent, non-overlapping matches are each replaced; the loop advances
    // start by rm_eo so consecutive matches do not merge.
    #[test]
    fn adjacent_matches_replaced_independently() {
        unsafe {
            assert_eq!(subst("aa", "X", "aaaa", EXT), "XX");
            assert_eq!(subst("ab", "-", "abab", EXT), "--");
        }
    }

    // Nested capture groups number left-to-right by opening paren: for
    // "((a)(b))" group 1 is the whole "ab", 2 is "a", 3 is "b". regsub_expand
    // copies each matched span in backref order (regsub.c:47-53).
    #[test]
    fn nested_capture_groups() {
        unsafe {
            assert_eq!(subst("((a)(b))", r"\1", "ab", EXT), "ab");
            assert_eq!(subst("((a)(b))", r"\2-\3", "ab", EXT), "a-b");
            assert_eq!(subst("((a)(b))", r"\3\2", "ab", EXT), "ba");
        }
    }

    // A negated character class matches any byte not in the set and each match
    // is replaced globally (POSIX ERE `[^...]`, regsub loops the whole string).
    #[test]
    fn negated_char_class_global() {
        unsafe {
            assert_eq!(subst("[^0-9]", "_", "a1b2", EXT), "_1_2");
            assert_eq!(subst("[^abc]+", "-", "abXYZcab", EXT), "ab-cab");
        }
    }

    // A matched backref followed by an unmatched one: for "(a)" `\1\2` copies the
    // matched group 1 ("a") then falls through on the absent group 2, appending
    // the literal digit "2" (regsub.c:49 guard `rm_so != rm_eo`).
    #[test]
    fn matched_then_unmatched_backref() {
        unsafe {
            assert_eq!(subst("(a)", r"\1\2", "a", EXT), "a2");
            assert_eq!(subst("(a)", r"\1x\3", "a", EXT), "ax3");
        }
    }

    // Only `^` is special-cased for early-stop (regsub.c:114 `*pattern == '^'`);
    // `$` has no such handling, so an end-anchored pattern flows through the
    // normal loop and replaces the trailing match. "o$" on "foo" -> "foX".
    #[test]
    fn end_anchor_is_not_special_cased() {
        unsafe {
            assert_eq!(subst("o$", "X", "foo", EXT), "foX");
            // A `$`-anchored capture still expands its backref normally.
            assert_eq!(subst("(o)$", r"[\1]", "foo", EXT), "fo[o]");
        }
    }
}
