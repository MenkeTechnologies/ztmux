# GAP: the scrollbar and control mouse locations.
#
# The C names 19 mouse locations per event family (tmux.h:178-197). ztmux's keyc
# mouse table is the older six -- Pane, Status, StatusLeft, StatusRight,
# StatusDefault, Border -- with no key code for SCROLLBAR_UP, SCROLLBAR_SLIDER,
# SCROLLBAR_DOWN or CONTROL0-9. server_client's own where_ enum collapses the
# three scrollbar locations into one variant that binds to nothing.
#
# So five default root bindings have no key to attach to, and the key names do
# not parse. copy-mode -S is unreachable for the same reason: its only default
# caller is the slider drag below.
$TM list-keys -T root MouseDown1ScrollbarUp
$TM list-keys -T root MouseDown1ScrollbarDown
$TM list-keys -T root MouseDrag1ScrollbarSlider
$TM list-keys -T root MouseDown1Control8
$TM list-keys -T root MouseDown1Control9
# Binding one by name fails rather than being accepted and never fired.
$TM bind-key -T root MouseDown1ScrollbarUp display-message gap 2>&1
$TM bind-key -T root MouseDown1Control0 display-message gap 2>&1
