# show-messages must not fail when no client is attached: the entry carries
# CMD_CLIENT_CANFAIL in C, so a missing client is quiet rather than an error.
# Print only the message bodies — timestamps and client ids are not stable.
$TM display-message -p hi >/dev/null
$TM show-messages -F '#{message_text}' | sed 's/client-[0-9]*/client-P/g' | sort
