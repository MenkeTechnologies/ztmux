$TM set-buffer -b b3 c
$TM set-buffer -b b1 a
$TM set-buffer -b b2 b
$TM list-buffers -F '#{buffer_name}' -O name
