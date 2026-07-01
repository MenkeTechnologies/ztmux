$TM set-buffer -b r original
$TM set-buffer -b r new
$TM list-buffers -F '#{buffer_name}=#{buffer_size}'
