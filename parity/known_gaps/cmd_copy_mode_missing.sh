# GAP: 11 entries of next-3.7's window_copy_cmd_table are absent from the port
# (window-copy.c: refresh_on/off/toggle behind window_copy_refresh_start/stop,
# scroll_exit_on/off/toggle behind data->scroll_exit, recentre_top_bottom with
# its RECENTRE_MIDDLE/TOP/BOTTOM state, cursor_centre_vertical/horizontal,
# scroll_to_mouse and selection_mode). The port also carries one entry the C
# does not have: refresh-from-pane.
#
# recentre-top-bottom is the visible one without a client: it cycles the view
# between the middle, top and bottom of the pane for a fixed cursor, so the
# scroll offset changes on every invocation in tmux and never changes here.
$TM new-window -d -n gap 'i=1; while [ $i -le 40 ]; do echo "l-$i"; i=$((i+1)); done; sleep 300'
sleep 1
$TM copy-mode -t gap
$TM send-keys -X -t gap goto-line 20
for c in recentre-top-bottom recentre-top-bottom recentre-top-bottom; do
  $TM send-keys -X -t gap "$c"
  $TM display-message -p -t gap "$c y=#{copy_cursor_y} off=#{scroll_position}"
done
# scroll-to-mouse is deliberately NOT driven here: the vendored tmux takes its
# server down when it runs with no mouse event to read (verified against
# vendor/tmux/tmux next-3.7 directly), so a case that calls it measures that
# upstream crash rather than this gap.
for c in cursor-centre-vertical cursor-centre-horizontal; do
  $TM send-keys -X -t gap "$c"
  $TM display-message -p -t gap "$c y=#{copy_cursor_y},#{copy_cursor_x} off=#{scroll_position}"
done
