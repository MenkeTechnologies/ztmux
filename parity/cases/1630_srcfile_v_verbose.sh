# -v echoes each command as it is parsed. Strip the leading path so only the
# command text is compared.
cfg="${TMPDIR:-/tmp}/ztpar_srcfile_v.conf"
cat > "$cfg" <<'CFG'
set -g @a one
set -g @b two
CFG
$TM source-file -v "$cfg" 2>&1 | perl -pe 's{^.*/}{}'
echo "rc=$?"
command rm -f "$cfg"
