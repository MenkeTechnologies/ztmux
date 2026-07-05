#!/usr/bin/env python3
"""Generate completions/_ztmux with full tmux depth plus ztmux extensions.

Strategy: take the canonical zsh `_tmux` completion (vendored at
scripts/_tmux.base.zsh) — which carries the full argument intelligence for
every tmux command (option names for set-option/set-window-option, format
variables, targets, layouts, key tables, styles, colours) — rewrite it for the
`ztmux` binary, and inject the ztmux client extensions as `_ztmux-<verb>`
functions.  The base's own auto-discovery loop lists every `_ztmux-*` function
alongside the real tmux commands, so the extensions appear in `ztmux <TAB>`
with their own argument completion.

The vendored base is zsh's upstream `_tmux` (see scripts/_tmux.base.zsh); to
refresh it, copy a newer `_tmux` from `$fpath` over that file and rerun this.

Run from the repo root:  python3 scripts/gen_zsh_completion.py
"""
from __future__ import annotations

BASE = "scripts/_tmux.base.zsh"
OUT = "completions/_ztmux"

# ── ztmux client extensions (no tmux C counterpart) ──────────────────────────
# (verb, _arguments specs, description).  Dynamic-value specs reference the
# base's helper functions, which are renamed __tmux-* -> __ztmux-* below.
EXTENSIONS = [
    ("dashboard", [], "live ratatui server dashboard (ztmux extension)"),
    ("switcher", [], "interactive session/window/pane picker (ztmux extension)"),
    ("sessions", [], "zellij-style session manager: switch/rename/kill/new (ztmux extension)"),
    ("tree", [], "print the session/window/pane tree (ztmux extension)"),
    (
        "doctor",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "environment / server health check (ztmux extension)",
    ),
    (
        "stats",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "one-shot server summary report (ztmux extension)",
    ),
    (
        "graph",
        ["-o[diagram format]:format:(dot mermaid html)"],
        "render the server tree as DOT/Mermaid/HTML (ztmux extension)",
    ),
    ("watch", [], "top-like live per-pane process monitor (ztmux extension)"),
    (
        "events",
        ["-n[poll interval in ms]:ms", "--count[exit after N events]:count"],
        "stream server lifecycle events as JSONL (ztmux extension)",
    ),
    (
        "triggers",
        [":subcommand:(list arm disarm add wizard test)"],
        "run a ztmux command when a regex matches pane output (ztmux extension)",
    ),
    (
        "ps",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "one-shot pipeable per-pane process table (ztmux extension)",
    ),
    ("snapshot", [], "dump the whole server as one nested JSON document (ztmux extension)"),
    (
        "prune",
        [
            "--dead[prune dead panes]",
            "--empty[prune window-less sessions]",
            "--idle[prune detached sessions idle > N seconds]:seconds",
            "-f[actually remove (default: dry-run)]",
            "--force[actually remove (default: dry-run)]",
            "-o[output format]:format:(json)",
            "--json[machine-readable JSON output]",
        ],
        "remove dead/empty/idle server objects (ztmux extension)",
    ),
    (
        "layout",
        [
            ":preset:(list even-h even-v main-h main-v tiled dev ide grid)",
            "-t[target window]:target",
            "-f[apply (default: dry-run)]",
            "--apply[apply (default: dry-run)]",
        ],
        "apply a named layout preset to a window (ztmux extension)",
    ),
    (
        "finder",
        [
            ":query:",
            "-o[output format]:format:(json)",
            "--json[machine-readable JSON output]",
        ],
        "search panes by command/path/title/window (ztmux extension)",
    ),
    (
        "recent",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "list sessions ranked by last activity (ztmux extension)",
    ),
    (
        "usage",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "per-session CPU/MEM/RSS resource rollup (ztmux extension)",
    ),
    (
        "grep",
        [
            ":pattern:",
            "-a[search full scrollback, not just the visible screen]",
            "--history[search full scrollback, not just the visible screen]",
            "-o[output format]:format:(json)",
            "--json[machine-readable JSON output]",
        ],
        "search the live contents of every pane (ztmux extension)",
    ),
    (
        "peek",
        [
            "-t[limit to panes whose location contains SUBSTR]:location:",
            "-o[output format]:format:(json)",
            "--json[machine-readable JSON output]",
        ],
        "dump the visible contents of every pane (ztmux extension)",
    ),
    (
        "bcast",
        [
            ":command:",
            "-c[only panes whose command contains SUBSTR]:command:",
            "-s[only panes in SESSION]:session:__ztmux-sessions",
            "-N[send keys without a trailing Enter]",
            "--no-enter[send keys without a trailing Enter]",
            "-f[actually send (default: dry-run)]",
            "--force[actually send (default: dry-run)]",
        ],
        "broadcast a command to many panes at once (ztmux extension)",
    ),
    (
        "pstree",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "process tree running under every pane (ztmux extension)",
    ),
    (
        "ports",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "listening TCP ports mapped to panes (ztmux extension)",
    ),
    (
        "info",
        [
            ":target:__ztmux-panes",
            "-o[output format]:format:(json)",
            "--json[machine-readable JSON output]",
        ],
        "deep inspector for a single pane (ztmux extension)",
    ),
    (
        "dedup",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "find redundant panes (same cwd + command) (ztmux extension)",
    ),
    (
        "size",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "report pane geometry, smallest first (ztmux extension)",
    ),
    (
        "groups",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "cluster sessions by session group (ztmux extension)",
    ),
    (
        "tty",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "map every pane to its terminal device (ztmux extension)",
    ),
    (
        "git",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "git branch + dirty state of every pane's repo (ztmux extension)",
    ),
    (
        "active",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "focused window/pane of every session (ztmux extension)",
    ),
    (
        "ssh",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "which panes hold an SSH connection, and where (ztmux extension)",
    ),
    (
        "disk",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "filesystem usage behind each pane's cwd (ztmux extension)",
    ),
    (
        "net",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "established outbound connections per pane (ztmux extension)",
    ),
    (
        "env",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "per-session environment overrides (ztmux extension)",
    ),
    (
        "history",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "rank panes by scrollback buffer size (ztmux extension)",
    ),
    (
        "mode",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "panes currently frozen in a mode (ztmux extension)",
    ),
    (
        "zoom",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "windows with a zoomed pane (ztmux extension)",
    ),
    (
        "marks",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "the marked pane(s) (ztmux extension)",
    ),
    (
        "alerts",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "windows with a pending bell/activity/silence alert (ztmux extension)",
    ),
    (
        "titles",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "every pane's advertised title (ztmux extension)",
    ),
    (
        "equalize",
        [
            ":layout:(even-horizontal even-vertical main-horizontal main-vertical tiled)",
            "-s[only windows in SESSION]:session:__ztmux-sessions",
            "-f[apply (default: dry-run)]",
            "--force[apply (default: dry-run)]",
        ],
        "re-balance every multi-pane window's layout (ztmux extension)",
    ),
    (
        "revive",
        [
            "-s[only dead panes in SESSION]:session:__ztmux-sessions",
            "-f[respawn (default: dry-run)]",
            "--force[respawn (default: dry-run)]",
        ],
        "revive every dead pane in place (ztmux extension)",
    ),
    (
        "clearall",
        [
            "-s[only panes in SESSION]:session:__ztmux-sessions",
            "-f[clear (default: dry-run)]",
            "--force[clear (default: dry-run)]",
        ],
        "free the scrollback of every pane (ztmux extension)",
    ),
    (
        "retitle",
        [
            "-s[only panes in SESSION]:session:__ztmux-sessions",
            "-f[apply (default: dry-run)]",
            "--force[apply (default: dry-run)]",
        ],
        "label every pane with its running command (ztmux extension)",
    ),
    (
        "cwd",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "working directories in use, busiest first (ztmux extension)",
    ),
    (
        "who",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "clients attached to the server, by session (ztmux extension)",
    ),
    (
        "age",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "sessions ranked by creation age, oldest first (ztmux extension)",
    ),
    (
        "cmd",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "histogram of commands running across panes (ztmux extension)",
    ),
    (
        "dead",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "dead panes still held open in a window (ztmux extension)",
    ),
    (
        "layouts",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "the current layout string of every window (ztmux extension)",
    ),
    (
        "detached",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "sessions with no client attached, freshest first (ztmux extension)",
    ),
    (
        "density",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "windows ranked by pane count, most first (ztmux extension)",
    ),
    (
        "nested",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "panes running a nested terminal multiplexer (ztmux extension)",
    ),
    (
        "solo",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "windows holding a single, unsplit pane (ztmux extension)",
    ),
    (
        "shells",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "panes sitting at a bare shell prompt (ztmux extension)",
    ),
    (
        "fanout",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "sessions ranked by total pane count (ztmux extension)",
    ),
    (
        "busy",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "panes running a program, not idle at a prompt (ztmux extension)",
    ),
    (
        "focus",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "the active pane of every window (ztmux extension)",
    ),
    (
        "named",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "windows with a deliberate name, not the running command (ztmux extension)",
    ),
    (
        "project",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "project kind and root behind every pane (ztmux extension)",
    ),
    (
        "remote",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "the git remote each pane's repo points at (ztmux extension)",
    ),
    (
        "ahead",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "how far each repo is ahead of/behind upstream (ztmux extension)",
    ),
    (
        "changes",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "uncommitted file count per pane's repo, dirtiest first (ztmux extension)",
    ),
    (
        "stash",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "repositories with stashed work behind a pane (ztmux extension)",
    ),
    (
        "elapsed",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "how long each pane's process has been running (ztmux extension)",
    ),
    (
        "mem",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "panes ranked by process resident memory (ztmux extension)",
    ),
    (
        "state",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "panes whose process is in an abnormal state (ztmux extension)",
    ),
    (
        "commit",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "the last commit in every pane's repo (ztmux extension)",
    ),
    (
        "linked",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "windows linked into more than one session (ztmux extension)",
    ),
    (
        "conflicts",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "repos with unresolved merge conflicts (ztmux extension)",
    ),
    (
        "user",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "the owner of every pane's process (ztmux extension)",
    ),
    (
        "tag",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "the git describe/tag context of every pane's repo (ztmux extension)",
    ),
    (
        "vcs",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "which version-control system each pane is under (ztmux extension)",
    ),
    (
        "gone",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "panes whose working directory no longer exists (ztmux extension)",
    ),
    (
        "buffers",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "the server's paste buffers, largest first (ztmux extension)",
    ),
    (
        "worktree",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "panes sitting in a linked git worktree (ztmux extension)",
    ),
    (
        "submodules",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "repos with submodules and how many are out of sync (ztmux extension)",
    ),
    (
        "term",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "histogram of attached client terminal types (ztmux extension)",
    ),
    (
        "startcmd",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "the command line each pane was launched with (ztmux extension)",
    ),
    (
        "writable",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "panes whose working directory is read-only (ztmux extension)",
    ),
    (
        "sync",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "windows with synchronize-panes turned on (ztmux extension)",
    ),
    (
        "piped",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "panes with an active pipe-pane capture (ztmux extension)",
    ),
    (
        "input",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "panes that are ignoring keyboard input (ztmux extension)",
    ),
    (
        "monitor",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "windows armed to alert on activity or silence (ztmux extension)",
    ),
    (
        "remain",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "windows that keep panes open after they exit (ztmux extension)",
    ),
    (
        "autoname",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "windows with a pinned name (automatic-rename off) (ztmux extension)",
    ),
    (
        "readonly",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "clients attached in read-only mode (ztmux extension)",
    ),
    (
        "idle",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "attached clients ranked by time since last activity (ztmux extension)",
    ),
    (
        "viewers",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "how many clients are attached to each session (ztmux extension)",
    ),
    (
        "connected",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "attached clients ranked by connection age (ztmux extension)",
    ),
    (
        "constrain",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "attached clients ranked by screen size, smallest first (ztmux extension)",
    ),
    (
        "hooks",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "the command hooks configured on each session (ztmux extension)",
    ),
    (
        "destroy",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "sessions that self-destruct when the last client detaches (ztmux extension)",
    ),
    (
        "status",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "sessions with the status line turned off (ztmux extension)",
    ),
    (
        "keys",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "how many key bindings live in each key table (ztmux extension)",
    ),
    (
        "limit",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "each session's scrollback capacity, largest first (ztmux extension)",
    ),
    (
        "winsize",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "windows whose sizing mode differs from the default (ztmux extension)",
    ),
    (
        "borders",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "windows that draw a status line on their pane borders (ztmux extension)",
    ),
    (
        "autolock",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "sessions set to lock themselves after idle time (ztmux extension)",
    ),
    (
        "titlebar",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "sessions that push a title to the outer terminal (ztmux extension)",
    ),
    (
        "visual",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "sessions that show alerts visually instead of just beeping (ztmux extension)",
    ),
    (
        "pick",
        [":subcommand:(sync unmark clear list)"],
        "batch sync/unmark/clear over the multi-pane mark set (ztmux extension)",
    ),
    (
        "tabs",
        [":subcommand:(toggle on off)"],
        "zellij-style top tab bar of windows (ztmux extension)",
    ),
    (
        "modal",
        [":subcommand:(toggle on off)"],
        "zellij-style modal keybindings: pane/tab/resize/session/lock modes (ztmux extension)",
    ),
    (
        "resurrect",
        [":subcommand:(save restore list)", "--run[re-run each pane's saved command]"],
        "save/restore all sessions across restarts (ztmux extension)",
    ),
    (
        "control",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "clients attached in control mode (-CC) (ztmux extension)",
    ),
    (
        "keytable",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "clients parked on a non-root key table (ztmux extension)",
    ),
    (
        "mouse",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "which sessions have mouse mode enabled (ztmux extension)",
    ),
    (
        "utf8",
        ["-o[output format]:format:(json)", "--json[machine-readable JSON output]"],
        "the UTF-8 state of every attached client (ztmux extension)",
    ),
]


