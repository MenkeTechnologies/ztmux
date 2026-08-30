# Inside a hook body #{hook} names the hook that is running, and the hook_*
# formats name the objects it fired for -- but only for the notification hooks
# (window-linked, session-created), not for the after-<command> ones, where they
# expand to nothing. `set -F` stores the expansion rather than the format text,
# which is what makes the difference visible.
$TM set -g automatic-rename off
$TM set -g @seen ''
$TM set-hook -g window-linked 'set -gF @seen "#{hook}|#{hook_session_name}|#{hook_window_name}|#{hook_window}"'
$TM new-window -d -n probe
echo "window-linked: [$($TM show -gv @seen)]"
$TM set-hook -g session-created 'set -gF @seen "#{hook}|#{hook_session_name}"'
$TM new-session -d -s made -x 80 -y 24
echo "session-created: [$($TM show -gv @seen)]"
$TM set-hook -g after-new-window 'set -gF @seen "#{hook}|#{hook_session_name}|#{hook_window_name}"'
$TM set-hook -gu window-linked
$TM new-window -d -n second
echo "after-new-window: [$($TM show -gv @seen)]"
$TM set-hook -gu session-created
$TM set-hook -gu after-new-window
