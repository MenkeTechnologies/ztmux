# Without -p the capture goes into a buffer (named with -b); -e keeps escape
# sequences and -C makes them printable text.
$TM set -g status off
$TM split-window -d 'printf "\033[31mred\033[0m\n"; cat'
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in 1 2 3 4 5 6 7 8 9 10; do
  [ -n "$($TM capture-pane -p -t "$pane" | head -1)" ] && break
  sleep 0.2
done
echo "== into a buffer =="
$TM capture-pane -b cap -t "$pane"; echo "rc=$?"
$TM show-buffer -b cap | head -1
echo "== -C makes escapes printable =="
$TM capture-pane -peC -t "$pane" | head -1
