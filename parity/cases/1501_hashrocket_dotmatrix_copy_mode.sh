# The four copy-mode commands hashrocket/dotmatrix binds, driven end to end.
#
# Case 1500 proves the bindings PARSE the same; this proves the commands behind
# them DO the same thing under the config's `setw -g mode-keys vi`:
#
#   v -> begin-selection            V -> rectangle-toggle
#   y -> copy-pipe-and-cancel       wheel -> halfpage-up / halfpage-down
#
# copy-pipe-and-cancel is the one with two jobs: it must both feed the selection
# to a shell command's stdin and leave copy mode, and dotmatrix's binding pipes
# into pbcopy, so a version that copies but does not cancel leaves every yank
# stuck in copy mode. The pipe target here is `cat`, whose output the pane shows,
# so what reached the command's stdin is visible rather than assumed.
$TM setw -g mode-keys vi
$TM show-window-options -g mode-keys
$TM new-window -d -n hr 'printf "0123456789\nabcdefghij\nABCDEFGHIJ\n"; sleep 300'
$TM new-window -d -n sink 'cat; sleep 300'
sleep 1

st() { $TM display-message -p -t hr "$1 in=#{pane_in_mode} sel=#{selection_present} rect=#{rectangle_toggle}"; }

$TM copy-mode -t hr
$TM send-keys -X -t hr history-top
$TM send-keys -X -t hr start-of-line
st entered

# v
$TM send-keys -X -t hr begin-selection
$TM send-keys -X -t hr cursor-right
$TM send-keys -X -t hr cursor-right
$TM send-keys -X -t hr cursor-down
st after-v

# V
$TM send-keys -X -t hr rectangle-toggle
st after-V

# y: pipe the selection into a command AND leave the mode.
$TM send-keys -X -t hr copy-pipe-and-cancel "cat >&2"
st after-y
$TM show-buffer | perl -pe "s{^(.*)\$}{[\$1]}"

# The wheel bindings: halfpage-up must actually move, halfpage-down come back.
$TM new-window -d -n scroll 'for i in $(seq 1 200); do echo "line $i"; done; sleep 300'
sleep 1
$TM copy-mode -t scroll
$TM display-message -p -t scroll "top offset=#{scroll_position}"
$TM send-keys -X -t scroll halfpage-up
$TM display-message -p -t scroll "up1 offset=#{scroll_position}"
$TM send-keys -X -t scroll halfpage-up
$TM display-message -p -t scroll "up2 offset=#{scroll_position}"
$TM send-keys -X -t scroll halfpage-down
$TM display-message -p -t scroll "down1 offset=#{scroll_position}"
$TM send-keys -X -t scroll halfpage-down
$TM display-message -p -t scroll "down2 offset=#{scroll_position} in=#{pane_in_mode}"
