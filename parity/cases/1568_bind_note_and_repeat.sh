# -N attaches a note (printed by list-keys -N) and -r marks the binding as
# repeating (printed as -r by list-keys).
$TM bind -T bindtest a -N "the note" set -g @a 1
$TM bind -T bindtest -r b set -g @b 1
$TM list-keys -T bindtest | sort
echo "== -N =="
$TM list-keys -N -T bindtest | sort
