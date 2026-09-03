#!/usr/bin/env bash
# bench_vs_tmux.sh — measure ztmux against the system tmux C binary on the same
# machine, same terminal size, same commands, back to back.
#
#   ./scripts/bench_vs_tmux.sh                 # release ztmux vs `tmux` in PATH
#   ZTMUX=/path/to/ztmux TMUX_BIN=/path/to/tmux ./scripts/bench_vs_tmux.sh
#   REPS=40 WINDOWS=40 ./scripts/bench_vs_tmux.sh
#
# What is measured, per binary:
#
#   cold start   wall time of `new-session -d` against a dead socket: fork and
#                daemonize the server, build session/window/pane, answer the
#                client. This is the number a user feels on first attach.
#   round trip   wall time of `list-sessions` against an already-running server:
#                client connect + command dispatch + reply + exit.
#   RSS 1 win    server resident set size with one 80x24 window.
#   RSS N win    server RSS after $WINDOWS windows exist.
#   binary       size of the on-disk executable.
#
# Both binaries run with `-f /dev/null` and `/bin/cat` as the pane command, so
# no user config and no login shell is timed — the numbers are the multiplexer
# itself. A real ~/.tmux.conf dominates cold start for both binaries equally.
#
# Every number printed is measured in this run; nothing is cached or assumed.
# Output is a markdown-pasteable table on stdout.
set -euo pipefail

cd "$(dirname "$0")/.."

ZTMUX="${ZTMUX:-./target/release/ztmux}"
TMUX_BIN="${TMUX_BIN:-$(command -v tmux)}"
REPS="${REPS:-50}"
WINDOWS="${WINDOWS:-20}"

[ -x "$ZTMUX" ] || { echo "no ztmux binary at $ZTMUX (cargo build --release)" >&2; exit 1; }
[ -x "$TMUX_BIN" ] || { echo "no tmux binary (set TMUX_BIN)" >&2; exit 1; }

# hi-res wall clock around a command; prints milliseconds to the caller's stdout
# while the command's own stdout is discarded.
timeit() {
  perl -MTime::HiRes=time -e '
    open(my $out, ">&", \*STDOUT) or die $!;
    open(STDOUT, ">", "/dev/null") or die $!;
    my $t0 = time;
    system(@ARGV) == 0 or exit 1;
    printf {$out} "%.2f\n", (time - $t0) * 1000;
  ' -- "$@"
}

# best (minimum) of the numbers on stdin. Process-latency noise on a loaded
# machine is one-sided — it only ever adds time — so the minimum is the stable
# estimate of the real cost while the median tracks whatever else is running.
best() {
  perl -e 'my @v = sort { $a <=> $b } map { chomp; $_ } <STDIN>;
           @v or die "no samples\n";
           printf "%.2f\n", $v[0];'
}

# rss of a pid, in KiB (portable across BSD/GNU ps)
rss_kb() { ps -o rss= -p "$1" | tr -d ' '; }

bench() { # bench <label> <binary>
  local bin="$2" sock="/tmp/ztmux-bench-$$-$1.sock"
  local i cold warm pid rss1 rssn

  "$bin" -S "$sock" kill-server 2>/dev/null || true
  rm -f "$sock"

  # cold start: dead socket -> answered new-session, repeated from scratch
  cold=$(for i in $(seq "$REPS"); do
           "$bin" -S "$sock" kill-server 2>/dev/null || true
           rm -f "$sock"
           timeit "$bin" -f /dev/null -S "$sock" new-session -d -x 80 -y 24 /bin/cat
         done | best)

  # server is up from the last cold iteration; measure warm round trips
  warm=$(for i in $(seq "$REPS"); do
           timeit "$bin" -S "$sock" list-sessions
         done | best)

  pid=$("$bin" -S "$sock" display-message -p '#{pid}')
  rss1=$(rss_kb "$pid")

  for i in $(seq 2 "$WINDOWS"); do "$bin" -S "$sock" new-window -d /bin/cat; done
  rssn=$(rss_kb "$pid")

  "$bin" -S "$sock" kill-server 2>/dev/null || true
  rm -f "$sock"

  printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$1" "$cold" "$warm" "$rss1" "$rssn" "$(wc -c < "$bin" | tr -d ' ')"
}

zt=$(bench ztmux "$ZTMUX")
tm=$(bench tmux "$TMUX_BIN")

printf '%s vs %s on %s, %s reps (best of), %s windows\n\n' \
  "$("$ZTMUX" -V)" "$("$TMUX_BIN" -V)" "$(uname -sm)" "$REPS" "$WINDOWS"

{
  printf 'binary\tcold_start_ms\troundtrip_ms\trss_1win_kb\trss_%swin_kb\tsize_bytes\n' "$WINDOWS"
  printf '%s\n%s\n' "$zt" "$tm"
} | column -t -s "$(printf '\t')"
