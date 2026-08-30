# %hidden FOO=bar defines a config variable usable as ${FOO} in later lines but
# hidden from show-environment; a plain FOO=bar is visible there.
cfg="${TMPDIR:-/tmp}/ztpar_cfg_hidden.conf"
cat > "$cfg" <<'CFG'
%hidden HID=secret
SHOWN=public
set -g @from_hidden ${HID}
set -g @from_shown ${SHOWN}
CFG
$TM source-file "$cfg"; echo "rc=$?"
echo "hidden value: $($TM show -gv @from_hidden)"
echo "shown value:  $($TM show -gv @from_shown)"
echo "== show-environment -g =="
$TM show-environment -g HID 2>&1; echo "rc=$?"
$TM show-environment -g SHOWN 2>&1; echo "rc=$?"
command rm -f "$cfg"
