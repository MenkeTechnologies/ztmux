# next/previous-paragraph scan for blank lines and next/previous-matching-bracket
# runs a nesting counter over the grid; both walk in the decreasing direction as
# well as the increasing one, which is where an unsigned line counter wraps (the
# previous-prompt crash, 1437). Running each one until it hits the end of the
# history is what exercises that guard.
$TM new-window -d -n para 'printf "one\ntwo\n\nthree (nested [pair] here)\n\nfour\n\n"; sleep 300'
sleep 1
$TM copy-mode -t para
at() { $TM display-message -p -t para "$1 #{copy_cursor_y},#{copy_cursor_x} [#{copy_cursor_line}]"; }
$TM send-keys -X -t para history-top
for c in next-paragraph next-paragraph next-paragraph next-paragraph; do
  $TM send-keys -X -t para "$c"; at "$c"
done
for c in previous-paragraph previous-paragraph previous-paragraph previous-paragraph; do
  $TM send-keys -X -t para "$c"; at "$c"
done
# Brackets: from the opening paren, from inside the nested pair, and from a
# position with no bracket at all.
$TM send-keys -X -t para goto-line 4
$TM send-keys -X -t para start-of-line
$TM send-keys -X -t para next-matching-bracket; at fwd-bracket
$TM send-keys -X -t para previous-matching-bracket; at back-bracket
$TM send-keys -X -t para history-top
$TM send-keys -X -t para next-matching-bracket; at no-bracket
$TM display-message -p -t para "alive #{pane_in_mode}"
