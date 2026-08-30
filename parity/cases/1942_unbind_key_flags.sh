# unbind-key: -a clears a whole table, -T names the table, -q swallows the error
# for a key that is not bound, and -n is the root table (cmd-unbind-key.c).
$TM bind -T zt-table X display-message one
$TM bind -T zt-table Y display-message two
echo "table has: $($TM list-keys -T zt-table | wc -l | tr -d ' ')"
$TM unbind -T zt-table X; echo "unbind one rc=$?"
echo "table now has: $($TM list-keys -T zt-table | wc -l | tr -d ' ')"
$TM unbind -a -T zt-table; echo "unbind -a rc=$?"
echo "after -a: [$($TM list-keys -T zt-table 2>&1)]"
echo "== a key that is not bound is not an error: key_bindings_remove is silent =="
$TM bind -T zt-quiet A display-message keep-the-table-alive
$TM unbind -T zt-quiet Z 2>&1; echo "rc=$?"
echo "== the four error paths, each silenced by -q =="
$TM unbind -T zt-quiet nosuchkeyname 2>&1; echo "unknown key rc=$?"
$TM unbind -q -T zt-quiet nosuchkeyname; echo "with -q rc=$?"
$TM unbind -T nosuchtable A 2>&1; echo "unknown table rc=$?"
$TM unbind -q -T nosuchtable A; echo "with -q rc=$?"
$TM unbind -a -T zt-quiet A 2>&1; echo "key given with -a rc=$?"
$TM unbind -q -a -T zt-quiet A; echo "with -q rc=$?"
$TM unbind 2>&1; echo "missing key rc=$?"
$TM unbind -q; echo "with -q rc=$?"
echo "== -a with no -T is the prefix table, and -a -n is root =="
$TM unbind -a -T nosuchtable 2>&1; echo "rc=$?"
echo "== -n is the root table =="
$TM bind -n F12 display-message root-binding
echo "root: $($TM list-keys -T root | grep -c ' F12 ')"
$TM unbind -n F12; echo "rc=$?"
echo "root now: $($TM list-keys -T root | grep -c ' F12 ')"
