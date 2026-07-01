$TM set-buffer -b mid bb
$TM set-buffer -b big ccc
$TM set-buffer -b small a
$TM list-buffers -F '#{buffer_size} #{buffer_name}' -O size
