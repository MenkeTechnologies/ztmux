$TM set-buffer -b b1 one
$TM set-buffer -b b2 two
$TM delete-buffer -b b1
$TM list-buffers -F '#{buffer_name}' -O name
