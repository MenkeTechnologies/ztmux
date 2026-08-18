# The DEFAULT window context menu (prefix <) as the client draws it, plus the
# -xP/-yP pane placement that the sibling pane menu (prefix >) uses.
#
# key-bindings.c:445 binds `<` to display-menu -xW -yW with DEFAULT_WINDOW_MENU
# (key-bindings.c:36-47). Every interesting part of that is client-only:
#   * -xW / -yW resolve through popup_window_status_line_x/y, which are computed
#     from the STATUS LINE's window ranges (cmd-display-menu.c:150-160) -- so the
#     menu's column follows the window's position in the status line, and its
#     row is anchored to the status line, not the top of the screen.
#   * "Swap Left"/"Swap Right" are prefixed with "-" (disabled, dim) whenever the
#     session has only one window, and "Swap Marked" whenever no pane is marked;
#     those are FORMATS expanded per item at menu build time (menu.c:88-90).
#   * "Mark" flips to "Unmark" once a pane is marked.
# None of it is reachable without an attached client, so the whole default menu
# was unpinned. Built like cases 1504/1507/1508: an inner server whose client
# lives in a pane of the outer server.
#
# NOTE: the sibling PANE menu (prefix >) is deliberately not captured here --
# ztmux extends DEFAULT_PANE_MENU with its own entries (floating panes, zellij
# stacking, ...), which is an intentional extension rather than a port bug, so
# its rows and width differ from tmux by design. The -xP/-yP placement code the
# pane menu uses is pinned below with an explicit menu instead.
set -- $TM
BIN="$1"
ISOCK="wmr_$$_inner"

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
$BIN -L "$ISOCK" set -g @ztmux-ratatui off

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

# Rows 10..24 with a line number, so the menu's ROW anchor and its COLUMN indent
# are both part of the diff.
box() { $TM capture-pane -p -t client | sed -n '10,24p' | cat -v |
        perl -ne 'printf "%2d|%s", $.+9, $_'; }

wait_for '[alpha] 0:one*'

# --- one window, nothing marked: Swap Left/Right/Marked are all disabled ------
$TM send-keys -t client C-b '<'
wait_for 'New At End'
echo "== window menu: single window, no mark =="
box
echo "== dim (disabled) rows, with SGR =="
$TM capture-pane -p -e -t client | grep -c '\[2mSwap'
$TM capture-pane -p -e -t client | sed -n '12p' | cat -v
$TM send-keys -t client Escape
wait_gone 'New At End'

# --- mark the pane: "Mark" becomes "Unmark", "Swap Marked" becomes enabled ----
$BIN -L "$ISOCK" select-pane -m -t alpha:one
$TM send-keys -t client C-b '<'
wait_for 'New At End'
echo "== window menu: pane marked =="
box
$TM send-keys -t client Escape
wait_gone 'New At End'
$BIN -L "$ISOCK" select-pane -M -t alpha:one

# --- second window: Swap Left/Right enable, and -xW follows the status line ---
$BIN -L "$ISOCK" new-window -d -t alpha: -n twotwotwo 'sleep 300'
$BIN -L "$ISOCK" select-window -t alpha:1
wait_for '1:twotwotwo*'
$TM send-keys -t client C-b '<'
wait_for 'New At End'
echo "== window menu: window 1 of 2, -xW tracks the status range =="
box
$TM send-keys -t client Escape
wait_gone 'New At End'

# --- -xP/-yP: the placement the default PANE menu uses. Pane 0 starts at
# column 0 and its bottom is the status line, so the box must be flush left and
# bottom-anchored; splitting moves the active pane and the box must follow.
$BIN -L "$ISOCK" bind P display-menu -xP -yP -T 'At Pane' Kill X 'kill-pane' Zoom z 'resize-pane -Z'
$TM send-keys -t client C-b P
wait_for 'At Pane'
echo "== -xP -yP, single full-width pane =="
box
$TM send-keys -t client Escape
wait_gone 'At Pane'

$BIN -L "$ISOCK" split-window -h -d -t alpha:1 'sleep 300'
$BIN -L "$ISOCK" select-pane -t alpha:1.1
$TM send-keys -t client C-b P
wait_for 'At Pane'
echo "== -xP -yP, active pane is the right half =="
box
$TM send-keys -t client Escape
wait_gone 'At Pane'

$BIN -L "$ISOCK" kill-server 2>/dev/null
