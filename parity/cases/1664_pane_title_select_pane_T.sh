# select-pane -T sets the pane title, which #{pane_title} reads back. Every pane
# is given a title here: the default is the host name, which is not comparable
# across machines, so no unset title is printed.
$TM select-pane -T 'first title'; echo "rc=$?"
$TM display-message -p 'title=[#{pane_title}]'
$TM split-window -d
$TM select-pane -T 'second title' -t 1
$TM list-panes -F '#{pane_index} [#{pane_title}]'
$TM select-pane -T ''
$TM display-message -p 'cleared=[#{pane_title}]'
echo "== -T on a target that does not exist =="
$TM select-pane -T 'x' -t 99 2>&1; echo "rc=$?"
