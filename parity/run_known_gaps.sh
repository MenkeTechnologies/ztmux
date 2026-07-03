#!/usr/bin/env bash
# Known-gaps runner — the INVERSE of run_parity.sh.
#
# parity/cases/ holds behaviour ztmux MATCHES (the green, blocking gate).
# parity/known_gaps/ holds behaviour ztmux does NOT match yet: next-3.7 features
# with no ztmux implementation (see parity/known_gaps/README.md). Each case is
# expected to DIVERGE between the vendored next-3.7 tmux and ztmux — that
# divergence is the proof the feature is unported.
#
# So the pass/fail sense is flipped:
#   diverges  -> "GAP" (expected; the feature is still unported)
#   matches   -> "CLOSED" (unexpected; the feature got ported — PROMOTE the case
#                to parity/cases/ and delete it here)
#
# Exit non-zero ONLY when a gap has unexpectedly closed, so this can run as an
# advisory tripwire without ever going red just because the gaps still exist.
#
# Usage: bash parity/run_known_gaps.sh [--summary]
# Env:   TMUX_REF / ZTMUX as in run_parity.sh.

set -uo pipefail

SUMMARY_ONLY=0
[[ "${1:-}" == "--summary" ]] && SUMMARY_ONLY=1

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export LC_ALL=C LANG=C
unset TMUX 2>/dev/null || true
PARITY_PID=$$
PARITY_TMPDIR="$(mktemp -d "/tmp/ztmux-gaps.${PARITY_PID}.XXXXXX")"
export TMUX_TMPDIR="$PARITY_TMPDIR"
cleanup() {
  pkill -f "ztgap_${PARITY_PID}_" 2>/dev/null || true
  rm -rf "$PARITY_TMPDIR" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

VENDOR_TMUX="$ROOT/vendor/tmux/tmux"
TMUX_REF="${TMUX_REF:-$VENDOR_TMUX}"
ZTMUX="${ZTMUX:-$ROOT/target/release/ztmux}"
if ! command -v "$TMUX_REF" >/dev/null 2>&1 && [[ ! -x "$TMUX_REF" ]]; then
  echo "known-gaps: reference tmux '$TMUX_REF' not found (build vendor/tmux first)" >&2
  exit 2
fi
if [[ ! -x "$ZTMUX" ]]; then
  echo "known-gaps: no executable at ZTMUX=$ZTMUX (cargo build --release)" >&2
  exit 2
fi

shopt -s nullglob
cases=("$ROOT"/parity/known_gaps/*.sh)
IFS=$'\n' cases=($(printf '%s\n' "${cases[@]}" | sort)); unset IFS
if [[ ${#cases[@]} -eq 0 ]]; then
  echo "known-gaps: no cases in parity/known_gaps/*.sh" >&2
  exit 2
fi

run_one() {
  local bin="$1" case="$2"
  local sock="ztgap_$$_${RANDOM}"
  timeout 15 "$bin" -L "$sock" -f /dev/null new-session -d -n base -x 80 -y 24 "sleep 300" >/dev/null 2>&1
  local out
  out=$(TM="$bin -L $sock" timeout 15 bash "$case" 2>&1)
  timeout 10 "$bin" -L "$sock" kill-server >/dev/null 2>&1
  printf '%s' "$out"
}

total=0 confirmed=0 closed=0
for f in "${cases[@]}"; do
  base=$(basename "$f")
  total=$((total + 1))
  ref_out=$(run_one "$TMUX_REF" "$f")
  port_out=$(run_one "$ZTMUX" "$f")
  if [[ "$ref_out" != "$port_out" ]]; then
    confirmed=$((confirmed + 1))
    [[ "$SUMMARY_ONLY" -eq 0 ]] && echo "GAP    $base"
  else
    closed=$((closed + 1))
    echo "CLOSED $base  (now matches next-3.7 — PROMOTE to parity/cases/)" >&2
  fi
done

printf 'known-gaps: %d/%d still diverging (unported) · %d unexpectedly closed\n' \
  "$confirmed" "$total" "$closed"

# Green while gaps remain open; red only when one closes and needs promoting.
[[ "$closed" -eq 0 ]]
