# PORT.md — Rules for Bots Contributing to `ztmux`

`ztmux` is a **1:1 Rust port of tmux**. The goal is 100% behavioral parity
with the exact upstream tmux vendored under `vendor/tmux/` (currently
`next-3.7`). This is **not** a reimplementation, not a rewrite, not
"inspired by" tmux, and not a wrapper around the `tmux` binary or control
mode. Every line of Rust code under `src/ported/` must trace back to a
specific line of upstream C code in `vendor/tmux/`.

If you are a bot (Copilot, Claude, GPT, Cursor, Aider, any LLM agent),
**read this file before writing a single line of code**. Violations are
deleted on sight by the maintainer. No exceptions.

The build enforces this: `tests/ported_fn_names_match_c.rs` fails the build
when a free `fn` is added under `src/` whose name has no counterpart in
`vendor/tmux/`. The [port report](port_report.html) tracks C→Rust coverage
per function.

---

## READ THIS FIRST — The Rules in One Screen

If you read nothing else in this file, read this. Every violation is
deleted on sight; the maintainer does not negotiate.

Do not use fully qualified names that are not in C. C imports the names, so
Rust does too. No imports inside functions — only imports at the top of the
file, organized by origin.

### Rule 0 — ASK BEFORE INVENTING ANY NEW FN/STRUCT/STATIC NAME

**This rule overrides every other rule below.** If you (the bot) catch
yourself about to write a `fn`, `struct`, `enum`, `type`, or `static`
under `src/ported/` whose name does NOT exist in upstream tmux C source,
you must **STOP and ASK THE MAINTAINER FIRST**. You do not get to:

- "just add a tiny helper because it's only 3 lines"
- "factor out a Rust-only wrapper for borrow-checker reasons"
- "add a `_take`/`_set`/`_get`/`_clear`/`_is_some`/`_fill_*`/`_check_*`
  accessor for a global"
- "split one C function into `foo` + `foo_impl` for argument routing"
- "add a Rust-only sentinel flag or recursion/iteration counter"
- "introduce a `*State`/`*Table`/`*Builder`/`*Config`/`*Context` aggregate"
- "add an `error()`/`set_error()`/`check_limit()`/`check_recursion()`
  paranoia helper"

even if the helper looks "obviously useful," "trivially small," "locally
scoped," "obviously safe," or "what any reasonable Rust programmer would
do." **None of those are reasons. Permission is the only reason.**

**The required flow when you think a Rust-only helper is needed:**

1. **STOP.** Do not write the helper.
2. State to the maintainer: *"I'm about to add `fn <name>` (or
   `struct <Name>` / `static <NAME>`) under `src/ported/<file>.rs`
   because <one-sentence reason>. This name does not exist in upstream
   tmux C. May I proceed?"*
3. **Wait for explicit permission.** Phrases that count as permission:
   "yes", "y", "ok", "go", "approved", "fine". Anything else — silence,
   "let me think", "why?", "what about X instead?" — is NOT permission.
4. If permission is granted, add the name AND immediately also add it to
   `tests/data/fake_fn_allowlist.txt`, with the approval recorded in the
   commit message ("approved 2026-MM-DD").
5. If permission is denied, the work goes back to either (a) using a real
   C-named port, (b) inlining the logic at call sites, or (c) abandoning
   the change.

**Test enforcement:** `tests/ported_fn_names_match_c.rs` scans every `.c`
and `.y` file under `vendor/tmux/` (including `vendor/tmux/compat/`) for
function definitions, then rejects any free `fn` under `src/` whose name is
neither in that C index nor in `tests/data/fake_fn_allowlist.txt`. Adding a
new name to the allowlist without prior maintainer approval is itself a
violation — the allowlist is not a free pass, it is the audit trail of
granted exceptions, to be shrunk over time, never grown casually.

Regenerate the allowlist only after **intentional, approved** churn:

```sh
cargo test --test ported_fn_names_match_c -- --nocapture 2>&1 \
  | sed -n 's/^FAKE-FN //p' | sort -u > tests/data/fake_fn_allowlist.txt
```

