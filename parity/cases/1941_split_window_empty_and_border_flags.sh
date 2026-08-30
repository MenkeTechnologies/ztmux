# split-window -I and -E make an empty pane, which refuses to be given a command
# (cmd-split-window.c:106-118). -S and -R set the two border styles on the NEW
# pane's own options, and -s sets both window styles there (cmd-split-window.c:173-200);
# the options have to be read back through the pane id, since a bare number in
# -t is a window. -B belongs to new-pane, not to split-window, and names a
# pane-border-lines choice.
$TM set -g status off
$TM split-window -d -E; echo "-E rc=$?"
echo "panes: $($TM list-panes | wc -l | tr -d ' ')"
echo "the empty pane runs nothing: [$($TM display-message -p -t '{last}' '#{pane_start_command}')]"
$TM split-window -d -I; echo "-I rc=$?"
echo "panes: $($TM list-panes | wc -l | tr -d ' ')"
echo "== a command with -E or -I is refused =="
$TM split-window -d -E 'sleep 300' 2>&1; echo "rc=$?"
$TM split-window -d -I 'sleep 300' 2>&1; echo "rc=$?"
echo "panes still: $($TM list-panes | wc -l | tr -d ' ')"
echo "== -S, -R and -s land on the new pane =="
$TM split-window -d -E -S 'fg=red' -R 'fg=blue' -s 'bg=green'; echo "rc=$?"
# The newest pane is the highest pane id, not the tail of list-panes: that list
# is in layout order, and a split of pane 0 puts the new pane next to it.
pane=%$($TM list-panes -F '#{pane_id}' | tr -d '%' | sort -n | tail -1)
first=$($TM list-panes -F '#{pane_id}' | head -1)
echo "active border: [$($TM show -p -t "$pane" -v pane-active-border-style)]"
echo "border:        [$($TM show -p -t "$pane" -v pane-border-style)]"
echo "window style:  [$($TM show -p -t "$pane" -v window-style)] [$($TM show -p -t "$pane" -v window-active-style)]"
echo "the first pane has none of them set: [$($TM show -p -t "$first" -v pane-border-style)][$($TM show -p -t "$first" -v window-style)]"
echo "== -B is new-pane's flag, not split-window's =="
$TM split-window -d -E -B double 2>&1; echo "rc=$?"
$TM new-pane -d -E -B double; echo "new-pane -B rc=$?"
float=%$($TM list-panes -F '#{pane_id}' | tr -d '%' | sort -n | tail -1)
echo "lines on the floating pane: [$($TM show -p -t "$float" -v pane-border-lines)]"
$TM new-pane -d -E -B nosuchlines 2>&1; echo "rc=$?"
