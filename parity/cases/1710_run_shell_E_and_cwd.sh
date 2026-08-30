# -c sets the working directory the command runs in, and -E ... is passed to the
# shell like any other command (cmd-run-shell.c:47).
d=$(mktemp -d)
$TM run-shell -c "$d" 'pwd' | perl -pe 's{^/private}{}' | perl -pe "s{\Q$d\E}{DIR}"
$TM run-shell 'echo $((6*7))'
$TM run-shell -c /nonexistent-dir-ztpar 'pwd' 2>&1 | perl -pe 's{^.*:\s*}{}'; echo "rc=${PIPESTATUS[0]}"
command rm -rf "$d"
