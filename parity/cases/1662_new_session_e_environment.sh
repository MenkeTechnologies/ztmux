# -e sets a session environment variable at creation; several -e are allowed and
# show-environment reads them back.
$TM new-session -d -s envs -e FOO=bar -e BAZ=qux -x 80 -y 24; echo "rc=$?"
$TM show-environment -t envs FOO
$TM show-environment -t envs BAZ
echo "== a malformed -e is an error =="
$TM new-session -d -s bad -e NOEQUALS -x 80 -y 24 2>&1; echo "rc=$?"
