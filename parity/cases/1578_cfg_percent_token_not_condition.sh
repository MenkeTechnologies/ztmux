# "% is a condition unless it is all % or all numbers, then it is a token"
# (cmd-parse.y:1344-1352): %% and %1 are ordinary words, so they reach the
# command as arguments instead of erroring as unknown conditions.
cfg="${TMPDIR:-/tmp}/ztpar_cfg_pcttok.conf"
cat > "$cfg" <<'CFG'
set -g @pct %%
set -g @num %1
CFG
$TM source-file "$cfg"; echo "rc=$?"
$TM show -gv @pct
$TM show -gv @num
command rm -f "$cfg"
