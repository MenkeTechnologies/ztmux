# Expansion is one level deep: CMD_PARSE_NOALIAS is set while the alias body is
# built (cmd-parse.y:815-817), so an alias naming another alias is not expanded.
$TM set -s command-alias[70] 'zzouter=zzinner'
$TM set -s command-alias[71] 'zzinner=display-message -p inner'
$TM zzinner; echo "direct rc=$?"
$TM zzouter 2>&1; echo "via alias rc=$?"
