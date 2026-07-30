# display-menu shares its argument parser and its position code with
# display-popup, which recently gained -k/-N and a custom args parser. The menu
# takes its items as trailing arguments in groups of three, so arity errors,
# separator items and the -x/-y position keywords are all parsed by code that
# has no other observable output without a client: every accepted form stops at
# the same "no current client" error, and every rejected one prints usage.
$TM display-menu -T title 2>&1
$TM display-menu -T title one o 'list-windows' 2>&1
$TM display-menu -x 0 -y 0 -T t item i 'list-windows' 2>&1
$TM display-menu -x R -y P -T t item i 'list-windows' 2>&1
$TM display-menu -x W -y S -T t item i 'list-windows' 2>&1
$TM display-menu -x C -y C -T t item i 'list-windows' 2>&1
$TM display-menu -O -T t item i 'list-windows' 2>&1
$TM display-menu -M -T t item i 'list-windows' 2>&1
$TM display-menu -b rounded -T t item i 'list-windows' 2>&1
$TM display-menu -c '%1' -T t item i 'list-windows' 2>&1
$TM display-menu -H 'fg=red' -s 'fg=blue' -S 'fg=green' -T t item i 'list-windows' 2>&1
# A separator is an empty item name, and an item with no command is a dash.
$TM display-menu -T t '' '' '' item i 'list-windows' 2>&1
# Wrong arity: two trailing arguments instead of three.
$TM display-menu -T t item i 2>&1
# Unknown flag.
$TM display-menu -Q -T t item i 'list-windows' 2>&1
