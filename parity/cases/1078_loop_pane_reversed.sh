# pane loop reversed by creation (P/r): panes iterate 2,1,0 → indices reversed
$TM split-window -d
$TM split-window -d
$TM display-message -p '#{P/r:#{pane_index} }'
