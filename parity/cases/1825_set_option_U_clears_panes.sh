# set -U removes a window option from the window AND from every pane in it
# (cmd-set-option.c:168), where -u only removes it from the window and leaves a
# pane's own value standing.
$TM new-window -d -n scoped
$TM split-window -d -t scoped
$TM setw -t scoped @w window-value
$TM set -p -t scoped.0 @w pane-zero
$TM set -p -t scoped.1 @w pane-one
$TM list-panes -t scoped -F '  pane #{pane_index}: #{@w}' | sort
echo "== -u leaves the pane values =="
$TM setw -u -t scoped @w
$TM list-panes -t scoped -F '  pane #{pane_index}: [#{@w}]' | sort
echo "== -U clears them too =="
$TM setw -t scoped @w window-value
$TM set -p -t scoped.0 @w pane-zero
$TM set -p -t scoped.1 @w pane-one
$TM setw -U -t scoped @w
$TM list-panes -t scoped -F '  pane #{pane_index}: [#{@w}]' | sort
