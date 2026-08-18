# Border lines and styles for popups and menus, as the client draws them.
#
# screen_write_box picks its glyph set from `enum box_lines`, and both
# display-popup and display-menu resolve that from either a flag or an option
# (cmd-display-menu.c:340 for the menu, :471 for the popup). Seven glyph sets
# (single/double/heavy/simple/rounded/padded/none) plus -B, plus flag-beats-option
# precedence -- and none of it is observable without an attached client, so a
# wrong glyph table, a missing "padded" variant, or a border-lines option that is
# parsed but never consulted would all pass the existing suite silently.
#
# The style half is the same story: popup-style/popup-border-style and
# menu-style/menu-selected-style/menu-border-style (and their -s/-S/-H flag
# overrides) only ever reach a client's tty, and menu_reapply_styles (menu.c:230)
# re-resolves them on every draw.
#
# NOTE: display-menu's -b FLAG is deliberately not exercised here. ztmux parses
# and validates it but discards the resolved value, so every menu renders with
# the "single" glyphs no matter what -b says; that is a real divergence, reported
# separately rather than baked into a case. The menu-border-lines OPTION path
# does match and is pinned below, so the menu glyph tables themselves are covered.
#
# Built like cases 1504/1507/1508: an inner server whose attached client lives in
# a pane of the outer server.
set -- $TM
BIN="$1"
ISOCK="pbl_$$_inner"

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

# Trailing blanks trimmed (but not the newline) so the diff is about glyphs.
rows() { $TM capture-pane -p -t client | sed -n "$1" | cat -v | perl -pe 's/[ \t]+$//'; }

wait_for '[alpha] 0:one*'

# --- popup border lines, via the -b flag -------------------------------------
# The popup is 14x5 at -x 0 -y 13, so it occupies rows 9..13. `stty size` inside
# it also proves the interior shrinks (or does not) with the border style.
for b in single double heavy simple rounded padded none; do
  $BIN -L "$ISOCK" bind p display-popup -b "$b" -w 14 -h 5 -x 0 -y 13 -T Ttl \
    'stty size; printf "XY\r\n"; sleep 300'
  $TM send-keys -t client C-b p
  wait_for XY
  echo "== popup -b $b =="
  rows '9,13p'
  $BIN -L "$ISOCK" display-popup -C -t alpha:one
  wait_gone XY
done

# -B is a hard "no border" that beats -b.
$BIN -L "$ISOCK" bind p display-popup -B -b double -w 14 -h 5 -x 0 -y 13 -T Ttl \
  'stty size; printf "XY\r\n"; sleep 300'
$TM send-keys -t client C-b p
wait_for XY
echo "== popup -B (with -b double, which -B overrides) =="
rows '9,13p'
$BIN -L "$ISOCK" display-popup -C -t alpha:one
wait_gone XY

# The popup-border-lines OPTION with no -b flag at all, then -b beating it.
$BIN -L "$ISOCK" set -g popup-border-lines heavy
$BIN -L "$ISOCK" bind p display-popup -w 14 -h 5 -x 0 -y 13 -T Ttl \
  'printf "XY\r\n"; sleep 300'
$TM send-keys -t client C-b p
wait_for XY
echo "== popup, popup-border-lines heavy, no -b =="
rows '9,13p'
$BIN -L "$ISOCK" display-popup -C -t alpha:one
wait_gone XY

$BIN -L "$ISOCK" bind p display-popup -b rounded -w 14 -h 5 -x 0 -y 13 -T Ttl \
  'printf "XY\r\n"; sleep 300'
$TM send-keys -t client C-b p
wait_for XY
echo "== popup -b rounded overrides popup-border-lines heavy =="
rows '9,13p'
$BIN -L "$ISOCK" display-popup -C -t alpha:one
wait_gone XY
$BIN -L "$ISOCK" set -gu popup-border-lines

# --- popup styles ------------------------------------------------------------
$BIN -L "$ISOCK" bind p display-popup -w 14 -h 5 -x 0 -y 13 -T Ttl \
  -s 'fg=colour196,bg=colour17' -S 'fg=colour46,bg=colour18' \
  'printf "XY\r\n"; sleep 300'
$TM send-keys -t client C-b p
wait_for XY
echo "== popup -s / -S with SGR =="
$TM capture-pane -p -e -t client | sed -n '9,10p' | cat -v | perl -pe 's/[ \t]+$//'
$BIN -L "$ISOCK" display-popup -C -t alpha:one
wait_gone XY

$BIN -L "$ISOCK" set -g popup-style 'fg=colour226,bg=colour53'
$BIN -L "$ISOCK" set -g popup-border-style 'fg=colour51,bg=colour22'
$BIN -L "$ISOCK" bind p display-popup -w 14 -h 5 -x 0 -y 13 -T Ttl \
  'printf "XY\r\n"; sleep 300'
$TM send-keys -t client C-b p
wait_for XY
echo "== popup-style / popup-border-style options with SGR =="
$TM capture-pane -p -e -t client | sed -n '9,10p' | cat -v | perl -pe 's/[ \t]+$//'
$BIN -L "$ISOCK" display-popup -C -t alpha:one
wait_gone XY
$BIN -L "$ISOCK" set -gu popup-style
$BIN -L "$ISOCK" set -gu popup-border-style

# --- menu border lines, via the menu-border-lines option ---------------------
# Two items plus a separator is a 5-row box; -y 12 puts it on rows 8..12.
$BIN -L "$ISOCK" bind m display-menu -x 0 -y 12 -T Mnu \
  One o 'list-windows' '' Two t 'list-panes'
for b in single double heavy simple rounded padded none; do
  $BIN -L "$ISOCK" set -g menu-border-lines "$b"
  $TM send-keys -t client C-b m
  wait_for Mnu
  echo "== menu-border-lines $b =="
  rows '8,12p'
  $TM send-keys -t client Escape
  wait_gone Mnu
done
$BIN -L "$ISOCK" set -gu menu-border-lines

# --- menu styles -------------------------------------------------------------
$BIN -L "$ISOCK" bind m display-menu -x 0 -y 12 -T Mnu \
  -s 'fg=colour196,bg=colour17' -S 'fg=colour46,bg=colour18' -H 'fg=colour15,bg=colour88' \
  One o 'list-windows' '' Two t 'list-panes'
$TM send-keys -t client C-b m
wait_for Mnu
echo "== menu -s / -S / -H with SGR =="
$TM capture-pane -p -e -t client | sed -n '8,12p' | cat -v | perl -pe 's/[ \t]+$//'
$TM send-keys -t client Escape
wait_gone Mnu

$BIN -L "$ISOCK" set -g menu-style 'fg=colour226,bg=colour53'
$BIN -L "$ISOCK" set -g menu-border-style 'fg=colour51,bg=colour22'
$BIN -L "$ISOCK" set -g menu-selected-style 'fg=colour16,bg=colour208'
$BIN -L "$ISOCK" bind m display-menu -x 0 -y 12 -T Mnu \
  One o 'list-windows' '' Two t 'list-panes'
$TM send-keys -t client C-b m
wait_for Mnu
echo "== menu-style / menu-border-style / menu-selected-style options with SGR =="
$TM capture-pane -p -e -t client | sed -n '8,12p' | cat -v | perl -pe 's/[ \t]+$//'
$TM send-keys -t client Escape
wait_gone Mnu

$BIN -L "$ISOCK" kill-server 2>/dev/null
