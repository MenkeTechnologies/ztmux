$TM set-buffer -b zzz a
$TM set-buffer -b mmm b
$TM set-buffer -b zzz -n aaa
$TM list-buffers -F '#{buffer_name}' -O name
