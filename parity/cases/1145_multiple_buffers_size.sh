$TM set-buffer -b a aaaa
$TM set-buffer -b b bb
$TM list-buffers -F '#{buffer_name}:#{buffer_size}' -O size
