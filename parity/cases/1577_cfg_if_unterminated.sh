# A %if with no %endif is a parse error, and nothing in the file takes effect.
cfg="${TMPDIR:-/tmp}/ztpar_cfg_unterm.conf"
cat > "$cfg" <<'CFG'
set -g @before set
%if 1
set -g @inside set
CFG
$TM set -g @before unset
$TM set -g @inside unset
$TM source-file "$cfg" 2>&1 | perl -pe 's{^.*/}{}'; echo "rc=${PIPESTATUS[0]}"
$TM show -gv @before
$TM show -gv @inside
command rm -f "$cfg"