---

**Rule A — Names must exist in upstream tmux C.** This applies to **every
declaration** under `src/ported/`, not just functions:

| Rust decl                        | Must exist in C as                                | Verify with                                                        |
|----------------------------------|---------------------------------------------------|--------------------------------------------------------------------|
| `fn <name>`                      | `<name>(` function definition                     | `grep -rnE '^[a-zA-Z_].*\b<name>\(' vendor/tmux/*.c`               |
| `struct <Name>` / `enum <Name>`  | `struct <name>` / `union <name>` / `enum <name>` / `typedef` | `grep -rnE '(struct\|union\|enum)[[:space:]]+<name>' vendor/tmux/*.{c,h}` |
| `static <NAME>` / global         | `static <name>` (file-scope) or `extern <name>`   | `grep -rnE 'static[[:space:]].*\b<name>\b' vendor/tmux/*.c`         |
| `type <Name>`                    | `typedef ... <name>`                              | `grep -rnE 'typedef[[:space:]].*\b<name>[[:space:]]*;' vendor/tmux/*.{c,h}` |

If `grep` returns nothing, the name is invented. **Delete or rename.**

**Rule B — Signatures must be identical to C.**

C `format_true(const char *s)` → Rust `format_true(s: *const u8) -> bool`.
NOT `format_true(s: &str, strict: bool)`. No threading state through as
extra params. No splitting one C fn into many Rust fns. No merging many C
fns into one. No reordering params. No renaming params for "Rust idiom"
(`nam`, not `name`; `argv`, not `args`; `wp`, not `pane`). The C→Rust type
map is in `## EXACT TRANSLATION` below.

**Rule C — Every decl lives in the file that mirrors its C definition
file.** tmux keeps its sources flat under `vendor/tmux/`; the Rust stem is
the C stem with `-` and `.` replaced by `_` (Rust module names cannot
contain `-`):

| C definition is in...          | Rust port goes in...              |
|--------------------------------|-----------------------------------|
| `vendor/tmux/format.c`         | `src/ported/format.rs`            |
| `vendor/tmux/cmd-break-pane.c` | `src/ported/cmd_break_pane.rs`    |
| `vendor/tmux/screen-write.c`   | `src/ported/screen_write.rs`      |
| `vendor/tmux/grid-reader.c`    | `src/ported/grid_reader.rs`       |
| `vendor/tmux/compat/*.c`       | `src/ported/compat/*.rs`          |
| `vendor/tmux/tmux.h`           | `src/lib.rs` (header struct home) |

A struct declared in `tmux.h` does not belong in `format.rs` just because
`format.c` uses it — header-defined types live in the header home
(`src/lib.rs`). Same for fns: `vendor/tmux/window.c` fns →
`src/ported/window.rs`, never re-homed to "wherever they're called from."

**Rule D — Bag-of-globals aggregator types are banned.**

❌ C declares N file-`static`s → Rust aggregates them into one struct.

If C has `static int foo; static int bar; static int baz;` (three separate
file-statics), the Rust port has three separate statics/globals. **No
`struct BagState { foo, bar, baz }`** unless C declares
`struct bag_state { ... }`. Invented `*Table` / `*State` / `*Builder` /
`*Config` / `*Context` aggregates are the bag-of-globals anti-pattern and
are deleted on sight when no matching C `struct` exists.

**Rule E — Local variables must use the C source's exact names.**

This applies to **every variable inside a function body**, not just
function names and file-level declarations:

```c
static char *
format_bool_op_1(struct format_expand_state *es, const char *fmt, int not)
{
	int	 result;
	char	*expanded;

	expanded = format_expand1(es, fmt);
	result = format_true(expanded);
	...
}
```

Rust port — same names, same order, same scope:

```rust
let result: i32;        // vendor/tmux/format.c: int result
let expanded: *mut u8;  // vendor/tmux/format.c: char *expanded
expanded = format_expand1(es, fmt);
```

