# pane_zoomed_flag reflects PANE_ZOOMED across a zoom toggle.
$TM split-window "sleep 60"
$TM resize-pane -Z
$TM display-message -p 'z=#{pane_zoomed_flag}'
$TM resize-pane -Z
$TM display-message -p 'u=#{pane_zoomed_flag}'
