# link-window puts one window in a second session. #{window_linked_sessions}
# counts SESSIONS holding the window -- one per session group plus each ungrouped
# session (format.c:2919) -- so linking a window twice into the SAME session
# still counts once, while the *_list format prints one entry per winlink.
# automatic-rename is off so no window name can drift into the output.
$TM set -g automatic-rename off
$TM new-session -d -s target -x 80 -y 24
$TM new-window -d -t 0: -n shared
show() { $TM list-windows -a -F '#{window_name} linked=#{window_linked} n=#{window_linked_sessions} [#{window_linked_sessions_list}]' | grep shared | sort -u; }
echo "== one session =="; show
$TM link-window -s 0:shared -t target:9
echo "== linked into a second session =="; show
$TM link-window -s 0:shared -t 0:8
echo "== plus a second link inside the same session =="; show
$TM unlink-window -t target:9
$TM unlink-window -t 0:8
echo "== after unlinking both =="; show
