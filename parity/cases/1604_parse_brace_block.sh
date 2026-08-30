# A { } block is one argument holding a command list: bind-key parses it and
# list-keys prints it back canonicalised.
$TM bind -T partest x { set -g @in_block yes ; display-message -p hi }
$TM list-keys -T partest
$TM bind -T partest y { if-shell true { set -g @nested yes } }
$TM list-keys -T partest | sort
