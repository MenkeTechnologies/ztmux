#!/usr/bin/env bash
# Parity: compare the VENDORED tmux (the reference) vs ztmux (the port) on the
# same inputs, byte-for-byte (stdout+stderr). ztmux is a from-source port of the
# tmux under vendor/tmux (next-3.7), so the truth we measure against is THAT exact
# tmux — built from vendor/tmux, not whatever version the system has. Override
# with TMUX_REF=/path/to/tmux if you really want a different reference.
#
# Usage: from repo root —  bash parity/run_parity.sh [--summary] [--json OUT] [--fail-log PATH]
# Env:   TMUX_REF=tmux            reference binary (the real tmux)
#        ZTMUX=target/release/ztmux   the port binary under test
#
# Cases live in parity/cases/ and come in two flavors:
#
#   *.fmt  — a single tmux FORMAT string (see FORMATS in tmux(1)). The runner
#            expands it with `display-message -p` against a fresh detached
#            session, e.g. a file containing `#{e|+|:2,3}` compares the two
#            binaries' expansion of that arithmetic.
#
#   *.sh   — a shell scenario for multi-command cases. The runner exports `$TM`
#            as the binary already bound to a private socket, so the script just
#            runs `$TM <cmd>` lines and prints deterministic output, e.g.
#               $TM new-window; $TM list-windows -F '#{window_index}'
#
# For every case the runner starts an isolated server per binary (`-L <uniq>`,
# `-f /dev/null`, fixed 80x24 geometry), runs the case under a timeout, captures
# stdout+stderr, kills the server, then compares the two captures with `cmp`.
#
# Determinism: cases MUST avoid host/time/version/pid-dependent output
# (`#{host}`, `#{version}`, `#{pid}`, times, etc.). Fixed geometry is provided so
# width/height formats are stable. See parity/PARITY_ROADMAP.md.
#
# Flags mirror the sibling parity suites (zshrs, strykelang):
#   --summary          Suppress per-case OK/FAIL lines; totals still print.
#   --json PATH        Write a JSON summary (total/passed/failed/percent).
#   --fail-log PATH    Per-case failure detail (both outputs + diff). `-` = stderr.
#                      Default: parity/parity_failures.log (truncated per run).

set -uo pipefail

SUMMARY_ONLY=0
JSON_OUT=""
FAIL_LOG=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --summary)    SUMMARY_ONLY=1; shift ;;
    --json)       JSON_OUT="${2:-}"; shift 2 ;;
    --json=*)     JSON_OUT="${1#--json=}"; shift ;;
    --fail-log)   FAIL_LOG="${2:-}"; shift 2 ;;
    --fail-log=*) FAIL_LOG="${1#--fail-log=}"; shift ;;
    *) echo "parity: unknown flag: $1" >&2; exit 2 ;;
  esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export LC_ALL=C LANG=C
