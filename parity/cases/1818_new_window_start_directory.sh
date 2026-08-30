# -c sets the working directory a window or pane starts in, and it is a format,
# so it can be computed. The pane prints its own directory, which is compared
# after the temporary path is masked.
$TM set -g status off
d=$(mktemp -d)
mask() { perl -pe "s{\Q$d\E}{DIR}g; s{^/private}{}"; }
$TM new-window -d -n started -c "$d" 'pwd; sleep 300'
for _ in $(seq 1 25); do
  [ -n "$($TM capture-pane -p -t started | head -1)" ] && break
  sleep 0.2
done
$TM capture-pane -p -t started | head -1 | mask
$TM display-message -p -t started 'start_path=[#{pane_start_path}]' | mask
echo "== -c takes a format =="
$TM new-window -d -n computed -c '#{pane_start_path}' 'pwd; sleep 300'
for _ in $(seq 1 25); do
  [ -n "$($TM capture-pane -p -t computed | head -1)" ] && break
  sleep 0.2
done
$TM capture-pane -p -t computed | head -1 | mask
echo "== a directory that does not exist =="
$TM new-window -d -n nodir -c /nonexistent-dir-ztpar 'sleep 300' 2>&1 | perl -pe 's{/nonexistent-dir-ztpar}{DIR}'; echo "rc=${PIPESTATUS[0]}"
command rm -rf "$d"
