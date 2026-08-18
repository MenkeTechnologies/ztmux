# OSC 8 hyperlinks as they reach the terminal, and the feature that carries them.
#
# ztmux shipped with hyperlinks OFF. The `Hls` capability sat behind a cargo
# feature (`hyperlinks`) that was not in `default`, so every build had an EMPTY
# capability list for tty_feature_hyperlinks and dropped OSC 8 entirely: a pane
# printing a hyperlink came back as bare text, and `#[link=...]` in a format drew
# nothing. Upstream gates the same capability on an ncurses-version #if
# (tty-features.c:98) that ztmux -- which links no ncurses and emits the string
# from its own table -- has nothing to test.
#
# The terminal-feature table had drifted too: the `tmux` entry was missing
# `extkeys` and `progressbar`, and foot, WezTerm and ghostty were absent
# outright, so a user on any of those got no feature detection at all.
#
# Read through a nested client the way cases 1504/1507/1508 do -- an inner server
# whose client lives in a pane of the outer one -- because `capture-pane -e` on
# the OUTER server is the only way this suite can see what was actually emitted.
set -- $TM
BIN="$1"
ISOCK="hlr_$$_inner"

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 \
  'printf "\033]8;;http://example.com/one\033\\FIRST\033]8;;\033\\ plain\n"; sleep 300'
$BIN -L "$ISOCK" set -g status-interval 0
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
$BIN -L "$ISOCK" set -g status-left-length 70
$BIN -L "$ISOCK" set -g status-right ''

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"
sleep 2

echo "== the inner client's negotiated feature set =="
# The inner client identifies the outer terminal as "tmux" via XDA, so this is
# the `tmux` row of tty_default_features rendered in array order -- the exact
# place extkeys and progressbar went missing.
$BIN -L "$ISOCK" display-message -p '#{client_termfeatures}'

echo "== a pane's own OSC 8 reaches the terminal =="
$TM capture-pane -p -e -t client | sed -n '1p' | cat -v | perl -pe 's/\s+$//'

echo "== #[link=] in a format, and nolink closing it =="
$BIN -L "$ISOCK" set -g status-left '#[link=http://example.com/a]LINK#[nolink]PLAIN'
sleep 1
$TM capture-pane -p -e -t client | sed -n '24p' | cat -v | perl -pe 's/\s+$//'

echo "== the same URI twice shares one id; a different URI gets its own =="
# The ids are sequential per screen, so this also pins that no extra entry is
# allocated for the repeat.
$BIN -L "$ISOCK" set -g status-left '#[link=http://x]A#[link=http://x]B#[link=http://y]C'
sleep 1
$TM capture-pane -p -e -t client | sed -n '24p' | cat -v | perl -pe 's/\s+$//'

echo "== set-default moves the base, so a later pop-default lands on it =="
# Not a hyperlink, but the other directive that was rejected outright and is only
# observable once something draws it.
$BIN -L "$ISOCK" set -g status-left '#[fg=red]R#[set-default,fg=green]S#[pop-default]P'
sleep 1
$TM capture-pane -p -e -t client | sed -n '24p' | cat -v | perl -pe 's/\s+$//'

echo "== against push-default, which restores the SAVED style instead =="
$BIN -L "$ISOCK" set -g status-left '#[fg=red]R#[push-default,fg=green]S#[pop-default]P'
sleep 1
$TM capture-pane -p -e -t client | sed -n '24p' | cat -v | perl -pe 's/\s+$//'

$BIN -L "$ISOCK" kill-server 2>/dev/null
