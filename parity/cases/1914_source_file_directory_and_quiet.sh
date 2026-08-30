# source-file on a directory is an error naming it, and -q does NOT silence that
# one: the C only skips a quiet ENOENT (cfg.c:load_cfg), so a directory -- which
# opens but cannot be read -- still reports. A file with no read permission is
# the same shape.
#
# The paths are fixed rather than mktemp'd: the name appears in the message, so
# a random one would differ between the two binaries' runs.
d="${TMPDIR:-/tmp}/ztpar_srcdir"
command rm -rf "$d"; mkdir -p "$d"
strip() { perl -pe 's{^(.*?): .*/([^/]+)$}{$1: $2}'; }
$TM source-file "$d" 2>&1 | strip; echo "rc=${PIPESTATUS[0]}"
$TM source-file -q "$d" 2>&1 | strip; echo "quiet rc=${PIPESTATUS[0]}"
echo "== a file with no read permission =="
printf 'set -g @nope yes\n' > "$d/unreadable.conf"
chmod 000 "$d/unreadable.conf"
$TM source-file "$d/unreadable.conf" 2>&1 | strip; echo "rc=${PIPESTATUS[0]}"
$TM source-file -q "$d/unreadable.conf" 2>&1 | strip; echo "quiet rc=${PIPESTATUS[0]}"
echo "== and the option in that file was never set =="
$TM show -gv @nope 2>&1; echo "rc=$?"
chmod 644 "$d/unreadable.conf"
command rm -rf "$d"
