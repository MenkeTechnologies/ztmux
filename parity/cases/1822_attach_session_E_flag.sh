# attach-session -E skips applying update-environment to the session, so a
# variable the client has does not overwrite the session's copy. Driven with a
# real client through the nested-client technique.
set -- $TM
BIN="$1"
ISOCK="ase_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 40 -y 6 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner set -g update-environment 'ZTPAR_UPD' >/dev/null
inner set-environment -t inner ZTPAR_UPD from-session >/dev/null

$TM new-window -d -n plain "$BIN -L $ISOCK attach -t inner"
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{session_attached}')" = 1 ] && break
  sleep 0.2
done
# If the client did not attach in time -- a slow or loaded machine, not a
# divergence -- say so once and stop, so both binaries produce the same short
# output instead of diverging further down error paths.
[ "$(inner display-message -p '#{session_attached}')" = 1 ] || {
  echo "client did not attach"; inner kill-server >/dev/null 2>&1; exit 0; }
echo "after a plain attach: [$(inner show-environment -t inner ZTPAR_UPD 2>&1)]"
inner detach-client -s inner >/dev/null
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{session_attached}')" = 0 ] && break
  sleep 0.2
done
inner set-environment -t inner ZTPAR_UPD from-session >/dev/null
$TM new-window -d -n withE "$BIN -L $ISOCK attach -E -t inner"
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{session_attached}')" = 1 ] && break
  sleep 0.2
done
echo "after attach -E:      [$(inner show-environment -t inner ZTPAR_UPD 2>&1)]"
inner kill-server >/dev/null 2>&1
$TM kill-window -t plain 2>/dev/null; $TM kill-window -t withE 2>/dev/null
