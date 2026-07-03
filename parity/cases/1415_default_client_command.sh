# default-client-command scalar COMMAND option: default + set round-trip.
$TM show-options -sg default-client-command
$TM set-option -s default-client-command "new-window"
$TM show-options -sg default-client-command
$TM set-option -s default-client-command "unknowncmd"
