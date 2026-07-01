$TM set-option -g buffer-limit 3
$TM set-buffer -b e1 a
$TM set-buffer -b e2 b
$TM set-buffer -b e3 c
$TM set-buffer -b e4 d
$TM set-buffer -b e5 e
$TM list-buffers -O name -F '#{buffer_name}'
