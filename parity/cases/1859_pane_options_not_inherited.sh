# A pane option belongs to that pane: a new pane split from it does not inherit
# the value, it inherits the WINDOW's, which is what the option scope chain says
# but nothing had checked at creation time.
$TM new-window -d -n scoped
$TM setw -t scoped @v window-value
$TM set -p -t scoped.0 @v pane-zero-value
$TM list-panes -t scoped -F '  pane #{pane_index}: #{@v}'
$TM split-window -d -t scoped.0
echo "after splitting the pane that had its own value:"
$TM list-panes -t scoped -F '  pane #{pane_index}: #{@v}' | sort
echo "== and a new window takes the global one =="
$TM set -g @v global-value
$TM new-window -d -n fresh
$TM list-panes -t fresh -F '  pane #{pane_index}: #{@v}'
$TM set -gu @v
