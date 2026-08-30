# Sessions in a group already share every window, so linking one into a sibling
# is either a no-op or an error depending on the index asked for -- and either
# way the group's window list must not gain a duplicate.
$TM set -g automatic-rename off
$TM new-session -d -s lead -n first -x 80 -y 24
$TM new-session -d -s follow -t lead -x 80 -y 24
$TM new-window -d -t lead -n shared
echo "group windows: $($TM list-windows -t follow -F '#{window_index}:#{window_name}' | sort | tr '\n' ' ')"
$TM link-window -s lead:shared -t follow:9 2>&1; echo "link to a free index rc=$?"
echo "after: $($TM list-windows -t follow -F '#{window_index}:#{window_name}' | sort | tr '\n' ' ')"
$TM link-window -s lead:shared -t follow:1 2>&1; echo "link onto its own index rc=$?"
echo "lead sees:  $($TM list-windows -t lead -F '#{window_index}:#{window_name}' | sort | tr '\n' ' ')"
