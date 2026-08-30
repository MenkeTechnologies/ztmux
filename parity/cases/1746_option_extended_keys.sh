# extended-keys is a three-way choice and extended-keys-format picks the escape
# form; xterm-keys is the flag they replaced.
$TM show -gv extended-keys
$TM show -gv extended-keys-format
$TM show -gv xterm-keys 2>&1; echo "rc=$?"
for v in off on always; do
  $TM set -g extended-keys "$v" >/dev/null 2>&1 && printf '%-7s %s\n' "$v" "$($TM show -gv extended-keys)"
done
$TM set -g extended-keys nonsense 2>&1; echo "rc=$?"
for v in csi-u xterm; do
  $TM set -g extended-keys-format "$v" >/dev/null 2>&1 && printf '%-7s %s\n' "$v" "$($TM show -gv extended-keys-format)"
done
$TM set -g extended-keys-format nonsense 2>&1; echo "rc=$?"
$TM set -gu extended-keys; $TM set -gu extended-keys-format
