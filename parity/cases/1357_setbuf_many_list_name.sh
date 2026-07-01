$TM set-buffer -b k1 a
$TM set-buffer -b k2 b
$TM set-buffer -b k3 c
$TM set-buffer -b k4 d
$TM set-buffer -b k5 e
$TM list-buffers -O name -F '#{buffer_name}'
