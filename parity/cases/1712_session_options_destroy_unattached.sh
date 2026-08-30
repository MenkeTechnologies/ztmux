# destroy-unattached destroys a session that has no attached client. With no
# client anywhere, setting it on takes the session (and so the server) down --
# which is the behaviour worth pinning. Socket paths are stripped: they differ
# between the binaries.
# Whether the client notices the server going away as "server exited
# unexpectedly" or finds the socket already gone ("no server running on ...") is
# a race with the server's exit, and it goes both ways on both binaries. Fold
# the two into one token, and strip the socket path, which names the binary.
strip() { perl -pe 's{^server exited unexpectedly$}{SERVER GONE}; s{^no server running on /\S+$}{SERVER GONE}'; }
$TM show -gv destroy-unattached
echo "== the accepted values =="
for v in off on keep-last keep-group; do
  $TM set -g destroy-unattached "$v" >/dev/null 2>&1 && printf '%-11s %s\n' "$v" "$($TM show -gv destroy-unattached 2>&1 | strip)"
done
$TM set -g destroy-unattached nonsense 2>&1 | strip; echo "rc=${PIPESTATUS[0]}"
