# -w sends the buffer to the clipboard as well as loading it; with no client
# there is nowhere to send it, so the load still happens and nothing is said.
d=$(mktemp -d)
printf 'clip contents\n' > "$d/in.txt"
$TM load-buffer -w -b clip "$d/in.txt"; echo "rc=$?"
$TM show-buffer -b clip
$TM list-buffers -F '#{buffer_name}:#{buffer_size}'
command rm -rf "$d"
