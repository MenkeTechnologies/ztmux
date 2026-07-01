# the raw layout produced by a single default split (strip layout checksum)
$TM split-window -d
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
