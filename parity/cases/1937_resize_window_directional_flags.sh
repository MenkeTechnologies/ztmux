# resize-window -L/-R/-D/-U adjust one dimension, by the ADJUSTMENT given as a
# positional argument rather than as the flag's own value (cmd-resize-window.c:56-98).
# Every one of them switches window-size to manual, -A included: -A does not
# restore the automatic size, it resizes to the largest client size and still
# pins window-size to manual (cmd-resize-window.c:99-111).
$TM resize-window -x 80 -y 24; echo "set rc=$?"
echo "start:      $($TM display-message -p '#{window_width}x#{window_height}')"
$TM resize-window -L 10; echo "-L rc=$?"
echo "after -L:   $($TM display-message -p '#{window_width}x#{window_height}')"
$TM resize-window -R 4; echo "-R rc=$?"
echo "after -R:   $($TM display-message -p '#{window_width}x#{window_height}')"
$TM resize-window -U 6; echo "-U rc=$?"
echo "after -U:   $($TM display-message -p '#{window_width}x#{window_height}')"
$TM resize-window -D 2; echo "-D rc=$?"
echo "after -D:   $($TM display-message -p '#{window_width}x#{window_height}')"
echo "== -A takes the largest client size, and window-size stays manual =="
$TM resize-window -A; echo "rc=$?"
echo "window-size is still: $($TM show -wv window-size)"
echo "with no client attached that is the default size: $($TM display-message -p '#{window_width}x#{window_height}') against default-size $($TM show -gv default-size)"
