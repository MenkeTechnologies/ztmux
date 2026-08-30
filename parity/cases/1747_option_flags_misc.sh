# scroll-on-clear, detach-on-destroy, visual-silence and exit-unattached: two
# flags and two choices, each with its default, its accepted values and its
# rejection of anything else. exit-unattached goes last on purpose: turning it
# on with no client attached ends the server, which is the behaviour worth
# pinning, and the socket path in the message is stripped because it names the
# binary.
strip() { perl -pe 's{ on /\S+}{ on SOCKET}'; }
printf 'scroll-on-clear   %s\n' "$($TM show -gwv scroll-on-clear 2>&1)"
printf 'detach-on-destroy %s\n' "$($TM show -gv detach-on-destroy 2>&1)"
printf 'visual-silence    %s\n' "$($TM show -gv visual-silence 2>&1)"
printf 'exit-unattached   %s\n' "$($TM show -sv exit-unattached 2>&1)"
$TM setw -g scroll-on-clear off; $TM show -gwv scroll-on-clear
for v in off on both; do
  $TM set -g visual-silence "$v" >/dev/null 2>&1 && printf 'visual-silence %-5s %s\n' "$v" "$($TM show -gv visual-silence)"
done
$TM set -g visual-silence nonsense 2>&1; echo "rc=$?"
for v in off on no-detached previous next; do
  $TM set -g detach-on-destroy "$v" >/dev/null 2>&1 && printf 'detach-on-destroy %-12s %s\n' "$v" "$($TM show -gv detach-on-destroy)"
done
$TM set -g detach-on-destroy nonsense 2>&1; echo "rc=$?"
$TM setw -gu scroll-on-clear; $TM set -gu visual-silence; $TM set -gu detach-on-destroy
echo "== exit-unattached on, with nothing attached =="
$TM set -s exit-unattached on 2>&1 | strip; echo "rc=${PIPESTATUS[0]}"
$TM show -sv exit-unattached 2>&1 | strip; echo "rc=${PIPESTATUS[0]}"
