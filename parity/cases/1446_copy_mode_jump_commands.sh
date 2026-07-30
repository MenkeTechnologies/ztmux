# The jump family scans one line for a character and stops either on it
# (jump-forward/backward) or one cell short of it (jump-to-forward/backward);
# jump-again and jump-reverse then replay that stored state in each direction.
# The "one cell short" variants and the replay are two separate places to get an
# index wrong, and both are invisible unless the resulting column is compared.
$TM new-window -d -n jmp 'printf "a-b-c-d-e-f\nxyzzy\n"; sleep 300'
sleep 1
$TM copy-mode -t jmp
$TM send-keys -X -t jmp history-top
$TM send-keys -X -t jmp start-of-line
at() { $TM display-message -p -t jmp "$1 #{copy_cursor_y},#{copy_cursor_x}"; }
$TM send-keys -X -t jmp jump-forward c; at jump-forward-c
$TM send-keys -X -t jmp jump-again; at again
$TM send-keys -X -t jmp jump-reverse; at reverse
$TM send-keys -X -t jmp end-of-line
$TM send-keys -X -t jmp jump-backward b; at jump-backward-b
$TM send-keys -X -t jmp start-of-line
$TM send-keys -X -t jmp jump-to-forward d; at jump-to-forward-d
$TM send-keys -X -t jmp jump-again; at to-again
$TM send-keys -X -t jmp end-of-line
$TM send-keys -X -t jmp jump-to-backward a; at jump-to-backward-a
# A character that is not on the line leaves the cursor alone.
$TM send-keys -X -t jmp jump-forward Q; at absent
# set-mark / jump-to-mark round-trip the saved position.
$TM send-keys -X -t jmp start-of-line
$TM send-keys -X -t jmp set-mark
$TM send-keys -X -t jmp cursor-down
$TM send-keys -X -t jmp cursor-right
$TM send-keys -X -t jmp jump-to-mark; at jump-to-mark
$TM send-keys -X -t jmp jump-to-mark; at jump-to-mark-again
