# set-window-option: -F expands the value, -a appends to a string option, -u
# unsets it back to the inherited value, -U also removes it from every pane in
# the window (cmd-set-option.c:168-178), and -q silences the option-name and
# scope failures -- but NOT a bad VALUE, which is reported after the -q checks
# are past (cmd-set-option.c:113,134,160 against :197).
$TM set -g status off
$TM split-window -d 'sleep 300'
$TM set -w '@wopt' base; echo "set rc=$?"
echo "window: [$($TM show -wv '@wopt')]"
echo "== -a appends =="
$TM set -w -a '@wopt' -and-more; echo "rc=$?"
echo "window: [$($TM show -wv '@wopt')]"
echo "== -F expands the value =="
$TM set -w -F '@wfmt' 'panes=#{window_panes}'; echo "rc=$?"
echo "window: [$($TM show -wv '@wfmt')]"
echo "== the same option set on each pane =="
for p in $($TM list-panes -F '#{pane_id}'); do $TM set -p -t "$p" '@wopt' "pane-$($TM display-message -p -t "$p" '#{pane_index}')"; done
$TM list-panes -F '  #{pane_index}: [#{?#{==:#{@wopt},},,#{@wopt}}]'
echo "== -u unsets the window one, leaving the pane ones =="
$TM set -w -u '@wopt'; echo "rc=$?"
echo "window: [$($TM show -wv '@wopt' 2>&1)]"
$TM list-panes -F '  #{pane_index}: [#{@wopt}]'
echo "== -U removes it from every pane too =="
$TM set -w '@wopt' back-again
$TM set -w -U -u '@wopt'; echo "rc=$?"
$TM list-panes -F '  #{pane_index}: [#{@wopt}]'
echo "== -q swallows an unknown option name =="
$TM set -w nosuchwindowoption x 2>&1; echo "rc=$?"
$TM set -w -q nosuchwindowoption x; echo "with -q rc=$?"
echo "== but a bad value is reported even with -q =="
$TM set -w status x 2>&1; echo "rc=$?"
$TM set -w -q status x 2>&1; echo "with -q rc=$?"
