# Tab stops live in a per-pane bitmap that HTS sets, TBC clears and a reset
# restores to every eighth column; #{pane_tabs} prints it. A tab advances to
# the next stop, or to the last column when there is none left, and CBT walks
# it backwards — a scan in each direction over the same bitmap.
$TM new-window -d -n tabs 'printf "\0033[3g"; printf "\0033[1;5H\0033H\0033[1;12H\0033H"; printf "\0033[2;1HA\011B\011C\011D\n"; sleep 300'
sleep 1
$TM display-message -p -t tabs 'tabs=#{pane_tabs}'
$TM capture-pane -p -S 0 -E 3 -t tabs | perl -pe "s{^(.*)\$}{[\$1]}"
# Default stops, a tab past the last one, and a backwards tab.
$TM new-window -d -n tabs2 'printf "A\011B\011C\n"; i=0; while [ $i -lt 12 ]; do printf "\011"; i=$((i+1)); done; printf "END\n"; printf "\0033[3;40H\0033[2Zback\n"; sleep 300'
sleep 1
$TM display-message -p -t tabs2 'tabs=#{pane_tabs} cur=#{cursor_x},#{cursor_y}'
$TM capture-pane -p -S 0 -E 4 -t tabs2 | perl -pe "s{^(.*)\$}{[\$1]}"
