# -n parses the file without executing it, so a syntax error is still reported
# but a valid file leaves no trace.
cfg="${TMPDIR:-/tmp}/ztpar_srcfile_n.conf"
printf 'set -g @n_ran yes\n' > "$cfg"
$TM set -g @n_ran no
$TM source-file -n "$cfg"; echo "rc=$?"
echo "after -n: $($TM show -gv @n_ran)"
$TM source-file "$cfg"; echo "rc=$?"
echo "after plain: $($TM show -gv @n_ran)"
command rm -f "$cfg"
