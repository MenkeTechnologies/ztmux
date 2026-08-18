# Pane hooks and the three alert hooks, driven by real pane output.
#
# pane-exited/pane-died come from server_destroy_pane() (server-fn.c:354,377) and
# are gated on remain-on-exit; alert-bell/-activity/-silence come from
# alerts_check_bell/_activity/_silence (alerts.c:208,244,280) via the alerts
# timer, gated on monitor-* and on {bell,activity,silence}-action
# (alerts_action_applies, alerts.c:70). None of that is reachable by inspecting
# options -- it needs a pane that actually exits and a pane that actually writes
# a BEL -- so the whole path was uncovered.
#
# NOTE for whoever extends this: #{hook_pane}, #{hook_window} and
# #{hook_window_name} are deliberately NOT used in the pane hooks below. They
# diverge today (ztmux prints "%%2" for hook_pane and leaves hook_window/
# hook_window_name empty on pane notifications, where tmux prints "%2"/"@1"/
# "w0" -- notify.c:210-213). The cmd_find-derived formats used here (#{pane_id},
# #{pane_index}, #{window_name}, #{session_name}) agree, and they still prove the
# hook fired against the right target.
set -u

# automatic-rename would rename these windows behind our back and perturb the
# window names the hooks print.
$TM set -g automatic-rename off

w() {
  local i=0 got=''
  while [ $i -lt 80 ]; do
    got=$($TM show-options -gqv "$1" 2>/dev/null)
    [ "$got" = "$2" ] && break
    i=$((i+1)); sleep 0.05
  done
  printf '%s\n' "$got"
}

##### pane-exited / pane-died #################################################
$TM set -g @plog ''
$TM set-hook -g pane-exited 'set -gF @plog "#{@plog}<#{hook}=#{pane_id}/#{pane_index}@#{window_name}/#{session_name}>"'
$TM set-hook -g pane-died   'set -gF @plog "#{@plog}<#{hook}=#{pane_id}/#{pane_index}@#{window_name}/#{session_name}>"'

$TM new-session -d -s host -n w0 -x 80 -y 24 'sleep 300'
# remain-on-exit off (the default): the pane is torn down and pane-exited fires.
# By the time the hook runs the pane is already unlinked, so the hook's target
# resolves to the window's remaining active pane -- that fallback is part of what
# is being pinned here.
$TM split-window -d -t host:w0 'true'
echo "exited: $(w @plog '<pane-exited=%1/0@w0/host>')"

# remain-on-exit on: pane-died instead, the pane survives as a dead pane.
$TM set -wg remain-on-exit on
$TM split-window -d -t host:w0 'true'
echo "died:   $(w @plog '<pane-exited=%1/0@w0/host><pane-died=%3/1@w0/host>')"
echo "panes:  $($TM list-panes -t host:w0 -F '#{pane_id}#{?pane_dead,D,L}' | tr '\n' ' ')"
$TM set -wg remain-on-exit off

##### pane-set-clipboard ######################################################
# Fires from input_osc_52() (input.c:3292) when a pane emits OSC 52, and only
# when set-clipboard is "on" (=2, input.c:3231) -- so this also pins the gate.
$TM set -g @clog ''
$TM set-hook -g pane-set-clipboard 'set -gF @clog "#{@clog}<#{hook}=#{pane_id}>"'

# Wait for the pane to have actually emitted the sequence before flipping the
# option, otherwise the flip races the pane's first write.
wait_marker() {
  local i=0
  while [ $i -lt 80 ]; do
    case "$($TM capture-pane -p -t "$1" 2>/dev/null)" in *"$2"*) return 0;; esac
    i=$((i+1)); sleep 0.05
  done
  echo "wait_marker: timed out on $1/$2"
}

$TM set -g set-clipboard external
$TM new-window -d -t host -n clip1 "printf '\033]52;c;bm90aGVyZQ==\a'; printf MARK1; sleep 300"
wait_marker host:clip1 MARK1
$TM set -g set-clipboard on
$TM new-window -d -t host -n clip2 "printf '\033]52;c;aGVsbG8=\a'; printf MARK2; sleep 300"
echo "clip:   $(w @clog '<pane-set-clipboard=%5>')"
echo "buffer: $($TM show-buffer 2>&1)"

##### alerts ##################################################################
$TM set -g @alog ''
$TM set-hook -g alert-bell     'set -gF @alog "#{@alog}<bell=#{hook_window_name}/#{hook_session_name}/#{window_index}>"'
$TM set-hook -g alert-activity 'set -gF @alog "#{@alog}<activity=#{hook_window_name}>"'
$TM set-hook -g alert-silence  'set -gF @alog "#{@alog}<silence=#{hook_window_name}>"'
$TM set -wg monitor-bell on
$TM set -wg monitor-activity on
$TM set -g bell-action any
# activity-action none must suppress alert-activity even though monitor-activity
# is on and every window below writes output (alerts.c:82, ALERT_NONE).
$TM set -g activity-action none

# Three BELs in one write must produce exactly ONE alert-bell: alerts_check_bell
# clears WINDOW_BELL for the whole batch (alerts.c:64) so the window is checked
# once.
$TM new-window -d -t host -n noisy 'printf "X\a\a\a"; sleep 300'
echo "bell1:  $(w @alog '<bell=noisy/host/3>')"
# Second window with a single BEL: the queue is FIFO, so if the first window had
# double-fired its extra entry would already sit between these two.
$TM new-window -d -t host -n noisy2 'printf "\a"; sleep 300'
echo "bell2:  $(w @alog '<bell=noisy/host/3><bell=noisy2/host/4>')"

# monitor-silence on ONE window only: the silence timer expires and
# alerts_check_silence fires once for that winlink (alerts.c:280).
$TM set -g silence-action any
$TM set -w -t host:w0 monitor-silence 1
echo "silence: $(w @alog '<bell=noisy/host/3><bell=noisy2/host/4><silence=w0>')"
echo "flags:  $($TM list-windows -t host -F '#{window_name}:#{window_activity_flag}#{window_bell_flag}#{window_silence_flag}' | tr '\n' ' ')"
