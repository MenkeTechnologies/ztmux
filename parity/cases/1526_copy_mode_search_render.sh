# Copy-mode SEARCH HIGHLIGHTING as a client paints it.
#
# window_copy_update_style (window-copy.c:4940) paints every hit of the current
# search with copy-mode-match-style, the one hit under the cursor with
# copy-mode-current-match-style, and the marked line with copy-mode-mark-style.
# None of that exists without a client: with the server alone the search still
# runs and #{search_count} still expands, so a port that found the matches but
# painted none of them -- or painted them all identically, or never moved the
# "current" one as search-again walks the hits -- passes every server-side case.
# The match COUNT reaches the user the same way, through
# copy-mode-position-format's "#{?search_count, (#{search_count} results),}"
# tail, drawn by format_draw on row 0 and by nothing else.
#
# The three styles are deliberately distinct so the capture separates "a match"
# from "the current match" from "the mark", rather than proving only that
# something got highlighted.
#
# n and N go through the client, so the copy-mode-vi table's search-again and
# search-reverse (key-bindings.c:586,630) are pinned end to end as well.
#
# Built like cases 1504/1507/1508: an inner server with a client attached inside
# a pane of the outer server.
set -- $TM
BIN="$1"
ISOCK="csh_$$_inner"

scrub() { perl -pe 's{/dev/tty[a-z0-9]+}{/dev/ttyDEV}g; s/\d\d:\d\d:\d\d/HH:MM:SS/g; s/\d\d:\d\d/HH:MM/g; s/[ \t]+$//'; }

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the
# PORTED rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
$BIN -L "$ISOCK" setw -g mode-keys vi
$BIN -L "$ISOCK" setw -g mode-style 'bg=cyan,fg=black'
$BIN -L "$ISOCK" setw -g copy-mode-match-style 'bg=green,fg=black'
$BIN -L "$ISOCK" setw -g copy-mode-current-match-style 'bg=red,fg=white'
$BIN -L "$ISOCK" setw -g copy-mode-mark-style 'bg=yellow,fg=black'
$BIN -L "$ISOCK" setw -g copy-mode-position-style 'bg=blue,fg=white'

inner() { $BIN -L "$ISOCK" display-message -p -t "$1" "$2" 2>/dev/null; }
wait_for() {
  local i=0 got
  while [ $i -lt 60 ]; do
    got=$(inner "$1" "$2")
    [ "$got" = "$3" ] && return 0
    i=$((i+1)); sleep 0.05
  done
  echo "wait_for: [$2] on [$1] wanted [$3] got [$got]"
}
settle() {
  local a='' b i=0
  while [ $i -lt 30 ]; do
    b=$($TM capture-pane -p -t client)
    [ -n "$b" ] && [ "$a" = "$b" ] && return 0
    a="$b"; i=$((i+1)); sleep 0.1
  done
}
screen() { $TM capture-pane -p -e -t client | sed -n '1,4p' | cat -v | scrub; }
state() { echo "state: $(inner alpha:data 'cur=#{copy_cursor_y},#{copy_cursor_x} count=[#{search_count}] partial=[#{search_count_partial}] present=#{search_present} timedout=#{search_timed_out}')"; }
key() { $TM send-keys -t client "$@"; }

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"
wait_for alpha '#{session_attached}' 1
wait_for alpha:one '#{pane_height}' 23
settle

# Four lower-case hits plus one upper-case FOO: the count and the highlighting
# together show that a lower-case pattern matches case-insensitively, which is
# a search-engine property no styling change can fake.
$BIN -L "$ISOCK" new-window -n data 'printf "foo bar foo\nbaz foo qux\nFOO upper\nend foo\n"; sleep 300'
wait_for alpha:data '#{pane_at_bottom}' 1
settle

key C-b; key '['
wait_for alpha:data '#{pane_mode}' copy-mode
key g; wait_for alpha:data '#{copy_cursor_y}' 0
key 0; wait_for alpha:data '#{copy_cursor_x}' 0
settle
echo "before search:"; screen
state

# From (0,0) the search moves to the NEXT hit, so the current match is the
# second "foo" on row 0 and the first one stays an ordinary match.
$BIN -L "$ISOCK" send-keys -X -t alpha:data search-forward foo
wait_for alpha:data '#{copy_cursor_y},#{copy_cursor_x}' '0,8'
wait_for alpha:data '#{search_count}' 5
settle
echo "search-forward foo:"; screen
state

# search-again (vi n) walks the current match forward; only the red cells move.
key n
wait_for alpha:data '#{copy_cursor_y},#{copy_cursor_x}' '1,4'
settle
echo "after n:"; screen

key n
wait_for alpha:data '#{copy_cursor_y},#{copy_cursor_x}' '2,0'
settle
echo "after n again (upper-case hit):"; screen

# search-reverse (vi N) walks it back.
key N
wait_for alpha:data '#{copy_cursor_y},#{copy_cursor_x}' '1,4'
settle
echo "after N:"; screen

# search-backward keeps every hit highlighted and picks the previous one.
$BIN -L "$ISOCK" send-keys -X -t alpha:data search-backward foo
wait_for alpha:data '#{copy_cursor_y},#{copy_cursor_x}' '0,8'
settle
echo "search-backward foo:"; screen
state

# set-mark (vi X) drops the search and paints the marked line, cursor cell
# inverted out of it.
key X
wait_for alpha:data '#{search_present}' 0
settle
echo "set-mark on row 0:"; screen
state

$BIN -L "$ISOCK" kill-server 2>/dev/null
