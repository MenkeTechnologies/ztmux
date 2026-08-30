# load-buffer - reads the buffer from standard input, which is how a shell
# pipeline gets data into tmux without a temporary file.
printf 'from stdin\nsecond line\n' | $TM load-buffer -b piped -; echo "rc=$?"
$TM show-buffer -b piped
$TM list-buffers -F '#{buffer_name}:#{buffer_size}' | grep piped
echo "== an empty stdin =="
printf '' | $TM load-buffer -b empty -; echo "rc=$?"
$TM list-buffers -F '#{buffer_name}' | grep -c empty
echo "== save-buffer - writes to stdout =="
$TM save-buffer -b piped -
