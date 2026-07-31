// Copyright (c) 2026 Nicholas Marriott <nicholas.marriott@gmail.com>
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

//! Fuzzy matching in the style of fzf. The pattern is split into groups by `|`
//! and each group is split on spaces into terms. A row matches if any group
//! matches; within a group all positive terms must match and all inverse terms
//! must not match.
//!
//! Plain positive terms are fuzzy subsequences. A leading `'` makes a term an
//! exact substring match, `^` anchors a term at the start and `$` anchors it at
//! the end. A leading `!` inverts the term. Plain inverse terms are exact
//! substring matches rather than inverse fuzzy matches, like fzf.
//!
//! Both the pattern and the text are UTF-8. The text may contain tmux style
//! directives (`#[...]`); these and their contents are invisible to matching and
//! occupy no columns, but `align=` styles do move the surrounding text and are
//! accounted for exactly as `format_draw` lays it out (the no-list layout, see
//! `format_draw_none`). Matching is smart-case: case is ignored unless the
//! pattern contains an uppercase character (ASCII case folding only; other
//! characters are compared exactly by their UTF-8 data).
//!
//! On a match a bit string of the requested display width is returned with a bit
//! set for every column occupied by a matched character, so the caller can
//! highlight them; `None` is returned if there is no match. A cheap fzf-style
//! score (matches at the start, after word boundaries and in contiguous runs
//! score higher) is also produced so callers can rank best-match-first.

use crate::bitstr::BitStr;
use crate::*;

const FUZZY_BONUS_EXACT: i32 = 1000;
const FUZZY_BONUS_PREFIX: i32 = 200;
const FUZZY_BONUS_SUFFIX: i32 = 100;
const FUZZY_BONUS_START: i32 = 12;
const FUZZY_BONUS_BOUNDARY: i32 = 8;
const FUZZY_BONUS_CONSECUTIVE: i32 = 6;
const FUZZY_PENALTY_LEADING: i32 = 1;
const FUZZY_PENALTY_LEADING_MAX: i32 = 10;
const FUZZY_PENALTY_GAP: i32 = 1;

/// Number of alignments, `STYLE_ALIGN_ABSOLUTE_CENTRE + 1` in the C.
const ALIGNS: usize = style_align::STYLE_ALIGN_ABSOLUTE_CENTRE as usize + 1;

/// A single visible character of the text.
/// C `vendor/tmux/fuzzy.c:65`: `struct fuzzy_char`
#[derive(Clone)]
struct fuzzy_char {
    align: style_align,
    /// Original UTF-8 data.
    ud: utf8_data,
    /// Display width.
    width: u32,
    /// Within its alignment.
    offset: u32,
}

/// One parsed query term.
/// C `vendor/tmux/fuzzy.c:72`: `struct fuzzy_term`
#[derive(Default)]
struct fuzzy_term<'a> {
    inverse: bool,
    exact: bool,
    prefix: bool,
    suffix: bool,
    text: &'a [u8],
}

/// Is this character a word boundary, so a match after it scores higher?
/// C `vendor/tmux/fuzzy.c:82`: `static int fuzzy_is_boundary(const struct utf8_data *ud)`
fn fuzzy_is_boundary(ud: &utf8_data) -> bool {
    const BOUNDARY: &[u8] = b" -_/.:";

    ud.size == 1 && BOUNDARY.contains(&ud.data[0])
}

/// Compare two characters, folding ASCII case if wanted. UTF-8 is compared
/// directly without case folding.
/// C `vendor/tmux/fuzzy.c:97`: `static int fuzzy_char_equal(const struct utf8_data *a, const struct utf8_data *b, int fold)`
fn fuzzy_char_equal(a: &utf8_data, b: &utf8_data, fold: bool) -> bool {
    if fold && a.size == 1 && b.size == 1 && a.data[0] < 0x80 && b.data[0] < 0x80 {
        return a.data[0].eq_ignore_ascii_case(&b.data[0]);
    }
    a.size == b.size && a.data[..a.size as usize] == b.data[..b.size as usize]
}

/// Map a style alignment onto one of the four layout columns.
/// C `vendor/tmux/fuzzy.c:110`: `static enum style_align fuzzy_align(enum style_align align)`
fn fuzzy_align(align: style_align) -> style_align {
    if align == style_align::STYLE_ALIGN_DEFAULT {
        style_align::STYLE_ALIGN_LEFT
    } else {
        align
    }
}

