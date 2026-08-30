# list-buffers -f filters with a format, -O orders by name/time/size and -r
# reverses that order (cmd-list-buffers.c).
$TM set-buffer -b alpha aaaa
$TM set-buffer -b beta bb
$TM set-buffer -b gamma cccccc
echo "== -F alone =="
$TM list-buffers -F '#{buffer_name} #{buffer_size}' -O name
echo "== -f keeps the buffers whose size is over two =="
$TM list-buffers -f '#{>:#{buffer_size},2}' -F '#{buffer_name}' -O name
echo "== -O size =="
$TM list-buffers -O size -F '#{buffer_name}'
echo "== -O size -r =="
$TM list-buffers -O size -r -F '#{buffer_name}'
echo "== -O name -r =="
$TM list-buffers -O name -r -F '#{buffer_name}'
echo "== a filter that matches nothing prints nothing =="
echo "[$($TM list-buffers -f '#{==:#{buffer_name},nope}' -F '#{buffer_name}')]"
