# resize-window -x/-y set an explicit size, -A makes the window the size of the
# largest attached client and -a the smallest; with no client those two leave it
# alone. The window-size option has to be manual for an explicit size to stick.
$TM setw -g window-size manual
$TM display-message -p 'start=#{window_width}x#{window_height}'
$TM resize-window -x 60 -y 20; echo "rc=$?"
$TM display-message -p 'after -x -y=#{window_width}x#{window_height}'
$TM resize-window -D 3; echo "-D rc=$?"
$TM display-message -p 'after -D=#{window_width}x#{window_height}'
$TM resize-window -R 5; echo "-R rc=$?"
$TM display-message -p 'after -R=#{window_width}x#{window_height}'
$TM resize-window -A 2>&1; echo "-A rc=$?"
$TM display-message -p 'after -A=#{window_width}x#{window_height}'
$TM resize-window -x 0 2>&1; echo "zero rc=$?"
