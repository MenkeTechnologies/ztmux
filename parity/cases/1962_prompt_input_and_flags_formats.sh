# `prompt_input` and `prompt_flags` (prompt.c:383-386), the two variables the
# prompt adds to its own format every time it redraws. Nothing else expands them
# and no case had: they live in `prompt_expand`, which runs only while a client
# has a prompt open, and the string they land in is `message-format` — which case
# 1480 pins the OPTION of, with no client to draw it.
#
# That makes this the only place the suite can see:
#
#   prompt_input    the buffer as typed so far, re-expanded on every keystroke
#   prompt_flags    prompt_flags_to_string(): a trailing-comma list built from
#                   the PROMPT_* bits (prompt.c:70-101), so each command-prompt
#                   flag has to reach the right bit
#   prompt_type     prompt_type_string() for -T
#   command_prompt  the 0/1 the DEFAULT message-format branches on to choose
#                   between message-style and message-command-style
#
# message-format is set to print the four of them instead of the message, so the
# status line reads back the values themselves rather than their styling — this
# case is about the formats, not about colours, and stays plain-text so it keeps
# clear of the theme divergence that quarantines the render cases.
#
# Each prompt is opened from a KEY, not by running `command-prompt` from the
# command line: the command wants a client, and run without one it hangs the
# invoking client instead of prompting anywhere visible.
#
# Built like 1540/1961: an inner server with an attached client in a pane of the
# outer one, so capture-pane on the OUTER server reads the inner client's status
# line.
set -- $TM
BIN="$1"
ISOCK="pif_$$_inner"

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the PORTED
# prompt. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
$BIN -L "$ISOCK" set -g message-format 'F=[#{prompt_flags}] T=[#{prompt_type}] I=[#{prompt_input}] C=#{command_prompt}'

$BIN -L "$ISOCK" bind    P command-prompt -p 'ask' 'display-message ok'
$BIN -L "$ISOCK" bind -N 'single'      1 command-prompt -1 -p 'one' 'display-message ok'
$BIN -L "$ISOCK" bind -N 'numeric'     2 command-prompt -N -p 'num' 'display-message ok'
$BIN -L "$ISOCK" bind -N 'incremental' 3 command-prompt -i -p 'inc' 'set -g @inc "%%"'
$BIN -L "$ISOCK" bind -N 'key'         4 command-prompt -k -p 'key' 'display-message ok'
$BIN -L "$ISOCK" bind -N 'search type' 5 command-prompt -T search -p 'sea' 'display-message ok'

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"
# Wait for the inner client to be attached instead of sleeping a fixed two
# seconds: every case here must finish inside the runner's 15s budget
# (run_parity.sh:172), and a blind sleep spends an eighth of it doing nothing.
wait_client() {
  local i=0
  while [ $i -lt 300 ]; do
    [ -n "$($BIN -L "$ISOCK" list-clients -F "#{client_tty}" 2>/dev/null)" ] && { sleep 0.2; return 0; }
    i=$((i+1)); sleep 0.05
  done
  echo "wait_client: TIMEOUT"
}
wait_client

snap()   { $TM capture-pane -p -t client 2>/dev/null; }
line()   { snap | tail -1 | cat -v | perl -pe 's/[ \t]+$//'; }
settle() { local a b i=0; a=$(line); while [ $i -lt 100 ]; do sleep 0.05; b=$(line);
             [ "$a" = "$b" ] && return 0; a=$b; i=$((i+1)); done; }
# Wait for the prompt to be on the status line rather than sleeping: the marker
# is the format's own "F=[" prefix, which only the prompt draws.
# A prompt that never appears usually means the client is gone, and every later
# step would then print its own error; say it once and stop.
alive()  { $TM list-windows -F "#{window_name}" 2>/dev/null | grep -qx client || { echo "the client window is gone"; exit 1; }; }
wait_prompt() {
  local i=0
  alive
  while [ $i -lt 200 ]; do
    case "$(line)" in *'F=['*) settle; return 0 ;; esac
    i=$((i+1)); sleep 0.05
  done
  echo "wait_prompt: TIMEOUT"
}
wait_gone() {
  local i=0
  while [ $i -lt 200 ]; do
    case "$(line)" in *'F=['*) ;; *) return 0 ;; esac
    i=$((i+1)); sleep 0.05
  done
  echo "wait_gone: the prompt would not close"
}
prefix() { $TM send-keys -t client C-b; sleep 0.3; }

echo "== a plain prompt: no flags set, type COMMAND, empty input =="
prefix; $TM send-keys -t client P; wait_prompt; line

echo "== prompt_input follows the buffer, keystroke by keystroke =="
$TM send-keys -t client a; settle; line
$TM send-keys -t client b; settle; line
$TM send-keys -t client c; settle; line
echo "-- and backspace takes one back off --"
$TM send-keys -t client BSpace; settle; line
$TM send-keys -t client Escape; wait_gone

echo "== -1 sets SINGLE =="
prefix; $TM send-keys -t client 1; wait_prompt; line
$TM send-keys -t client z; wait_gone

echo "== -N sets NUMERIC =="
prefix; $TM send-keys -t client 2; wait_prompt; line
$TM send-keys -t client Escape; wait_gone

$BIN -L "$ISOCK" kill-server 2>/dev/null
