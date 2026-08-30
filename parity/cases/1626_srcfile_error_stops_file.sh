# A command that fails mid-file does not abort the rest of the file, but a parse
# error does: the first form still applies the following line, the second does not.
cfg="${TMPDIR:-/tmp}/ztpar_srcfile_err.conf"
printf 'set -g @start yes\nkill-window -t nosuchwindow\nset -g @after yes\n' > "$cfg"
$TM set -g @start no
$TM set -g @after no
$TM source-file "$cfg" 2>&1 | perl -pe 's{^.*/}{}'; echo "rc=${PIPESTATUS[0]}"
echo "start=$($TM show -gv @start) after=$($TM show -gv @after)"
command rm -f "$cfg"
