# %if/%else in a config: the condition is a format expanded by the parser
# (cmd-parse.y, IF/ELSE tokens at :1359-1369). A true condition runs the first
# arm and skips the else arm entirely.
cfg="${TMPDIR:-/tmp}/ztpar_cfg_if_true.conf"
cat > "$cfg" <<'CFG'
%if 1
set -g @taken then
%else
set -g @taken else
%endif
CFG
$TM source-file "$cfg"; echo "rc=$?"
$TM show -gv @taken
command rm -f "$cfg"
