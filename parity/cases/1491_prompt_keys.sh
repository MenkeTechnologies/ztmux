# The command prompt's KEY path: typing, backspace, and cancel.
#
# `send-keys` writes into a pane and never reaches a client-level prompt, so
# until now nothing in the suite drove `prompt_key` at all. The trick is the
# nested client used by 1484/1486/1489: keys sent to the OUTER pane are the
# inner client's terminal input, so they go through `server_client_handle_key`
# → `status_prompt_key` → `prompt_key` exactly as a real keyboard would.
#
# The prompt's own drawing is deliberately not captured: ztmux floats the prompt
# as an overlay box instead of taking over the status row, which is an intended
# extension. What is compared is the effect — what the prompt handed to the
# command it was collecting for.
set -- $TM
BIN=$1
SOCK=$3
IN="${SOCK}_pk"

inner() { $BIN -L "$IN" "$@" >/dev/null 2>&1; }

inner -f /dev/null new-session -d -s S -n w1 -x 60 -y 12 'sleep 300'
inner set-option -g status off
$TM new-session -d -s outer -x 60 -y 12 "$BIN -L $IN attach -t S" >/dev/null 2>&1
sleep 1

# Plain typing then Enter: the buffer reaches the template as `%%`.
inner command-prompt -b -p '(x)' 'set-option -g @typed "%%"'
sleep 1
$TM send-keys -t outer:0 hello Enter
sleep 1
printf '== typed: [%s]\n' "$($BIN -L "$IN" show-options -gqv @typed 2>&1)"

# Backspace deletes from the buffer, not from the pane underneath.
inner set-option -gu @typed
inner command-prompt -b -p '(x)' 'set-option -g @typed "%%"'
sleep 1
$TM send-keys -t outer:0 abcde BSpace BSpace Enter
sleep 1
printf '== after 2 backspaces: [%s]\n' "$($BIN -L "$IN" show-options -gqv @typed 2>&1)"

# Escape closes the prompt without running the command.
inner set-option -gu @typed
inner command-prompt -b -p '(x)' 'set-option -g @typed "%%"'
sleep 1
$TM send-keys -t outer:0 zz Escape
sleep 1
printf '== after escape: [%s]\n' "$($BIN -L "$IN" show-options -gqv @typed 2>&1)"

$TM kill-session -t outer >/dev/null 2>&1
inner kill-server
