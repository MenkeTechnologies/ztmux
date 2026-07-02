#!/usr/bin/env python3
"""Generate completions/_ztmux from ztmux's actual command surface.

Sources of truth:
  * the `cmd_entry { name, alias, usage }` table in src/ported/cmd_*.rs
  * the client subcommands added under src/extensions/
    (dashboard, switch, tree, doctor, stats, graph, watch, events, ps,
    snapshot, prune, layout, find, recent, usage)
  * the global option string parsed by tmux_main() in src/ported/tmux.rs

Run from the repo root:  python3 scripts/gen_zsh_completion.py
"""
from __future__ import annotations

import glob
import re
import sys

# ── extract (name, alias, usage) from the cmd_entry table ────────────────────

CMD_RE = re.compile(r"cmd_entry\s*\{(.*?)\}", re.S)


def extract_commands() -> list[tuple[str, str, str]]:
    rows: dict[str, tuple[str, str, str]] = {}
    for path in sorted(glob.glob("src/ported/cmd_*.rs")):
        src = open(path, encoding="utf-8").read()
        for m in CMD_RE.finditer(src):
            blk = m.group(1)
            nm = re.search(r'name:\s*"([^"]+)"', blk)
            if not nm:
                continue
            al = re.search(r'alias:\s*Some\("([^"]+)"\)', blk)
            us = re.search(r'usage:\s*"((?:[^"\\]|\\.)*)"', blk)
            name = nm.group(1)
            rows[name] = (name, al.group(1) if al else "", us.group(1) if us else "")
    return [rows[k] for k in sorted(rows)]


# ── usage string → zsh _arguments specs ──────────────────────────────────────

TARGET_COMPL = {
    "target-session": "__ztmux_sessions",
    "target-client": "__ztmux_clients",
    "target-window": "__ztmux_windows",
    "src-window": "__ztmux_windows",
    "dst-window": "__ztmux_windows",
    "target-pane": "__ztmux_panes",
    "src-pane": "__ztmux_panes",
    "dst-pane": "__ztmux_panes",
    "buffer-name": "__ztmux_buffers",
    "target-buffer": "__ztmux_buffers",
    "key-table": "__ztmux_key_tables",
    "shell-command": " ",
    "working-directory": "_files -/",
    "file": "_files",
    "path": "_files",
}


def sanitize(text: str) -> str:
    """Make a description safe inside a single-quoted _arguments spec."""
    return text.replace("'", "").replace(":", " ").replace("[", "(").replace("]", ")")


def arg_completion(argword: str) -> str:
    """Return the `:desc:action` tail for an option that takes `argword`."""
    if "|" in argword:  # fixed value set, e.g. json|jsonl|csv
        vals = " ".join(argword.split("|"))
        return f":format:({vals})"
    key = argword.strip()
    action = TARGET_COMPL.get(key, "")
    return f":{sanitize(key)}:{action}"


def parse_usage(usage: str) -> tuple[list[str], list[str]]:
    """Return (option specs, positional specs) for one usage string."""
    opts: list[str] = []
    positional: list[str] = []
    # Split into `[...]` groups and bare tokens.
    tokens = re.findall(r"\[[^\]]*\]|\S+", usage)
    for tok in tokens:
        if tok.startswith("[") and tok.endswith("]"):
            inner = tok[1:-1].strip()
        else:
            inner = tok
        if not inner:
            continue
        if inner.startswith("-") and len(inner) > 1 and inner[1] != " ":
            body = inner[1:]
            if " " in body:  # a single flag that takes an argument: `-c working-dir`
                flag, argword = body.split(" ", 1)
                # only the first letter is the flag; rest is the arg name
                letter = flag[0]
                opts.append(f"-{letter}[option {letter}]{arg_completion(argword)}")
            else:  # a bundle of boolean flags: `-abc`
                for letter in body:
                    opts.append(f"-{letter}[flag {letter}]")
        else:
            # a positional such as `key`, `command`, `template`
            positional.append(sanitize(inner.strip("[]")))
    return opts, positional


def command_specs(usage: str) -> list[str]:
    opts, positional = parse_usage(usage)
    specs = list(opts)
    if positional:
        # everything trailing (command + args, key, template, …) is free-form.
        specs.append("*:::arg:_normal")
    return specs


def describe(name: str) -> str:
    return sanitize(name.replace("-", " "))


# ── emit the completion ──────────────────────────────────────────────────────

# (name, arg-spec list for _arguments, description). Client subcommands under
# src/extensions/ with no tmux C counterpart. An empty spec list means the
# subcommand takes no arguments.
EXTENSIONS = [
    ("dashboard", [], "live ratatui server dashboard (ztmux extension)"),
    ("switch", [], "interactive session/window/pane picker (ztmux extension)"),
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
        "find",
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
]


