$TM set-buffer -b p0 "first"
$TM set-buffer -b p1 "second"
$TM set-buffer -b p2 "third"
$TM list-buffers -F '#{buffer_name}'
