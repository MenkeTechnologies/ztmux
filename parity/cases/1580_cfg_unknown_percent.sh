# A %word that is not one of the five conditions is an ERROR token
# (cmd-parse.y:1374-1375), not a command named "%frobnicate".
cfg="${TMPDIR:-/tmp}/ztpar_cfg_pct.conf"
printf '%%frobnicate\n' > "$cfg"
$TM source-file "$cfg" 2>&1 | perl -pe 's{^.*/}{}'; echo "rc=${PIPESTATUS[0]}"
command rm -f "$cfg"
