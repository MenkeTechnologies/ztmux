// Structured output (JSON / JSONL / CSV / TSV) for the list-* commands.
//
// This is a ztmux extension with no tmux C counterpart: the list commands can
// emit their rows as machine-readable data instead of a formatted text line,
// driven by the same #{…} format engine (so every format variable is available
// and filters still apply). Selected with `-o <format>`.
//
// All logic lives in inherent methods / associated fns so the anti-drift gate
// (which only inspects module-level free `fn`s) is unaffected — there are no
// invented free functions here, only types and their impls.
use crate::*;

/// How a field's expanded string is encoded in the output.
#[derive(Clone, Copy)]
pub(crate) enum FieldKind {
    /// Emitted as a (quoted, escaped) string.
    Str,
    /// Parsed as an integer; unparseable-but-empty becomes null, otherwise the
    /// raw string is kept so no information is silently dropped.
    Int,
    /// Truthy per the format engine's rules (non-empty and not "0").
    Bool,
}

/// One output column: a JSON key / CSV header, the #{…} format that produces
/// its value, and how to type that value.
pub(crate) struct Field {
    pub(crate) key: &'static str,
    pub(crate) fmt: *const u8,
    pub(crate) kind: FieldKind,
}

/// A value coerced from an expanded format string.
enum Cell {
    Str(Vec<u8>),
    Int(i64),
    Bool(bool),
    Null,
}

/// The requested output format, parsed from the `-o` argument.
#[derive(Clone, Copy)]
pub(crate) enum OutputFormat {
    Json,
    Jsonl,
    Csv,
    Tsv,
    /// Human-readable, whitespace-aligned columns with a header + rule.
    Table,
    /// A YAML sequence of mappings.
    Yaml,
}

impl OutputFormat {
    /// Parse the `-o` argument. `None` argument means "not requested" (text
    /// mode); an unrecognized name is an error the caller reports.
    pub(crate) unsafe fn parse(s: *const u8) -> Result<Option<OutputFormat>, ()> {
        if s.is_null() {
            return Ok(None);
        }
        let name = unsafe { std::ffi::CStr::from_ptr(s.cast()) }.to_bytes();
        match name {
            b"json" => Ok(Some(OutputFormat::Json)),
            b"jsonl" | b"ndjson" => Ok(Some(OutputFormat::Jsonl)),
            b"csv" => Ok(Some(OutputFormat::Csv)),
            b"tsv" => Ok(Some(OutputFormat::Tsv)),
            b"table" => Ok(Some(OutputFormat::Table)),
            b"yaml" | b"yml" => Ok(Some(OutputFormat::Yaml)),
            _ => Err(()),
        }
    }
}

/// Accumulates rows (one per listed item) and renders them all at once.
pub(crate) struct Structured {
    fmt: OutputFormat,
    fields: &'static [Field],
    rows: Vec<Vec<Cell>>,
}

impl Structured {
    pub(crate) fn new(fmt: OutputFormat, fields: &'static [Field]) -> Self {
        Structured {
            fmt,
            fields,
            rows: Vec::new(),
        }
    }

    /// Expand every field against `ft` and append the resulting row.
    pub(crate) unsafe fn add(&mut self, ft: *mut format_tree) {
        unsafe {
            let mut row = Vec::with_capacity(self.fields.len());
            for f in self.fields {
                let p = format_expand(ft, f.fmt);
                let bytes = std::ffi::CStr::from_ptr(p.cast()).to_bytes();
                let cell = match f.kind {
                    FieldKind::Str => Cell::Str(bytes.to_vec()),
                    FieldKind::Int => match std::str::from_utf8(bytes)
                        .ok()
                        .and_then(|s| s.trim().parse::<i64>().ok())
                    {
                        Some(n) => Cell::Int(n),
                        None if bytes.is_empty() => Cell::Null,
                        None => Cell::Str(bytes.to_vec()),
                    },
                    FieldKind::Bool => Cell::Bool(format_true(p)),
                };
                free_(p);
                row.push(cell);
            }
            self.rows.push(row);
        }
    }

