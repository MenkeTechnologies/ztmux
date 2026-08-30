# Config variable expansion: ${NAME} and $NAME both expand, an unset name
# expands to nothing, and expansion happens inside a double-quoted word.
cfg="${TMPDIR:-/tmp}/ztpar_cfg_var.conf"
cat > "$cfg" <<'CFG'
V=abc
set -g @braced ${V}
set -g @bare $V
set -g @quoted "x${V}y"
set -g @unset "[${NOPE}]"
set -g @single 'literal ${V}'
CFG
$TM source-file "$cfg"; echo "rc=$?"
for o in @braced @bare @quoted @unset @single; do printf '%s=%s\n' "$o" "$($TM show -gv $o)"; done
command rm -f "$cfg"