❌ **Forbidden renames inside function bodies**:
- `nam` → `name`, `argv` → `args`, `wp` → `pane`, `wl` → `winlink` (params)
- `s` → `string`, `cp` → `ptr`, `gc` → `grid_cell` (locals)
- `i` → `idx`, `n` → `count`, `ret` → `result`
- Combining multiple C locals into a tuple or struct
- Reordering local declarations
- Dropping a local just because Rust doesn't force you to declare it

**The C name is the canonical name.** If the C author chose `cp` for a
cursor pointer, the Rust port keeps `cp`. "Rust idiom" is not an excuse to
rename anything.

---

## Scope: `src/ported/` Is Strict-Port Territory

**Every Rust file under `src/ported/` is bound by every rule in this
document — no grandfathering, no "legacy" exemptions, no "we'll fix it
later".**

If a file lives under `src/ported/`:

- It **must** mirror a real C file under `vendor/tmux/` (same stem modulo
  `-`/`.` → `_`, same relative subpath — see Rule C).
- Every `fn` in it **must** carry the `/// C \`vendor/tmux/<file>.c:NNNN\`:
  \`<C signature>\`` doc-comment (see EXACT TRANSLATION §2).
- Every `fn` name **must** appear as a function in `vendor/tmux/` (verify
  via `grep` or the anti-drift test) or be one of the narrow trait-impl /
  test exemptions.
- Every line **must** trace back to a specific upstream C line. No invented
  helpers, no "cleaner" abstractions, no idiomatic-Rust refactors, no
  convenience wrappers.

A file under `src/ported/` that fails any of these tests is treated as
adhoc code and deleted on sight regardless of when it was added, who added
it, or how much of the build depends on it. Fix it by porting it properly,
move it to `src/extensions/` if it implements a feature tmux C does not
have, or delete it. There is no fourth option.

The only location in the tree where new, non-ported code may exist is
`src/extensions/` (see The Hard Rules §1). `src/lib.rs` and `src/main.rs`
are the crate-root glue (the tmux.h header home and the `main()` entry) —
they are not a precedent for adding more non-port files.

---

## NO SHORTCUTS — 100% LINE-BY-LINE COVERAGE

When the maintainer asks to port a C source file, the result is a
**complete 1:1 port**, not a partial one. "Faithful port" means every
function, struct, enum, `#define`, table, and file-static the C source
defines has a real Rust counterpart with matching name, signature, and
control flow.

The forbidden pattern:

❌ Port the entry point and the dispatch table → ship the rest as
`WARNING: NOT IN <FILE>.C` stubs / `*_notavail` placeholders / bare
`0`-returning entries → claim "faithful port".

### When stubs ARE acceptable

A stub marker with a `file:line` citation is **only** appropriate when the
C definition genuinely lives in a *different* `vendor/tmux/*.c` file (a
cross-file extern the current port calls but does not own). It is **not**
appropriate for any function defined in the *same* C source file the port
covers. If `vendor/tmux/window.c` defines `window_pane_set_event` and the
Rust port skips it, that is the lazy pattern. Port it.

### Audit requirement before declaring a port done

Before writing the commit message, run a sanity check:

```sh
# Every function defined in the C source file:
grep -nE '^[a-zA-Z_][a-zA-Z_0-9]*[[:space:]]*\(' vendor/tmux/<file>.c

# Every fn in the Rust port:
grep -nE '^[[:space:]]*(pub )?(unsafe )?fn ' src/ported/<file>.rs

# Walk both lists side-by-side. Every C name must appear in the Rust list.
# Stubs that don't match a different-file definition are blockers — not
# "follow-up commits".
```

---

## EXACT TRANSLATION — Same Names, Same Types, Same Calls, Every Line Cited

A true port is a **LINE-BY-LINE EXACT TRANSLATION** of the C source.
"Faithful port" is not a vibe; it is a checklist of literal correspondences
any reviewer can audit in seconds. If your port fails any rule below, it is
not a port — it is a paraphrase, and it gets deleted.

