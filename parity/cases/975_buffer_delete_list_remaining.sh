$TM set-buffer -b b1 one
$TM set-buffer -b b2 two
$TM set-buffer -b b3 three
$TM delete-buffer -b b2
$TM list-buffers -F '#{buffer_name}' -O name
