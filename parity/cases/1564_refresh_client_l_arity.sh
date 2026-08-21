# `refresh-client -l` takes no argument in next-3.7.
#
# The port declared `l::` where the C declares `l` (cmd-refresh-client.c:39), so
# `-l` swallowed the next character as its value: `refresh-client -lZ` was read
# as "-l with value Z" and got as far as looking for a client, where the C
# rejects the unknown flag outright. Underneath, the port still implemented the
# pre-next-3.7 `-l [target-pane]` semantics, registering panes in
# `clipboard_panes`; next-3.7 just calls tty_clipboard_query and lets the input
# request queue route the answer back to whichever pane asked.

echo "== -lZ is an unknown flag, not an argument =="
$TM refresh-client -lZ 2>&1; echo "rc=$?"

echo "== -l alone is accepted (no client attached, so it says so) =="
$TM refresh-client -l 2>&1; echo "rc=$?"

echo "== -l does not consume a following argument =="
$TM refresh-client -l -t nosuchclient 2>&1; echo "rc=$?"

echo "== usage still lists it among the no-argument flags =="
$TM refresh-client -Z 2>&1
