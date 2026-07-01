$TM set-buffer -b g one
$TM set-buffer -a -b g two
$TM list-buffers -F '#{buffer_name}=#{buffer_size}'
