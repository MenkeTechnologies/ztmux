# show-options -H includes the hooks in the listing, which they are absent from
# otherwise; the hooks have their own command (show-hooks) as well.
$TM set-hook -g after-new-window 'display-message hooked'
echo "without -H, hooks are absent: $($TM show-options -g | grep -c '^after-new-window')"
echo "with -H, they are there:      $($TM show-options -gH | grep -c '^after-new-window')"
echo "show-hooks agrees:            $($TM show-hooks -g | grep -c '^after-new-window')"
echo "== -H on a window scope =="
$TM set-hook -w 'after-select-pane' 'display-message w'
$TM show-options -wH | grep -c '^after-select-pane'
$TM set-hook -gu after-new-window
$TM set-hook -wu after-select-pane