/// Add a visible character to the array, updating the alignment width.
/// C `vendor/tmux/fuzzy.c:119`: `static void fuzzy_add(struct fuzzy_char **cs, u_int *ncs, u_int *alloc, enum style_align a, const struct utf8_data *ud, u_int *widths)`
fn fuzzy_add(cs: &mut Vec<fuzzy_char>, a: style_align, ud: &utf8_data, widths: &mut [u32; ALIGNS]) {
    cs.push(fuzzy_char {
        align: a,
        ud: *ud,
        width: ud.width as u32,
        offset: widths[a as usize],
    });
    widths[a as usize] += ud.width as u32;
}

/// Decode a character as UTF-8, returning how many bytes were consumed.
/// C `vendor/tmux/fuzzy.c:138`: `static const char *fuzzy_decode_one(const char *cp, const char *end, struct utf8_data *ud)`
fn fuzzy_decode_one(cp: &[u8], ud: &mut utf8_data) -> usize {
    // SAFETY: the utf8 helpers only read and write through the supplied
    // `utf8_data`, which is a live local of the caller.
    unsafe {
        if utf8_open(ud, cp[0]) == utf8_state::UTF8_MORE {
            let mut more = utf8_state::UTF8_MORE;
            let mut i = 1;
            while i != cp.len() && more == utf8_state::UTF8_MORE {
                more = utf8_append(ud, cp[i]);
                i += 1;
            }
            if more == utf8_state::UTF8_DONE {
                return i;
            }
        }
        utf8_set(ud, cp[0]);
        1
    }
}

/// Scan the text into an array of visible characters, skipping styles and
/// recording the alignment and intra-alignment offset of each. Returns the
/// array and fills in the per-alignment widths.
/// C `vendor/tmux/fuzzy.c:160`: `static struct fuzzy_char *fuzzy_scan(const char *text, u_int *ncs, u_int *widths)`
fn fuzzy_scan(text: &[u8], widths: &mut [u32; ALIGNS]) -> Vec<fuzzy_char> {
    let mut cs: Vec<fuzzy_char> = Vec::new();
    let mut current = style_align::STYLE_ALIGN_LEFT;
    let mut hash: utf8_data = unsafe { zeroed() };
    let mut bracket: utf8_data = unsafe { zeroed() };
    let mut sy: style = unsafe { zeroed() };

    *widths = [0; ALIGNS];
    // SAFETY: all three are live locals; style_set only writes into `sy`.
    unsafe {
        style_set(&raw mut sy, &raw const GRID_DEFAULT_CELL);
        utf8_set(&raw mut hash, b'#');
        utf8_set(&raw mut bracket, b'[');
    }

    let mut cp = 0usize;
    while cp < text.len() && text[cp] != b'\0' {
        // Handle a run of #s, which may introduce a style.
        if text[cp] == b'#' {
            let mut n = 0;
            while cp + n < text.len() && text[cp + n] == b'#' {
                n += 1;
            }
            if cp + n >= text.len() || text[cp + n] != b'[' {
                // Escaped #s: ## -> #, so half (rounded up).
                let leading = if n % 2 == 0 { n / 2 } else { n / 2 + 1 };
                for _ in 0..leading {
                    fuzzy_add(&mut cs, current, &hash, widths);
                }
                cp += n;
                continue;
            }

            // Even count: all #s escaped, the [ is literal.
            for _ in 0..n / 2 {
                fuzzy_add(&mut cs, current, &hash, widths);
            }
            if n % 2 == 0 {
                fuzzy_add(&mut cs, current, &bracket, widths);
                cp += n + 1;
                continue;
            }

            // Odd count: this is a style, find and parse it.
            let after = &text[cp + n + 1..];
            // SAFETY: `after` is a NUL-terminated view of the caller's string.
            let end = unsafe { format_skip(after.as_ptr(), c!("]")) };
            if end.is_null() {
                break;
            }
            // SAFETY: format_skip returns a pointer within `after`.
            let len = unsafe { end.offset_from(after.as_ptr()) } as usize;
            let tmp = std::ffi::CString::new(&after[..len]).unwrap_or_default();
            // SAFETY: `tmp` outlives the call; style_parse writes only `sy`.
            if unsafe { style_parse(&raw mut sy, &raw const GRID_DEFAULT_CELL, tmp.as_ptr().cast()) }
                == 0
            {
                current = fuzzy_align(sy.align);
            }
            cp += n + 1 + len + 1;
            continue;
        }

        // Decode one character, multibyte or single byte.
        let mut ud: utf8_data = unsafe { zeroed() };
        cp += fuzzy_decode_one(&text[cp..], &mut ud);

        // Skip non-printable single bytes (control characters and raw bytes
        // left over from a failed decode); keep printable ASCII and any
        // decoded UTF-8.
        if ud.size == 1 && (ud.data[0] <= 0x1f || ud.data[0] >= 0x7f) {
            continue;
        }
        fuzzy_add(&mut cs, current, &ud, widths);
    }
    cs
}

