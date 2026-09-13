# The rest of `prompt_flags`, plus `prompt_type` and the command-mode branch.
# Case 1962 covers the plain prompt, the input buffer and the two flags a plain
# `command-prompt` can set; these are the ones that need their own prompt each,
# and they are a separate case so both stay inside the runner's 15s budget.
#
#   INCREMENTAL     -i, which also runs its command on every keystroke
#   KEY             -k, a prompt that takes a single key
#   prompt_type     -T, prompt_type_string(), a field of its own next to the flags
#   COMMANDMODE     reached by pressing Escape in an open prompt under vi
#                   status-keys, and the only thing #{command_prompt} is 1 for --
#                   which is what the DEFAULT message-format branches on to pick
#                   message-command-style
#
# Same harness as 1962: an inner server with an attached client in a pane of the
# outer one, prompts opened from keys, and message-format set to print the values
# instead of the message.
set -- $TM
BIN="$1"
ISOCK="pkf_$$_inner"

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

echo "== -i sets INCREMENTAL =="
# The command is a silent `set` rather than display-message: an incremental
# prompt runs its command on every keystroke (prompt.c, PROMPT_INCREMENTAL), and
# a message would paint over the prompt line before it could be read.
prefix; $TM send-keys -t client 3; wait_prompt; line
$TM send-keys -t client Escape; wait_gone

echo "== -k sets KEY =="
prefix; $TM send-keys -t client 4; wait_prompt; line
$TM send-keys -t client z; wait_gone

echo "== -T names the type, which is a separate field from the flags =="
prefix; $TM send-keys -t client 5; wait_prompt; line
$TM send-keys -t client Escape; wait_gone

echo "== with vi status-keys, Escape switches the OPEN prompt to command mode =="
$BIN -L "$ISOCK" set -g status-keys vi
prefix; $TM send-keys -t client P; wait_prompt; line
$TM send-keys -t client Escape; settle; line
# Left open on purpose: in vi command mode Escape is not a cancel, and the only
# ways out either run the command (painting a message over the line) or need a
# mode change first. The server is torn down next, which is the same thing.

$BIN -L "$ISOCK" kill-server 2>/dev/null
