# OSC 9;4 progress bar: #{pane_pb_state} / #{pane_pb_progress}.
#
# The sequence has to come from the pane's own output, not send-keys, since it
# is the pane's input parser that reads it. Each pane below prints one sequence
# and then sleeps, so the state is stable when the format is expanded.
pb() {
  $TM new-window -d -n "$1" "printf '$2'; sleep 300"
  sleep 0.4
  printf '%s: %s\n' "$1" "$($TM display-message -p -t "$1" '#{pane_pb_state}|#{pane_pb_progress}')"
}

# A state with a percentage.
pb normal   '\033]9;4;1;42\007'
pb error    '\033]9;4;2;7\007'
pb paused   '\033]9;4;4;100\007'
pb hidden   '\033]9;4;0;5\007'

# Indeterminate carries no percentage of its own, so one given is ignored.
pb indet    '\033]9;4;3;61\007'

# A bare state keeps whatever progress was already set.
pb keep     '\033]9;4;1;33\007\033]9;4;2\007'
# ...and so does a state with an empty trailing field.
pb keepsemi '\033]9;4;1;77\007\033]9;4;4;\007'

# Malformed sequences must leave the bar alone rather than half-apply.
pb bad_over '\033]9;4;1;25\007\033]9;4;1;101\007'
pb bad_huge '\033]9;4;1;25\007\033]9;4;1;999999\007'
pb bad_stat '\033]9;4;1;25\007\033]9;4;5;3\007'
pb bad_junk '\033]9;4;1;25\007\033]9;4;1;4x\007'
pb bad_nosc '\033]9;4;1;25\007\033]9;41;3\007'

# OSC 9 without the 4 sub-parameter is a notification, not a progress bar.
pb notify   '\033]9;hello\007'

# The default, on a pane that printed nothing.
$TM new-window -d -n plain 'sleep 300'
sleep 0.3
printf 'plain: %s\n' "$($TM display-message -p -t plain '#{pane_pb_state}|#{pane_pb_progress}')"
