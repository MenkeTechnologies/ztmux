# The command prompt's history and its single-key (PROMPT_SINGLE) form, driven
# through a real client the way 1491 does.
#
# Up walks `prompt_up_history` over the per-type list `prompt_add_history` fills
# on Enter, and `show-prompt-history` reads that same list back — so the two
# halves of prompt-history.c are checked against each other as well as against
# the reference. `confirm-before` is the PROMPT_SINGLE case: one key closes the
# prompt and fires the callback, with no Enter.
set -- $TM
BIN=$1
SOCK=$3
IN="${SOCK}_ph"

inner() { $BIN -L "$IN" "$@" >/dev/null 2>&1; }

inner -f /dev/null new-session -d -s S -n w1 -x 60 -y 12 'sleep 300'
inner set-option -g status off
$TM new-session -d -s outer -x 60 -y 12 "$BIN -L $IN attach -t S" >/dev/null 2>&1
sleep 1

inner command-prompt -b -p '(x)' 'set-option -g @typed "%%"'
sleep 1
$TM send-keys -t outer:0 first Enter
sleep 1

# Up recalls the last entry; Enter re-runs it.
inner set-option -gu @typed
inner command-prompt -b -p '(x)' 'set-option -g @typed "%%"'
sleep 1
$TM send-keys -t outer:0 Up Enter
sleep 1
printf '== after history Up: [%s]\n' "$($BIN -L "$IN" show-options -gqv @typed 2>&1)"

# A repeated entry is not stored twice.
printf '== history: %s\n' "$($BIN -L "$IN" show-prompt-history -T command 2>&1 | tr '\n' '|')"

# PROMPT_SINGLE: one key, no Enter.
inner set-option -gu @typed
inner confirm-before -b -p 'go? (y/n)' 'set-option -g @typed confirmed'
sleep 1
$TM send-keys -t outer:0 y
sleep 1
printf '== confirm y: [%s]\n' "$($BIN -L "$IN" show-options -gqv @typed 2>&1)"

# ...and a key that is not the confirm key closes it without running anything.
inner set-option -gu @typed
inner confirm-before -b -p 'go? (y/n)' 'set-option -g @typed confirmed'
sleep 1
$TM send-keys -t outer:0 n
sleep 1
printf '== confirm n: [%s]\n' "$($BIN -L "$IN" show-options -gqv @typed 2>&1)"

$TM kill-session -t outer >/dev/null 2>&1
inner kill-server
