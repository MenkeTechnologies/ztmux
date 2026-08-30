# Window target tokens (cmd-find.c:52-56): {start} = ^, {end} = $, {last} = !,
# {next} = + and {previous} = -. Each resolves against the session's window list.
$TM set -g automatic-rename off
$TM new-window -d -n w1
$TM new-window -d -n w2
$TM new-window -d -n w3
$TM select-window -t w1
$TM select-window -t w2
for t in '{start}' '{end}' '{last}' '{next}' '{previous}'; do
  printf '%-12s %s\n' "$t" "$($TM display-message -p -t "$t" '#{window_index}:#{window_name}')"
done
echo "== the short spellings resolve the same =="
for t in '^' '$' '!' '+' '-'; do
  printf '%-12s %s\n' "$t" "$($TM display-message -p -t "$t" '#{window_index}:#{window_name}')"
done