/// Work out the display column of a visible character given the trimmed widths
/// and start columns of each alignment. Returns the column if the character is
/// visible.
/// C `vendor/tmux/fuzzy.c:237`: `static int fuzzy_column(const struct fuzzy_char *fc, const u_int *start, const u_int *src, const u_int *vis, u_int *column)`
fn fuzzy_column(
    fc: &fuzzy_char,
    start: &[u32; ALIGNS],
    src: &[u32; ALIGNS],
    vis: &[u32; ALIGNS],
) -> Option<u32> {
    let a = fc.align as usize;

    if fc.offset < src[a] || fc.offset >= src[a] + vis[a] {
        return None;
    }
    Some(start[a] + (fc.offset - src[a]))
}

/// Decode a UTF-8 term into an array of characters.
/// C `vendor/tmux/fuzzy.c:250`: `static u_int fuzzy_decode(const char *tok, size_t len, struct utf8_data *out)`
fn fuzzy_decode(tok: &[u8]) -> Vec<utf8_data> {
    let mut out = Vec::new();
    let mut cp = 0;

    while cp != tok.len() {
        let mut ud: utf8_data = unsafe { zeroed() };
        cp += fuzzy_decode_one(&tok[cp..], &mut ud);
        out.push(ud);
    }
    out
}

/// Add the score for a fuzzy token matched at the given positions.
/// C `vendor/tmux/fuzzy.c:262`: `static int fuzzy_score_positions(const u_int *pos, u_int npos, const struct fuzzy_char *cs)`
fn fuzzy_score_positions(pos: &[u32], cs: &[fuzzy_char]) -> i32 {
    let mut score = 0;

    if pos.is_empty() {
        return 0;
    }
    if pos[0] == 0 {
        score += FUZZY_BONUS_START;
    } else {
        if fuzzy_is_boundary(&cs[pos[0] as usize - 1].ud) {
            score += FUZZY_BONUS_BOUNDARY;
        }
        if (pos[0] as i32) < FUZZY_PENALTY_LEADING_MAX {
            score -= pos[0] as i32 * FUZZY_PENALTY_LEADING;
        } else {
            score -= FUZZY_PENALTY_LEADING_MAX * FUZZY_PENALTY_LEADING;
        }
    }
    for i in 1..pos.len() {
        if pos[i] == pos[i - 1] + 1 {
            score += FUZZY_BONUS_CONSECUTIVE;
        } else if fuzzy_is_boundary(&cs[pos[i] as usize - 1].ud) {
            score += FUZZY_BONUS_BOUNDARY;
        }
    }
    let span = pos[pos.len() - 1] - pos[0] + 1;
    let gap = span - pos.len() as u32;
    score -= gap as i32 * FUZZY_PENALTY_GAP;
    score
}

