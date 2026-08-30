# #{pane_key_mode} names the keyboard mode a pane has negotiated; a fresh pane
# is in the plain VT mode and copy mode does not change it.
$TM display-message -p 'mode=[#{pane_key_mode}]'
$TM copy-mode
$TM display-message -p 'in copy mode: [#{pane_key_mode}] in_mode=#{pane_in_mode}'
$TM send-keys -X cancel
$TM display-message -p 'after cancel: [#{pane_key_mode}] in_mode=#{pane_in_mode}'
