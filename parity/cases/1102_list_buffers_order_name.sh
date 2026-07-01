$TM set-buffer -b z9 "a"
$TM set-buffer -b a1 "b"
$TM set-buffer -b m5 "c"
$TM list-buffers -O name -F '#{buffer_name}'
