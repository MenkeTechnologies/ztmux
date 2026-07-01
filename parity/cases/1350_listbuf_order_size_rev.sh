$TM set-buffer -b s0 aaaa
$TM set-buffer -b s1 bb
$TM set-buffer -b s2 cccccc
$TM list-buffers -O size -r -F '#{buffer_name}:#{buffer_size}'
