# Window lifecycle hooks firing: window-linked / window-unlinked /
# window-renamed / session-window-changed, plus the after-* command hooks for
# new-window, rename-window and select-window.
#
# These come from three different C sites and a demo hits all of them:
#   session_link_window()   -> notify_session_window("window-linked")   session.c:333
#   session_unlink_window() -> notify_session_window("window-unlinked") session.c:349
#   window_set_name()       -> notify_window("window-renamed")          window.c
#   session_set_current()   -> notify_session("session-window-changed") session.c:496
#   cmd-new-window.c:163 / cmd-select-window.c:125 / cmd-queue.c:662 for after-*.
#
# window-renamed is a WINDOW-scope hook (OPTIONS_TABLE_WINDOW_HOOK,
# options-table.c:1946), so it also pins that notify_insert_hook() falls through
# session options to the window's options tree (notify.c:79-86) -- a lookup order
# a port can easily get wrong and still pass every show-hooks test.
set -u

# automatic-rename would rename a "sleep 300" window on its own and fire
# window-renamed at an unpredictable moment; every window below is named
# explicitly as well.
$TM set -g automatic-rename off

w() {
  local i=0 got=''
  while [ $i -lt 60 ]; do
    got=$($TM show-options -gqv "$1" 2>/dev/null)
    [ "$got" = "$2" ] && break
    i=$((i+1)); sleep 0.05
  done
  printf '%s\n' "$got"
}

$TM set -g @wlog ''
$TM set -g @alog ''
$TM set-hook -g window-linked   'set -gF @wlog "#{@wlog}<#{hook}=#{hook_window_name}/#{hook_window}@#{hook_session_name}>"'
$TM set-hook -g window-unlinked 'set -gF @wlog "#{@wlog}<#{hook}=#{hook_window_name}@#{hook_session_name}>"'
$TM set-hook -g window-renamed  'set -gF @wlog "#{@wlog}<#{hook}=#{hook_window_name}/#{hook_window}>"'
$TM set-hook -g session-window-changed 'set -gF @wlog "#{@wlog}<#{hook}=#{hook_session_name}:#{session_name}>"'
$TM set-hook -g after-new-window    'set -gF @alog "#{@alog}<#{hook}:#{window_name}>"'
$TM set-hook -g after-rename-window 'set -gF @alog "#{@alog}<#{hook}:#{window_name}>"'
$TM set-hook -g after-select-window 'set -gF @alog "#{@alog}<#{hook}:#{window_name}>"'

$TM new-session -d -s host -n w0 -x 80 -y 24 'sleep 300'
$TM new-window -d -t host -n w1 'sleep 300'
echo "new:      $(w @wlog '<window-linked=w0/@1@host><window-linked=w1/@2@host>')"
echo "after:    $(w @alog '<after-new-window:w1>')"

$TM rename-window -t host:w1 renamed1
echo "renamed:  $(w @wlog '<window-linked=w0/@1@host><window-linked=w1/@2@host><window-renamed=renamed1/@2>')"
echo "after:    $(w @alog '<after-new-window:w1><after-rename-window:renamed1>')"

# select-window moves the session's current window: session-window-changed
# (notify_session) and after-select-window (cmdq_insert_hook) are two separate
# delivery paths, kept in two separate options so their interleaving is moot.
$TM select-window -t host:renamed1
echo "select:   $(w @wlog '<window-linked=w0/@1@host><window-linked=w1/@2@host><window-renamed=renamed1/@2><session-window-changed=host:host>')"
echo "after:    $(w @alog '<after-new-window:w1><after-rename-window:renamed1><after-select-window:renamed1>')"

# link-window into a second session, then unlink it again.
$TM new-session -d -s guest -n g0 -x 80 -y 24 'sleep 300'
$TM link-window -d -s host:renamed1 -t guest:9
echo "linked:   $(w @wlog '<window-linked=w0/@1@host><window-linked=w1/@2@host><window-renamed=renamed1/@2><session-window-changed=host:host><window-linked=g0/@3@guest><window-linked=renamed1/@2@guest>')"

$TM unlink-window -t guest:9
echo "unlinked: $(w @wlog '<window-linked=w0/@1@host><window-linked=w1/@2@host><window-renamed=renamed1/@2><session-window-changed=host:host><window-linked=g0/@3@guest><window-linked=renamed1/@2@guest><window-unlinked=renamed1@guest>')"

# A window-scope hook set on ONE window must not fire for its sibling.
$TM set -g @wlog ''
$TM set-hook -gu window-renamed
$TM set-hook -t host:w0 window-renamed 'set -gF @wlog "#{@wlog}[only-w0:#{hook_window_name}]"'
$TM rename-window -t host:renamed1 nope
$TM rename-window -t host:w0 yes
echo "perwin:   $(w @wlog '[only-w0:yes]')"
