// Copyright (c) 2009 Joshua Elsasser <josh@elsasser.org>
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
use std::borrow::Cow;

use crate::grid_attr;

#[rustfmt::skip]
/// C `vendor/tmux/attributes.c:26`: `const char *attributes_tostring(int attr)`
pub fn attributes_tostring(attr: grid_attr) -> Cow<'static, str> {
    if attr.is_empty() {
        return Cow::Borrowed("none");
    }

    Cow::Owned(format!(
        "{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
        if attr.intersects(grid_attr::GRID_ATTR_CHARSET) { "acs," } else { "" },
        if attr.intersects(grid_attr::GRID_ATTR_BRIGHT) { "bright," } else { "" },
        if attr.intersects(grid_attr::GRID_ATTR_DIM ) { "dim," } else { "" },
        if attr.intersects(grid_attr::GRID_ATTR_UNDERSCORE) { "underscore," } else { "" },
        if attr.intersects(grid_attr::GRID_ATTR_BLINK) { "blink," } else { "" },
        if attr.intersects(grid_attr::GRID_ATTR_REVERSE ) { "reverse," } else { "" },
        if attr.intersects(grid_attr::GRID_ATTR_HIDDEN) { "hidden," } else { "" },
        if attr.intersects(grid_attr::GRID_ATTR_ITALICS ) { "italics," } else { "" },
        if attr.intersects(grid_attr::GRID_ATTR_STRIKETHROUGH) { "strikethrough," } else { "" },
        if attr.intersects(grid_attr::GRID_ATTR_UNDERSCORE_2) { "double-underscore," } else { "" },
        if attr.intersects(grid_attr::GRID_ATTR_UNDERSCORE_3) { "curly-underscore," } else { "" },
        if attr.intersects(grid_attr::GRID_ATTR_UNDERSCORE_4) { "dotted-underscore," } else { "" },
        if attr.intersects(grid_attr::GRID_ATTR_UNDERSCORE_5) { "dashed-underscore," } else { "" },
        if attr.intersects(grid_attr::GRID_ATTR_OVERLINE) { "overline," } else { "" },
    ))
}

