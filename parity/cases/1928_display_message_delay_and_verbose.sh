# -d sets how long the message stays up (0 means until a key), -N waits for a
# key and -a lists the format variables; with no client the ones that need one
# say so, and -p prints regardless.
$TM display-message -p -d 0 'printed with -d 0'; echo "rc=$?"
$TM display-message -d 100 'no client to show this to'; echo "rc=$?"
$TM display-message -p -F '#{session_windows}'; echo "-F rc=$?"
echo "== -C clears the message from a client =="
$TM display-message -C 2>&1; echo "rc=$?"
echo "== -I sends to the pane instead =="
$TM display-message -I 'sent to the pane' 2>&1; echo "rc=$?"