/// Match a token as a subsequence of the visible characters.
/// C `vendor/tmux/fuzzy.c:298`: `static int fuzzy_match_fuzzy(const struct utf8_data *tok, u_int toklen, struct fuzzy_char *cs, u_int ncs, int fold, int *score, char *matched)`
fn fuzzy_match_fuzzy(
    tok: &[utf8_data],
    cs: &[fuzzy_char],
    fold: bool,
    score: &mut i32,
    matched: Option<&mut [bool]>,
) -> bool {
    if tok.is_empty() || cs.is_empty() {
        return false;
    }
    let mut pos = vec![0u32; tok.len()];

    // First find a subsequence from the start.
    let mut ci = 0usize;
    for (pi, t) in tok.iter().enumerate() {
        while ci != cs.len() && !fuzzy_char_equal(t, &cs[ci].ud, fold) {
            ci += 1;
        }
        if ci == cs.len() {
            return false;
        }
        pos[pi] = ci as u32;
        ci += 1;
    }

    // Then compact it backwards to prefer a shorter span.
    ci = pos[tok.len() - 1] as usize;
    let mut pi = tok.len();
    while pi > 0 {
        let mut found = false;
        loop {
            if fuzzy_char_equal(&tok[pi - 1], &cs[ci].ud, fold) {
                pos[pi - 1] = ci as u32;
                found = true;
                break;
            }
            if ci == 0 {
                break;
            }
            ci -= 1;
        }
        if !found {
            return false;
        }
        if pi != 1 {
            ci -= 1;
        }
        pi -= 1;
    }

    *score += fuzzy_score_positions(&pos, cs);
    if let Some(matched) = matched {
        for p in &pos {
            matched[*p as usize] = true;
        }
    }
    true
}

/// Score an exact, prefix or suffix match.
/// C `vendor/tmux/fuzzy.c:351`: `static int fuzzy_score_exact(u_int start, u_int toklen, u_int ncs, const struct fuzzy_char *cs, int prefix, int suffix)`
fn fuzzy_score_exact(
    start: u32,
    toklen: u32,
    ncs: u32,
    cs: &[fuzzy_char],
    prefix: bool,
    suffix: bool,
) -> i32 {
    let mut score = FUZZY_BONUS_EXACT + toklen as i32 * FUZZY_BONUS_CONSECUTIVE;

    if prefix {
        score += FUZZY_BONUS_PREFIX;
    }
    if suffix {
        score += FUZZY_BONUS_SUFFIX;
    }
    if start == 0 {
        score += FUZZY_BONUS_START;
    } else if fuzzy_is_boundary(&cs[start as usize - 1].ud) {
        score += FUZZY_BONUS_BOUNDARY;
    }
    if (start as i32) < FUZZY_PENALTY_LEADING_MAX {
        score -= start as i32 * FUZZY_PENALTY_LEADING;
    } else {
        score -= FUZZY_PENALTY_LEADING_MAX * FUZZY_PENALTY_LEADING;
    }
    if !prefix && !suffix {
        score -= (ncs - (start + toklen)) as i32;
    }
    score
}

/// Match an exact, prefix or suffix term against the visible characters.
/// C `vendor/tmux/fuzzy.c:374`: `static int fuzzy_match_exact(const struct utf8_data *tok, u_int toklen, struct fuzzy_char *cs, u_int ncs, int fold, int prefix, int suffix, int *score, char *matched)`
fn fuzzy_match_exact(
    tok: &[utf8_data],
    cs: &[fuzzy_char],
    fold: bool,
    prefix: bool,
    suffix: bool,
    score: &mut i32,
    matched: Option<&mut [bool]>,
) -> bool {
    let toklen = tok.len();
    let ncs = cs.len();

    if toklen == 0 || toklen > ncs {
        return false;
    }

    let (start, end) = if prefix && suffix {
        if toklen != ncs {
            return false;
        }
        (0, 1)
    } else if prefix {
        (0, 1)
    } else if suffix {
        (ncs - toklen, ncs - toklen + 1)
    } else {
        (0, ncs - toklen + 1)
    };

    let mut found = false;
    let mut best = 0;
    let mut bestscore = 0;
    for i in start..end {
        if !(0..toklen).all(|j| fuzzy_char_equal(&tok[j], &cs[i + j].ud, fold)) {
            continue;
        }
        let value = fuzzy_score_exact(i as u32, toklen as u32, ncs as u32, cs, prefix, suffix);
        if !found || value > bestscore {
            found = true;
            best = i;
            bestscore = value;
        }
    }
    if !found {
        return false;
    }
    *score += bestscore;
    if let Some(matched) = matched {
        for i in 0..toklen {
            matched[best + i] = true;
        }
    }
    true
}

