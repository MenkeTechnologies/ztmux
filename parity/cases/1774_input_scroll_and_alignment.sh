# SU/SD scroll the region, DECALN fills the screen with E, and DECSC/DECRC save
# and restore the cursor. All are server-side grid changes, so capture-pane and
# the cursor formats show them.
$TM set -g status off
$TM split-window -d 'cat > /dev/null'
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
w() { $TM send-keys -t "$pane" -H $(printf '%s' "$1" | perl -ne 'print join(" ", map { sprintf "%02x", ord } split //)'); }
settle() { for _ in $(seq 1 30); do [ -n "$($TM capture-pane -p -t "$pane" | head -1)" ] && return; sleep 0.1; done; }

w "$(printf 'one\r\ntwo\r\nthree\r\n')"; settle
echo "start:"; $TM capture-pane -p -t "$pane" | head -3 | sed 's/^/  /'
w "$(printf '\033[2S')"
echo "after SU(2):"; $TM capture-pane -p -t "$pane" | head -3 | sed 's/^/  /'
w "$(printf '\033[1T')"
echo "after SD(1):"; $TM capture-pane -p -t "$pane" | head -3 | sed 's/^/  /'
echo "== DECSC / DECRC =="
w "$(printf '\033[5;7H\0337\033[1;1H')"
echo "cursor after moving home: $($TM display-message -p -t "$pane" '#{cursor_x},#{cursor_y}')"
w "$(printf '\0338')"
echo "cursor after DECRC:       $($TM display-message -p -t "$pane" '#{cursor_x},#{cursor_y}')"
echo "== DECALN =="
w "$(printf '\033#8')"
$TM capture-pane -p -t "$pane" | head -2 | perl -pe 's/^(E+)$/EEE...(length @{[length $1]})/' | sed 's/^/  /'
