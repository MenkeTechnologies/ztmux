# Each object type has its own "can't find" wording, and an ambiguous session
# name is reported as ambiguous rather than missing (cmd-find.c).
$TM new-session -d -s alphaone -x 80 -y 24
$TM new-session -d -s alphatwo -x 80 -y 24
$TM display-message -p -t nosuchsession:  'x' 2>&1; echo "session rc=$?"
$TM display-message -p -t 0:nosuchwindow  'x' 2>&1; echo "window rc=$?"
$TM display-message -p -t 0:0.99          'x' 2>&1; echo "pane rc=$?"
$TM display-message -p -t alpha:          'x' 2>&1; echo "ambiguous session rc=$?"
$TM display-message -p -t alphaone:       'ok' 2>&1; echo "unambiguous rc=$?"
$TM kill-window -t nosuchwindow 2>&1; echo "kill-window rc=$?"
$TM kill-session -t nosuchsession 2>&1; echo "kill-session rc=$?"
$TM swap-pane -s 0.99 2>&1; echo "swap-pane rc=$?"
