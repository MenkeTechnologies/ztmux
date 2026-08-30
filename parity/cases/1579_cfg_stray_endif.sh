# %endif and %else with no open %if are parse errors naming the file and line.
cfg="${TMPDIR:-/tmp}/ztpar_cfg_stray.conf"
printf 'set -g @x one\n%%endif\n' > "$cfg"
$TM source-file "$cfg" 2>&1 | perl -pe 's{^.*/}{}'; echo "rc=${PIPESTATUS[0]}"
cfg2="${TMPDIR:-/tmp}/ztpar_cfg_stray_else.conf"
printf 'set -g @x one\n%%else\n' > "$cfg2"
$TM source-file "$cfg2" 2>&1 | perl -pe 's{^.*/}{}'; echo "rc=${PIPESTATUS[0]}"
command rm -f "$cfg" "$cfg2"
