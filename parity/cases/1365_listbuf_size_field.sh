$TM set-buffer -b w1 aaa
$TM set-buffer -b w2 bbbbb
$TM list-buffers -O name -F '#{buffer_name}:#{buffer_size}'
