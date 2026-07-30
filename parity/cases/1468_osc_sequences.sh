# The OSC handlers write server-side state a client never has to be attached to
# see: OSC 0/2 set the pane title, OSC 52 writes a paste buffer, OSC 4/104
# change and reset the pane palette, and OSC 10/11/12 set fg/bg/cursor colour
# which #{pane_fg}/#{pane_bg} report back. Each one parses a different payload
# shape, and a malformed one must be dropped rather than corrupting state.
$TM new-window -d -n osc 'printf "\0033]2;window-title\0033\0134"; printf "\0033]0;icon-and-title\0033\0134"; printf "\0033]52;c;aGVsbG8td29ybGQ=\0033\0134"; printf "\0033]10;#ff0000\0033\0134\0033]11;#0000ff\0033\0134"; printf "\0033]4;1;#00ff00\0033\0134"; sleep 300'
sleep 1
$TM display-message -p -t osc 'title=[#{pane_title}] fg=#{pane_fg} bg=#{pane_bg}'
$TM show-buffer 2>&1 | perl -pe "s{^(.*)\$}{[\$1]}"
$TM list-buffers -F '#{buffer_name} #{buffer_size}'
# Reset the palette and the colours, then read them back.
$TM new-window -d -n osc2 'printf "\0033]4;1;#00ff00\0033\0134\0033]104\0033\0134"; printf "\0033]10;#ff0000\0033\0134\0033]110\0033\0134"; printf "\0033]2;t2\0033\0134"; sleep 300'
sleep 1
$TM display-message -p -t osc2 'title=[#{pane_title}] fg=#{pane_fg} bg=#{pane_bg}'
# Malformed payloads: no terminator argument, an unknown OSC number, a bad
# colour and a bad base64 body. None may change the title that follows them.
$TM new-window -d -n osc3 'printf "\0033]52;c;!!!notbase64!!!\0033\0134"; printf "\0033]4;999;#123456\0033\0134"; printf "\0033]10;notacolour\0033\0134"; printf "\0033]9999;x\0033\0134"; printf "\0033]2;survivor\0033\0134"; sleep 300'
sleep 1
$TM display-message -p -t osc3 'title=[#{pane_title}] fg=#{pane_fg} bg=#{pane_bg}'
$TM list-buffers -F '#{buffer_name} #{buffer_size}'
