//! Anti-drift gate: every free `fn` in `src/**.rs` must correspond to a
//! function that actually exists in the vendored tmux C source under
//! `vendor/tmux/`. ztmux is a *port* of tmux — a Rust function whose name has
//! no C counterpart is a "fake fn": an invented helper that inflates the port's
//! apparent completeness without porting anything.
//!
//! This test FAILS THE BUILD when such a function is added, so a contributor (or
//! a bot) cannot quietly slip in `helper_to_make_it_work` and claim progress.
//! Pre-existing fake fns (Rust glue, libc/libevent wrappers, transpile-era
//! helpers) are frozen in `tests/data/fake_fn_allowlist.txt`. Anything NOT on
//! that list and NOT in the C source fails. The list is an AUDIT TRAIL of
//! accepted exceptions, not a free pass — the goal is to shrink it as helpers
//! are inlined or replaced by real ports, never to grow it.
//!
//! Methods inside `impl`/`trait` blocks are skipped (they map onto C's
//! struct-of-fn-pointers indirection, which doesn't preserve the name); only
//! module-level free functions count. `#[cfg(test)] mod tests { … }` is skipped.
//!
//! Regenerate the allowlist after intentional churn:
//!   cargo test --test ported_fn_names_match_c -- --nocapture 2>&1 \
//!     | sed -n 's/^FAKE-FN //p' | sort -u > tests/data/fake_fn_allowlist.txt

// Auxiliary test infra: the module docs mention bare identifiers as prose, and
// the `match { Ok => .., Err => return }` idiom reads fine here.
#![allow(clippy::doc_markdown, clippy::manual_let_else)]

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn collect_files(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, ext, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some(ext) {
            out.push(path);
        }
    }
}

/// Extract module-level free-`fn` names from Rust source. Tracks brace depth
/// char-by-char (ignoring braces inside comments/strings/char-literals) so a
/// `fn` at depth 0 is a free function and anything deeper is a method or nested
/// item. `mod tests`/`mod test` blocks are skipped wholesale.
fn collect_free_fns(src: &str) -> Vec<(String, usize)> {
    let mut fns = Vec::new();
    let mut depth: i32 = 0;
    let mut in_test_mod = false;
    let mut test_mod_depth: i32 = 0;
    let mut in_block_comment: i32 = 0;

    for (lineno, line) in src.lines().enumerate() {
        let lineno = lineno + 1;
        let trimmed = line.trim_start();

        if depth == 0
            && (trimmed.starts_with("mod tests {")
                || trimmed.starts_with("mod test {")
                || trimmed.starts_with("mod tests{")
                || trimmed.starts_with("mod test{"))
        {
            in_test_mod = true;
            test_mod_depth = depth + 1;
        }

        let bytes = line.as_bytes();
        let mut i = 0;
        let mut delta: i32 = 0;
        while i < bytes.len() {
            let b = bytes[i];
            if in_block_comment > 0 {
                if b == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    in_block_comment -= 1;
                    i += 2;
                    continue;
                }
                if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                    in_block_comment += 1;
                    i += 2;
                    continue;
                }
                i += 1;
                continue;
            }
            match b {
                b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => break,
                b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                    in_block_comment += 1;
                    i += 2;
                }
                b'"' => {
                    i += 1;
                    while i < bytes.len() {
                        let c = bytes[i];
                        if c == b'\\' {
                            i += 2;
                            continue;
                        }
                        if c == b'"' {
                            i += 1;
                            break;
                        }
                        i += 1;
                    }
                }
                b'r' if i + 1 < bytes.len() && (bytes[i + 1] == b'"' || bytes[i + 1] == b'#') => {
                    let mut hashes = 0;
                    let mut j = i + 1;
                    while j < bytes.len() && bytes[j] == b'#' {
                        hashes += 1;
                        j += 1;
                    }
                    if j < bytes.len() && bytes[j] == b'"' {
                        i = j + 1;
                        loop {
                            if i >= bytes.len() {
                                break;
                            }
                            if bytes[i] == b'"' {
                                let mut closed = 0;
                                let mut k = i + 1;
                                while k < bytes.len() && bytes[k] == b'#' && closed < hashes {
                                    closed += 1;
                                    k += 1;
                                }
                                if closed >= hashes {
                                    i = k;
                                    break;
                                }
                            }
                            i += 1;
                        }
                    } else {
                        i += 1;
                    }
                }
                b'\'' => {
                    let mut j = i + 1;
                    let mut found_close = false;
                    let mut escape = false;
                    while j < bytes.len() && j - i < 12 {
                        if !escape && bytes[j] == b'\'' {
                            found_close = true;
                            break;
                        }
                        escape = bytes[j] == b'\\' && !escape;
                        j += 1;
                    }
                    if found_close {
                        i = j + 1;
                    } else {
                        i += 1;
                        while i < bytes.len()
                            && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_')
                        {
                            i += 1;
                        }
                    }
                }
                b'{' => {
                    delta += 1;
                    i += 1;
                }
                b'}' => {
                    delta -= 1;
                    i += 1;
                }
                _ => i += 1,
            }
        }
        let pre_depth = depth;
        depth += delta;
        if in_test_mod && depth < test_mod_depth {
            in_test_mod = false;
        }
        if in_test_mod || pre_depth != 0 {
            continue;
        }

        let stripped = trimmed
            .strip_prefix("pub(crate) ")
            .or_else(|| trimmed.strip_prefix("pub(super) "))
            .unwrap_or_else(|| trimmed.strip_prefix("pub ").unwrap_or(trimmed));
        let stripped = stripped.strip_prefix("unsafe ").unwrap_or(stripped);
        let stripped = stripped.strip_prefix("async ").unwrap_or(stripped);
        let stripped = stripped.strip_prefix(r#"extern "C" "#).unwrap_or(stripped);
        let stripped = stripped.strip_prefix("const ").unwrap_or(stripped);

        if let Some(rest) = stripped.strip_prefix("fn ") {
            let name_end = rest
                .find(|c: char| c == '(' || c == '<' || c.is_whitespace())
                .unwrap_or(0);
            if name_end > 0 {
                let mut name = rest[..name_end].to_string();
                // raw identifier `r#loop` -> `loop`
                if let Some(s) = name.strip_prefix("r#") {
                    name = s.to_string();
                }
                if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    fns.push((name, lineno));
                }
            }
        }
    }
    fns
}

