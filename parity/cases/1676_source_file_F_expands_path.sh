# -F expands the path as a format before opening it, so a format can choose the
# file (cmd-source-file.c:42). The session has one window, so #{session_windows}
# selects 1.conf and not 2.conf. Paths are stripped from the output.
d="${TMPDIR:-/tmp}/ztpar_srcF"
command rm -rf "$d"; mkdir -p "$d"
printf 'set -g @sourced one-window\n' > "$d/1.conf"
printf 'set -g @sourced two-windows\n' > "$d/2.conf"
$TM set -g @sourced no
$TM source-file -F "$d/#{session_windows}.conf"; echo "rc=$?"
echo "sourced=$($TM show -gv @sourced)"
$TM new-window -d
$TM source-file -F "$d/#{session_windows}.conf"; echo "rc=$?"
echo "sourced=$($TM show -gv @sourced)"
echo "== a format that names a missing file =="
$TM source-file -F "$d/#{e|+|:#{session_windows},7}.conf" 2>&1 | perl -pe 's{^.*/}{}'; echo "rc=${PIPESTATUS[0]}"
command rm -rf "$d"
