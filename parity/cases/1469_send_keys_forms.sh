# send-keys turns its arguments into key codes through three different paths:
# named keys, -l literal strings and -H hex byte values. What actually reaches
# the pane is only observable by reading the pane back, and key codes are wide
# enough that a truncation there is invisible until the wrong byte lands in the
# grid.
$TM new-window -d -n keys 'cat > /dev/null; sleep 300'
sleep 1
$TM new-window -d -n echo 'cat'
sleep 1
$TM send-keys -t echo -l 'literal text'
$TM send-keys -t echo Enter
sleep 1
$TM send-keys -t echo -H 68 65 78 0a
sleep 1
$TM send-keys -t echo 'a' 'b' 'c' Enter
sleep 1
$TM send-keys -t echo -l -- '-dashes-'
$TM send-keys -t echo Enter
sleep 1
$TM send-keys -N 3 -t echo z
$TM send-keys -t echo Enter
sleep 1
$TM capture-pane -p -S 0 -E 8 -t echo | perl -pe "s{^(.*)\$}{[\$1]}"
# Unknown key names and a bad hex byte are errors, and neither reaches the pane.
$TM send-keys -t echo NoSuchKey 2>&1
$TM send-keys -t echo -H zz 2>&1
$TM send-keys -t echo -H 1ff 2>&1
$TM display-message -p -t echo 'alive #{pane_dead} #{window_name}'
