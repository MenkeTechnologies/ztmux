# display-menu position formats when the menu is TALLER than the screen.
# The C computes several of these in u_int and lets them wrap; a port that
# subtracts in signed width (or panics on overflow) diverges here. Regression
# for a right-click on a pane border killing the server.
$TM display-message -p 'centre #{popup_centre_x}|#{popup_centre_y}'
$TM display-message -p 'pane #{popup_pane_top}|#{popup_pane_bottom}|#{popup_pane_left}|#{popup_pane_right}'
# A real mouse event is needed for the popup_mouse_* set, so drive one through
# a menu bound to a border click; -O keeps it from blocking.
$TM set -g mouse on
$TM display-menu -O -x0 -y0 -T tall \
  a a {} b b {} c c {} d d {} e e {} f f {} g g {} h h {} i i {} j j {} \
  k k {} l l {} m m {} n n {} o o {} p p {} q q {} r r {} s s {} t t {} \
  u u {} v v {} w w {} x x {} y y {} z z {}
$TM display-message -p 'survived #{window_panes}'
