# capture-pane -L prefixes each line with its number relative to the top of the
# screen, so history lines come out negative — a subtraction the C does once in
# u_int and once in int depending on which side of the history the line is on.
# -F prefixes the line's own grid flags, which is the only way to see the
# wrapped/extended/start-of-output bits from outside the server.
$TM new-window -d -n cf 'i=1; while [ $i -le 30 ]; do echo "row-$i"; i=$((i+1)); done; printf "%s\n" "$(i=0; while [ $i -lt 100 ]; do printf y; i=$((i+1)); done)"; sleep 300'
sleep 1
$TM display-message -p -t cf 'hist=#{history_size}'
echo "== -L over the screen/history boundary"
$TM capture-pane -pL -S -3 -E 3 -t cf | perl -pe "s{^(.*)\$}{[\$1]}"
echo "== -F"
$TM capture-pane -pF -S -3 -E 3 -t cf | perl -pe "s{^(.*)\$}{[\$1]}"
echo "== -LF together"
$TM capture-pane -pLF -S 0 -E 4 -t cf | perl -pe "s{^(.*)\$}{[\$1]}"
echo "== -F on the wrapped line"
$TM capture-pane -pF -S -2 -E -1 -t cf | perl -pe "s{^(.*)\$}{[\$1]}"
echo "== -LF with -J (joined lines get one number)"
$TM capture-pane -pLFJ -S -3 -E - -t cf | perl -pe "s{^(.*)\$}{[\$1]}" | head -8
echo "== -L with -e"
$TM capture-pane -pLe -S 0 -E 1 -t cf | perl -pe 's/\e/<ESC>/g' | perl -pe "s{^(.*)\$}{[\$1]}"
