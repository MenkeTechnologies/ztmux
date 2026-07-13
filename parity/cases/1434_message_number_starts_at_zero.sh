# server_add_message assigns msg_num = message_next++ (assign, then bump). The port
# incremented first, shifting every #{message_number} up by one.
$TM display-message -p hi >/dev/null
$TM show-messages -F '#{message_number}' | sort -n | head -1
