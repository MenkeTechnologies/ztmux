$TM set-buffer -b d0 a
$TM set-buffer -b d1 b
$TM delete-buffer -b d0
$TM list-buffers -O name -F '#{buffer_name}'