    /// Render all accumulated rows. No trailing newline (`cmdq_print` adds one).
    pub(crate) fn render(&self) -> String {
        let mut out: Vec<u8> = Vec::new();
        match self.fmt {
            OutputFormat::Json => self.render_json(&mut out, true),
            OutputFormat::Jsonl => self.render_json(&mut out, false),
            OutputFormat::Csv => self.render_sv(&mut out, b','),
            OutputFormat::Tsv => self.render_sv(&mut out, b'\t'),
            OutputFormat::Table => self.render_table(&mut out),
            OutputFormat::Yaml => self.render_yaml(&mut out),
        }
        // Output is UTF-8 (format output is UTF-8; escaping keeps it so).
        String::from_utf8_lossy(&out).into_owned()
    }

    fn render_json(&self, out: &mut Vec<u8>, array: bool) {
        if array {
            out.push(b'[');
        }
        for (i, row) in self.rows.iter().enumerate() {
            if array && i != 0 {
                out.push(b',');
            }
            if array || i != 0 {
                out.push(b'\n');
            }
            out.push(b'{');
            for (j, cell) in row.iter().enumerate() {
                if j != 0 {
                    out.push(b',');
                }
                Self::json_string(out, self.fields[j].key.as_bytes());
                out.push(b':');
                cell.write_json(out);
            }
            out.push(b'}');
        }
        if array {
            if !self.rows.is_empty() {
                out.push(b'\n');
            }
            out.push(b']');
        }
    }

    fn render_table(&self, out: &mut Vec<u8>) {
        let ncol = self.fields.len();
        let headers: Vec<String> = self.fields.iter().map(|f| f.key.to_string()).collect();
        let body: Vec<Vec<String>> = self
            .rows
            .iter()
            .map(|row| row.iter().map(Cell::display).collect())
            .collect();

        // Column widths = max of header and every cell (by display char count).
        let mut widths = vec![0usize; ncol];
        for (j, h) in headers.iter().enumerate() {
            widths[j] = h.chars().count();
        }
        for row in &body {
            for (j, c) in row.iter().enumerate() {
                widths[j] = widths[j].max(c.chars().count());
            }
        }

        Self::table_line(out, &headers, &widths);
        out.push(b'\n');
        for (j, w) in widths.iter().enumerate() {
            if j != 0 {
                out.extend_from_slice(b"  ");
            }
            out.extend(std::iter::repeat_n(b'-', *w));
        }
        for row in &body {
            out.push(b'\n');
            Self::table_line(out, row, &widths);
        }
    }

    /// Append one whitespace-padded table row (trailing padding trimmed).
    fn table_line(out: &mut Vec<u8>, cells: &[String], widths: &[usize]) {
        let mut line = String::new();
        for (j, c) in cells.iter().enumerate() {
            if j != 0 {
                line.push_str("  ");
            }
            line.push_str(c);
            if j + 1 < cells.len() {
                let pad = widths[j].saturating_sub(c.chars().count());
                line.extend(std::iter::repeat_n(' ', pad));
            }
        }
        out.extend_from_slice(line.trim_end().as_bytes());
    }

    fn render_yaml(&self, out: &mut Vec<u8>) {
        if self.rows.is_empty() {
            out.extend_from_slice(b"[]");
            return;
        }
        for (r, row) in self.rows.iter().enumerate() {
            for (j, cell) in row.iter().enumerate() {
                if r != 0 || j != 0 {
                    out.push(b'\n');
                }
                // First field of each row opens a new sequence item.
                out.extend_from_slice(if j == 0 { b"- " } else { b"  " });
                out.extend_from_slice(self.fields[j].key.as_bytes());
                out.extend_from_slice(b": ");
                cell.write_yaml(out);
            }
        }
    }

