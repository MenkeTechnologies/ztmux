# Rectangle mode changes how the selection's end coordinates are derived: the
# columns come from the cursor's own column pair rather than from line ends, and
# toggling it mid-selection has to recompute both ends. rectangle-on/off are
# separate entries in the command table from rectangle-toggle and can drift
# apart in a port that implements only the toggle.
$TM new-window -d -n rect 'printf "0123456789\nabcdefghij\nABCDEFGHIJ\n"; sleep 300'
sleep 1
$TM copy-mode -t rect
$TM send-keys -X -t rect history-top
$TM send-keys -X -t rect start-of-line
show() {
  $TM display-message -p -t rect \
    "$1 rect=#{rectangle_toggle} mode=#{selection_mode} s=#{selection_start_y},#{selection_start_x} e=#{selection_end_y},#{selection_end_x} cur=#{copy_cursor_y},#{copy_cursor_x}"
}
$TM send-keys -X -t rect begin-selection
$TM send-keys -X -t rect cursor-right; $TM send-keys -X -t rect cursor-right
$TM send-keys -X -t rect cursor-down; show linewise
$TM send-keys -X -t rect rectangle-toggle; show toggled-on
$TM send-keys -X -t rect cursor-right; show extended
$TM send-keys -X -t rect rectangle-off; show off
$TM send-keys -X -t rect rectangle-on; show on
$TM send-keys -X -t rect rectangle-toggle; show toggled-off
# The rectangle's contents are what actually got selected.
$TM send-keys -X -t rect rectangle-on
$TM send-keys -X -t rect copy-selection-no-clear
$TM show-buffer | perl -pe "s{^(.*)\$}{[\$1]}" | head -5