### 1. Argument names match C exactly (case-sensitive)

C `cmd_break_pane_exec(struct cmd *self, struct cmdq_item *item)` ports as
Rust `cmd_break_pane_exec(self_: *mut cmd, item: *mut cmdq_item)`.

- `wp`, NOT `pane`. `wl`, NOT `winlink`. `gc`, NOT `cell`. `es`, NOT
  `state`. The C name is the canonical name; renaming for "Rust idiom" is a
  violation.
- Where a C name collides with a Rust keyword, suffix a single underscore
  (`self` → `self_`, `type` → `type_`) — this is the only sanctioned
  deviation, and it is minimal.
- UNUSED parameters stay as parameters, with a leading underscore
  (`_func: i32`). Never delete a parameter just because it is unused —
  call sites bind by position.

### 2. Datatypes match C, and every fn cites its C origin

ztmux is a low-level port: it mirrors tmux's raw-pointer / intrusive-list
data model rather than reshaping it into idiomatic Rust. The canonical map:

| C type            | Rust type                                  |
|-------------------|--------------------------------------------|
| `char *`          | `*mut u8`   (C strings; build with `c!("…")`, display with `_s(…)`) |
| `const char *`    | `*const u8`                                |
| `int`             | `i32`                                       |
| `u_int`           | `u32`                                       |
| `struct foo *`    | `*mut foo` / `*const foo`                  |
| `enum foo`        | the ported `foo` enum (`#[repr(i32)]`)     |
| bit-flag `#define`s | a `bitflags!` type with the same constant names |
| `void *`          | `*mut c_void` — match nearest semantic     |

Do NOT substitute an idiomatic Rust type for a C data shape (`&str` for
`char *`, a `Vec<T>` for an intrusive `TAILQ`, a `HashMap` for an RB-tree)
unless the surrounding module has already committed to that representation
for the whole struct. The port must round-trip through the same data shape
C operates on.

Every `fn` carries a doc-comment naming its C origin, in the tree's
established form:

```rust
/// C `vendor/tmux/format.c:4403`: `int format_true(const char *s)`
pub unsafe fn format_true(s: *const u8) -> bool {
    ...
}
```

Required: the C path relative to the repo root, the definition line number
(the line with the return type / signature, not the brace), and the full C
signature in backticks. For a port of a *macro*, cite it the same way and
note `(macro)`.

### 3. Called function names match C exactly

If C calls `colour_fromstring(new)`, Rust calls `colour_fromstring(...)`.
NEVER `parse_colour`, NEVER `colour_lookup_safe`. Same for every helper:
`format_expand1`, `xstrdup`, `window_count_panes`, `server_redraw_window`.
The drift gate rejects fn names with no C counterpart for exactly this
reason — if you find yourself wanting a new helper name, the right move is
either to find the existing C function it duplicates, or to port the
missing C function into its proper file and call THAT.

### 4. Every non-trivial line carries a C-line citation

```rust
before = args_has(args, 'b');                              // vendor/tmux/cmd-break-pane.c:113
if args_has(args, 'a') || before {                         // c:114
    idx = winlink_shuffle_up(dst_s, target_wl, before);   // c:115-118
    ...
}
```

Block-level `// c:NNN-MMM` is acceptable for contiguous chunks. The
doc-comment above the `fn` cites the function origin (§2); the inline
comments cite each statement.

### 5. Local variables: same names, same order, same scope

C declares locals at function top; the Rust port mirrors that block — same
names, same order, same visibility (function-scope, not block-scope).
Don't combine into tuples, don't reorder, don't drop the ones Rust would
let you elide — be conservative.

### 6. Control flow keeps C idioms

| C construct                    | Rust mirror                                          |
|--------------------------------|------------------------------------------------------|
| `while ((s = *argv++))`        | keep the assignment-in-condition as a `let` immediately before the loop test |
| `for (i = 0; i != N; i++)`     | `i = 0; while i != N { ...; i += 1; }` (preserve `!=`) |
| `for (i = 0; i < N; i++)`      | `for i in 0..N` (preserve `<`)                       |
| `do { ... } while (cond);`     | `loop { ...; if !(cond) { break; } }`                |
| `goto fail;` / `fail:`         | labelled `'fail: loop { ... break 'fail; }` (as `format_replace` already does) |

