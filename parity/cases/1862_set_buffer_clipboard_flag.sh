# set-buffer -w sends the buffer to the clipboard as well as storing it; with no
# client there is nowhere to send it, so the buffer is still set and nothing is
# said.
$TM set-buffer -w -b clip 'clipboard contents'; echo "rc=$?"
$TM show-buffer -b clip
$TM list-buffers -F '#{buffer_name}:#{buffer_size}' | grep clip
echo "== -w with -a appends and still sets =="
$TM set-buffer -w -a -b clip ' more'; echo "rc=$?"
$TM show-buffer -b clip
echo "== set-clipboard governs whether it is sent at all =="
$TM show -sv set-clipboard
$TM set -s set-clipboard off
$TM set-buffer -w -b clip2 'quiet'; echo "rc=$?"
$TM show-buffer -b clip2
$TM set -su set-clipboard
