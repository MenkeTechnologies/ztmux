# command-alias entries add command names. The alias text is parsed as a command
# list and the caller's remaining arguments are appended to its LAST command
# (cmd_parse_expand_alias, cmd-parse.y:810-812) -- there is no %N substitution.
$TM set -s command-alias[50] 'zzgreet=display-message -p'
$TM zzgreet hello; echo "rc=$?"
$TM set -s command-alias[51] 'zzboth=set -g @a one ; set -g @b'
$TM zzboth two; echo "rc=$?"
echo "a=$($TM show -gv @a) b=$($TM show -gv @b)"
$TM show -s command-alias[50]
