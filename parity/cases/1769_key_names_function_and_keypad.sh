# Every name in key_string_table has to parse to a key and print back as the
# same name. The function keys, the editing keys and the keypad block are the
# ones nothing else in the suite names; bind/list-keys is the round trip.
for k in F1 F2 F3 F4 F5 F6 F7 F8 F9 F10 F11 F12; do
  $TM bind -T ztkeys "$k" display-message "$k" >/dev/null 2>&1 || echo "bind $k failed"
done
$TM list-keys -T ztkeys -F '#{key}' | sort
echo "== editing keys, including the aliases =="
for k in IC Insert DC Delete Home End NPage PageDown PgDn PPage PageUp PgUp; do
  printf '%-9s -> %s\n' "$k" "$($TM bind -T ztedit "$k" display-message x 2>&1 && $TM list-keys -T ztedit -F '#{key}' | tail -1)"
  $TM unbind -T ztedit -a 2>/dev/null
done
echo "== the keypad block =="
for k in KP0 KP1 KP2 KP3 KP4 KP5 KP6 KP7 KP8 KP9 KPEnter; do
  $TM bind -T ztkp "$k" display-message x >/dev/null 2>&1 || echo "bind $k failed"
done
$TM list-keys -T ztkp -F '#{key}' | sort
$TM unbind -T ztkeys -a; $TM unbind -T ztkp -a
