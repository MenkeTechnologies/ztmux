# pane-border-lines: every choice value is accepted and renders its own border
# glyphs; "none" and "spaces" draw blanks.
for v in single double heavy simple number spaces none; do
  $TM set -w pane-border-lines "$v"
  $TM display-message -p "$v=#{pane-border-lines}"
done
$TM set -w pane-border-lines bogus
