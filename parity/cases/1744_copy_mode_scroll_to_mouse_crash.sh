# scroll-to-mouse with no mouse event behind it takes the server down -- on the
# vendored next-3.7 reference as well as here, which is the point: this is a
# faithful port of an upstream defect, and the case exists so that stays true
# rather than quietly diverging in either direction. It is the last thing the
# case does, and the socket path is stripped because it names the binary.
$TM copy-mode
echo "in copy mode: $($TM display-message -p '#{pane_in_mode}')"
# Whether a client sees "server exited unexpectedly" or finds the socket already
# gone is a race with the exit and goes both ways on both binaries, so the two
# messages are folded into one token.
gone() { perl -pe 's{^server exited unexpectedly$}{SERVER GONE}; s{^no server running on /\S+$}{SERVER GONE}'; }
$TM send-keys -X scroll-to-mouse 2>&1 | gone; echo "rc=${PIPESTATUS[0]}"
$TM display-message -p 'still alive' 2>&1 | gone
