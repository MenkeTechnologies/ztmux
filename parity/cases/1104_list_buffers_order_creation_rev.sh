$TM set-buffer -b p0 "a"
$TM set-buffer -b p1 "b"
$TM set-buffer -b p2 "c"
$TM list-buffers -O creation -r -F '#{buffer_name}'
