# save-buffer -a appends instead of truncating.
d=$(mktemp -d)
$TM set-buffer -b one 'first
'
$TM set-buffer -b two 'second
'
$TM save-buffer -b one "$d/acc.txt"
$TM save-buffer -a -b two "$d/acc.txt"; echo "rc=$?"
cat "$d/acc.txt"
command rm -rf "$d"
