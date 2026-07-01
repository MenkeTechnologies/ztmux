#!/usr/bin/env python3
"""Annotate ported Rust functions with the C source location AND full C signature.

Every Rust function in `src/**/*.rs` whose name matches a function in the tmux C
sources (`vendor/tmux/**/*.c`) gets a one-line comment citing the C file, line,
and the *full C signature* — return type, name, and parameter types **and
names** — e.g.:

    /// C `vendor/tmux/grid.c:320`: `struct grid *grid_create(u_int sx, u_int sy, u_int hlimit)`
    pub unsafe fn grid_create(sx: u32, sy: u32, hlimit: u32) -> *mut grid {

The signature (not just the name) is the point: it lets a reviewer diff the Rust
parameter names against the C parameter names and catch "fake" params — renamed,
reordered, dropped, or invented arguments that a bare `grid_create()` citation
would hide. This mirrors the zshrs port's `C signature: …` annotations.

The C index is built by the same heuristic as `gen_port_report.py` (a
line-initial identifier followed by `(`, with a `{` within a few lines). Which C
location is cited, per Rust fn:

  1. the C definition whose file stem matches the Rust file's stem
     (grid.rs -> grid.c, cmd_kill_pane.rs -> cmd-kill-pane.c); else
  2. if the name is defined in exactly one C file, that one; else
  3. skip (ambiguous cross-file name — better no citation than a wrong one).

`///` doc citations (zshrs style); path and signature are backticked code
spans so rustdoc and clippy doc-lints stay quiet. Idempotent and self-updating: a function that already has a
`// vendor/tmux/…` citation directly above it has that line REPLACED with the
current signature, so re-running refreshes stale citations and fills gaps.

Usage:  python3 scripts/annotate_c_links.py [--check]
        --check exits non-zero if any citation is missing/outdated (writes nothing).
"""
from __future__ import annotations
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import gen_port_report as g  # noqa: E402  (reuse ROOT / RS_DIRS / RE_RS_FN / C walk config)

CITE_PREFIX = "vendor/tmux/"
# Matches an existing citation, whether the old plain-`//` form or the new
# `///` doc form, so re-runs replace it in place.
RE_EXISTING = re.compile(r"^\s*//(?:/)?\s*(?:C\s+`)?(?:vendor/tmux/|tmux/)\S+\.c:\d+")

C_KEYWORDS = {
    "if", "for", "while", "switch", "return", "else", "do", "sizeof", "static",
    "extern", "struct", "union", "enum", "typedef", "const", "volatile", "inline",
    "register", "auto", "goto", "break", "continue", "case", "default",
}
RE_C_NAME = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)\s*\(")


def c_signature(lines: list[str], idx: int, name: str) -> str:
    """Reconstruct `<return type> <name>(<params>)` for the C fn defined at
    line index `idx` (0-based, the line that starts with `name(`)."""
    # Parameters: accumulate from this line until the matching ')'.
    buf = []
    depth = 0
    started = False
    i = idx
    while i < len(lines) and i < idx + 40:
        for ch in lines[i]:
            buf.append(ch)
            if ch == "(":
                depth += 1
                started = True
            elif ch == ")":
                depth -= 1
        if started and depth == 0:
            break
        buf.append(" ")
        i += 1
    name_and_params = " ".join("".join(buf).split())  # collapse whitespace

    # Return type: the non-empty line(s) just above, if they look like a type
    # (tmux puts the return type on its own line above the name).
    rt = ""
    j = idx - 1
    while j >= 0:
        prev = lines[j].strip()
        if prev == "":
            j -= 1
            continue
        if (
            prev.startswith(("/", "*", "#"))
            or prev.endswith((";", "{", "}", ":"))
            or "(" in prev
        ):
            break
        rt = prev
        break

    if not rt:
        return name_and_params
    # `struct grid *` + name -> `struct grid *grid_create`, not `* grid_create`.
    sep = "" if rt.endswith("*") else " "
    return f"{rt}{sep}{name_and_params}"


def walk_c_sigs() -> dict[str, list[tuple[str, int, str]]]:
    """name -> [(rel_path, 1-based line, full signature)]."""
    idx: dict[str, list[tuple[str, int, str]]] = {}
    for c in g.c_source_paths():
        if c.stem in getattr(g, "C_EXCLUDE_STEMS", set()):
            continue
        rel = c.relative_to(g.ROOT).as_posix()
        try:
            lines = c.read_text(errors="replace").splitlines()
        except OSError:
            continue
        for i, line in enumerate(lines):
            if not line or line[0].isspace() or line[0] in "/*#":
                continue
            m = RE_C_NAME.match(line)
            if not m:
                continue
            name = m.group(1)
            if name in C_KEYWORDS:
                continue
            tail = " ".join(lines[i:i + 7])
            if "{" not in tail:
                continue
            if ";" in line and "{" not in line:
                continue
            idx.setdefault(name, []).append((rel, i + 1, c_signature(lines, i, name)))
    return idx


def c_stem_for(rust_path: Path) -> str:
    return rust_path.stem.replace("_", "-")


def choose(name: str, want_stem: str, cidx: dict) -> tuple[str, int, str] | None:
    locs = cidx.get(name)
    if not locs:
        return None
    same = [t for t in locs if Path(t[0]).stem == want_stem]
    if same:
        return same[0]
    if len(locs) == 1:
        return locs[0]
    return None  # ambiguous


def annotate_file(path: Path, cidx: dict, check: bool) -> tuple[int, int]:
    text = path.read_text()
    lines = text.splitlines()
    want_stem = c_stem_for(path)
    out: list[str] = []
    changed = stale = 0
    for line in lines:
        m = g.RE_RS_FN.match(line)
        if m:
            loc = choose(m.group(1), want_stem, cidx)
            if loc is not None:
                rel, ln, sig = loc
                indent = line[: len(line) - len(line.lstrip())]
                # zshrs-style `///` doc citation. Path and signature are code
                # spans (backticks) so rustdoc/clippy's doc_markdown lint stays
                # quiet and the `*`/`(...)` don't render as markdown.
                want = f"{indent}/// C `{rel}:{ln}`: `{sig}`"
                # Replace an existing citation directly above, else insert.
                if out and RE_EXISTING.match(out[-1]):
                    if out[-1] != want:
                        stale += 1
                        if not check:
                            out[-1] = want
                            changed += 1
                else:
                    stale += 1
                    if not check:
                        out.append(want)
                        changed += 1
        out.append(line)
    if changed and not check:
        path.write_text("\n".join(out) + ("\n" if text.endswith("\n") else ""))
    return changed, stale


def main() -> int:
    check = "--check" in sys.argv[1:]
    cidx = walk_c_sigs()
    total_changed = total_stale = files = 0
    for d in g.RS_DIRS:
        for f in sorted(d.rglob("*.rs")):
            changed, stale = annotate_file(f, cidx, check)
            total_changed += changed
            total_stale += stale
            if changed:
                files += 1
    if check:
        print(f"{total_stale} functions missing/outdated a C-signature citation")
        return 1 if total_stale else 0
    print(f"wrote {total_changed} C-signature citations across {files} files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
