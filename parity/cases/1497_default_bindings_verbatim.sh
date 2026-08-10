# Specific default bindings, printed one key at a time.
#
# A whole-table `list-keys` can never be a parity case: ztmux's prefix and root
# tables are supersets (its own dashboard, switcher and floating-pane bindings),
# so the tables differ by design. Naming one key at a time sidesteps that and
# still pins the binding STRINGS, which is where a hand-transcribed default table
# drifts from the C without any behaviour looking broken.
#
# Every line below was a real divergence found by diffing the two tables:
#   MouseDown1Status      ran select-window instead of switch-client
#   WheelUpPane           omitted #{alternate_on} from the || chain, so the wheel
#                         opened copy-mode inside a full-screen alternate-screen
#                         app instead of being forwarded
#   MouseDown3Status      carried a -O the C uses on exactly one binding
#   MouseDown3StatusLeft  built its menu directly instead of through run -C, and
#                         so could not expand the #{S/t:} per-session loop
#   C-Left / C-Up / M-*   lacked the floating-pane branch, so the four resize
#                         directions that need the opposite edge were no-ops
#   copy-mode-vi # and *  dropped the -- before the search argument
$TM list-keys -T root MouseDown1Status
$TM list-keys -T root C-MouseDown1Status
$TM list-keys -T root WheelUpPane
$TM list-keys -T root WheelUpStatus
$TM list-keys -T root WheelDownStatus
$TM list-keys -T root MouseDown3Status
$TM list-keys -T root M-MouseDown3Status
$TM list-keys -T root MouseDown3StatusLeft
$TM list-keys -T root M-MouseDown3StatusLeft
$TM list-keys -T prefix C-Left
$TM list-keys -T prefix C-Right
$TM list-keys -T prefix C-Up
$TM list-keys -T prefix C-Down
$TM list-keys -T prefix M-Left
$TM list-keys -T prefix M-Right
$TM list-keys -T prefix M-Up
$TM list-keys -T prefix M-Down
# The -- separator before a search argument: without it a copy_cursor_word that
# begins with a dash is parsed as a flag.
$TM list-keys -T copy-mode-vi '#'
$TM list-keys -T copy-mode-vi '*'
