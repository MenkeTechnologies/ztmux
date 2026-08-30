# source-file on a path that does not exist is an error; -q silences it
# (cmd-source-file.c:42, "-q"). The error names the path, so strip the directory.
missing="${TMPDIR:-/tmp}/ztpar_no_such_file.conf"
command rm -f "$missing"
$TM source-file "$missing" 2>&1 | perl -pe 's{^.*/}{}'; echo "rc=${PIPESTATUS[0]}"
$TM source-file -q "$missing" 2>&1 | perl -pe 's{^.*/}{}'; echo "quiet rc=${PIPESTATUS[0]}"
