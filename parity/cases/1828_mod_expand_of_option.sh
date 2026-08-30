# #{E:...} expands the value of what it names a second time, so an option whose
# value is itself a format resolves; #{T:...} does the same with the time
# formats applied. Without them the stored text comes back as-is.
$TM set -g @ztpar_fmt '#{session_windows} windows'
echo "raw:       [$($TM display-message -p '#{@ztpar_fmt}')]"
echo "expanded:  [$($TM display-message -p '#{E:@ztpar_fmt}')]"
$TM set -g @ztpar_nested '#{E:@ztpar_fmt}'
echo "nested:    [$($TM display-message -p '#{E:@ztpar_nested}')]"
echo "expandtime:[$($TM display-message -p '#{T:@ztpar_fmt}')]"
$TM set -gu @ztpar_fmt; $TM set -gu @ztpar_nested
