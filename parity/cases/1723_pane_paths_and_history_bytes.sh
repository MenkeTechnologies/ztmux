# The path formats read back the directory the pane was started in, and
# #{history_all_bytes} counts what every pane's history holds. Both binaries run
# from the same working directory, so the paths compare.
d=$(mktemp -d)
$TM new-window -d -n paths -c "$d" 'sleep 300'
$TM list-panes -t paths -F 'start=[#{pane_start_path}] current=[#{pane_current_path}]' | perl -pe "s{\Q$d\E}{DIR}g" | perl -pe 's{^(.*?)/private(DIR)}{$1$2}'
echo "== history on a fresh pane =="
$TM display-message -p -t paths 'bytes=#{history_bytes} all=#{history_all_bytes} size=#{history_size}'
echo "== session path =="
$TM display-message -p '[#{session_path}]' | perl -pe 's{\Q'"$PWD"'\E}{CWD}'
command rm -rf "$d"
