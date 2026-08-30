# A false %if condition falls through to %else.
cfg="${TMPDIR:-/tmp}/ztpar_cfg_if_false.conf"
cat > "$cfg" <<'CFG'
%if 0
set -g @taken then
%else
set -g @taken else
%endif
CFG
$TM source-file "$cfg"; echo "rc=$?"
$TM show -gv @taken
command rm -f "$cfg"
