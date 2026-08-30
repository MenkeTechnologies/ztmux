# Nested %if inside both a taken and a skipped arm: the inner conditions of a
# skipped arm must not run, and their %endif must still be consumed.
cfg="${TMPDIR:-/tmp}/ztpar_cfg_nested.conf"
cat > "$cfg" <<'CFG'
set -g @inner none
set -g @outer none
%if 1
set -g @outer taken
%if 0
set -g @inner wrong
%else
set -g @inner right
%endif
%else
%if 1
set -g @outer skipped-inner
%endif
%endif
CFG
$TM source-file "$cfg"; echo "rc=$?"
$TM show -gv @outer
$TM show -gv @inner
command rm -f "$cfg"
