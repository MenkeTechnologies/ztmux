# set-buffer -a appends to a buffer; naming one that does not exist creates it
# rather than failing, and -n renames while keeping the contents.
$TM set-buffer -a -b fresh 'first '
$TM show-buffer -b fresh
$TM set-buffer -a -b fresh 'second'
$TM show-buffer -b fresh
$TM set-buffer -n renamed -b fresh 2>&1; echo "rename rc=$?"
$TM show-buffer -b renamed
$TM list-buffers -F '#{buffer_name}' | sort | tr '\n' ' '; echo
echo "== appending to a buffer that was just renamed away =="
$TM set-buffer -a -b fresh 'again'
$TM show-buffer -b fresh
