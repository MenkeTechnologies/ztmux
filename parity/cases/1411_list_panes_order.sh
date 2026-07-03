# list-panes -O sort / -r reverse (sorts panes within the window).
$TM split-window "sleep 300"
$TM split-window "sleep 300"
$TM list-panes -O size -F '#{pane_index}'
$TM list-panes -O activity -r -F '#{pane_index}'
$TM list-panes -O bogus