const C_KEYWORDS: &[&str] = &[
    "if", "for", "while", "switch", "return", "else", "do", "sizeof", "static", "extern", "struct",
    "union", "enum", "typedef", "const", "volatile", "inline", "register", "auto", "goto", "break",
    "continue", "case", "default",
];

/// Live index of function names defined in the vendored tmux C sources
/// (`vendor/tmux/*.c` + `vendor/tmux/compat/*.c`). Same heuristic as
/// scripts/gen_port_report.py: a line-initial identifier followed by `(`, with
/// a `{` on this line or within the next few, and not a `;`-terminated
/// prototype.
fn c_fn_names() -> HashSet<String> {
    let mut names = HashSet::new();
    let mut files = Vec::new();
    collect_files(&root().join("vendor/tmux"), "c", &mut files);
    // cmd-parse.y (yacc) defines real functions (cmd_parse_*) in its C sections.
    collect_files(&root().join("vendor/tmux"), "y", &mut files);
    for f in files {
        // Only the flat tmux tree + compat/ shims are ports; skip nested tools/.
        let rel = f.strip_prefix(root()).unwrap_or(&f);
        let comps: Vec<_> = rel.components().collect();
        // vendor/tmux/<file>.c  OR  vendor/tmux/compat/<file>.c
        let depth_ok =
            comps.len() == 3 || (comps.len() == 4 && rel.to_string_lossy().contains("/compat/"));
        if !depth_ok {
            continue;
        }
        let src = match fs::read_to_string(&f) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let lines: Vec<&str> = src.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let first = match line.chars().next() {
                Some(c) => c,
                None => continue,
            };
            if first.is_whitespace() || first == '/' || first == '*' || first == '#' {
                continue;
            }
            // identifier immediately followed by '('
            let name: String = line
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if name.is_empty() {
                continue;
            }
            let after = &line[name.len()..];
            if !after.trim_start().starts_with('(') {
                continue;
            }
            if C_KEYWORDS.contains(&name.as_str()) {
                continue;
            }
            // require a '{' on this line or within ~6 lines (fn definition,
            // not a call or prototype)
            let tail: String = lines[idx..(idx + 7).min(lines.len())].join(" ");
            if !tail.contains('{') {
                continue;
            }
            if line.contains(';') && !line.contains('{') {
                continue;
            }
            names.insert(name);
        }
    }
    names
}

fn load_allowlist() -> HashSet<String> {
    let path = root().join("tests/data/fake_fn_allowlist.txt");
    let src = fs::read_to_string(&path).unwrap_or_default();
    src.lines()
        .map(|l| match l.find('#') {
            Some(i) => &l[..i],
            None => l,
        })
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

#[test]
fn ported_fns_match_c_source() {
    let mut rust_files = Vec::new();
    collect_files(&root().join("src"), "rs", &mut rust_files);
    rust_files.sort();

    let c_names = c_fn_names();
    assert!(
        c_names.len() > 500,
        "C index looks too small ({}) — is vendor/tmux present?",
        c_names.len()
    );
    let allowlist = load_allowlist();

    let mut violations: Vec<(String, String, usize)> = Vec::new();
    for f in &rust_files {
        let rel = f
            .strip_prefix(root())
            .unwrap_or(f)
            .to_string_lossy()
            .to_string();
        let src = match fs::read_to_string(f) {
            Ok(s) => s,
            Err(_) => continue,
        };
        for (name, line) in collect_free_fns(&src) {
            // No trailing-`_` leniency: a `foo_` variant of C's `foo` is a
            // deviation from the C name. Solo ones are renamed to the real C
            // name; genuine one-C-fn-split-into-many variants (xstrdup_/__/___)
            // are listed explicitly in the allowlist so they stay visible and
            // get burned down, not silently accepted by a pattern rule.
            if c_names.contains(&name) || allowlist.contains(&name) {
                continue;
            }
            violations.push((name, rel.clone(), line));
        }
    }

    if !violations.is_empty() {
        violations.sort();
        for (name, file, line) in &violations {
            // `FAKE-FN <name>` is greppable for regenerating the allowlist.
            eprintln!("FAKE-FN {name}  ({file}:{line}) — no tmux C counterpart");
        }
        panic!(
            "{} free fn(s) in src/ have no function of that name in vendor/tmux \
             and are not in tests/data/fake_fn_allowlist.txt. Each is a possible \
             \"fake fn\" that fakes port progress. Port it against the C source, \
             inline it, or (with justification) add it to the allowlist — see the \
             header of that file and this test's module docs.",
            violations.len()
        );
    }
}