def main() -> int:
    cmds = extract_commands()

    # command list for _describe (name + alias entries)
    describe_lines: list[str] = []
    for name, alias, _ in cmds:
        describe_lines.append(f"    '{name}:{describe(name)}'")
        if alias:
            describe_lines.append(f"    '{alias}:{describe(name)} (alias)'")
    for name, _, desc in EXTENSIONS:
        describe_lines.append(f"    '{name}:{sanitize(desc)}'")

    # per-command arg dispatch
    case_lines: list[str] = []
    for name, alias, usage in cmds:
        pattern = f"{name}|{alias}" if alias else name
        specs = command_specs(usage)
        if specs:
            spec_str = " \\\n        ".join(f"'{s}'" for s in specs)
            case_lines.append(f"      {pattern})\n        _arguments -s \\\n        {spec_str} && ret=0\n        ;;")
        else:
            case_lines.append(f"      {pattern})\n        _message 'no arguments'\n        ;;")
    for name, specs, _ in EXTENSIONS:
        if specs:
            spec_str = " \\\n        ".join(f"'{s}'" for s in specs)
            case_lines.append(f"      {name})\n        _arguments -s \\\n        {spec_str} && ret=0\n        ;;")
        else:
            case_lines.append(f"      {name})\n        _message 'no arguments'\n        ;;")

    out = f"""#compdef ztmux
# ztmux zsh completion — GENERATED from the cmd_entry table + src/extensions.
# Do not edit by hand; regenerate with:  python3 scripts/gen_zsh_completion.py
#
# Covers every implemented command (and alias), the client subcommands
# (dashboard, switch, tree, doctor, stats, graph, watch, events, ps, snapshot,
# prune, layout, find, recent, usage), and the structured
# `-o json|jsonl|csv|tsv|table` output flag on the list-* commands.

__ztmux_run() {{ ztmux "$@" 2>/dev/null }}
__ztmux_sessions()  {{ local -a v; v=(${{(f)"$(__ztmux_run list-sessions -F '#{{session_name}}')"}}); compadd -a v }}
__ztmux_clients()   {{ local -a v; v=(${{(f)"$(__ztmux_run list-clients  -F '#{{client_name}}')"}}); compadd -a v }}
__ztmux_windows()   {{ local -a v; v=(${{(f)"$(__ztmux_run list-windows -a -F '#{{session_name}}:#{{window_index}}')"}}); compadd -a v }}
__ztmux_panes()     {{ local -a v; v=(${{(f)"$(__ztmux_run list-panes  -a -F '#{{session_name}}:#{{window_index}}.#{{pane_index}}')"}}); compadd -a v }}
__ztmux_buffers()   {{ local -a v; v=(${{(f)"$(__ztmux_run list-buffers   -F '#{{buffer_name}}')"}}); compadd -a v }}
__ztmux_key_tables() {{ compadd root prefix copy-mode copy-mode-vi }}

_ztmux() {{
  local curcontext="$curcontext" state line ret=1
  local -a commands
  commands=(
{chr(10).join(describe_lines)}
  )

  _arguments -C \\
    '2[force 256 colours]' \\
    '-C[start in control mode]' \\
    '-D[do not daemonise the server]' \\
    '-l[behave as a login shell]' \\
    '-N[do not start the server]' \\
    '-q[suppress errors]' \\
    '-u[assume UTF-8]' \\
    '-U[unlock the server]' \\
    '-v[request verbose logging]' \\
    '-V[report version]' \\
    '-c[execute shell-command]:shell command: ' \\
    '-f[specify configuration file]:config file:_files' \\
    '-L[socket name]:socket name: ' \\
    '-S[socket path]:socket path:_files' \\
    '-T[terminal features]:features: ' \\
    '1: :->cmds' \\
    '*:: :->args' && ret=0

  case $state in
    cmds)
      _describe -t commands 'ztmux command' commands && ret=0
      ;;
    args)
      curcontext="${{curcontext%:*:*}}:ztmux-$line[1]:"
      case $line[1] in
{chr(10).join(case_lines)}
      *)
        _default && ret=0
        ;;
      esac
      ;;
  esac
  return ret
}}

_ztmux "$@"
"""
    open("completions/_ztmux", "w", encoding="utf-8").write(out)
    print(f"wrote completions/_ztmux — {len(cmds)} commands + {len(EXTENSIONS)} extensions")
    return 0


if __name__ == "__main__":
    sys.exit(main())
