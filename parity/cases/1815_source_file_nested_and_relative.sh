# A sourced file can source another, and a path inside it is resolved against
# the working directory rather than the sourcing file, so a bare name only works
# when it is in the current directory.
d="${TMPDIR:-/tmp}/ztpar_src_nested"
command rm -rf "$d"; mkdir -p "$d"
printf 'set -g @inner sourced-inner\n' > "$d/inner.conf"
printf 'set -g @outer sourced-outer\nsource-file %s/inner.conf\n' "$d" > "$d/outer.conf"
printf 'set -g @rel relative\nsource-file inner.conf\n' > "$d/relative.conf"
$TM set -g @inner no; $TM set -g @outer no; $TM set -g @rel no
$TM source-file "$d/outer.conf"; echo "nested rc=$?"
echo "outer=$($TM show -gv @outer) inner=$($TM show -gv @inner)"
$TM source-file "$d/relative.conf" 2>&1 | perl -pe 's{^.*/}{}'; echo "relative rc=${PIPESTATUS[0]}"
echo "rel=$($TM show -gv @rel)"
command rm -rf "$d"
