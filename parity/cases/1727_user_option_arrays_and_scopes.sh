# A user option (@name) can be set at every scope and read back with the
# matching flag; the pane value shadows the window one, which shadows the
# session one, which shadows the global one.
$TM new-window -d -n scoped
$TM split-window -d -t scoped
$TM set -g @u global
$TM set @u session
$TM setw -t scoped @u window
$TM set -p -t scoped.1 @u pane
printf 'global:  %s\n' "$($TM show -gv @u)"
printf 'session: %s\n' "$($TM show -v @u)"
printf 'window:  %s\n' "$($TM show -wv -t scoped @u)"
printf 'pane:    %s\n' "$($TM show -pv -t scoped.1 @u)"
echo "== as formats, per pane =="
$TM list-panes -t scoped -F '#{pane_index}:#{@u}'
$TM set -pu -t scoped.1 @u
$TM list-panes -t scoped -F '#{pane_index}:#{@u}'
