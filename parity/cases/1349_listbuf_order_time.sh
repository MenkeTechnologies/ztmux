$TM set-buffer -b t0 "x"
$TM set-buffer -b t1 "y"
$TM set-buffer -b t2 "z"
$TM list-buffers -O time -F '#{buffer_name}'
