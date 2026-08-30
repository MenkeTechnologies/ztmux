# The character-level editing sequences: ICH (insert), DCH (delete), ECH (erase)
# and REP (repeat the last character). Written straight into a pane and read
# back with capture-pane, which is the only way to see what the parser did.
$TM set -g status off
$TM split-window -d 'cat > /dev/null'
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
w() { $TM send-keys -t "$pane" -H $(printf '%s' "$1" | perl -ne 'print join(" ", map { sprintf "%02x", ord } split //)'); }
settle() { for _ in $(seq 1 30); do [ -n "$($TM capture-pane -p -t "$pane" | head -1)" ] && return; sleep 0.1; done; }

w "$(printf 'ABCDEF')"; settle
echo "start:            [$($TM capture-pane -p -t "$pane" | head -1)]"
w "$(printf '\033[1;3H\033[2@')"          # cursor to column 3, insert 2 blanks
echo "after ICH(2):     [$($TM capture-pane -p -t "$pane" | head -1)]"
w "$(printf '\033[1;3H\033[2P')"          # delete 2 characters
echo "after DCH(2):     [$($TM capture-pane -p -t "$pane" | head -1)]"
w "$(printf '\033[1;2H\033[3X')"          # erase 3 characters
echo "after ECH(3):     [$($TM capture-pane -p -t "$pane" | head -1)]"
w "$(printf '\033[2;1Hx\033[4b')"         # x then repeat it 4 times
echo "after REP(4):     [$($TM capture-pane -p -t "$pane" | sed -n '2p')]"
