$TM set-buffer x
$TM set-buffer y
$TM set-buffer z
$TM list-buffers -F '#{buffer_name}' | wc -l | tr -d ' '
