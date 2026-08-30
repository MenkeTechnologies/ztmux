# +N and -N move that many windows along the list and wrap at the ends.
$TM set -g automatic-rename off
$TM new-window -d -n a
$TM new-window -d -n b
$TM new-window -d -n c
$TM select-window -t 0
for t in '+1' '+2' '+3' '+4' '-1' '-2'; do
  printf '%-4s %s\n' "$t" "$($TM display-message -p -t "$t" '#{window_index}:#{window_name}')"
done