# Keep the reference tmux from reading a user config or environment session, and
# give this run its OWN private socket directory so every per-case server socket
# is isolated under it. Both tmux and ztmux honour TMUX_TMPDIR, so all sockets
# (tmux-<uid>/… and ztmux-<uid>/…) land here and are wiped wholesale on exit —
# belt-and-suspenders over each case's kill-server, which previously left
# thousands of orphaned `ztpar_*` sockets in the shared /tmp/tmux-<uid> dir.
unset TMUX 2>/dev/null || true
PARITY_PID=$$
# Base the private dir on /tmp (tmux's own default tmpdir), NOT $TMPDIR: on macOS
# $TMPDIR is a long /private/var/folders/… path, and appending tmux-<uid>/<sock>
# would blow past the ~104-char AF_UNIX sun_path limit ("File name too long").
PARITY_TMPDIR="$(mktemp -d "/tmp/ztmux-parity.${PARITY_PID}.XXXXXX")"
export TMUX_TMPDIR="$PARITY_TMPDIR"
tmp_json=""
cleanup() {
  # Kill any servers this run started (their -L labels are prefixed
  # `ztpar_<pid>_`) in case a kill-server timed out, then remove the private
  # socket dir and any stray summary temp file. Runs on normal exit and on
  # interrupt.
  pkill -f "ztpar_${PARITY_PID}_" 2>/dev/null || true
  rm -rf "$PARITY_TMPDIR" 2>/dev/null || true
  [[ -n "$tmp_json" ]] && rm -f "$tmp_json" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

# The reference is the VENDORED tmux — the exact source ztmux ports (next-3.7),
# NOT whatever tmux the system happens to have. Version matters: layout rounding,
# div-by-zero formatting, and other format details change between releases, so a
# system tmux of a different version produces false diffs. Build vendor/tmux once
# (gitignored artifacts) and use it; fall back to system tmux only if that build
# is impossible, with a loud warning.
VENDOR_TMUX="$ROOT/vendor/tmux/tmux"
if [[ -z "${TMUX_REF:-}" ]]; then
  if [[ ! -x "$VENDOR_TMUX" && -f "$ROOT/vendor/tmux/configure.ac" ]]; then
    echo "parity: building vendored tmux reference (vendor/tmux, next-3.7)…" >&2
    (
      builtin cd "$ROOT/vendor/tmux"
      [[ -x ./configure ]] || sh autogen.sh
      [[ -f Makefile ]] || ./configure --disable-utf8proc
      make -j"$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 2)"
    ) >"${TMPDIR:-/tmp}/ztmux-vendor-tmux-build.log" 2>&1 ||
      echo "parity: vendored tmux build failed (see ${TMPDIR:-/tmp}/ztmux-vendor-tmux-build.log)" >&2
  fi
  if [[ -x "$VENDOR_TMUX" ]]; then
    TMUX_REF="$VENDOR_TMUX"
  else
    TMUX_REF="tmux"
    echo "parity: WARNING falling back to system tmux ($(tmux -V 2>/dev/null || echo '?')); version drift vs the vendored next-3.7 may cause false diffs" >&2
  fi
fi
ZTMUX="${ZTMUX:-$ROOT/target/release/ztmux}"

if ! command -v "$TMUX_REF" >/dev/null 2>&1; then
  echo "parity: reference tmux '$TMUX_REF' not found on PATH" >&2
  exit 2
fi
if [[ ! -x "$ZTMUX" ]]; then
  echo "parity: building release ztmux (cargo build --release)…" >&2
  (builtin cd "$ROOT" && cargo build --release --locked -q) || true
fi
if [[ ! -x "$ZTMUX" ]]; then
  echo "parity: no executable at ZTMUX=$ZTMUX" >&2
  exit 2
fi

shopt -s nullglob
cases=("$ROOT"/parity/cases/*.fmt "$ROOT"/parity/cases/*.sh)
IFS=$'\n' cases=($(printf '%s\n' "${cases[@]}" | sort)); unset IFS
if [[ ${#cases[@]} -eq 0 ]]; then
  echo "parity: no cases in parity/cases/*.{fmt,sh}" >&2
  exit 2
fi

if [[ -z "$FAIL_LOG" ]]; then FAIL_LOG="$ROOT/parity/parity_failures.log"; fi
if [[ "$FAIL_LOG" == "-" ]]; then exec 7>&2; else : >"$FAIL_LOG"; exec 7>"$FAIL_LOG"; fi

# Run one case against one binary; echo its captured stdout+stderr.
# $1 = binary, $2 = case file, $3 = kind (fmt|sh)
run_one() {
  local bin="$1" case="$2" kind="$3"
  local sock="ztpar_$$_${RANDOM}"
  local out
  # Fresh isolated server with a fixed geometry and a long-lived dummy pane.
  timeout 15 "$bin" -L "$sock" -f /dev/null new-session -d -n base -x 80 -y 24 "sleep 300" >/dev/null 2>&1
  if [[ "$kind" == "fmt" ]]; then
    local fmt; fmt="$(cat "$case")"
    out=$(timeout 15 "$bin" -L "$sock" display-message -p "$fmt" 2>&1)
  else
    out=$(TM="$bin -L $sock" timeout 15 bash "$case" 2>&1)
  fi
  timeout 10 "$bin" -L "$sock" kill-server >/dev/null 2>&1
  printf '%s' "$out"
}

total=0 passed=0 failed=0
for f in "${cases[@]}"; do
  base=$(basename "$f")
  kind="${f##*.}"
  total=$((total + 1))
  ref_out=$(run_one "$TMUX_REF" "$f" "$kind")
  port_out=$(run_one "$ZTMUX" "$f" "$kind")
  if [[ "$ref_out" == "$port_out" ]]; then
    [[ "$SUMMARY_ONLY" -eq 0 ]] && echo "parity OK:   $base"
    passed=$((passed + 1))
  else
    echo "parity FAIL: $base" >&2
    {
      echo "==== $base ===="
      echo "--- tmux (reference) ---"; printf '%s\n' "$ref_out"
      echo "--- ztmux (port) ---";     printf '%s\n' "$port_out"
      echo "--- diff (tmux vs ztmux) ---"
      diff -u <(printf '%s\n' "$ref_out") <(printf '%s\n' "$port_out") || true
      echo
    } >&7
    failed=$((failed + 1))
  fi
done

exec 7>&-

pct=$(awk -v p="$passed" -v t="$total" 'BEGIN{ if (t==0) print "0.00"; else printf "%.2f", 100*p/t }')
version=$(awk -F'"' '/^name[[:space:]]*=/{next} /^version[[:space:]]*=/{print $2; exit}' "$ROOT/Cargo.toml")
version="${version:-unknown}"
generated=$(date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || echo unknown)

printf 'parity: %d/%d passed (%s%%) · failed %d · ztmux v%s vs %s\n' \
  "$passed" "$total" "$pct" "$failed" "$version" "$("$TMUX_REF" -V 2>/dev/null || echo tmux)"

if [[ "$failed" -gt 0 && "$FAIL_LOG" != "-" ]]; then
  echo "parity: failure details in $FAIL_LOG" >&2
fi

if [[ -n "$JSON_OUT" ]]; then
  tmp_json=$(mktemp "${TMPDIR:-/tmp}/parity.summary.$$.XXXXXX")
  printf '{\n  "total": %d,\n  "passed": %d,\n  "failed": %d,\n  "percent": %s,\n  "ztmux_version": "%s",\n  "reference": "%s",\n  "generated_at": "%s"\n}\n' \
    "$total" "$passed" "$failed" "$pct" "$version" "$("$TMUX_REF" -V 2>/dev/null || echo tmux)" "$generated" >"$tmp_json"
  command mv "$tmp_json" "$JSON_OUT"
fi

# Exit code is the pass/fail signal. The CI job runs this advisory
# (continue-on-error) while the port's server is still coming up.
[[ "$failed" -eq 0 ]]
