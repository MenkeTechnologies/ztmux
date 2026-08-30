# #{pane_start_command} keeps the command the pane was created with, even after
# it is used to respawn, while #{pane_input_off} and #{pane_last} stay off.
$TM split-window -d 'sleep 300'
$TM list-panes -F '[#{pane_start_command}] input_off=#{pane_input_off} last=#{pane_last}' | sort
