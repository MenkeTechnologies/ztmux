# Numeric options are range-checked when they are set; the error names the
# option and the value that was refused.
$TM set -g history-limit 0; echo "rc=$?"
$TM show -gv history-limit
$TM set -g history-limit -1 2>&1; echo "rc=$?"
$TM set -g display-time -5 2>&1; echo "rc=$?"
$TM setw -g main-pane-height 0; echo "rc=$?"
$TM show -gwv main-pane-height
$TM set -g status-interval notanumber 2>&1; echo "rc=$?"
$TM set -g status-position middle 2>&1; echo "rc=$?"
$TM set -g history-limit 2000
