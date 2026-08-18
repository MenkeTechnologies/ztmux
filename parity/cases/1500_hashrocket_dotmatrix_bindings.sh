# hashrocket/dotmatrix .tmux.conf, sourced as a file -- the bindings it sets.
#
# Cases 1497/1498 pin the DEFAULT binding table. This pins a real user config
# layered on top of it, which is different code: unbinding the prefix key,
# rebinding keys the defaults already own, binding into copy-mode-vi and root,
# a `\;` command chain inside one binding (only the config lexer produces that
# -- passed as argv, `;` ends the bind and starts a new command), and a
# run-shell argument that has to survive quoting intact.
#
# What is compared is what each binary PARSED, read back one key at a time, not
# the source text.
d=$(mktemp -d)
cat >"$d/hr.conf" <<'CONF'
unbind C-b
set -g prefix C-z

bind z send-keys C-z
bind C-z last-window

bind | split-window -h

bind h select-pane -L
bind j select-pane -D
bind k select-pane -U
bind l select-pane -R
bind ` select-window -t 0

# Search for previous error
bind-key e copy-mode \; send-keys "?Error" C-m

# Use up and down arrows for temporary "maximize"
unbind Up; bind Up resize-pane -Z; unbind Down; bind Down resize-pane -Z

# Copy/paste interop
bind C-c run "tmux show-buffer | reattach-to-user-namespace pbcopy"
bind C-v run "reattach-to-user-namespace pbpaste | tmux load-buffer - && tmux paste-buffer"

bind -T copy-mode-vi y send-keys -X copy-pipe-and-cancel 'reattach-to-user-namespace pbcopy'
bind -T copy-mode-vi v send-keys -X begin-selection
bind -T copy-mode-vi V send-keys -X rectangle-toggle

# Clear screen/history
bind-key C-k send-keys -R \; clear-history
bind-key C-l send-keys -R

bind-key -T root WheelUpPane if-shell -F -t = "#{alternate_on}" "send-keys -M" "select-pane -t =; copy-mode -e; send-keys -M"
bind-key -T root WheelDownPane if-shell -F -t = "#{alternate_on}" "send-keys -M" "select-pane -t =; send-keys -M"
bind-key -T copy-mode-vi WheelUpPane send-keys -X halfpage-up
bind-key -T copy-mode-vi WheelDownPane send-keys -X halfpage-down
CONF
$TM source-file "$d/hr.conf"

# C-b must be gone from the prefix table, and asking for it must say so.
$TM list-keys -T prefix C-b
echo "unbound rc=$?"

for k in z C-z '|' h j k l '`' e Up Down C-c C-v C-k C-l; do
  $TM list-keys -T prefix "$k"
done
for k in y v V WheelUpPane WheelDownPane; do
  $TM list-keys -T copy-mode-vi "$k"
done
$TM list-keys -T root WheelUpPane
$TM list-keys -T root WheelDownPane

# Rebinding an already-bound key replaces it; it does not stack a second entry.
$TM list-keys -T prefix | grep -c -- '-T prefix h '
rm -rf "$d"
