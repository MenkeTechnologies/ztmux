# select-layout accepts a layout string with its checksum, and refuses one whose
# checksum or geometry does not parse.
$TM split-window -d
$TM split-window -d
layout=$($TM display-message -p '#{window_layout}')
echo "round trip: $($TM select-layout "$layout" >/dev/null 2>&1; echo rc=$?)"
echo "same layout back: $([ "$($TM display-message -p '#{window_layout}')" = "$layout" ] && echo yes || echo no)"
bad_sum="0000,${layout#*,}"
$TM select-layout "$bad_sum" 2>&1; echo "bad checksum rc=$?"
$TM select-layout 'b25d,not-a-layout' 2>&1; echo "bad geometry rc=$?"
$TM select-layout '' 2>&1; echo "empty rc=$?"
echo "layout unchanged: $([ "$($TM display-message -p '#{window_layout}')" = "$layout" ] && echo yes || echo no)"
