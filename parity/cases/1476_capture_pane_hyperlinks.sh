# capture-pane -H prints the hyperlink URIs on each line instead of its text.
# The link ids are stored out of line in a per-screen table, and only lines
# carrying the hyperlink grid-line flag are scanned, so this covers both the
# flag being set when a linked cell is written and the per-line de-duplication:
# a run of cells sharing one id yields one URI, two links on a line yield two,
# and a line with none is skipped entirely rather than emitting a blank.
$TM new-window -d -n hl 'printf "\0033]8;;https://example.com/one\0033\0134first\0033]8;;\0033\0134 plain \0033]8;;https://example.com/two\0033\0134second\0033]8;;\0033\0134\n"; printf "no links here\n"; printf "\0033]8;id=xyz;https://example.com/three\0033\0134third\0033]8;;\0033\0134\n"; printf "\0033]8;;https://example.com/one\0033\0134again\0033]8;;\0033\0134\n"; sleep 300'
sleep 1
echo "== text"
$TM capture-pane -p -S 0 -E 3 -t hl | perl -pe "s{^(.*)\$}{[\$1]}"
echo "== -H"
$TM capture-pane -pH -S 0 -E 3 -t hl | perl -pe "s{^(.*)\$}{[\$1]}"
echo "== -H over the whole history"
$TM capture-pane -pH -S - -E - -t hl | perl -pe "s{^(.*)\$}{[\$1]}"
echo "== -HF (the H flag marks the lines that carry links)"
$TM capture-pane -pF -S 0 -E 3 -t hl | perl -pe "s{^(.*)\$}{[\$1]}"
echo "== -e keeps the OSC 8 sequences in the text capture"
$TM capture-pane -pe -S 0 -E 0 -t hl | perl -pe 's/\e/<ESC>/g' | perl -pe "s{^(.*)\$}{[\$1]}"
