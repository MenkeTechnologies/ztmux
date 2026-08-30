# Command names may be abbreviated to a unique prefix; an ambiguous prefix is an
# error that lists the candidates, and a unique one resolves.
$TM ne 2>&1; echo "rc=$?"
$TM lis 2>&1; echo "rc=$?"
$TM new-w -d -n abbrev; echo "rc=$?"
$TM list-windows -F '#{window_name}' | sort
