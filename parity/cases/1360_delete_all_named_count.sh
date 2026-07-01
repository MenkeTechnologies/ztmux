$TM set-buffer -b c0 a
$TM set-buffer -b c1 b
$TM delete-buffer -b c0
$TM delete-buffer -b c1
$TM list-buffers -F '#{buffer_name}'
