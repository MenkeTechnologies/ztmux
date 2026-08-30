# %elif: only the first true arm runs, and a later true arm in the same chain is
# skipped once one has been taken.
cfg="${TMPDIR:-/tmp}/ztpar_cfg_elif.conf"
cat > "$cfg" <<'CFG'
%if 0
set -g @arm first
%elif 1
set -g @arm second
%elif 1
set -g @arm third
%else
set -g @arm else
%endif
CFG
$TM source-file "$cfg"; echo "rc=$?"
$TM show -gv @arm
command rm -f "$cfg"
