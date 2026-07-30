# A scroll region changes where new lines come from and where scrolled-off ones
# go: only lines pushed out of the TOP of a full-height region enter the
# history. Getting that wrong shows up as history_size drifting from the
# reference, and as content appearing in capture-pane's scrollback that should
# never have been saved.
$TM new-window -d -n scrl 'printf "\0033[5;10r"; i=1; while [ $i -le 20 ]; do printf "\0033[10;1Hline-%s\n" "$i"; i=$((i+1)); done; sleep 300'
sleep 1
$TM display-message -p -t scrl 'hist=#{history_size} region=#{scroll_region_upper},#{scroll_region_lower}'
$TM capture-pane -p -S - -E - -t scrl | perl -pe "s{^(.*)\$}{[\$1]}" | head -30
# A full-height region does push to history; reverse index at the top pulls a
# blank line in instead.
$TM new-window -d -n ri 'i=1; while [ $i -le 30 ]; do echo "h-$i"; i=$((i+1)); done; printf "\0033[1;1H\0033M\0033M"; sleep 300'
sleep 1
$TM display-message -p -t ri 'hist=#{history_size}'
$TM capture-pane -p -S 0 -E 3 -t ri | perl -pe "s{^(.*)\$}{[\$1]}"
