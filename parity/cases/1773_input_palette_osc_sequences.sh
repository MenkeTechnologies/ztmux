# OSC 4 sets a palette entry for the pane and OSC 104 resets it; the effect is
# visible through capture-pane -e, which re-serialises what the pane holds.
$TM set -g status off
$TM split-window -d 'cat > /dev/null'
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
w() { $TM send-keys -t "$pane" -H $(printf '%s' "$1" | perl -ne 'print join(" ", map { sprintf "%02x", ord } split //)'); }
settle() { for _ in $(seq 1 30); do [ -n "$($TM capture-pane -p -t "$pane" | head -1)" ] && return; sleep 0.1; done; }

w "$(printf '\033[31mred text\033[0m\r\n')"; settle
echo "coloured text, -e:"
$TM capture-pane -p -e -t "$pane" | head -1 | cat -v | sed 's/^/  /'
w "$(printf '\033]4;1;rgb:00/ff/00\033\\')"
w "$(printf '\033[31msame code, new palette\033[0m\r\n')"
echo "after OSC 4 on colour 1:"
$TM capture-pane -p -e -t "$pane" | sed -n '2p' | cat -v | sed 's/^/  /'
w "$(printf '\033]104\033\\')"
w "$(printf '\033[31mafter reset\033[0m\r\n')"
echo "after OSC 104:"
$TM capture-pane -p -e -t "$pane" | sed -n '3p' | cat -v | sed 's/^/  /'
