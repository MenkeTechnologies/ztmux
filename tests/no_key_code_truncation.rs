//! Anti-drift gate: a `key_code` must never be truncated before it is dispatched.
//!
//! tmux dispatches keys with `switch (key)` over the full 64-bit `key_code`. Ported
//! matches use byte literals (`b'x'`, `b'\r'`), so writing `match key as u8` silently
//! throws away the top bits — and every `KEYC_*` code whose low byte happens to equal a
//! command letter then *executes that command*.
//!
//! The codes are laid out sequentially from `KEYC_BASE` (0x10e000), so the collisions
//! are not exotic; 18 real keys alias a command letter in the mode-tree key handlers:
//!
//!   * `KEYC_MOUSEUP11_STATUS_DEFAULT` (0x10e078) -> `'x'` — the **Kill** prompt
//!   * `KEYC_TRIPLECLICK7_STATUS_LEFT` (0x10e178) -> `'x'` — the **Kill** prompt
//!   * `KEYC_DOUBLECLICK11_PANE`       (0x10e158) -> `'X'` — **Kill Tagged**
//!   * `KEYC_MOUSEMOVE_BORDER`         (0x10e00d) -> `'\r'` — run the command on the row
//!
//! i.e. a mouse event could kill a window or a session. The handlers now gate on
//! `key < 0x80` so only a genuine bare ASCII byte reaches the byte-literal arms, exactly
//! as C's full-width `switch` does.
//!
//! `mode_tree.rs` shows the other correct shape: compare against `u64` constants.

use std::fs;
use std::path::{Path, PathBuf};

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

/// Strip a `//` comment so this file's own prose never trips the scan.
fn code(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

#[test]
fn key_codes_are_never_truncated_before_dispatch() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut paths = Vec::new();
    rs_files(&root, &mut paths);
    paths.sort();
    assert!(!paths.is_empty(), "no Rust sources found under src/");

    let mut violations = Vec::new();
    for path in &paths {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        for (i, raw) in text.lines().enumerate() {
            let line = code(raw);
            // Dispatching on a narrowed key: `match key as u8`, `match key as u32`, ...
            if line.contains("match") && line.contains("key as u") {
                let rel = path.strip_prefix(&root).unwrap_or(path).display();
                violations.push(format!(
                    "src/{rel}:{}: dispatches on a truncated key_code (`{}`). A KEYC_* code \
                     would alias an ASCII command — KEYC_MOUSEUP11_STATUS_DEFAULT ends in \
                     0x78 ('x', Kill). Match the full key_code, or gate on `key < 0x80`.",
                    i + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "key_code truncated before dispatch ({} site(s)):\n{}",
        violations.len(),
        violations.join("\n")
    );
}
