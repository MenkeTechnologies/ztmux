# A key name may carry modifiers, and list-keys prints back the canonical
# spelling: the modifiers are ordered and the case of the base key is kept.
for k in C-a M-b C-M-c S-Up C-Left M-F1 'C-Space' 'M-Enter'; do
  $TM bind -T ztmod "$k" display-message x >/dev/null 2>&1 || echo "bind $k failed"
done
$TM list-keys -T ztmod -F '#{key}' | sort
echo "== an unknown modifier =="
$TM bind -T ztmod 'Q-a' display-message x 2>&1; echo "rc=$?"
echo "== a key that is only a modifier =="
$TM bind -T ztmod 'C-' display-message x 2>&1; echo "rc=$?"
$TM unbind -a -T ztmod
