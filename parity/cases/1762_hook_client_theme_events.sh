# client-dark-theme and client-light-theme fire when a client reports which one
# its terminal is using; with no report they stay quiet and the option that
# records the answer keeps its default.
$TM set -g @log ''
$TM set-hook -g client-dark-theme 'set -ga @log ",dark"'
$TM set-hook -g client-light-theme 'set -ga @log ",light"'
echo "with no client: [$($TM show -gv @log)]"
$TM display-message -p 'theme option default: [#{client_theme}]'
$TM show -sv default-terminal
$TM set-hook -gu client-dark-theme
$TM set-hook -gu client-light-theme
