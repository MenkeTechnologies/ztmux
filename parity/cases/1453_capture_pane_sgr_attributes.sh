# capture-pane -e re-emits the grid's cell attributes as SGR, so it is the only
# way to compare what the input parser actually stored per cell without a
# client. Attribute handling is the layer the recent redraw bugs sat on top of:
# if a cell's attribute set is wrong here, every drawing path downstream of it
# is wrong too, and a pure-text capture cannot see it.
$TM new-window -d -n sgr 'printf "\0033[1mbold\0033[0m \0033[3mital\0033[0m \0033[4munder\0033[0m\n\0033[31mred\0033[0m \0033[42mgreenbg\0033[0m \0033[7mrev\0033[0m\n\0033[38;5;99m256\0033[0m \0033[38;2;10;20;30mtruecolour\0033[0m\n\0033[1;4;31mcombined\0033[0m\n\0033[9mstrike\0033[0m \0033[2mdim\0033[0m \0033[5mblink\0033[0m\n"; sleep 300'
sleep 1
$TM capture-pane -p -t sgr | perl -pe "s{^(.*)\$}{[\$1]}"
echo "--- with escapes:"
$TM capture-pane -pe -t sgr | perl -pe 's/\e/<ESC>/g' | perl -pe "s{^(.*)\$}{[\$1]}"
