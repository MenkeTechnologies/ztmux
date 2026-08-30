# -a lists the format variables one per line and -N waits for a key, so it needs
# a client. -p prints to the caller; without -p the message goes to a client.
$TM display-message -p 'plain=#{session_windows}'
echo "== -a lists variables =="
$TM display-message -a | grep -c '^session_windows='
$TM display-message -a | grep -c '^window_index='
echo "== -N without a client =="
$TM display-message -N 'x' 2>&1; echo "rc=$?"
echo "== without -p and with no client =="
$TM display-message 'nowhere to show this'; echo "rc=$?"
