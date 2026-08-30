# -J joins wrapped lines and preserves trailing spaces; -N keeps trailing spaces
# without joining (cmd-capture-pane.c:45). Write a line longer than the 80-column
# pane so there is a wrap to join.
$TM set -g status off
$TM split-window -d 'cat'
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
long=$(printf 'x%.0s' $(seq 1 100))
$TM send-keys -t "$pane" -l "$long"
for _ in 1 2 3 4 5 6 7 8 9 10; do
  [ -n "$($TM capture-pane -p -t "$pane" | head -1)" ] && break
  sleep 0.2
done
echo "== plain: wrapped into two lines =="
$TM capture-pane -p -t "$pane" | perl -ne 'print length($_)-1, "\n" if $. <= 2'
echo "== -J: joined =="
$TM capture-pane -pJ -t "$pane" | perl -ne 'print length($_)-1, "\n" if $. <= 2'
echo "== -N: trailing spaces kept =="
$TM capture-pane -pN -t "$pane" | perl -ne 'print length($_)-1, "\n" if $. == 1'
