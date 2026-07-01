$TM set-buffer -b a hello
$TM set-buffer -b a -n z
$TM list-buffers -F '#{buffer_name}'
