#!/usr/bin/env bash
# memcheck.sh — reproduce the ztmux server memory bug (dangling ibuf node that
# crashes msgbuf_write with SIGBUS) under a memory checker so the fault traps at
# the exact use-after-free instead of a wild read far downstream.
#
# Two modes, cheapest first:
#
#   ./scripts/memcheck.sh guard   [-- ztmux args...]   # no rebuild, uses Guard Malloc
#   ./scripts/memcheck.sh asan    [-- ztmux args...]   # rebuild with AddressSanitizer
#
# In BOTH modes the server must run in the foreground (NOFORK) so the checker
# stays attached to the process that crashes — a normally daemonized server
# forks away from the harness. We pass a dedicated socket and rely on the
# in-code guards (ibuf_free / ibuf_enqueue / msgbuf_write) plus ~/.ztmux crash
# dumps as a backstop.
set -euo pipefail

cd "$(dirname "$0")/.."

MODE="${1:-}"; shift || true
# drop a leading "--" separator if present
[ "${1:-}" = "--" ] && shift || true
SOCK="${ZTMUX_SOCK:-/tmp/ztmux-memcheck.sock}"

case "$MODE" in
guard)
  # macOS Guard Malloc: places each allocation on its own page and unmaps it on
  # free, so the first access to a freed ibuf faults immediately (SIGSEGV) with
  # the stack pointing at the use. No rebuild needed — works on the debug binary.
  # MallocScribble fills freed memory with 0x55 as a second signal. Slow: fine
  # for reproduction, not for daily use.
  cargo build --bin ztmux
  echo ">> running debug ztmux under Guard Malloc on socket $SOCK"
  exec env \
    DYLD_INSERT_LIBRARIES=/usr/lib/libgmalloc.dylib \
    MallocScribble=1 \
    MALLOC_PROTECT_BEFORE=1 \
    MallocGuardEdges=1 \
    ./target/debug/ztmux -vv -S "$SOCK" "$@"
  ;;
asan)
  # AddressSanitizer: intercepts malloc/free and maintains a quarantine +
  # redzones, so a use-after-free or heap overflow traps with a full report
  # (allocation, free, and use stacks). Requires nightly + build-std. aarch64
  # macOS is supported.
  echo ">> building ztmux with AddressSanitizer (nightly)"
  RUSTFLAGS="-Zsanitizer=address" \
    cargo +nightly build --bin ztmux \
    -Zbuild-std \
    --target aarch64-apple-darwin
  BIN=./target/aarch64-apple-darwin/debug/ztmux
  echo ">> running ASan ztmux on socket $SOCK"
  # halt_on_error=0 keeps it alive past the first non-fatal report; detect_leaks
  # is off (tmux intentionally leaks some globals at exit).
  exec env \
    ASAN_OPTIONS="abort_on_error=1:detect_leaks=0:halt_on_error=1:print_stats=1" \
    "$BIN" -vv -S "$SOCK" "$@"
  ;;
*)
  echo "usage: $0 {guard|asan} [-- ztmux args...]" >&2
  echo "  guard  no-rebuild macOS Guard Malloc (fast to start, slow to run)" >&2
  echo "  asan   rebuild with AddressSanitizer (nightly, most detailed report)" >&2
  exit 2
  ;;
esac
