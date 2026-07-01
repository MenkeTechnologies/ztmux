$TM set-buffer -b e ''
$TM list-buffers -F '#{buffer_name}' | wc -l | tr -d ' '
