$TM set-buffer -b b1 aaa
$TM set-buffer -b b2 bb
$TM set-buffer -b b3 c
$TM set-buffer -b b2 -n b9
$TM delete-buffer -b b1
$TM list-buffers -F '#{buffer_name}=#{buffer_size}' -O name