def ext_function(name: str, specs: list[str], desc: str) -> str:
    """Emit a `_ztmux-<verb>` subcommand function in the base's own style:
    print the description when $tmux_describe is set (drives the command list),
    otherwise run _arguments for the verb's own flags."""
    lines = [
        f"_ztmux-{name}() {{",
        f'  [[ -n ${{tmux_describe}} ]] && print "{desc}" && return',
    ]
    if specs:
        spec_str = " \\\n    ".join(f"'{s}'" for s in specs)
        lines.append(f"  _arguments -s \\\n    {spec_str}")
    lines.append("}")
    return "\n".join(lines)


def main() -> int:
    base = open(BASE, encoding="utf-8").read()

    # Rewrite every _tmux/_tmux-*/__tmux-*/_tmux_* identifier to its ztmux form.
    # This is a single self-consistent rename (functions, helpers and arrays all
    # move together), so no collision with a separately-loaded upstream _tmux.
    base = base.replace("_tmux", "_ztmux")
    # These carry a bare `tmux` token that the identifier rename does not touch.
    base = base.replace("#compdef tmux", "#compdef ztmux")
    base = base.replace("command tmux", "command ztmux")
    base = base.replace(
        "# tmux <http://tmux.github.io> completion for zsh <http://zsh.sf.net>.",
        "# ztmux completion for zshrs — full tmux depth + ztmux extensions.\n"
        "# GENERATED by scripts/gen_zsh_completion.py; do not edit by hand.",
    )

    # Inject the extension subcommand functions just before the main _ztmux().
    anchor = "# And here is the actual _ztmux(), that puts it all together:"
    if anchor not in base:
        raise SystemExit(f"anchor not found in {BASE}: {anchor!r}")
    ext_block = "\n\n".join(ext_function(n, s, d) for n, s, d in EXTENSIONS)
    marker = "# ── ztmux client extensions ─────────────────────────────────────────────────"
    base = base.replace(anchor, f"{marker}\n{ext_block}\n\n{anchor}", 1)

    open(OUT, "w", encoding="utf-8").write(base)
    print(f"wrote {OUT} — upstream _tmux depth + {len(EXTENSIONS)} ztmux extensions")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
