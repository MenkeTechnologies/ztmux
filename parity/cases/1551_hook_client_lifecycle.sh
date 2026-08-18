# Client hooks and the pane focus hooks -- the ones a demo fires every time
# somebody attaches, switches session or moves between panes, and the ones no
# server-side case can reach: client-attached/-detached/-session-changed need a
# REAL client (cmd-attach-session.c:166, server-client.c:432,467), and
# pane-focus-in/-out only fire while some client has CLIENT_FOCUSED and is
# showing that window (window_pane_update_focus, window.c:517-550).
#
# Same nesting shape as cases 1504/1507/1508: a second server inside a pane of
# the harness's server, with a client attached to it. Unlike those, this one
# does not screen-scrape -- the hooks write into the INNER server's global user
# options and we read them straight back, so nothing here depends on drawing.
#
# hook_client / client_name are the client's tty path, which is whatever pty the
# OS handed out, so they are scrubbed. Everything else printed is a hook name, a
# session name or a pane id.
set -u

set -- $TM
BIN="$1"
ISOCK="hcl_$$_inner"
IT="$BIN -L $ISOCK"

scrub() { perl -pe 's{/dev/tty[a-z0-9]+}{TTY}g'; }

$IT -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$IT new-session -d -s beta -n solo -x 80 -y 24 'sleep 300'
# A clock in the status line is not deterministic, and ztmux's floating overlay
# is an intentional extension that tmux knows nothing about; neither is under
# test here.
$IT set -g status-right ''
$IT set -g status-interval 0
$IT set -g @ztmux-ratatui off
$IT set -g automatic-rename off
$IT split-window -d -t alpha:one 'sleep 300'

$IT set -g @clog ''
$IT set-hook -g client-attached        'set -gF @clog "#{@clog}<#{hook}:#{hook_client}:#{session_name}>"'
$IT set-hook -g client-session-changed 'set -gF @clog "#{@clog}<#{hook}:#{session_name}>"'
$IT set-hook -g client-detached        'set -gF @clog "#{@clog}<#{hook}>"'
$IT set-hook -g pane-focus-in          'set -gF @clog "#{@clog}<#{hook}:#{pane_id}>"'
$IT set-hook -g pane-focus-out         'set -gF @clog "#{@clog}<#{hook}:#{pane_id}>"'

# Poll the inner server's option instead of sleeping: the attach, the switch and
# the detach all deliver their hooks asynchronously on the global notify queue
# (notify.c:230) and under suite load a fixed sleep races them. The ACTUAL value
# is printed, so a port that never fires a hook prints its stale value and diffs.
# The tty path is scrubbed BEFORE comparing, so the expected strings below can
# name TTY directly.
w() {
  local i=0 got=''
  while [ $i -lt 80 ]; do
    got=$($IT show-options -gqv @clog 2>/dev/null | scrub)
    [ "$got" = "$1" ] && break
    i=$((i+1)); sleep 0.05
  done
  printf '%s\n' "$got"
}

# Attaching runs server_client_set_session() before cmd-attach-session's own
# notify, so the focus-in for the session's active pane and client-session-changed
# both land BEFORE client-attached. That ordering is the point of this line.
$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"
echo "attach:  $(w '<pane-focus-in:%0><client-session-changed:alpha><client-attached:TTY:alpha>')"

# Moving the active pane while attached does NOT by itself re-run the focus
# update -- window_set_active_pane's notify path does not call
# window_pane_update_focus; the pending focus change is only reconciled the next
# time the client's session changes. Pinning the absence is as valuable as
# pinning a fire.
$IT select-pane -t alpha:one.1
$IT switch-client -t beta
echo "switch:  $(w '<pane-focus-in:%0><client-session-changed:alpha><client-attached:TTY:alpha><pane-focus-in:%1><client-session-changed:beta>')"

$IT detach-client -s beta
echo "detach:  $(w '<pane-focus-in:%0><client-session-changed:alpha><client-attached:TTY:alpha><pane-focus-in:%1><client-session-changed:beta><pane-focus-out:%1><client-detached>')"
echo "clients: [$($IT list-clients -F '#{client_name}' | scrub | tr '\n' ' ')]"

$IT kill-server 2>/dev/null
