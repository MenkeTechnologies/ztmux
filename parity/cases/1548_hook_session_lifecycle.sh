# Session lifecycle hooks actually FIRING, not just being stored.
#
# Cases 1181/1387 only prove `set-hook` + `show-hooks` round-trip a string. That
# leaves the whole notify path uncovered: notify_session() -> notify_add()
# (notify.c:267,179) building the hook_* format tree, notify_insert_hook()
# (notify.c:57) resolving which options tree owns the hook, and
# cmdq_insert_hook() (cmd-queue.c:353) for the after-* command hooks. A demo
# fires these constantly -- every new-session/rename-session/kill-session.
#
# Each hook writes into a *different* global user option so nothing here depends
# on the relative ordering of the two DIFFERENT delivery mechanisms:
#   - session-created / -renamed / -closed go through cmdq_append(NULL, ...) on
#     the global queue (notify.c:230), and
#   - after-new-session / after-rename-session are inserted directly after the
#     running item (cmd-new-session.c:362, cmd-queue.c:662).
# Ordering WITHIN one option is just the order of the commands below, which is
# sequential and deterministic.
#
# Nothing here is time-, pid-, tty- or host-derived: session ids, session names
# and the literal hook name are the only things printed.
set -u

# Wait for the async (global-queue) hook to land instead of sleeping blind: the
# client command returns before the notify callback runs, and under suite load a
# fixed sleep races it. Prints whatever the option ACTUALLY holds, so a port that
# never fires the hook prints its empty/stale value and diffs.
w() {
  local i=0 got=''
  while [ $i -lt 60 ]; do
    got=$($TM show-options -gqv "$1" 2>/dev/null)
    [ "$got" = "$2" ] && break
    i=$((i+1)); sleep 0.05
  done
  printf '%s\n' "$got"
}

$TM set -g @slog ''
$TM set -g @alog ''
$TM set-hook -g session-created 'set -gF @slog "#{@slog}<#{hook}=#{hook_session_name}/#{hook_session}>"'
$TM set-hook -g session-renamed 'set -gF @slog "#{@slog}<#{hook}=#{hook_session_name}>"'
$TM set-hook -g session-closed  'set -gF @slog "#{@slog}<#{hook}=#{hook_session_name}>"'
$TM set-hook -g after-new-session    'set -gF @alog "#{@alog}<#{hook}:#{session_name}>"'
$TM set-hook -g after-rename-session 'set -gF @alog "#{@alog}<#{hook}:#{session_name}>"'

$TM new-session -d -s alpha -x 80 -y 24 'sleep 300'
echo "created:   $(w @slog '<session-created=alpha/$1>')"
echo "after-new: $(w @alog '<after-new-session:alpha>')"

$TM rename-session -t alpha beta
echo "renamed:   $(w @slog '<session-created=alpha/$1><session-renamed=beta>')"
echo "after-ren: $(w @alog '<after-new-session:alpha><after-rename-session:beta>')"

$TM kill-session -t beta
echo "closed:    $(w @slog '<session-created=alpha/$1><session-renamed=beta><session-closed=beta>')"

# A hook set on a SESSION overrides the global one for that session
# (notify_insert_hook walks fs.s->options before global_s_options, notify.c:74).
$TM new-session -d -s gamma -x 80 -y 24 'sleep 300'
$TM set-hook -t gamma session-renamed 'set -gF @slog "#{@slog}[local:#{hook_session_name}]"'
$TM rename-session -t gamma delta
echo "local:     $(w @slog '<session-created=alpha/$1><session-renamed=beta><session-closed=beta><session-created=gamma/$2>[local:delta]')"

# Unsetting the hook stops it firing at all. To prove that with no blind sleep,
# rename, then create a session -- session-created is still hooked, and the
# global notify queue is FIFO (notify.c:230), so if session-renamed had wrongly
# stayed live its entry would already be in @slog ahead of the zeta entry.
$TM set-hook -gu session-renamed
$TM set-hook -t delta -u session-renamed
$TM rename-session -t delta epsilon
$TM new-session -d -s zeta -x 80 -y 24 'sleep 300'
echo "unset:     $(w @slog '<session-created=alpha/$1><session-renamed=beta><session-closed=beta><session-created=gamma/$2>[local:delta]<session-created=zeta/$3>')"
