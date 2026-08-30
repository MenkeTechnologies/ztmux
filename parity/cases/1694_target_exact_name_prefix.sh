# A window target matches by exact name first, then as a prefix, then as an
# fnmatch pattern; the = prefix forces the exact match only.
$TM set -g automatic-rename off
$TM new-window -d -n abc
$TM new-window -d -n abcdef
echo "exact:  $($TM display-message -p -t 'abc' '#{window_name}')"
echo "=exact: $($TM display-message -p -t '=abc' '#{window_name}')"
echo "prefix: $($TM display-message -p -t 'abcd' '#{window_name}')"
echo "glob:   $($TM display-message -p -t 'abc*' '#{window_name}')"
echo "== =name with no exact match =="
$TM display-message -p -t '=abcd' '#{window_name}' 2>&1; echo "rc=$?"
echo "== a name matching nothing =="
$TM display-message -p -t 'zzz' '#{window_name}' 2>&1; echo "rc=$?"
