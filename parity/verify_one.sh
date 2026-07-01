#!/usr/bin/env bash
# Verify ONE parity case in isolation: run it through the vendored tmux
# (reference) and a ztmux binary, compare byte-for-byte. Mirrors run_parity.sh's
# per-case setup (fresh 80x24 detached "base" session, isolated socket) but for a
# single file, so a case author can check determinism/parity without running the
# whole suite. Exit 0 = parity, 1 = mismatch (prints both outputs).
#
#   ZTMUX=path/to/ztmux bash parity/verify_one.sh parity/cases/NAME.fmt
set -uo pipefail
case_file="$1"
Z="${ZTMUX:-target/debug/ztmux}"
T="${TMUX_REF:-vendor/tmux/tmux}"
export LC_ALL=C LANG=C
tmpd="$(mktemp -d)"; export TMUX_TMPDIR="$tmpd"
kind="${case_file##*.}"

run() {
  local bin="$1" sock="$2" out
  timeout 15 $bin -L "$sock" -f /dev/null new-session -d -n base -x 80 -y 24 "sleep 300" >/dev/null 2>&1
  if [ "$kind" = fmt ]; then
    out=$(timeout 15 $bin -L "$sock" display-message -p "$(cat "$case_file")" 2>&1)
  else
    out=$(TM="$bin -L $sock" timeout 15 bash "$case_file" 2>&1)
  fi
  timeout 10 $bin -L "$sock" kill-server >/dev/null 2>&1
  printf '%s' "$out"
}

zo="$(run "$Z" "z$$_${RANDOM}")"
to="$(run "$T" "t$$_${RANDOM}")"
rm -rf "$tmpd"
if [ "$zo" = "$to" ]; then
  echo "OK   $(basename "$case_file")"
else
  echo "FAIL $(basename "$case_file")"
  echo "  tmux=[$to]"
  echo "  ztmux=[$zo]"
  exit 1
fi
