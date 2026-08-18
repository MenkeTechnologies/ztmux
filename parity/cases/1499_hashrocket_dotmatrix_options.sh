# hashrocket/dotmatrix .tmux.conf, sourced as a file -- the options it sets.
#
# dotmatrix is Hashrocket's shared dotfiles repo; its .tmux.conf is the config a
# whole shop runs, so "does ztmux load it to the same state as tmux" is a real
# acceptance question, not a synthetic one. The file is written out and read with
# source-file rather than replayed as `$TM set ...` lines, because that is the
# path a user actually takes: the config LEXER (comments, `\;`, quoting, `-q`)
# runs, not just the command parser.
#
# The interesting lines are not the plain `set -g`s:
#
#   setw -g mode-keys vi        a WINDOW option set globally
#   set -sa terminal-overrides  APPEND to a server option that already holds a
#                               default, so the result pins both the separator
#                               and the index the appended entry lands on
#   set -sg escape-time 0       -s with -g, which tmux accepts and ignores
#   set -g -q mouse on          -q, the quiet flag, mid-arguments
d=$(mktemp -d)
cat >"$d/hr.conf" <<'CONF'
# Force vi mode keys if $EDITOR is not explicity 'vi'
setw -g mode-keys vi

set -g default-terminal "screen-256color"
set -sa terminal-overrides ',*256*:Tc'

set -g prefix C-z

set -sg escape-time 0

# scrollback buffer size increase
set -g history-limit 100000

# Mouse options for tmux >= 2.5
set-option -g -q mouse on

# Better project name in status bar
set -g status-left-length 18
set -g status-left '#[fg=colour235,bg=colour76,bold] #S '
CONF
$TM show-options -s terminal-overrides
$TM source-file "$d/hr.conf"

$TM show-window-options -g mode-keys
$TM show-options -g default-terminal
$TM show-options -s terminal-overrides
$TM show-options -g prefix
$TM show-options -s escape-time
$TM show-options -g history-limit
$TM show-options -g mouse
$TM show-options -g status-left-length
$TM show-options -g status-left

# A repeated append is not deduplicated: the same override lands twice.
$TM set -sa terminal-overrides ',*256*:Tc'
$TM show-options -s terminal-overrides

# The style string has to survive the round trip and still expand.
$TM display-message -p '#{E:status-left}'
rm -rf "$d"