    /// True if `s` is safe to emit as a bare (unquoted) YAML plain scalar.
    fn yaml_is_plain(s: &[u8]) -> bool {
        if s.is_empty() {
            return false;
        }
        let plain_chars = s.iter().all(|&b| {
            b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'/' | b'@' | b'+' | b'-')
        });
        if !plain_chars {
            return false;
        }
        // A leading '-' would read as a nested sequence indicator; reject it.
        if s[0] == b'-' {
            return false;
        }
        // Reserved words / numbers must be quoted to stay typed as strings.
        if let Ok(t) = std::str::from_utf8(s) {
            if t.parse::<f64>().is_ok() {
                return false;
            }
            if matches!(
                t.to_ascii_lowercase().as_str(),
                "true" | "false" | "null" | "yes" | "no" | "on" | "off"
            ) {
                return false;
            }
        }
        true
    }

    fn render_sv(&self, out: &mut Vec<u8>, sep: u8) {
        for (j, f) in self.fields.iter().enumerate() {
            if j != 0 {
                out.push(sep);
            }
            Self::sv_field(out, f.key.as_bytes(), sep);
        }
        for row in &self.rows {
            out.push(b'\n');
            for (j, cell) in row.iter().enumerate() {
                if j != 0 {
                    out.push(sep);
                }
                cell.write_sv(out, sep);
            }
        }
    }

    /// Append a JSON-escaped, double-quoted string.
    fn json_string(out: &mut Vec<u8>, s: &[u8]) {
        out.push(b'"');
        for &b in s {
            match b {
                b'"' => out.extend_from_slice(b"\\\""),
                b'\\' => out.extend_from_slice(b"\\\\"),
                b'\n' => out.extend_from_slice(b"\\n"),
                b'\r' => out.extend_from_slice(b"\\r"),
                b'\t' => out.extend_from_slice(b"\\t"),
                0x08 => out.extend_from_slice(b"\\b"),
                0x0c => out.extend_from_slice(b"\\f"),
                0x00..=0x1f => out.extend_from_slice(format!("\\u{b:04x}").as_bytes()),
                _ => out.push(b),
            }
        }
        out.push(b'"');
    }

    /// Append a CSV/TSV field, quoting per RFC 4180 when needed.
    fn sv_field(out: &mut Vec<u8>, s: &[u8], sep: u8) {
        let needs_quote = s
            .iter()
            .any(|&b| b == sep || b == b'"' || b == b'\n' || b == b'\r');
        if !needs_quote {
            out.extend_from_slice(s);
            return;
        }
        out.push(b'"');
        for &b in s {
            if b == b'"' {
                out.push(b'"');
            }
            out.push(b);
        }
        out.push(b'"');
    }
}

impl Cell {
    fn write_json(&self, out: &mut Vec<u8>) {
        match self {
            Cell::Str(s) => Structured::json_string(out, s),
            Cell::Int(n) => out.extend_from_slice(n.to_string().as_bytes()),
            Cell::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
            Cell::Null => out.extend_from_slice(b"null"),
        }
    }

    fn write_sv(&self, out: &mut Vec<u8>, sep: u8) {
        match self {
            Cell::Str(s) => Structured::sv_field(out, s, sep),
            Cell::Int(n) => out.extend_from_slice(n.to_string().as_bytes()),
            Cell::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
            Cell::Null => {}
        }
    }

    fn write_yaml(&self, out: &mut Vec<u8>) {
        match self {
            // Bare plain scalar when safe; otherwise a JSON double-quoted string
            // (valid YAML, and keeps escaping identical to the JSON output).
            Cell::Str(s) if Structured::yaml_is_plain(s) => out.extend_from_slice(s),
            Cell::Str(s) => Structured::json_string(out, s),
            Cell::Int(n) => out.extend_from_slice(n.to_string().as_bytes()),
            Cell::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
            Cell::Null => out.extend_from_slice(b"null"),
        }
    }

