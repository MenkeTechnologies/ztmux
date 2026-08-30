# list-keys -1 prints one key, -P sets the prefix string and -F the format.
$TM bind -T lktest c set -g @c 1
$TM list-keys -T lktest -F '#{key_table}:#{key}:#{command}'
$TM list-keys -1 -T lktest c
$TM list-keys -T lktest -P 'PFX ' 
$TM list-keys -T nosuchtable 2>&1; echo "rc=$?"
