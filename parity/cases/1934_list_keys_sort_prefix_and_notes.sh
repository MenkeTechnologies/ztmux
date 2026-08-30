# list-keys: -O picks the sort order (sort.c:357-377 spells the names, and "key"
# is a synonym for "index"), -r reverses it, -P supplies the prefix that
# #{key_prefix} expands to, -1 stops after one key and -N prints notes only, with
# -a including the keys that have none. The format vars are the ones
# cmd_list_keys_format_add_key_binding installs (cmd-list-keys.c:140-164), and a
# line that expands to nothing is not printed at all (cmd-list-keys.c:247).
$TM bind -T zt-list b display-message beta
$TM bind -T zt-list -N 'the note for a' a display-message alpha
$TM bind -T zt-list c display-message gamma
echo "== -O key =="
$TM list-keys -T zt-list -O key -F '#{key_string}'
echo "== -O key -r =="
$TM list-keys -T zt-list -O key -r -F '#{key_string}'
echo "== -O index is the same order as key =="
$TM list-keys -T zt-list -O index -F '#{key_string}'
echo "== an unknown order is an error =="
$TM list-keys -T zt-list -O nosuchorder 2>&1; echo "rc=$?"
echo "== -P supplies #{key_prefix} =="
$TM list-keys -T zt-list -O key -P 'PREFIX' -F '#{key_prefix} #{key_string} #{key_table}'
echo "== the key argument filters, and -1 stops after the first =="
$TM list-keys -T zt-list -1 -F '#{key_string} #{key_command}' a
$TM list-keys -T zt-list -1 -O key -F '#{key_string}'
echo "== a key that is bound nowhere in the table is an error =="
$TM list-keys -T zt-list -F '#{key_string}' z 2>&1; echo "rc=$?"
echo "== -N prints the note, and only the keys that have one =="
$TM list-keys -T zt-list -N -O key -F '#{key_string}: #{key_note}'
echo "== -N -a includes the keys with no note =="
$TM list-keys -T zt-list -N -a -O key -F '#{key_string}: [#{key_note}]'
echo "== an empty expansion prints no line at all =="
echo "lines: $($TM list-keys -T zt-list -O key -F '#{key_note}' | wc -l | tr -d ' ')"
