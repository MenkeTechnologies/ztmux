#!/usr/bin/env python3
"""Insert C back-link comments above ported Rust functions.

Every Rust function in `src/**/*.rs` whose name matches a function in the tmux
C sources (`vendor/tmux/**/*.c`) gets a one-line comment citing the C file and
line where that function is defined, e.g.:

    // vendor/tmux/grid.c:142  grid_create()
    pub unsafe fn grid_create(sx: u32, sy: u32, hlimit: u32) -> *mut grid {

This mirrors the per-function C citations in the zshrs port and makes the
mapping navigable from the Rust side. The citation form `vendor/tmux/<f>.c:<n>`
is exactly what `gen_port_report.py` recognizes (RE_C_PATH_CITATION), so these
comments also feed the port report's "cited C paths" signal.

The C function index is reused verbatim from `gen_port_report.py` so citations
stay consistent with the report. Which C location is cited, per Rust fn:

  1. the C definition whose file stem matches the Rust file's stem
     (grid.rs -> grid.c, cmd_kill_pane.rs -> cmd-kill-pane.c); else
  2. if the name is defined in exactly one C file, that one; else
  3. skip (ambiguous cross-file name — better no citation than a wrong one).

Plain `//` comments are used (not `///`) so nothing attaches as rustdoc and the
strict clippy doc-lints stay quiet. The script is idempotent: a function that
already has a `vendor/tmux/...` citation directly above it is left untouched, so
re-running only fills in gaps (e.g. after new functions are ported).

Usage:  python3 scripts/annotate_c_links.py [--check]
        --check exits non-zero if any citation is missing (for CI), writing nothing.
"""
from __future__ import annotations
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import gen_port_report as g  # noqa: E402  (reuse the report's C index + Rust fn regex)

CITE = "vendor/tmux/"


def c_stem_for(rust_path: Path) -> str:
    """Rust file stem -> expected tmux C file stem (underscores back to dashes)."""
    return rust_path.stem.replace("_", "-")


def choose_loc(name: str, want_stem: str, cidx: dict) -> tuple[str, int] | None:
    locs = cidx.get(name)
    if not locs:
        return None
    same = [(p, ln) for (p, ln) in locs if Path(p).stem == want_stem]
    if same:
        return same[0]
    if len(locs) == 1:
        return locs[0]
    return None  # ambiguous — skip


def annotate_file(path: Path, cidx: dict, check: bool) -> tuple[int, int]:
    """Returns (added, missing). In check mode nothing is written."""
    text = path.read_text()
    lines = text.splitlines(keepends=False)
    want_stem = c_stem_for(path)
    out: list[str] = []
    added = missing = 0
    for line in lines:
        m = g.RE_RS_FN.match(line)
        if m:
            name = m.group(1)
            loc = choose_loc(name, want_stem, cidx)
            if loc is not None:
                prev = out[-1].strip() if out else ""
                already = prev.startswith("//") and CITE in prev
                if not already:
                    if check:
                        missing += 1
                    else:
                        indent = line[: len(line) - len(line.lstrip())]
                        rel, ln = loc
                        out.append(f"{indent}// {rel}:{ln}  {name}()")
                        added += 1
        out.append(line)
    if added and not check:
        path.write_text("\n".join(out) + ("\n" if text.endswith("\n") else ""))
    return added, missing


def main() -> int:
    check = "--check" in sys.argv[1:]
    cidx = g.walk_c()
    total_added = total_missing = files_touched = 0
    for d in g.RS_DIRS:
        for f in sorted(d.rglob("*.rs")):
            added, missing = annotate_file(f, cidx, check)
            total_added += added
            total_missing += missing
            if added:
                files_touched += 1
    if check:
        print(f"{total_missing} functions missing a C back-link citation")
        return 1 if total_missing else 0
    print(f"added {total_added} C back-link citations across {files_touched} files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
