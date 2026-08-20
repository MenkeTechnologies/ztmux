# The condition the default WheelUpPane binding tests before it scrolls.
#
# key-bindings.c:457 binds WheelUpPane to
#   if -F '#{||:#{alternate_on},#{pane_in_mode},#{mouse_any_flag}}' { send -M } { copy-mode -e }
# so a wheel-up in an ordinary pane must take the ELSE branch and enter copy
# mode; only a full-screen application, an already-open mode, or a pane that
# asked for the mouse itself takes `send -M`. That is a THREE-operand `||`, and
# a two-operand `||` reading it splits at the first comma and finds "0,0" on the
# right -- non-empty, so true -- which sends the wheel to the pane instead and
# copy mode never opens.

echo "== the binding is the three-operand form =="
$TM list-keys -T root -N WheelUpPane 2>/dev/null | head -1
$TM list-keys -T root | grep -w WheelUpPane

echo "== plain pane: every operand false, so the condition is false =="
$TM display-message -p '#{alternate_on}|#{pane_in_mode}|#{mouse_any_flag}|#{||:#{alternate_on},#{pane_in_mode},#{mouse_any_flag}}'

echo "== running the binding's if -F for real opens copy mode =="
$TM if -F '#{||:#{alternate_on},#{pane_in_mode},#{mouse_any_flag}}' 'display-message "sent"' 'copy-mode -e'
$TM display-message -p 'pane_in_mode=#{pane_in_mode} mode=#{pane_mode}'

echo "== and in copy mode the condition flips, so the wheel goes to the mode =="
$TM display-message -p '#{||:#{alternate_on},#{pane_in_mode},#{mouse_any_flag}}'
$TM send-keys -X cancel
$TM display-message -p 'pane_in_mode=#{pane_in_mode}'
