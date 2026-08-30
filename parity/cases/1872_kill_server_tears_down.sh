# kill-server ends every session and the server with them; afterwards a command
# on that socket says there is no server, and the socket path is stripped since
# it names the binary.
gone() { perl -pe 's{^server exited unexpectedly$}{SERVER GONE}; s{^no server running on /\S+$}{SERVER GONE}'; }
set -- $TM
BIN="$1"
ISOCK="kst_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }
inner -f /dev/null new-session -d -s one -x 40 -y 6 'sleep 300' >/dev/null
inner new-session -d -s two -x 40 -y 6 'sleep 300' >/dev/null
echo "sessions: $(inner list-sessions -F '#{session_name}' | sort | tr '\n' ' ')"
inner kill-server 2>&1 | gone; echo "rc=$?"
inner list-sessions 2>&1 | gone; echo "rc=$?"
inner display-message -p ok 2>&1 | gone
