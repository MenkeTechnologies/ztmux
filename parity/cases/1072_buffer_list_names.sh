$TM set-buffer -b a 1
$TM set-buffer -b b 22
$TM list-buffers -F '#{buffer_name}=#{buffer_size}' -O name
