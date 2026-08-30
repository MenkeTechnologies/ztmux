# customize-mode opens the option editor in a pane, which needs a client; the
# flags are parsed before that, so a bad one is rejected either way.
$TM customize-mode 2>&1; echo "rc=$?"
$TM customize-mode -Z 2>&1; echo "rc=$?"
$TM customize-mode -N 2>&1; echo "rc=$?"
$TM customize-mode -Q 2>&1; echo "rc=$?"
$TM list-commands customize-mode
