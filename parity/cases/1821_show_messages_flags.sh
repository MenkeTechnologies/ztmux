# show-messages prints the server's message log; -J adds the running jobs and -T
# the terminal entries. The contents name pids, times and terminal names, so the
# case compares the SHAPE: which sections appear and that each line is of the
# expected form.
$TM display-message -p 'seed a message' >/dev/null
$TM show-messages | perl -pe 's/^.*$/MESSAGE-LINE/' | sort -u
echo "== -J: jobs =="
$TM show-messages -J | perl -pe 's/^job [^:]+: .*$/JOB-LINE/; s/^.+$/OTHER-LINE/ unless /^JOB-LINE$/' | sort -u
echo "== -T: terminals =="
$TM show-messages -T | perl -pe 's/^terminal \d+: .*$/TERMINAL-LINE/; s/^ .*$/DETAIL-LINE/; s/^(?!TERMINAL-LINE|DETAIL-LINE).+$/OTHER-LINE/' | sort -u
echo "== both together =="
$TM show-messages -JT >/dev/null; echo "rc=$?"
