# display-menu AS THE CLIENT DRAWS IT.
#
# Everything about a menu is client-side: menu_draw_cb -> screen_write_menu ->
# format_draw runs only for a client with an overlay, so no server-side case can
# see any of it. Case 1465 pins the ARGUMENT PARSER (every form stops at "no
# current client"); this case pins the PIXELS: the box, the title in the top
# border, the right-aligned "(key)" column, the separator row's tee glyphs, the
# dim rendering of a disabled "-" entry, the selected-item colours, and the fact
# that -y names the BOTTOM edge (cmd-display-menu.c:262, `n -= h`).
#
# Also pins menu_key_cb navigation: Down/Up must SKIP both the separator and the
# disabled entry (menu.c:377-380 and menu.c:394-400), which is invisible unless
# you can see which row is highlighted.
#
# Built like cases 1504/1507/1508: an inner server with a real attached client
# living in a pane of the outer server, so capture-pane on the OUTER server
# reads back exactly what the inner client painted.
set -- $TM
BIN="$1"
ISOCK="dmr_$$_inner"

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the
# PORTED rendering. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off

# NOTE: only ONE separator between items. tmux collapses a run of consecutive
# separators into a single line (menu.c:77-78); ztmux currently draws one line
# per empty argument, so a run of them is a known real divergence and is
# deliberately left out of this case rather than baked into it.
$BIN -L "$ISOCK" bind M display-menu -T Demo -x 2 -y 12 \
  Alpha a 'list-windows' \
  Beta b 'list-panes' \
  '' \
  '-Nope' n '' \
  Gamma g 'list-sessions'

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"

# Poll the drawn screen instead of sleeping: this case runs alongside ~1200
# others, so a blind sleep races the client's paint under load. Once the
# expected text is there, wait until two consecutive captures agree, so a
# half-painted overlay can never be read back either.
snap()      { $TM capture-pane -p -t client; }
settle()    { local a b i=0; a=$(snap); while [ $i -lt 100 ]; do sleep 0.05; b=$(snap);
                [ "$a" = "$b" ] && return 0; a=$b; i=$((i+1)); done; }
wait_for()  { local i=0; while [ $i -lt 300 ]; do
                snap | grep -qF "$1" && { settle; return 0; }
                i=$((i+1)); sleep 0.05; done; echo "wait_for TIMEOUT [$1]"; return 1; }
wait_gone() { local i=0; while [ $i -lt 300 ]; do
                snap | grep -qF "$1" || { settle; return 0; }
                i=$((i+1)); sleep 0.05; done; echo "wait_gone TIMEOUT [$1]"; return 1; }

# Which row of the menu is highlighted, by name. The selected row is the only
# one drawn with a background colour, so grep the SGR capture for it.
selected() {
  $TM capture-pane -p -e -t client |
    grep -o '\[48;2;184;134;11m [A-Za-z]*' | perl -pe 's/.* //' | tr '\n' ' '
}
# Send a navigation key and report where the highlight actually landed, polling
# until it moves rather than sleeping a fixed amount.
step() {
  local before after i=0
  before=$(selected)
  $TM send-keys -t client "$1"
  while [ $i -lt 300 ]; do
    after=$(selected)
    [ "$after" != "$before" ] && { echo "$1 -> [$after]"; return 0; }
    i=$((i+1)); sleep 0.05
  done
  echo "step TIMEOUT [$1] still=[$before]"; return 1
}

wait_for '[alpha] 0:one*'
$TM send-keys -t client C-b M
wait_for 'Demo'

# The whole box. -y 12 with a 7-row box must land the TOP edge on row 5 (12-7),
# and -x 2 must indent it two columns: an off-by-one in either axis moves the
# whole block and this diff catches it.
echo "== menu box (rows 1..14, -x 2 -y 12) =="
$TM capture-pane -p -t client | sed -n '1,14p' | cat -v | perl -ne 'printf "%2d|%s", $., $_'

# Styles: selected row (item 0), plain row, separator row, dim disabled row.
echo "== menu rows with SGR =="
$TM capture-pane -p -e -t client | sed -n '6,11p' | cat -v

echo "== selection walk =="
echo "open -> [$(selected)]"
step Down          # Alpha -> Beta
step Down          # Beta  -> Gamma: must skip the separator AND "-Nope"
step Up            # back over both
step Up            # -> Alpha
step Up            # wraps to the last selectable entry

# Escape dismisses without running the item's command.
$TM send-keys -t client Escape
wait_gone 'Demo'
echo "== after Escape, row 6 =="
$TM capture-pane -p -t client | sed -n '6p' | cat -v | perl -pe 's/^$/(blank)/'

$BIN -L "$ISOCK" kill-server 2>/dev/null