/// Parse one term.
/// C `vendor/tmux/fuzzy.c:428`: `static int fuzzy_parse_term(const char *start, const char *end, struct fuzzy_term *term)`
fn fuzzy_parse_term<'a>(mut text: &'a [u8], term: &mut fuzzy_term<'a>) -> bool {
    *term = fuzzy_term::default();
    if text.is_empty() {
        return false;
    }
    if text[0] == b'!' {
        term.inverse = true;
        text = &text[1..];
    }
    if text.is_empty() {
        return false;
    }
    if text[0] == b'\'' {
        term.exact = true;
        text = &text[1..];
    } else if text[0] == b'^' {
        term.exact = true;
        term.prefix = true;
        text = &text[1..];
    }
    if text.is_empty() {
        return false;
    }
    if text[text.len() - 1] == b'$' {
        term.exact = true;
        term.suffix = true;
        text = &text[..text.len() - 1];
    }
    if text.is_empty() {
        return false;
    }

    if term.inverse {
        term.exact = true;
    }
    term.text = text;
    true
}

/// Match one parsed term.
/// C `vendor/tmux/fuzzy.c:462`: `static int fuzzy_match_term(const struct fuzzy_term *term, struct utf8_data *tok, struct fuzzy_char *cs, u_int ncs, int fold, int *score, char *matched)`
fn fuzzy_match_term(
    term: &fuzzy_term,
    cs: &[fuzzy_char],
    fold: bool,
    score: &mut i32,
    matched: &mut [bool],
) -> bool {
    let tok = fuzzy_decode(term.text);
    let mut value = 0;

    // An inverse term never highlights: it says what must NOT be there.
    let target = if term.inverse { None } else { Some(matched) };
    let matched_term = if term.exact {
        fuzzy_match_exact(
            &tok,
            cs,
            fold,
            term.prefix,
            term.suffix,
            &mut value,
            target,
        )
    } else {
        fuzzy_match_fuzzy(&tok, cs, fold, &mut value, target)
    };

    if term.inverse {
        return !matched_term;
    }
    if !matched_term {
        return false;
    }
    *score += value;
    true
}

/// Match one AND group of terms.
/// C `vendor/tmux/fuzzy.c:496`: `static int fuzzy_match_group(const char *start, const char *end, struct utf8_data *tok, struct fuzzy_char *cs, u_int ncs, int fold, int *score, char *matched)`
fn fuzzy_match_group(
    group: &[u8],
    cs: &[fuzzy_char],
    fold: bool,
    score: &mut i32,
    matched: &mut [bool],
) -> bool {
    let mut cp = 0usize;
    let mut any = false;

    *score = 0;
    while cp != group.len() {
        while cp != group.len() && group[cp] == b' ' {
            cp += 1;
        }
        if cp == group.len() {
            break;
        }
        let sp = cp;
        while cp != group.len() && group[cp] != b' ' {
            cp += 1;
        }
        let mut term = fuzzy_term::default();
        if !fuzzy_parse_term(&group[sp..cp], &mut term) {
            return false;
        }
        any = true;
        if !fuzzy_match_term(&term, cs, fold, score, matched) {
            return false;
        }
    }
    any
}

