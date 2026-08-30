# -s, -w and -p select the server, window and pane option sets; asking for an
# option in the wrong set is an error naming it.
$TM set -s message-limit 42
$TM setw window-status-separator '|'
echo "server: $($TM show-options -sv message-limit)"
echo "window: $($TM show-options -wv window-status-separator)"
echo "== window option asked for in the server set =="
$TM show-options -sv window-status-separator 2>&1; echo "rc=$?"
echo "== server option asked for in the window set =="
$TM show-options -wv message-limit 2>&1; echo "rc=$?"