/// C `vendor/tmux/attributes.c:57`: `int attributes_fromstring(const char *str)`
pub fn attributes_fromstring(str: &str) -> Result<grid_attr, ()> {
    struct table_entry {
        name: &'static str,
        attr: grid_attr,
    }

    #[rustfmt::skip]
    const TABLE: [table_entry; 15] = [
        table_entry { name: "acs", attr: grid_attr::GRID_ATTR_CHARSET, },
        table_entry { name: "bright", attr: grid_attr::GRID_ATTR_BRIGHT, },
        table_entry { name: "bold", attr: grid_attr::GRID_ATTR_BRIGHT, },
        table_entry { name: "dim", attr: grid_attr::GRID_ATTR_DIM, },
        table_entry { name: "underscore", attr: grid_attr::GRID_ATTR_UNDERSCORE, },
        table_entry { name: "blink", attr: grid_attr::GRID_ATTR_BLINK, },
        table_entry { name: "reverse", attr: grid_attr::GRID_ATTR_REVERSE, },
        table_entry { name: "hidden", attr: grid_attr::GRID_ATTR_HIDDEN, },
        table_entry { name: "italics", attr: grid_attr::GRID_ATTR_ITALICS, },
        table_entry { name: "strikethrough", attr: grid_attr::GRID_ATTR_STRIKETHROUGH, },
        table_entry { name: "double-underscore", attr: grid_attr::GRID_ATTR_UNDERSCORE_2, },
        table_entry { name: "curly-underscore", attr: grid_attr::GRID_ATTR_UNDERSCORE_3, },
        table_entry { name: "dotted-underscore", attr: grid_attr::GRID_ATTR_UNDERSCORE_4, },
        table_entry { name: "dashed-underscore", attr: grid_attr::GRID_ATTR_UNDERSCORE_5, },
        table_entry { name: "overline", attr: grid_attr::GRID_ATTR_OVERLINE, },
    ];

    // C `delimiters[] = " ,|"` (attributes.c:59). Delimiters are single-byte
    // ASCII; UTF-8 continuation bytes are all >= 0x80 so a delimiter never lands
    // mid-codepoint and byte indices stay on char boundaries.
    fn is_delim(b: u8) -> bool {
        matches!(b, b' ' | b',' | b'|')
    }

    let bytes = str.as_bytes();

    // C attributes.c:84-85 `*str == '\0' || strcspn(str, delimiters) == 0`:
    // reject empty input or a leading delimiter.
    if bytes.is_empty() || is_delim(bytes[0]) {
        return Err(());
    }

    // C attributes.c:86-87 `strchr(delimiters, str[strlen(str) - 1])`: reject a
    // trailing delimiter.
    if is_delim(*bytes.last().unwrap()) {
        return Err(());
    }

    if str.eq_ignore_ascii_case("default") || str.eq_ignore_ascii_case("none") {
        return Ok(grid_attr::empty());
    }

    // C attributes.c:92-106 do/while loop. `end = strcspn(str, delimiters)`
    // takes the run of non-delimiter chars; `str += end + strspn(str + end,
    // delimiters)` then skips the token AND the whole following RUN of
    // delimiters, so consecutive delimiters ("bright,,underscore") collapse
    // rather than yielding an empty token.
    let mut attr = grid_attr::empty();
    let mut pos = 0usize;
    loop {
        let start = pos;
        while pos < bytes.len() && !is_delim(bytes[pos]) {
            pos += 1;
        }
        let token = &str[start..pos];
        let Some(i) = TABLE.iter().position(|t| token.eq_ignore_ascii_case(t.name)) else {
            return Err(());
        };
        attr |= TABLE[i].attr;

        // strspn: skip the run of delimiters after the token.
        while pos < bytes.len() && is_delim(bytes[pos]) {
            pos += 1;
        }
        if pos >= bytes.len() {
            break;
        }
    }

    Ok(attr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attributes_tostring_none() {
        assert_eq!(attributes_tostring(grid_attr::empty()).as_ref(), "none");
    }

    #[test]
    fn test_attributes_tostring_single() {
        assert_eq!(
            attributes_tostring(grid_attr::GRID_ATTR_BRIGHT).as_ref(),
            "bright,"
        );
        assert_eq!(
            attributes_tostring(grid_attr::GRID_ATTR_UNDERSCORE).as_ref(),
            "underscore,"
        );
    }

    #[test]
    fn test_attributes_tostring_combined() {
        // Emitted in table order: bright before underscore, each with a trailing comma.
        let attr = grid_attr::GRID_ATTR_BRIGHT | grid_attr::GRID_ATTR_UNDERSCORE;
        assert_eq!(attributes_tostring(attr).as_ref(), "bright,underscore,");
    }

    #[test]
    fn test_attributes_fromstring_single() {
        // "bold" and "bright" are both aliases for GRID_ATTR_BRIGHT.
        assert_eq!(
            attributes_fromstring("bold"),
            Ok(grid_attr::GRID_ATTR_BRIGHT)
        );
        assert_eq!(
            attributes_fromstring("bright"),
            Ok(grid_attr::GRID_ATTR_BRIGHT)
        );
        assert_eq!(
            attributes_fromstring("underscore"),
            Ok(grid_attr::GRID_ATTR_UNDERSCORE)
        );
    }

    #[test]
    fn test_attributes_fromstring_combined() {
        let expected = grid_attr::GRID_ATTR_BRIGHT | grid_attr::GRID_ATTR_UNDERSCORE;
        // Comma, space and pipe are all valid delimiters.
        assert_eq!(attributes_fromstring("bright,underscore"), Ok(expected));
        assert_eq!(attributes_fromstring("bold underscore"), Ok(expected));
        assert_eq!(attributes_fromstring("bright|underscore"), Ok(expected));
    }

    #[test]
    fn test_attributes_fromstring_none_and_default() {
        assert_eq!(attributes_fromstring("none"), Ok(grid_attr::empty()));
        assert_eq!(attributes_fromstring("default"), Ok(grid_attr::empty()));
    }

    #[test]
    fn test_attributes_fromstring_invalid() {
        // Empty, unknown names and trailing delimiters are rejected.
        assert_eq!(attributes_fromstring(""), Err(()));
        assert_eq!(attributes_fromstring("bogus"), Err(()));
        assert_eq!(attributes_fromstring("bright,"), Err(()));
        assert_eq!(attributes_fromstring(",bright"), Err(()));
    }

    // Every table entry (attributes.c:66) must parse to its flag. "bold" is a
    // second alias for GRID_ATTR_BRIGHT alongside "bright".
    #[test]
    fn test_attributes_fromstring_all_table_entries() {
        let cases: [(&str, grid_attr); 15] = [
            ("acs", grid_attr::GRID_ATTR_CHARSET),
            ("bright", grid_attr::GRID_ATTR_BRIGHT),
            ("bold", grid_attr::GRID_ATTR_BRIGHT),
            ("dim", grid_attr::GRID_ATTR_DIM),
            ("underscore", grid_attr::GRID_ATTR_UNDERSCORE),
            ("blink", grid_attr::GRID_ATTR_BLINK),
            ("reverse", grid_attr::GRID_ATTR_REVERSE),
            ("hidden", grid_attr::GRID_ATTR_HIDDEN),
            ("italics", grid_attr::GRID_ATTR_ITALICS),
            ("strikethrough", grid_attr::GRID_ATTR_STRIKETHROUGH),
            ("double-underscore", grid_attr::GRID_ATTR_UNDERSCORE_2),
            ("curly-underscore", grid_attr::GRID_ATTR_UNDERSCORE_3),
            ("dotted-underscore", grid_attr::GRID_ATTR_UNDERSCORE_4),
            ("dashed-underscore", grid_attr::GRID_ATTR_UNDERSCORE_5),
            ("overline", grid_attr::GRID_ATTR_OVERLINE),
        ];
        for (name, flag) in cases {
            assert_eq!(attributes_fromstring(name), Ok(flag), "name = {name}");
        }
    }

    // Every distinct flag must render to its exact label with a trailing comma
    // (attributes.c:34, %s columns).
    #[test]
    fn test_attributes_tostring_all_labels() {
        let cases: [(grid_attr, &str); 14] = [
            (grid_attr::GRID_ATTR_CHARSET, "acs,"),
            (grid_attr::GRID_ATTR_BRIGHT, "bright,"),
            (grid_attr::GRID_ATTR_DIM, "dim,"),
            (grid_attr::GRID_ATTR_UNDERSCORE, "underscore,"),
            (grid_attr::GRID_ATTR_BLINK, "blink,"),
            (grid_attr::GRID_ATTR_REVERSE, "reverse,"),
            (grid_attr::GRID_ATTR_HIDDEN, "hidden,"),
            (grid_attr::GRID_ATTR_ITALICS, "italics,"),
            (grid_attr::GRID_ATTR_STRIKETHROUGH, "strikethrough,"),
            (grid_attr::GRID_ATTR_UNDERSCORE_2, "double-underscore,"),
            (grid_attr::GRID_ATTR_UNDERSCORE_3, "curly-underscore,"),
            (grid_attr::GRID_ATTR_UNDERSCORE_4, "dotted-underscore,"),
            (grid_attr::GRID_ATTR_UNDERSCORE_5, "dashed-underscore,"),
            (grid_attr::GRID_ATTR_OVERLINE, "overline,"),
        ];
        for (flag, label) in cases {
            assert_eq!(attributes_tostring(flag).as_ref(), label, "flag = {flag:?}");
        }
    }

    // With all fourteen flags set, tostring emits them in fixed table order,
    // comma-separated, with a trailing comma (attributes.c:34).
    #[test]
    fn test_attributes_tostring_all_flags_order() {
        let all = grid_attr::all();
        assert_eq!(
            attributes_tostring(all).as_ref(),
            "acs,bright,dim,underscore,blink,reverse,hidden,italics,\
             strikethrough,double-underscore,curly-underscore,\
             dotted-underscore,dashed-underscore,overline,"
        );
    }

    // Parsing is case-insensitive (strncasecmp, attributes.c:97).
    #[test]
    fn test_attributes_fromstring_case_insensitive() {
        assert_eq!(
            attributes_fromstring("BRIGHT"),
            Ok(grid_attr::GRID_ATTR_BRIGHT)
        );
        assert_eq!(
            attributes_fromstring("Bold"),
            Ok(grid_attr::GRID_ATTR_BRIGHT)
        );
        assert_eq!(
            attributes_fromstring("OverLine"),
            Ok(grid_attr::GRID_ATTR_OVERLINE)
        );
        assert_eq!(
            attributes_fromstring("Double-Underscore"),
            Ok(grid_attr::GRID_ATTR_UNDERSCORE_2)
        );
    }

    // All three delimiters (space, comma, pipe) may be mixed in one string
    // (delimiters[] = " ,|", attributes.c:60).
    #[test]
    fn test_attributes_fromstring_mixed_delimiters() {
        let expected = grid_attr::GRID_ATTR_CHARSET
            | grid_attr::GRID_ATTR_DIM
            | grid_attr::GRID_ATTR_BLINK
            | grid_attr::GRID_ATTR_REVERSE;
        assert_eq!(
            attributes_fromstring("acs,dim|blink reverse"),
            Ok(expected)
        );
    }

    // C attributes.c:105 collapses consecutive delimiters via
    // `str += end + strspn(str + end, delimiters)`, which skips the whole run of
    // delimiters after each token. So "bright,,underscore" parses identically to
    // "bright,underscore", and "bright  dim" to "bright dim" — interior runs of
    // delimiters are collapsed, never yielding an empty token.
    #[test]
    fn test_attributes_fromstring_consecutive_delimiters_collapsed() {
        assert_eq!(
            attributes_fromstring("bright,,underscore"),
            Ok(grid_attr::GRID_ATTR_BRIGHT | grid_attr::GRID_ATTR_UNDERSCORE)
        );
        assert_eq!(
            attributes_fromstring("bright,underscore"),
            attributes_fromstring("bright,,underscore")
        );
        assert_eq!(
            attributes_fromstring("bright  dim"),
            Ok(grid_attr::GRID_ATTR_BRIGHT | grid_attr::GRID_ATTR_DIM)
        );
        // A mixed run of different delimiters between tokens also collapses.
        assert_eq!(
            attributes_fromstring("bright, |dim"),
            Ok(grid_attr::GRID_ATTR_BRIGHT | grid_attr::GRID_ATTR_DIM)
        );
    }

    // A single delimiter, or a leading/trailing delimiter of any kind, is
    // rejected (attributes.c:85-89).
    #[test]
    fn test_attributes_fromstring_delimiter_edges() {
        assert_eq!(attributes_fromstring(","), Err(()));
        assert_eq!(attributes_fromstring(" "), Err(()));
        assert_eq!(attributes_fromstring("|"), Err(()));
        assert_eq!(attributes_fromstring("bright dim "), Err(())); // trailing space
        assert_eq!(attributes_fromstring("bright|dim|"), Err(())); // trailing pipe
        assert_eq!(attributes_fromstring("|bright"), Err(())); // leading pipe
        assert_eq!(attributes_fromstring(" bright"), Err(())); // leading space
    }

    // One unknown token anywhere in an otherwise-valid list fails the whole
    // parse (attributes.c:106 `if (i == nitems(table)) return (-1)`).
    #[test]
    fn test_attributes_fromstring_one_bad_token_fails_all() {
        assert_eq!(attributes_fromstring("bright,bogus,dim"), Err(()));
        assert_eq!(attributes_fromstring("underscore reverse xyzzy"), Err(()));
    }

    // Repeating a flag is idempotent (bitwise OR), so "bright,bright" == bright.
    #[test]
    fn test_attributes_fromstring_repeat_is_idempotent() {
        assert_eq!(
            attributes_fromstring("bright,bright"),
            Ok(grid_attr::GRID_ATTR_BRIGHT)
        );
        assert_eq!(
            attributes_fromstring("dim|dim|dim"),
            Ok(grid_attr::GRID_ATTR_DIM)
        );
    }

    // Round-trip: each distinct flag's tostring label (minus the trailing
    // comma) parses back to the same flag.
    #[test]
    fn test_attributes_tostring_fromstring_roundtrip() {
        for flag in [
            grid_attr::GRID_ATTR_CHARSET,
            grid_attr::GRID_ATTR_BRIGHT,
            grid_attr::GRID_ATTR_DIM,
            grid_attr::GRID_ATTR_UNDERSCORE,
            grid_attr::GRID_ATTR_BLINK,
            grid_attr::GRID_ATTR_REVERSE,
            grid_attr::GRID_ATTR_HIDDEN,
            grid_attr::GRID_ATTR_ITALICS,
            grid_attr::GRID_ATTR_STRIKETHROUGH,
            grid_attr::GRID_ATTR_UNDERSCORE_2,
            grid_attr::GRID_ATTR_UNDERSCORE_3,
            grid_attr::GRID_ATTR_UNDERSCORE_4,
            grid_attr::GRID_ATTR_UNDERSCORE_5,
            grid_attr::GRID_ATTR_OVERLINE,
        ] {
            let s = attributes_tostring(flag);
            let trimmed = s.trim_end_matches(',');
            assert_eq!(attributes_fromstring(trimmed), Ok(flag), "flag = {flag:?}");
        }
    }

    // A comma-joined multi-flag tostring output round-trips through fromstring
    // (delimiters accepted directly; only the trailing comma must be stripped).
    #[test]
    fn test_attributes_multiflag_roundtrip() {
        let attr = grid_attr::GRID_ATTR_BRIGHT
            | grid_attr::GRID_ATTR_UNDERSCORE
            | grid_attr::GRID_ATTR_OVERLINE;
        let s = attributes_tostring(attr);
        assert_eq!(s.as_ref(), "bright,underscore,overline,");
        let trimmed = s.trim_end_matches(',');
        assert_eq!(attributes_fromstring(trimmed), Ok(attr));
    }

    // "default"/"none" are matched case-insensitively against the WHOLE string
    // (attributes.c:90-91, strcasecmp) before tokenizing, so mixed case resolves
    // to the empty attribute set.
    #[test]
    fn test_attributes_fromstring_default_none_case() {
        for s in ["none", "None", "NONE", "nOnE", "default", "Default", "DEFAULT"] {
            assert_eq!(attributes_fromstring(s), Ok(grid_attr::empty()), "s = {s}");
        }
    }

    // "none"/"default" are only special as the entire string. Inside a delimited
    // list they are ordinary tokens, absent from the table (attributes.c:66), so
    // the whole parse fails — they are NOT silent no-ops.
    #[test]
    fn test_attributes_fromstring_none_as_token_rejected() {
        assert_eq!(attributes_fromstring("none,bright"), Err(()));
        assert_eq!(attributes_fromstring("bright,none"), Err(()));
        assert_eq!(attributes_fromstring("default|dim"), Err(()));
    }

    // The complete tostring output of every flag set at once round-trips through
    // fromstring after stripping the single trailing comma (delimiters accepted
    // directly, attributes.c:34 emit order vs :92 parse loop).
    #[test]
    fn test_attributes_fromstring_all_flags_roundtrip() {
        let all = grid_attr::all();
        let s = attributes_tostring(all);
        let trimmed = s.trim_end_matches(',');
        assert_eq!(attributes_fromstring(trimmed), Ok(all));
    }

    // "acs" alone parses to just GRID_ATTR_CHARSET and tostring names it "acs,",
    // pinning the first table entry independently (attributes.c:67, :28).
    #[test]
    fn test_attributes_acs_only() {
        assert_eq!(attributes_fromstring("acs"), Ok(grid_attr::GRID_ATTR_CHARSET));
        assert_eq!(
            attributes_tostring(grid_attr::GRID_ATTR_CHARSET).as_ref(),
            "acs,"
        );
    }
}
