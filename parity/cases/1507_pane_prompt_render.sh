# The in-pane prompt as a CLIENT actually paints it.
#
# Case 1506 proves the -P flag parses and reaches the 32 default bindings. This
# proves the prompt lands where the C puts it, which is the half no state-level
# check can see: `command-prompt -P` draws over the PANE's own bottom row and
# leaves the status bar alone, where a status-line prompt would replace it.
#
# Built the same way as case 1504: a second server inside a pane of the first,
# with a client attached to it, so capture-pane on the outer server reads back
# what the inner client drew.
#
# Both rows are asserted, not just the prompt's: a port that drew the prompt in
# the right place but ate the status bar would still be wrong, and that is
# exactly what the status-line fallback does.
set -- $TM
BIN="$1"
ISOCK="pprt_$$_inner"

$BIN -L "$ISOCK" -f /dev/null new-session -d -s hr -n one -x 80 -y 24 \
  'printf "ok line 1\nError: disk full\nok line 3\nError: timeout\nok line 5\n"; sleep 300'
$BIN -L "$ISOCK" setw -g mode-keys vi
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating prompt is an intentional extension; this case is about the
# ported placement, so pin the option the extension reads. tmux ignores it.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off

$TM new-window -d -n client "$BIN -L $ISOCK attach -t hr"
sleep 2

# A plain -P prompt, opened with -b so it does not block the command queue.
$BIN -L "$ISOCK" command-prompt -b -P -p '(pane prompt)' 'display-message hi'
sleep 1
echo "pane-prompt:"
$TM capture-pane -p -t client | grep -n . | perl -pe 's/\s+$//'

# Escape closes it and the pane row comes back.
$TM send-keys -t client Escape
sleep 1
echo "after-escape:"
$TM capture-pane -p -t client | grep -n . | perl -pe 's/\s+$//'

# The copy-mode search prompt, which is where the flag actually reaches a user:
# `?` in vi copy mode is one of the 32 bindings that carries -P.
$TM send-keys -t client C-b
sleep 0.4
$TM send-keys -t client '['
sleep 0.5
$TM send-keys -t client '?'
sleep 0.8
echo "search-prompt:"
$TM capture-pane -p -t client | grep -n . | perl -pe 's/\s+$//'

for ch in E r r o r; do $TM send-keys -t client "$ch"; sleep 0.25; done
$TM send-keys -t client Enter
sleep 1
echo "search-result:"
$BIN -L "$ISOCK" display-message -p -t one \
  'cursor=#{copy_cursor_y},#{copy_cursor_x} matches=#{search_count} present=#{search_present} line=[#{copy_cursor_line}]'

$BIN -L "$ISOCK" kill-server 2>/dev/null
