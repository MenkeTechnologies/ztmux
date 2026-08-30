# assume-paste-time, prefix-timeout and prompt-history-limit are numbers with
# their own ranges; default-size is a WIDTHxHEIGHT string; default-command and
# key-table are plain strings.
for o in assume-paste-time prefix-timeout prompt-history-limit default-size; do
  printf '%-22s %s\n' "$o" "$($TM show -gv "$o" 2>&1)"
done
printf '%-22s [%s]\n' default-command "$($TM show -gv default-command 2>&1)"
printf '%-22s [%s]\n' key-table "$($TM show -gv key-table 2>&1)"
echo "== setting them =="
$TM set -g assume-paste-time 5; $TM show -gv assume-paste-time
$TM set -g prefix-timeout 500; $TM show -gv prefix-timeout
$TM set -g prompt-history-limit 42; $TM show -gv prompt-history-limit
$TM set -g default-size 120x40; $TM show -gv default-size
$TM set -g key-table mytable; $TM show -gv key-table
echo "== and refusing bad values =="
$TM set -g assume-paste-time -1 2>&1; echo "rc=$?"
$TM set -g default-size notasize 2>&1; echo "rc=$?"
$TM set -gu assume-paste-time; $TM set -gu prefix-timeout; $TM set -gu prompt-history-limit
$TM set -gu default-size; $TM set -gu key-table