Don't "improve" C control flow into iterator chains. The structure of the C
code IS the structure of the Rust code.

### 7. C source comments port over

**This is a hard rule, not a "nice to have."** The C source's inline
comments encode load-bearing context the code alone doesn't: WHY a flag
combination is rejected, WHICH bug a workaround fixes, WHEN a branch is
reachable, the **WHY behind the WHAT**.

- Every C inline comment in the function body MUST appear in the Rust port,
  in the same position. Translate `/* ... */` to `//` line-form; preserve
  multi-line blocks as `//`-prefixed blocks at the same indentation.
- C function-header comments become Rust doc-comments (`///`) on the port,
  alongside the `/// C \`…\`` citation.
- File-header comments (copyright + purpose block atop every C file) become
  the Rust file's header comments.
- **Rust-only architectural notes** (e.g. "uses a raw pointer here because
  the intrusive list aliases") go in their **own** comment block, clearly
  separated from the verbatim C carry-overs, so a reader can tell at a
  glance whether a comment originated in C or is a port-specific note.

**Completeness check:** before declaring a port faithful, compare the C
function's comment density against the Rust port. A C body with 30 inline
comments porting to a Rust body with 3 is a warning sign — comments were
dropped.

### 8. Top-level declaration order matches C exactly

The order of `enum` / `struct` / `#define` / `static` table / `static`
global / function definitions in the Rust port mirrors the order in the C
source file, top to bottom. This makes side-by-side review trivial and lets
the `// c:NNN` citations climb monotonically down the Rust file. If two
adjacent Rust fns cite `c:670` then `c:519`, the ordering is wrong — fix it
before committing. Reorder ONLY when forced by the Rust compiler, and
document the deviation with a `// reordered from c:NNN — Rust requires X`
comment.

### 9. Function bodies port too — never bare-`return` a fn whose body is unported

When porting a fn whose body depends on subsystems not yet ported, DO NOT
take the shortcut of "this depends on X which isn't ported, so the whole
body returns 0 / no-op." The body lives in the same C file as the
declaration — by the coverage rule above, it must be ported in full:

1. Port the FULL C body line-by-line, every C statement → matching Rust
   statement with its `// c:NNN` citation.
2. Stub only the EXTERN dependencies (fns / globals from OTHER C files)
   with citations to their home file.
3. The body STILL EXECUTES — branches still take, mutations to file-local
   statics still apply.

**Why the body shortcut is worse than a file shortcut:** the file shortcut
leaves a clearly-named gap; the body shortcut produces a fn that LOOKS
complete — same signature, drift gate green — but silently elides the
logic. Port the full body now so the eventual integration is "swap the
stubs for real impls," not "translate the C from scratch a second time."

---

## The Hard Rules

### 1. PORT-ONLY. NO ADHOC IMPLEMENTATIONS.

You are translating C → Rust. You are not designing software.

- You **may** write a Rust function if and only if it is a port of a
  specific C function that exists in `vendor/tmux/**/*.c`.
- You **may not** invent helper functions, utility wrappers, "cleaner"
  abstractions, traits, builders, or any other code with no direct C
  counterpart in upstream tmux.
- "Refactoring for idiomatic Rust" is **forbidden**. The structure of the C
  code is the structure of the Rust code: same function names, same control
  flow, same globals, same field layout.
- If C uses `goto`, your Rust port uses a labelled `loop`/`break` to mirror
  it. Do not "improve" it.
- If you cannot find a matching C function for code you want to write,
  **stop and do not write it.** Ask the maintainer or pick a different task.

#### The ONE exception

There is exactly one location where new, non-ported code is permitted:

**`src/extensions/`** — the **only** place for features tmux C does not
have (structured/JSON output, dashboards, query helpers, etc.). Code here
is not a port and is not expected to map to any C function. Two rules still
apply:

- Every file under `src/extensions/` must implement a feature tmux C
  demonstrably does **not** have. If a similar feature exists in tmux, port
  it instead — the port belongs under `src/ported/`.
- Extensions are additive only: they may not duplicate or shadow a port. If
  your "extension" is really a reimplementation of something tmux already
  does, delete it and port the C version.

Everything outside `src/extensions/` (and the `src/lib.rs` / `src/main.rs`
crate glue) is a **port**. No exceptions. No "this one little helper."

### 2. EVERY FUNCTION MUST CITE ITS C SOURCE.

Every `fn` under `src/ported/` carries the `/// C \`vendor/tmux/<file>.c:NNNN\`:
\`<C signature>\`` doc-comment immediately above the signature (see EXACT
TRANSLATION §2). If a large C function is split across Rust helpers, each
helper cites the same C function and indicates the chunk
(`(chunk 3/7 — option parsing)`).

### 3. NAMES MUST EXIST IN UPSTREAM tmux.

A Rust function name under `src/ported/` is **legal** if and only if it is
one of:

1. **Identical** to a function defined in `vendor/tmux/**/*.c` (or `*.y`).
   Verify with `grep -rnE '\b<name>\(' vendor/tmux/*.c` or by running
   `cargo test --test ported_fn_names_match_c`.
2. A standard Rust trait-impl method (`fn new`, `fn drop`, `fn fmt`,
   `fn clone`, `fn default`, `fn from`, `fn eq`, `fn cmp`, `fn next`, …) —
   and only when it directly wraps a C function call or a struct layout.
   These live inside `impl`/`trait` blocks, which the drift gate skips.
3. A `#[cfg(test)]` / `#[test]` function.

Anything else — `make_pretty_helper`, `parse_args_v2`, `init_state_new`,
`RustyOptions::build` — **will be deleted**. A pre-existing exception must
be recorded in `tests/data/fake_fn_allowlist.txt`; that file is an audit
trail to burn down, not a free pass.

---

## File Layout: 1:1 with tmux

The Rust source tree under `src/` is:

| path              | purpose                                                        |
|-------------------|----------------------------------------------------------------|
| `src/ported/`     | The 1:1 port. Every file mirrors a `vendor/tmux/<...>.c`.       |
| `src/ported/compat/` | Ports of `vendor/tmux/compat/*.c`.                          |
| `src/extensions/` | Features tmux C does **not** have. The only sanctioned non-port dir. |
| `src/lib.rs`      | Crate root + `tmux.h` header struct home (`struct client`, `window`, `session`, flag `bitflags!`, …). |
| `src/main.rs`     | `main()` entry (`vendor/tmux/tmux.c`).                          |
| `src/cmd_parse.lalrpop` | The command-language grammar (`vendor/tmux/cmd-parse.y`). |

Rules:

- ❌ No `src/ported/helpers.rs`, `common.rs`, `types.rs`, `prelude.rs`,
  `macros.rs`, `state.rs`, `runtime.rs`, `safe_*.rs`, `rusty_*.rs` — none
  of these names correspond to any `vendor/tmux/*.c`; they are invented
  helper-bucket names by definition.
- ✅ A new file under `src/ported/` is legal **only** as the 1:1 mirror of
  a real `vendor/tmux/<x>.c` that has no Rust home yet, named with the
  `-`/`.` → `_` convention. It is not a catch-basin: the only functions
  that may live in it are ports of fns whose C definition is in that exact
  `.c` file.
- ❌ No new directories under `src/ported/` that don't exist under
  `vendor/tmux/`.
- ❌ No "support crate," no workspace splits that don't mirror tmux's
  layout. (`Cargo.toml` already declares its own `[workspace]` excluding
  `vendor/`.)
- ✅ The only legal way to add a file under `src/extensions/`: it
  implements a feature tmux C demonstrably does **not** have, and does not
  duplicate or shadow any existing port.

No renames of any kind beyond the mechanical `-`/`.` → `_`. No `_port`,
`_rs`, `_impl`, `_v2`, `_safe` suffixes. If your port of `cmd_kill_pane_exec`
(defined in `vendor/tmux/cmd-kill-pane.c`) ends up anywhere other than
`src/ported/cmd_kill_pane.rs`, you have done it wrong. Move it. If it ends
up outside `src/ported/` (e.g. a crate-root `src/foo.rs` or under
`src/extensions/`), it will be deleted on sight.

---

## Adhoc Code: 100% Banned, Deleted on Sight

Adhoc implementation is **forbidden absolutely**. Not "discouraged." Not
"should be ported eventually." **Banned.** The maintainer runs purges that
delete any function or file which:

- Has **no** `/// C \`vendor/tmux/…\`` citation, **or**
- Has a name that is not a function in `vendor/tmux/` and is not one of the
  allowed exemptions (trait-impls, tests, allowlist), **or**
- Lives in a Rust file under `src/ported/` with no corresponding C file
  under `vendor/tmux/`, **or**
- Lives in the wrong file per the 1:1 mapping, **or**
- Lives outside `src/ported/` and `src/extensions/` (other than the
  sanctioned `src/lib.rs` / `src/main.rs` glue).

If your change adds adhoc code, **all of it will be deleted** — the
function, the file, the module declaration — without discussion. Either
port the corresponding C function properly into the matching Rust file
under `src/ported/`, place a genuinely new feature under `src/extensions/`,
or do not write the code at all.

---

## Workflow for Bots

Before writing any code:

1. Identify the C function you intend to port. Get its exact name, file,
   and line under `vendor/tmux/`. Confirm the name via
   `grep -rnE '\b<name>\(' vendor/tmux/*.c`.
2. Identify the destination Rust file using the 1:1 mapping (`-`/`.` → `_`).
3. Read the C function in full. Read every helper it calls. Read the
   relevant `struct` definitions in `tmux.h` (→ `src/lib.rs`).
4. Translate line-by-line. Preserve identifier names; use a trailing `_`
   only for Rust-keyword collisions (`self_`, `type_`).
5. Add the `/// C \`vendor/tmux/<file>.c:NNNN\`: \`<sig>\`` doc-comment.
6. Add inline `// c:NNN` (or `// vendor/tmux/<file>.c:NNN`) tags on
   non-obvious translations so the next bot can verify.
7. Run `cargo build` and `cargo clippy` — keep them clean. Run
   `cargo test --test ported_fn_names_match_c` — the drift gate must stay
   green.
8. Verify behavior against the vendored tmux with the parity suite
   (`bash parity/run_parity.sh`), adding a case under `parity/cases/` that
   pins the newly-ported behavior byte-for-byte. See
   [`parity/PARITY_ROADMAP.md`](../parity/PARITY_ROADMAP.md).

---

## What You Must Never Do

- ❌ **Add any `fn` under `src/ported/` whose name is not a function in
  `vendor/tmux/**/*.c`.** Verify before writing the signature. Trait-impl
  and `#[test]` exemptions are the only carve-outs.
- ❌ **Add any `struct` / `enum` / `type` / `union` / top-level `static`
  under `src/ported/` whose name is not in upstream tmux C.** The
  bag-of-globals `*State` / `*Table` / `*Builder` / `*Config` / `*Context`
  pattern is deleted on sight.
- ❌ **Place a struct in the wrong file.** Header-defined types (anything in
  `tmux.h`) live in `src/lib.rs`, NOT in the module that consumes them.
- ❌ **Change a function signature.** Same name, same arity, same order, no
  threading state as extra params, no splitting one fn into many, no
  merging many into one.
- ❌ **Rename a local variable inside a function body.** `wp` stays `wp`,
  `gc` stays `gc`, `cp` stays `cp`, `i` stays `i`.
- ❌ Invent a function name with no `vendor/tmux/` counterpart.
- ❌ Write "helper" / "utility" / "convenience" functions or files.
- ❌ Add modules like `helpers`, `common`, `prelude`, `state`, `runtime`,
  `macros`, `types`, `safe_*`, `rusty_*` — none correspond to a
  `vendor/tmux/*.c`.
- ❌ Refactor C control flow into Rust iterators / combinators / traits
  unless the C already does the equivalent.
- ❌ Split one C function across multiple Rust files, or combine multiple C
  functions into one.
- ❌ Add `_port`, `_rs`, `_impl`, `_v2`, `_safe`, `_ext` suffixes.
- ❌ Skip the `/// C \`…\`` doc-comment, or cite a C function that doesn't
  exist / doesn't correspond.
- ❌ "Stub" a function with `unimplemented!()`/`todo!()` and call it ported.
- ❌ Translate from your memory of tmux's behavior. Read the C source.

---

## What You Should Do

- ✅ Pick one C function, port it faithfully, cite it precisely.
- ✅ Mirror C identifier names, struct field names, file layout.
- ✅ Mirror C control flow (`goto` → labelled `loop`/`break`).
- ✅ Mirror globals as `static` / `Mutex<…>` / thread-locals as needed for
  parity, not Rust elegance.
- ✅ Diff your port against the vendored C, function by function, before
  claiming coverage.
- ✅ Add a parity case that pins the behavior byte-for-byte vs vendored
  tmux. Keep `cargo build`, `cargo clippy`, and the drift gate green.

---

## Sources of Truth

- **C source:** `vendor/tmux/**/*.c` and headers `*.h` (the exact
  `next-3.7` tmux `src/` is ported from — see `vendor/VENDOR.md`).
- **Function-name gate:** `tests/ported_fn_names_match_c.rs` (live-scans
  `vendor/tmux/`); accepted exceptions in `tests/data/fake_fn_allowlist.txt`.
- **Port progress — C↔Rust coverage:** [`docs/port_report.html`](port_report.html)
  (generated by `scripts/gen_port_report.py`). Consult it before adding or
  moving a port.
- **Parity:** [`parity/PARITY_ROADMAP.md`](../parity/PARITY_ROADMAP.md) and
  the bug log [`docs/BUGS.md`](BUGS.md).

---

## TL;DR

> **Rule 0: ASK FIRST.** Adding any `fn`/`struct`/`enum`/`static` under
> `src/ported/` whose name does NOT exist in upstream tmux C requires
> EXPLICIT MAINTAINER PERMISSION before you write the code. No "tiny
> helpers," no "obvious wrappers." Stop, ask, wait for an explicit yes.
>
> Inside `src/ported/`, **every name must exist in upstream tmux C**:
> - **Functions**: name appears in `vendor/tmux/**/*.c` (verify with `grep`
>   or the drift-gate test).
> - **Structs / enums / typedefs / statics**: name appears as
>   `struct <name>` / `enum <name>` / `typedef … <name>` / `static … <name>`
>   in `vendor/tmux/**/*.{c,h}`. Bag-of-globals aggregates are deleted on
>   sight.
> - **Local variables**: same names as C, same order, same scope. NEVER
>   rename for "Rust idiom" — not params, not locals, not loop iterators.
> - **Signatures**: identical to C. Same name, arity, and param order.
> - **File placement**: every fn / struct lives in the Rust file that
>   mirrors its C definition file (`-`/`.` → `_`). Header-defined types
>   (`tmux.h`) live in `src/lib.rs`.
> - **Code order**: top-to-bottom order of decls/fns/comments matches C.
> - **Citations**: every fn carries `/// C \`vendor/tmux/<file>.c:NNNN\`:
>   \`<sig>\``; every non-trivial statement carries an inline `// c:NNN`.
>
> Every file is a strict 1:1 port of its `vendor/tmux/*.c`. No
> grandfathering, no helpers, no "legacy" exemptions. Genuinely new
> features go under `src/extensions/`. Adhoc code anywhere else is deleted
> on sight.