/// Fuzzy match `pattern` against `text`, which is drawn into a region of the
/// given display width. Returns a bit string of `width` bits with a bit set for
/// each column occupied by a matched character, or `None` if there is no match,
/// along with the score — higher is better.
/// C `vendor/tmux/fuzzy.c:528`: `bitstr_t *fuzzy_match(const char *pattern, const char *text, u_int width, u_int *score)`
pub fn fuzzy_match(pattern: &[u8], text: &[u8], width: u32) -> Option<(BitStr, u32)> {
    if width == 0 {
        return None;
    }

    // An empty query matches everything, with nothing highlighted.
    let mut cp = 0;
    while cp < pattern.len() && (pattern[cp] == b' ' || pattern[cp] == b'|') {
        cp += 1;
    }
    if cp == pattern.len() {
        return Some((BitStr::new(width), 0));
    }

    // Smart-case: fold unless the pattern has an uppercase character.
    let fold = !pattern.iter().any(u8::is_ascii_uppercase);

    // Scan the text into visible characters.
    let mut widths = [0u32; ALIGNS];
    let cs = fuzzy_scan(text, &mut widths);
    let mut matched = vec![false; cs.len()];
    let mut best = vec![false; cs.len()];

    // Match each |-separated group and keep the best-scoring one.
    let mut found = false;
    let mut bestscore = 0;
    let mut cp = 0usize;
    while cp != pattern.len() {
        while cp != pattern.len() && (pattern[cp] == b' ' || pattern[cp] == b'|') {
            cp += 1;
        }
        if cp == pattern.len() {
            break;
        }
        let sp = cp;
        while cp != pattern.len() && pattern[cp] != b'|' {
            cp += 1;
        }
        matched.fill(false);
        let mut groupscore = 0;
        if fuzzy_match_group(&pattern[sp..cp], &cs, fold, &mut groupscore, &mut matched)
            && (!found || groupscore > bestscore)
        {
            found = true;
            bestscore = groupscore;
            best.copy_from_slice(&matched);
        }
    }
    if !found {
        return None;
    }

    // Work out the trimmed widths and start columns of each alignment,
    // mirroring format_draw_none.
    let mut wl = widths[style_align::STYLE_ALIGN_LEFT as usize];
    let mut wc = widths[style_align::STYLE_ALIGN_CENTRE as usize];
    let mut wr = widths[style_align::STYLE_ALIGN_RIGHT as usize];
    let mut wa = widths[style_align::STYLE_ALIGN_ABSOLUTE_CENTRE as usize];
    while wl + wc + wr > width {
        if wc > 0 {
            wc -= 1;
        } else if wr > 0 {
            wr -= 1;
        } else {
            wl -= 1;
        }
    }
    if wa > width {
        wa = width;
    }

    let mut start = [0u32; ALIGNS];
    let mut src = [0u32; ALIGNS];
    let mut vis = [0u32; ALIGNS];

    start[style_align::STYLE_ALIGN_LEFT as usize] = 0;
    src[style_align::STYLE_ALIGN_LEFT as usize] = 0;
    vis[style_align::STYLE_ALIGN_LEFT as usize] = wl;

    start[style_align::STYLE_ALIGN_RIGHT as usize] = width - wr;
    src[style_align::STYLE_ALIGN_RIGHT as usize] =
        widths[style_align::STYLE_ALIGN_RIGHT as usize] - wr;
    vis[style_align::STYLE_ALIGN_RIGHT as usize] = wr;

    start[style_align::STYLE_ALIGN_CENTRE as usize] = wl + ((width - wr) - wl) / 2 - wc / 2;
    src[style_align::STYLE_ALIGN_CENTRE as usize] =
        widths[style_align::STYLE_ALIGN_CENTRE as usize] / 2 - wc / 2;
    vis[style_align::STYLE_ALIGN_CENTRE as usize] = wc;

    start[style_align::STYLE_ALIGN_ABSOLUTE_CENTRE as usize] = (width - wa) / 2;
    src[style_align::STYLE_ALIGN_ABSOLUTE_CENTRE as usize] = 0;
    vis[style_align::STYLE_ALIGN_ABSOLUTE_CENTRE as usize] = wa;

    // Set a bit for each column of each matched character.
    let mut mask = BitStr::new(width);
    for (i, fc) in cs.iter().enumerate() {
        if !best[i] {
            continue;
        }
        let Some(column) = fuzzy_column(fc, &start, &src, &vis) else {
            continue;
        };
        let mut j = 0;
        while j < fc.width && column + j < width {
            mask.bit_set(column + j);
            j += 1;
        }
    }

    Some((mask, if bestscore < 0 { 0 } else { bestscore as u32 }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The columns a match highlighted, as a string of carets under the text —
    /// the shape a reader can check against the text by eye.
    fn marks(pattern: &str, text: &str, width: u32) -> Option<String> {
        let (mask, _) = fuzzy_match(pattern.as_bytes(), text.as_bytes(), width)?;
        Some(
            (0..width)
                .map(|i| if mask.bit_test(i) { '^' } else { ' ' })
                .collect::<String>()
                .trim_end()
                .to_string(),
        )
    }

    fn score(pattern: &str, text: &str) -> Option<u32> {
        fuzzy_match(pattern.as_bytes(), text.as_bytes(), 40).map(|(_, s)| s)
    }

    #[test]
    fn subsequence_marks_the_tightest_span_not_the_first_one() {
        // The match is compacted backwards after the first subsequence is
        // found, so it lands on the LAST possible start rather than the first:
        // "ab" over "alpha bravo" marks the 'a' of "alpha" that is nearest the
        // 'b', not the leading one.
        //                                          alpha bravo
        assert_eq!(marks("ab", "alpha bravo", 40).as_deref(), Some("    ^ ^"));
        assert!(marks("zz", "alpha bravo", 40).is_none());
    }

    #[test]
    fn an_empty_query_matches_everything_and_marks_nothing() {
        assert_eq!(marks("", "alpha", 10).as_deref(), Some(""));
        assert_eq!(marks("   ", "alpha", 10).as_deref(), Some(""));
        assert_eq!(score("", "alpha"), Some(0));
    }

    #[test]
    fn quote_anchors_and_inverse_terms() {
        // ' is an exact substring, ^ anchors the start, $ the end.
        assert!(score("'pha", "alpha").is_some());
        assert!(score("'phx", "alpha").is_none());
        assert!(score("^alp", "alpha").is_some());
        assert!(score("^lph", "alpha").is_none());
        assert!(score("pha$", "alpha").is_some());
        assert!(score("alp$", "alpha").is_none());
        // ! inverts, and matches as a substring rather than a subsequence.
        assert!(score("!zulu", "alpha").is_some());
        assert!(score("!alp", "alpha").is_none());
        // An inverse term highlights nothing.
        assert_eq!(marks("!zulu", "alpha", 10).as_deref(), Some(""));
    }

    #[test]
    fn terms_are_anded_and_groups_are_ored() {
        // Both terms must match within a group.
        assert!(score("al ph", "alpha bravo").is_some());
        assert!(score("al zz", "alpha bravo").is_none());
        // Either group may match.
        assert!(score("zz | br", "alpha bravo").is_some());
        assert!(score("zz | yy", "alpha bravo").is_none());
    }

    #[test]
    fn matching_is_smart_case() {
        // All-lowercase query folds case.
        assert!(score("alpha", "ALPHA").is_some());
        // An uppercase character in the query makes it case sensitive.
        assert!(score("Alpha", "ALPHA").is_none());
        assert!(score("ALPHA", "ALPHA").is_some());
    }

    #[test]
    fn scores_rank_better_matches_higher() {
        // A match at the start beats one further in.
        assert!(score("al", "alpha").unwrap() > score("al", "zz alpha").unwrap());
        // An exact term beats a fuzzy one by a wide margin.
        assert!(score("'alp", "alpha").unwrap() > score("alp", "alpha").unwrap());
        // A match after a word boundary beats one mid-word.
        assert!(score("b", "a b").unwrap() > score("b", "ab").unwrap());
        // Each character landing on a word boundary is worth more than each
        // one being adjacent (8 against 6), so a scattered-but-aligned match
        // outscores a contiguous one: "a l p" beats "alpha" for "alp".
        assert_eq!(score("alp", "alpha"), Some(24));
        assert_eq!(score("alp", "a l p"), Some(26));
    }

    #[test]
    fn style_directives_are_invisible_to_matching() {
        // The style itself must not match, and must not occupy columns: the
        // caret sits under the 'a' of alpha, which is column 0 on screen.
        assert!(score("dim", "#[dim]alpha").is_none());
        assert_eq!(marks("a", "#[dim]alpha", 20).as_deref(), Some("^"));
        // ## is an escaped literal #, and #[ after an even run is literal too.
        assert_eq!(marks("#", "##alpha", 20).as_deref(), Some("^"));
        assert!(score("[", "##[alpha").is_some());
    }

    #[test]
    fn right_aligned_text_marks_the_column_it_is_drawn_in() {
        // The right-aligned part is drawn at the end of the width, so its
        // highlight must land there and not at its position in the string.
        let m = marks("zz", "a#[align=right]zz", 10).unwrap();
        assert_eq!(m.len(), 10, "expected the marks at the right edge: {m:?}");
        assert!(m.ends_with("^^"));
    }

    #[test]
    fn multibyte_characters_decode_and_match_as_one_character() {
        // Both the pattern and the text are decoded as UTF-8 and compared by
        // their bytes, so a multibyte character matches itself and not another.
        // (How many columns it then highlights follows its display width,
        // which comes from the locale the server sets at startup, so it is not
        // asserted here.)
        assert!(marks("é", "café", 10).is_some());
        assert!(marks("é", "cafe", 10).is_none());
        assert!(marks("横", "a横b", 10).is_some());
        assert!(marks("横", "abc", 10).is_none());
        // Case folding is ASCII-only: a multibyte character is never folded.
        assert!(marks("É", "café", 10).is_none());
    }
}

