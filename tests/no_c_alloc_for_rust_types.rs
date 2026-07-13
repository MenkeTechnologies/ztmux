//! Anti-drift gate: a struct holding a Rust type with a validity invariant must never
//! be allocated the C way.
//!
//! `Vec`, `String`, `CString` and `Box` all require a non-null data pointer. `xcalloc`
//! (libc `calloc`) and `zeroed()` hand back all-zero bytes, so such a field comes out
//! with a NULL pointer — a value the type system says cannot exist. Nothing complains
//! at the allocation; it detonates later, far from the cause:
//!
//!   * `window_client_modedata` (`item_list: Vec`) was `xcalloc`'d, so the first
//!     `item_list.drain(..)` in `window_client_build` dereferenced null and killed the
//!     server on `choose-client`.
//!   * `window_buffer_itemdata` (`name: String`) and `sixel_image` (`colours: Vec`) had
//!     the same defect, papered over by assigning a fresh empty value before use.
//!
//! This is the failure mode the C-to-Rust ownership migration keeps re-introducing: the
//! moment a `char *` field becomes an owned `CString`, every C-style allocation of its
//! struct silently turns into UB. This gate fails the build at that moment instead.
//!
//! The fix is always the same: build the struct with `Box::new(...)`, initializing every
//! field to a valid Rust value, and reclaim it with `Box::from_raw` so `Drop` runs.

use std::fs;
use std::path::{Path, PathBuf};

/// Field types that carry a non-null (or otherwise non-zero) validity invariant.
const RUST_INVARIANT_TYPES: &[&str] = &["Vec<", "String", "CString", "Box<"];

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Strip a `//` comment so a struct name merely *mentioned* in prose never counts.
fn code(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// Names of structs that declare at least one field of an invariant-carrying type.
fn structs_with_rust_fields(sources: &[(PathBuf, String)]) -> Vec<String> {
    let mut found = Vec::new();

    for (_, text) in sources {
        let mut current: Option<String> = None;
        let mut depth = 0i32;

        for raw in text.lines() {
            let line = code(raw);

            if current.is_none()
                && let Some(rest) = line.split_once("struct ")
                && let Some(name) = rest
                    .1
                    .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .next()
                && !name.is_empty()
                && line.contains('{')
            {
                current = Some(name.to_owned());
                depth = 0;
            }

            let Some(name) = current.clone() else {
                continue;
            };

            depth += line.matches('{').count() as i32;
            depth -= line.matches('}').count() as i32;

            // A field line looks like `name: Type,` — the type sits after the colon.
            if let Some((_, ty)) = line.split_once(':')
                && RUST_INVARIANT_TYPES
                    .iter()
                    .any(|t| ty.trim_start().starts_with(t) || ty.contains(&format!("<{t}")))
                && !found.contains(&name)
            {
                found.push(name.clone());
            }

            if depth <= 0 {
                current = None;
            }
        }
    }

    found
}

/// Every way the codebase allocates or zero-fills a struct C-style.
fn c_allocation_patterns(struct_name: &str) -> Vec<String> {
    vec![
        format!("xcalloc1::<{struct_name}>"),
        format!("xcalloc_::<{struct_name}>"),
        format!("zeroed::<{struct_name}>"),
        format!("MaybeUninit::<{struct_name}>"),
        format!(": {struct_name} = zeroed()"),
        format!("*mut {struct_name} = xcalloc"),
    ]
}

#[test]
fn structs_with_rust_fields_are_never_c_allocated() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut paths = Vec::new();
    rs_files(&root, &mut paths);
    paths.sort();

    let sources: Vec<(PathBuf, String)> = paths
        .iter()
        .filter_map(|p| fs::read_to_string(p).ok().map(|t| (p.clone(), t)))
        .collect();
    assert!(!sources.is_empty(), "no Rust sources found under src/");

    let guarded = structs_with_rust_fields(&sources);
    assert!(
        !guarded.is_empty(),
        "found no structs with Vec/String/CString/Box fields — the scanner is broken"
    );

    let patterns: Vec<(String, &String)> = guarded
        .iter()
        .flat_map(|name| {
            c_allocation_patterns(name)
                .into_iter()
                .map(move |p| (p, name))
        })
        .collect();

    let mut violations = Vec::new();
    for (path, text) in &sources {
        for (i, raw) in text.lines().enumerate() {
            let line = code(raw);
            // Only lines that allocate or zero-fill can violate; skip the rest cheaply.
            if !(line.contains("xcalloc")
                || line.contains("zeroed")
                || line.contains("MaybeUninit"))
            {
                continue;
            }
            for (pattern, name) in &patterns {
                if line.contains(pattern.as_str()) {
                    let rel = path.strip_prefix(&root).unwrap_or(path).display();
                    violations.push(format!(
                        "src/{rel}:{}: `{name}` holds a Rust type with a non-null \
                         invariant but is allocated C-style (`{pattern}`). \
                         Build it with Box::new(..) and free it with Box::from_raw.",
                        i + 1
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "C-style allocation of structs holding Rust types ({} violation(s)):\n{}",
        violations.len(),
        violations.join("\n")
    );
}
