# The scrollbar and control mouse locations.
#
# The C names 19 mouse locations per event family (tmux.h:177-197). This was a
# known gap while ztmux's keyc mouse table carried only the older six -- Pane,
# Status, StatusLeft, StatusRight, StatusDefault, Border -- so SCROLLBAR_UP,
# SCROLLBAR_SLIDER, SCROLLBAR_DOWN and CONTROL0-9 had no key code, the names did
# not parse, five default root bindings had nothing to attach to, and copy-mode
# -S was unreachable because the slider drag is its only default caller.
#
# All 19 locations are now present, so this pins the names, the bindings and the
# flag rather than recording their absence.
$TM list-keys -T root MouseDown1ScrollbarUp
$TM list-keys -T root MouseDown1ScrollbarDown
$TM list-keys -T root MouseDrag1ScrollbarSlider
$TM list-keys -T root MouseDown1Control8
$TM list-keys -T root MouseDown1Control9
# Binding by name is accepted, including a location with no default binding.
$TM bind-key -T root MouseDown1ScrollbarUp display-message ok 2>&1
echo "rc=$?"
$TM bind-key -T root MouseDown1Control0 display-message ok 2>&1
echo "rc=$?"
$TM list-keys -T root MouseDown1ScrollbarUp
$TM list-keys -T root MouseDown1Control0
# Every location parses, across event families.
for k in MouseUp1ScrollbarDown MouseDrag1ScrollbarUp MouseDragEnd1ScrollbarSlider \
         WheelUpScrollbarSlider DoubleClick1Control3 SecondClick1Control7; do
  $TM bind-key -T root "$k" display-message ok >/dev/null 2>&1
  printf '%s parses: %s\n' "$k" "$?"
done
# copy-mode -S, whose only default caller is the slider drag.
$TM copy-mode -S 2>&1; echo "copy-mode -S rc=$?"
