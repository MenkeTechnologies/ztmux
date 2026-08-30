# -Z keeps the zoom across the rotation; without it the window comes out
# unzoomed.
$TM split-window -d
$TM split-window -d
$TM resize-pane -Z
$TM display-message -p 'zoomed before=#{window_zoomed_flag}'
$TM rotate-window -Z; echo "-Z rc=$?"
$TM display-message -p 'zoomed after -Z=#{window_zoomed_flag}'
$TM rotate-window -D; echo "-D rc=$?"
$TM display-message -p 'zoomed after -D=#{window_zoomed_flag}'