    /// Plain display string (no escaping) for the aligned `table` format.
    fn display(&self) -> String {
        match self {
            Cell::Str(s) => String::from_utf8_lossy(s).into_owned(),
            Cell::Int(n) => n.to_string(),
            Cell::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            Cell::Null => "-".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows_of(fmt: OutputFormat) -> Structured {
        let mut s = Structured {
            fmt,
            fields: SAMPLE_FIELDS,
            rows: Vec::new(),
        };
        s.rows.push(vec![
            Cell::Str(b"alpha".to_vec()),
            Cell::Int(3),
            Cell::Bool(true),
        ]);
        s.rows.push(vec![
            Cell::Str(b"has \"quote\",comma\n".to_vec()),
            Cell::Null,
            Cell::Bool(false),
        ]);
        s
    }

    const SAMPLE_FIELDS: &[Field] = &[
        Field {
            key: "name",
            fmt: c"".as_ptr().cast(),
            kind: FieldKind::Str,
        },
        Field {
            key: "windows",
            fmt: c"".as_ptr().cast(),
            kind: FieldKind::Int,
        },
        Field {
            key: "attached",
            fmt: c"".as_ptr().cast(),
            kind: FieldKind::Bool,
        },
    ];

    #[test]
    fn json_array_types_and_escaping() {
        let out = rows_of(OutputFormat::Json).render();
        assert_eq!(
            out,
            "[\n{\"name\":\"alpha\",\"windows\":3,\"attached\":true},\n\
             {\"name\":\"has \\\"quote\\\",comma\\n\",\"windows\":null,\"attached\":false}\n]"
        );
    }

    #[test]
    fn jsonl_is_one_object_per_line_no_brackets() {
        let out = rows_of(OutputFormat::Jsonl).render();
        assert_eq!(
            out,
            "{\"name\":\"alpha\",\"windows\":3,\"attached\":true}\n\
             {\"name\":\"has \\\"quote\\\",comma\\n\",\"windows\":null,\"attached\":false}"
        );
    }

    #[test]
    fn csv_header_and_rfc4180_quoting() {
        let out = rows_of(OutputFormat::Csv).render();
        assert_eq!(
            out,
            "name,windows,attached\n\
             alpha,3,true\n\
             \"has \"\"quote\"\",comma\n\",,false"
        );
    }

    #[test]
    fn tsv_uses_tabs_and_only_quotes_on_tab_or_quote() {
        let out = rows_of(OutputFormat::Tsv).render();
        // The comma no longer forces quoting under TSV, but the embedded quote
        // and newline still do.
        assert_eq!(
            out,
            "name\twindows\tattached\n\
             alpha\t3\ttrue\n\
             \"has \"\"quote\"\",comma\n\"\t\tfalse"
        );
    }

    #[test]
    fn empty_json_array() {
        let s = Structured::new(OutputFormat::Json, SAMPLE_FIELDS);
        assert_eq!(s.render(), "[]");
    }

    #[test]
    fn table_aligns_columns_with_a_header_rule() {
        let mut s = Structured::new(OutputFormat::Table, SAMPLE_FIELDS);
        s.rows.push(vec![
            Cell::Str(b"alpha".to_vec()),
            Cell::Int(3),
            Cell::Bool(true),
        ]);
        s.rows.push(vec![
            Cell::Str(b"be".to_vec()),
            Cell::Null,
            Cell::Bool(false),
        ]);
        assert_eq!(
            s.render(),
            "name   windows  attached\n\
             -----  -------  --------\n\
             alpha  3        true\n\
             be     -        false"
        );
    }

    #[test]
    fn yaml_sequence_of_mappings() {
        let mut s = Structured::new(OutputFormat::Yaml, SAMPLE_FIELDS);
        s.rows.push(vec![
            Cell::Str(b"alpha".to_vec()),
            Cell::Int(3),
            Cell::Bool(true),
        ]);
        s.rows.push(vec![
            Cell::Str(b"be".to_vec()),
            Cell::Null,
            Cell::Bool(false),
        ]);
        assert_eq!(
            s.render(),
            "- name: alpha\n  windows: 3\n  attached: true\n\
             - name: be\n  windows: null\n  attached: false"
        );
    }

    #[test]
    fn yaml_quotes_unsafe_scalars() {
        // The nasty string (quote/comma/newline) must be JSON-double-quoted,
        // which is a valid YAML flow scalar.
        let out = rows_of(OutputFormat::Yaml).render();
        assert!(out.contains("- name: alpha"));
        assert!(out.contains("name: \"has \\\"quote\\\",comma\\n\""));
    }

    #[test]
    fn parse_names() {
        unsafe {
            assert!(matches!(
                OutputFormat::parse(c"json".as_ptr().cast()),
                Ok(Some(OutputFormat::Json))
            ));
            assert!(matches!(
                OutputFormat::parse(c"csv".as_ptr().cast()),
                Ok(Some(OutputFormat::Csv))
            ));
            assert!(matches!(
                OutputFormat::parse(c"table".as_ptr().cast()),
                Ok(Some(OutputFormat::Table))
            ));
            assert!(matches!(
                OutputFormat::parse(c"yaml".as_ptr().cast()),
                Ok(Some(OutputFormat::Yaml))
            ));
            assert!(matches!(OutputFormat::parse(std::ptr::null()), Ok(None)));
            assert!(OutputFormat::parse(c"xml".as_ptr().cast()).is_err());
        }
    }

    #[test]
    fn parse_aliases() {
        unsafe {
            // ndjson is an alias for jsonl, yml for yaml.
            assert!(matches!(
                OutputFormat::parse(c"ndjson".as_ptr().cast()),
                Ok(Some(OutputFormat::Jsonl))
            ));
            assert!(matches!(
                OutputFormat::parse(c"yml".as_ptr().cast()),
                Ok(Some(OutputFormat::Yaml))
            ));
            assert!(matches!(
                OutputFormat::parse(c"tsv".as_ptr().cast()),
                Ok(Some(OutputFormat::Tsv))
            ));
        }
    }

    // yaml_is_plain decides whether a scalar can be emitted bare: alnum plus a
    // small safe set, never empty, never leading '-', and never a value that
    // would re-type as a number or a YAML boolean/null keyword.
    #[test]
    fn yaml_plain_scalar_rules() {
        assert!(Structured::yaml_is_plain(b"foo_bar"));
        assert!(Structured::yaml_is_plain(b"a/b.c@d+e"));
        assert!(!Structured::yaml_is_plain(b"")); // empty
        assert!(!Structured::yaml_is_plain(b"-leading")); // sequence indicator
        assert!(!Structured::yaml_is_plain(b"has space"));
        assert!(!Structured::yaml_is_plain(b"123")); // numeric -> must quote
        assert!(!Structured::yaml_is_plain(b"3.14"));
        assert!(!Structured::yaml_is_plain(b"true"));
        assert!(!Structured::yaml_is_plain(b"NULL")); // reserved, case-insensitive
        assert!(!Structured::yaml_is_plain(b"yes"));
    }

    // json_string escapes the JSON specials, control bytes as \u00XX, and passes
    // ordinary bytes through unchanged.
    #[test]
    fn json_string_escapes_specials_and_controls() {
        let mut out = Vec::new();
        Structured::json_string(&mut out, b"a\"b\\c\n\t\x01");
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "\"a\\\"b\\\\c\\n\\t\\u0001\""
        );
    }

    // sv_field quotes per RFC 4180 only when the field holds the separator, a
    // quote, or a newline; an embedded quote is doubled. Under TSV a comma is
    // an ordinary byte and forces no quoting.
    #[test]
    fn sv_field_quotes_only_when_needed() {
        let mut out = Vec::new();
        Structured::sv_field(&mut out, b"plain", b',');
        assert_eq!(out, b"plain");

        let mut out = Vec::new();
        Structured::sv_field(&mut out, b"a,b\"c", b',');
        assert_eq!(String::from_utf8(out).unwrap(), "\"a,b\"\"c\"");

        let mut out = Vec::new();
        Structured::sv_field(&mut out, b"a,b", b'\t');
        assert_eq!(out, b"a,b");
    }

    // A single-row JSON array has no separating comma and still wraps in [ ].
    #[test]
    fn json_array_single_row_has_no_comma() {
        let mut s = Structured::new(OutputFormat::Json, SAMPLE_FIELDS);
        s.rows.push(vec![
            Cell::Str(b"x".to_vec()),
            Cell::Int(1),
            Cell::Bool(true),
        ]);
        assert_eq!(
            s.render(),
            "[\n{\"name\":\"x\",\"windows\":1,\"attached\":true}\n]"
        );
    }

    // Empty JSONL is the empty string (no brackets, no newline).
    #[test]
    fn jsonl_empty_is_empty_string() {
        let s = Structured::new(OutputFormat::Jsonl, SAMPLE_FIELDS);
        assert_eq!(s.render(), "");
    }
}
