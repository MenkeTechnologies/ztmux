# GAP: pane progress bar (pane_pb_state/pane_pb_progress) is unported.
# The floating-pane formats that used to be tested here (pane_floating_flag,
# pane_x, pane_y, pane_z) are ported and moved to parity/cases/1094_fmt_floating_pane.sh.
$TM display-message -p '#{pane_pb_state}|#{pane_pb_progress}'
