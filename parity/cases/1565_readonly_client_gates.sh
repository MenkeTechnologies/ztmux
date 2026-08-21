# Read-only clients: what they may and may not do.
#
# `detach-client` carries CMD_READONLY so a read-only client can detach ITSELF,
# but detaching OTHER clients is a write and is refused: `-s` (a whole session),
# `-a` (every other client), or naming a different target
# (cmd-detach-client.c:73-78). The port had the command flag but not the gate,
# so a read-only client could detach everyone else.
#
# The read-only client is real and it is the one issuing the command: an inner
# server is attached with `attach -r` from a pane of the outer one, a key is
# bound there to the command under test, and that key is typed into the OUTER
# pane -- which is the inner client's terminal input. Anything else would run
# the command as a fresh, writable client and test nothing.
set -- $TM
BIN="$1"
ISOCK="ro_$$_inner"

inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s ro -x 40 -y 6 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner bind -n F1 detach-client -a >/dev/null
inner bind -n F2 detach-client >/dev/null

# A second, writable client on the same session: it is what `-a` would detach.
$TM new-window -d -n keeper "$BIN -L $ISOCK attach -t ro"
sleep 1
$TM new-window -d -n roclient "$BIN -L $ISOCK attach -r -t ro"
sleep 2

echo "== two clients, one of them read-only =="
inner list-clients -F '#{client_readonly}' | sort

echo "== the read-only client presses F1 (detach-client -a) =="
$TM send-keys -t roclient F1
sleep 2
echo "clients still attached:"
inner list-clients -F '#{client_readonly}' | sort

echo "== the read-only client presses F2 (detach itself) =="
$TM send-keys -t roclient F2
sleep 2
echo "clients still attached:"
inner list-clients -F '#{client_readonly}' | sort

inner kill-server >/dev/null 2>&1
$TM kill-window -t keeper 2>/dev/null
$TM kill-window -t roclient 2>/dev/null
