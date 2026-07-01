$TM set-buffer -b a 1
$TM set-buffer -b b 2
$TM delete-buffer -b a
$TM list-buffers -F '#{buffer_name}' -O name
