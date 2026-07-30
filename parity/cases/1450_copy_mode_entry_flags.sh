# copy-mode's own flags decide what state the mode starts in: -u enters already
# scrolled back one page, -e leaves it on scroll-to-bottom exit, -H puts it in
# hidden mode, -q cancels it, -M/-d/-s/-t select the pane. Each flag lands in a
# different field of the mode data, and a flag the port silently ignores looks
# identical to one it honours unless the resulting state is read back.
$TM new-window -d -n flags 'i=1; while [ $i -le 50 ]; do echo "line-$i"; i=$((i+1)); done; sleep 300'
sleep 1
st() { $TM display-message -p -t flags "$1 mode=#{pane_mode} in=#{pane_in_mode} y=#{copy_cursor_y} off=#{scroll_position}"; }
$TM copy-mode -t flags; st plain
$TM copy-mode -q -t flags; st after-q
$TM copy-mode -u -t flags; st after-u
$TM copy-mode -q -t flags
$TM copy-mode -e -t flags; st after-e
$TM copy-mode -q -t flags
$TM copy-mode -H -t flags; st after-H
$TM copy-mode -q -t flags
# -s takes a source pane to copy the history from; with only one pane it is the
# same pane, which must still be accepted.
$TM copy-mode -s flags -t flags; st after-s
$TM copy-mode -q -t flags
# An unknown flag is a usage error, and a bad target is a target error.
$TM copy-mode -Z -t flags 2>&1
$TM copy-mode -t nosuchwindow 2>&1
st final
