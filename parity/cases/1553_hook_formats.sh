# The #{hook_*} formats a hook body actually sees.
#
# notify_hook builds a format tree for every notification (notify.c:199-216) and
# that tree is the only channel a hook has for "which thing did this happen to".
# Two of them were wrong in the port and both are the kind of bug that never
# throws:
#
#   * hook_pane read `%%3` instead of `%3`. C formats it with printf, where `%%`
#     is one literal percent; the port's format_add! is a RUST format string,
#     where `%%` is two. Every `run-shell` that fed #{hook_pane} to another tmux
#     command got an unusable target and failed at the far end.
#
#   * hook_window and hook_window_name were only set on the `w != NULL` branch.
#     The C sets them a SECOND time from wp->window on the pane branch
#     (notify.c:213-215), because a pane notification passes no window. Without
#     that, both expanded EMPTY for every pane-scoped hook.
#
# Hook bodies are appended to a file rather than printed, since the notification
# is delivered asynchronously and the ordering of the run-shell output against
# the rest of the case's stdout is not stable. Ids are masked: `%3`/`@1` depend on
# how many panes and windows the server has created, which the shared suite makes
# no promise about -- but the masks keep the SHAPE, so a doubled percent or an
# empty field still shows.
OUT="${TMPDIR:-/tmp}/hookfmt.$$"
: > "$OUT"

# `%%PANE` (not `%PANE`) is what the double-percent bug renders, and `[]` is what
# a missing hook_window renders, so both defects survive the mask.
mask() { perl -pe 's/%(\d+)/%PANE/g; s/\@(\d+)/\@WIN/g; s/\$(\d+)/\$SESS/g'; }

# The inner echo SINGLE-quotes its payload: hook_session expands to `$0`, and an
# unquoted `$0` would then be expanded again by the shell run-shell spawns (to
# `sh`), hiding the field this case exists to read.
rec='run-shell "echo '"'"'EV=#{hook} pane=[#{hook_pane}] win=[#{hook_window}] winname=[#{hook_window_name}] sess=[#{hook_session}] sessname=[#{hook_session_name}]'"'"' >> '"$OUT"'"'

# pane-focus-in / -out need an attached client and never fire here; the events
# below all reach a pane WITHOUT one, which is what makes the pane branch of
# notify_hook reachable from this suite at all.
# notify_pane's full event set (notify.c:306 call sites), plus a few
# window/session ones so the pane branch is compared against the branch it was
# wrongly sharing behaviour with.
for ev in pane-died pane-exited pane-mode-changed pane-title-changed \
          after-split-window window-linked after-new-window \
          session-window-changed after-select-pane; do
  $TM set-hook -g "$ev" "$rec"
done

$TM new-window -d -n second 'sleep 300'

# pane-exited: a pane whose command ends normally.
$TM split-window -d -t second 'sleep 0.2'
sleep 1.5

# pane-died: remain-on-exit keeps the dead pane, which is the other notify_pane
# path and reaches it with a DIFFERENT pane id than the one above.
$TM set -w -t second remain-on-exit on
$TM split-window -d -t second 'exit 3'
sleep 1.5

# pane-mode-changed: entering and leaving copy mode, twice per toggle.
$TM copy-mode -t second.0
sleep 0.5
$TM send-keys -X -t second.0 cancel
sleep 0.5

# pane-title-changed.
$TM select-pane -t second.0 -T retitled
sleep 0.5

$TM select-pane -t second.0
$TM select-window -t second
sleep 1.5

echo "== events, sorted (delivery order is not deterministic) =="
sort "$OUT" | mask

echo "== every event that carries a pane also carries its window =="
# The exact regression. Restricted to the lines that HAVE a pane, since the
# window-scoped and session-scoped events legitimately report an empty pane.
paned=$(sort "$OUT" | mask | grep 'pane=\[%PANE\]')
printf 'pane events: %s\n' "$(printf '%s\n' "$paned" | grep -c .)"
printf 'of those, missing window: %s\n' \
  "$(printf '%s\n' "$paned" | grep -c 'win=\[\]\|winname=\[\]')"
printf 'of those, doubled percent: %s\n' \
  "$(printf '%s\n' "$paned" | grep -c 'pane=\[%%')"

echo "== the raw pane id, unmasked, is a single percent =="
sort "$OUT" | grep -o 'pane=\[%*[0-9]*\]' | perl -pe 's/\[(%*)\d*\]/[$1N]/' | sort -u

rm -f "$OUT"
