# -b runs the command in the background: the calling queue does not wait for it,
# so a command issued afterwards can land first. Both orderings are observed
# through a file the commands append to, and the case waits for the background
# one rather than assuming a timing.
out="${TMPDIR:-/tmp}/ztpar_runshell_bg.out"
command rm -f "$out"
$TM run-shell -b "sleep 0.5; printf 'background\n' >> $out"
$TM run-shell "printf 'foreground\n' >> $out"
echo "immediately after both were issued:"
cat "$out" 2>/dev/null | sed 's/^/  /'
for _ in $(seq 1 40); do
  [ "$(grep -c background "$out" 2>/dev/null)" = 1 ] && break
  sleep 0.2
done
echo "once the background one has run:"
sort "$out" | sed 's/^/  /'
echo "== if-shell -b is the same shape =="
command rm -f "$out"
$TM if-shell -b true "run-shell \"printf 'if-background\n' >> $out\""
for _ in $(seq 1 40); do
  [ -s "$out" ] && break
  sleep 0.2
done
cat "$out" | sed 's/^/  /'
command rm -f "$out"
