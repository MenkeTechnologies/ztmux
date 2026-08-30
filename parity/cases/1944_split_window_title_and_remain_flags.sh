# The rest of split-window's per-pane flags, which land on the NEW pane after
# spawn (cmd-split-window.c:172-217): -T sets the pane title through a format,
# -k sets remain-on-exit to 3 (the "until it exits once" choice) and -m sets both
# that and remain-on-exit-format.
$TM set -g status off
$TM split-window -d -T 'title-#{session_name}' 'sleep 300'; echo "-T rc=$?"
pane=%$($TM list-panes -F '#{pane_id}' | tr -d '%' | sort -n | tail -1)
echo "title: [$($TM display-message -p -t "$pane" '#{pane_title}')]"
$TM split-window -d -k 'sleep 300'; echo "-k rc=$?"
pane=%$($TM list-panes -F '#{pane_id}' | tr -d '%' | sort -n | tail -1)
echo "remain-on-exit: [$($TM show -p -t "$pane" -v remain-on-exit)]"
echo "format is untouched: [$($TM show -p -t "$pane" -v remain-on-exit-format 2>&1)]"
$TM split-window -d -m 'pane #{pane_index} is done' 'sleep 300'; echo "-m rc=$?"
pane=%$($TM list-panes -F '#{pane_id}' | tr -d '%' | sort -n | tail -1)
echo "remain-on-exit: [$($TM show -p -t "$pane" -v remain-on-exit)]"
echo "format: [$($TM show -p -t "$pane" -v remain-on-exit-format)]"
echo "== a bad style is refused and the pane is not left behind =="
before=$($TM list-panes | wc -l | tr -d ' ')
$TM split-window -d -s 'not a style' 'sleep 300' 2>&1; echo "rc=$?"
echo "panes: $before -> $($TM list-panes | wc -l | tr -d ' ')"
