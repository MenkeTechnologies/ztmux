$TM set-buffer -b b1 a
$TM set-buffer -b b2 b
$TM set-buffer -b b3 c
$TM list-buffers -F '#{buffer_name}' -O creation -r
