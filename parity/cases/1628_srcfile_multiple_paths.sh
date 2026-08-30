# source-file takes any number of paths (args 1,-1) and runs them in order, so a
# later file overrides an earlier one.
a="${TMPDIR:-/tmp}/ztpar_srcfile_a.conf"
b="${TMPDIR:-/tmp}/ztpar_srcfile_b.conf"
printf 'set -g @who a\nset -g @only_a yes\n' > "$a"
printf 'set -g @who b\n' > "$b"
$TM source-file "$a" "$b"; echo "rc=$?"
echo "who=$($TM show -gv @who) only_a=$($TM show -gv @only_a)"
command rm -f "$a" "$b"
