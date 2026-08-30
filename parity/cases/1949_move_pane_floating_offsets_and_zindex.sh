# move-pane's offset flags move a floating pane: -X/-Y set the offset outright
# (plus one for the border), and -U/-L/-R adjust it by their argument, which
# defaults to 1 (cmd-join-pane.c:228-251). The C's loop there also reads 'D',
# but move-pane's args string has no D (cmd-join-pane.c:55), so that arm is
# unreachable and -D is rejected by the parser -- pinned below rather than
# assumed. -z sets the stacking index and the z-order names reorder the floating
# panes among themselves; both are read back through #{pane_z}.
$TM set -g status off
$TM new-pane -d -E -x 20 -y 6 -X 10 -Y 4
pane=%$($TM list-panes -F '#{pane_id}' | tr -d '%' | sort -n | tail -1)
at() { $TM display-message -p -t "$pane" '#{pane_left},#{pane_top}'; }
echo "start:   $(at)"
$TM move-pane -t "$pane" -X 30; echo "-X 30 rc=$? -> $(at)"
$TM move-pane -t "$pane" -Y 8;  echo "-Y 8  rc=$? -> $(at)"
$TM move-pane -t "$pane" -L 5;  echo "-L 5  rc=$? -> $(at)"
$TM move-pane -t "$pane" -R 2;  echo "-R 2  rc=$? -> $(at)"
$TM move-pane -t "$pane" -U 3;  echo "-U 3  rc=$? -> $(at)"
$TM move-pane -t "$pane" -D 2>&1; echo "-D    rc=$? -> $(at)"
echo "== a non-numeric adjustment is an error =="
$TM move-pane -t "$pane" -R nope 2>&1; echo "rc=$?"
echo "still at $(at)"
echo "== z-order: three floating panes, named front to back =="
$TM new-pane -d -E -x 10 -y 4
second=%$($TM list-panes -F '#{pane_id}' | tr -d '%' | sort -n | tail -1)
$TM new-pane -d -E -x 10 -y 4
third=%$($TM list-panes -F '#{pane_id}' | tr -d '%' | sort -n | tail -1)
# list-panes is in layout order, so the stacking has to be read per pane.
order() { $TM list-panes -F '#{pane_id}:#{pane_z}' | tr '\n' ' '; }
echo "list order: $(order)"
for p in front back forward backward forward-loop backward-loop; do
  $TM move-pane -t "$second" -P "$p"; printf '%-14s rc=%s order=%s\n' "$p" "$?" "$(order)"
done
echo "== -z takes an index =="
$TM move-pane -t "$third" -z 0; echo "rc=$? order=$(order)"
$TM move-pane -t "$third" -z 99 2>&1; echo "rc=$?"
$TM move-pane -t "$third" -z notanumber 2>&1; echo "rc=$?"
