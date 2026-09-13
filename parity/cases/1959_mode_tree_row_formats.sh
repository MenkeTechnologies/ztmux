# The nine mode_tree_* variables (mode-tree.c:884-908) drive exactly one format:
# MODE_TREE_PREFIX_FORMAT (mode-tree.c:43-55), a compile-time constant no `-F`
# or `-K` can reach. So the only way to read them is the prefix each row is
# DRAWN with, which makes the glyph column a direct assertion on the flags:
#
#   mode_tree_key / _key_width   the "(N)" column, padded with #{p/...}
#   mode_tree_repeat             the #{R:} indent, depth - 1 copies
#   mode_tree_parent_last        that indent is blank vs an acs `x` riser
#   mode_tree_branch / _last     acs `tq` mid-branch vs `mq` last-branch
#   mode_tree_has_children       a +/- toggle vs nothing
#   mode_tree_expanded           which of + and - it is
#   mode_tree_flat               a flat list pads instead of drawing a toggle
#   mode_tree_selected           the row style, so it needs -e to be visible
#
# Case 1508 captures six rows of a two-session tree, which reaches depth 1 and
# never leaves everything expanded: no #{R:} repeat above zero, no parent_last
# riser, no collapsed row, no flat list. This case drives all of them -- panes
# put rows at depth 2, the tree gets collapsed and re-expanded under the cursor,
# and choose-buffer supplies the flat case.
#
# Like 1504/1507/1508 it needs a real client, because mode_tree_draw only runs
# for one: a second server inside a pane of the first, with the inner client
# attached, so capture-pane on the OUTER server reads back what the inner client
# drew. Keys go to that client too -- `send-keys -X` against the pane is
# refused with "not in a mode", because the mode belongs to the client.
set -- $TM
BIN="$1"
ISOCK="mtf_$$_inner"

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" split-window -d -t alpha:one 'sleep 300'
$BIN -L "$ISOCK" split-window -d -t alpha:one 'sleep 300'
$BIN -L "$ISOCK" new-window -d -t alpha -n two 'sleep 300'
$BIN -L "$ISOCK" new-session -d -s beta -n solo -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the PORTED
# rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
$BIN -L "$ISOCK" set-buffer -b b1 hello
$BIN -L "$ISOCK" set-buffer -b b2 world

wait_mode() {   # $1 = expected mode name, or "" for no mode
  local want="$1" i=0 got
  while [ $i -lt 100 ]; do
    got=$($BIN -L "$ISOCK" display-message -p -t alpha:one '#{pane_mode}' 2>/dev/null)
    [ "$got" = "$want" ] && { sleep 0.2; return 0; }
    i=$((i+1)); sleep 0.1
  done
  echo "wait_mode: timed out waiting for [$want], last=[$got]"
  return 1
}

# choose-buffer rows carry a clock, which can cross a minute between the two
# runs; mask it. Rows are picked by their "(N)" key column so the box border and
# any status row cannot drift into the comparison.
scrub() { perl -pe "s/\\d\\d:\\d\\d/HH:MM/g"; }

# The tree occupies the top rows; -N drops the preview so every row is a tree
# row. Trailing blanks are trimmed so a row that ends in padding compares equal
# to one that does not.
trim()  { perl -pe 's/[ \t]+$//'; }
rows()  { $TM capture-pane -p -t client    | cat -v | perl -ne 'print if /^\(/' | scrub | trim | sed -n "1,${1:-9}p"; }
erows() { $TM capture-pane -p -e -t client | cat -v | perl -ne 'print if /\(/' | scrub | trim | sed -n "1,${1:-3}p"; }

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"
# Wait for the inner client to be attached instead of sleeping a fixed two
# seconds: every case here must finish inside the runner's 15s budget
# (run_parity.sh:172), and a blind sleep spends an eighth of it doing nothing.
wait_client() {
  local i=0
  while [ $i -lt 300 ]; do
    [ -n "$($BIN -L "$ISOCK" list-clients -F "#{client_tty}" 2>/dev/null)" ] && { sleep 0.2; return 0; }
    i=$((i+1)); sleep 0.05
  done
  echo "wait_client: TIMEOUT"
}
wait_client

echo "== sessions, windows and panes: depth 0, 1 and 2 =="
$BIN -L "$ISOCK" choose-tree -N -t alpha:one
wait_mode tree-mode
rows 10

echo "== the selected row carries the style, the others do not =="
erows 2

echo "== collapse the row under the cursor: + replaces -, children go away =="
$TM send-keys -t client Left
sleep 0.3
rows 6

echo "== and expanding it again restores them =="
$TM send-keys -t client Right
sleep 0.3
rows 6

echo "== the selection moves down two rows =="
$TM send-keys -t client Down
$TM send-keys -t client Down
sleep 0.3
erows 4

$TM send-keys -t client q
wait_mode ''

echo "== a flat list draws no branch glyph and no toggle =="
$BIN -L "$ISOCK" choose-buffer -N -t alpha:one
wait_mode buffer-mode
rows 3
$TM send-keys -t client q
wait_mode ''

$BIN -L "$ISOCK" kill-server 2>/dev/null
