# scroll-exit decides whether reaching the bottom of the history leaves copy
# mode. It is set by copy-mode -e at entry and by the scroll-exit-on/off/toggle
# commands afterwards, and it is observable without a client: with it on, a
# page-down that lands at the bottom cancels the mode, so #{pane_in_mode} flips
# to 0. Each of the three commands is driven twice in a row so an implementation
# that wires the explicit setters to the toggle diverges.
$TM new-window -d -n se 'i=1; while [ $i -le 60 ]; do echo "row-$i"; i=$((i+1)); done; sleep 300'
sleep 1
st() { $TM display-message -p -t se "$1 in=#{pane_in_mode} off=#{scroll_position}"; }
# Default (no -e): scrolling to the bottom keeps the mode.
$TM copy-mode -t se
$TM send-keys -X -t se history-top
for _ in 1 2 3 4; do $TM send-keys -X -t se page-down; done
st default-bottom
# scroll-exit-on, then page-down off the bottom: the mode ends.
$TM send-keys -X -t se history-top
$TM send-keys -X -t se scroll-exit-on
$TM send-keys -X -t se scroll-exit-on
for _ in 1 2 3 4; do $TM send-keys -X -t se page-down; done
st exit-on
# Re-enter and turn it back off explicitly.
$TM copy-mode -t se
$TM send-keys -X -t se history-top
$TM send-keys -X -t se scroll-exit-on
$TM send-keys -X -t se scroll-exit-off
$TM send-keys -X -t se scroll-exit-off
for _ in 1 2 3 4; do $TM send-keys -X -t se page-down; done
st exit-off
# Toggle from off to on, and back.
$TM send-keys -X -t se history-top
$TM send-keys -X -t se scroll-exit-toggle
$TM send-keys -X -t se scroll-exit-toggle
for _ in 1 2 3 4; do $TM send-keys -X -t se page-down; done
st toggled-twice
$TM send-keys -X -t se history-top
$TM send-keys -X -t se scroll-exit-toggle
for _ in 1 2 3 4; do $TM send-keys -X -t se page-down; done
st toggled-once
# copy-mode -e sets the same flag at entry.
$TM copy-mode -q -t se
$TM copy-mode -e -t se
$TM send-keys -X -t se history-top
for _ in 1 2 3 4; do $TM send-keys -X -t se page-down; done
st entered-with-e
