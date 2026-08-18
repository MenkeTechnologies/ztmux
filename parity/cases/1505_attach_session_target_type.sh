# attach-session picks its target TYPE from the target string.
#
# C cmd-attach-session.c:80: `tflag[strcspn(tflag, ":.")] != '\0'` -- strcspn
# stops at the first ':' or '.', so the test is "the target CONTAINS one of
# them". Contains one: resolve as a PANE. Otherwise: resolve as a SESSION, with
# CMD_FIND_PREFER_UNATTACHED.
#
# Getting that backwards is invisible until the target names something that
# exists as a window but not as a session, which is the common case -- a
# `hr` session whose windows are `code` and `docs`. Resolving `code` as a pane
# target finds the window and attaches; the C refuses, because `code` is not a
# session. dotmatrix users hit this through `tmux attach -t <name>` in shell
# aliases, where the wrong answer silently attaches to something else.
#
# Every attach below runs from a non-terminal, so a target that RESOLVES fails
# later with "open terminal failed"; that message is the marker for "the target
# was accepted", and "can't find session" for "the target was rejected".
$TM new-session -d -s hr -n code
$TM new-window -d -t hr -n docs
$TM list-sessions -F '#{session_name}'
$TM list-windows -t hr -F '#{window_index}:#{window_name}'
for t in code docs hr hr:docs hr:code.0 hr. hr: : . nosuch; do
  printf 'attach -t %-10s ' "$t"
  $TM attach-session -t "$t" 2>&1 | head -1
done
# No -t at all: the session is chosen, not looked up.
printf 'attach (no -t)   '
$TM attach-session 2>&1 | head -1
