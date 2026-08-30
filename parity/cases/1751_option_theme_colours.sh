# The theme options give the eight ANSI names a concrete colour per theme
# (options-table.c). They are colour-typed, so they round-trip through the
# colour parser and reject nonsense.
for o in dark-theme-black dark-theme-red dark-theme-green dark-theme-yellow \
         dark-theme-blue dark-theme-magenta dark-theme-cyan dark-theme-white \
         dark-theme-dark-grey dark-theme-light-grey; do
  printf '%-22s %s\n' "$o" "$($TM show -gv "$o" 2>&1)"
done
for o in light-theme-black light-theme-red light-theme-green light-theme-yellow \
         light-theme-blue light-theme-magenta light-theme-cyan light-theme-white \
         light-theme-dark-grey light-theme-light-grey; do
  printf '%-22s %s\n' "$o" "$($TM show -gv "$o" 2>&1)"
done
echo "== they take a colour and give it back =="
$TM set -g dark-theme-red '#ff0000'; $TM show -gv dark-theme-red
$TM set -g light-theme-blue colour33; $TM show -gv light-theme-blue
echo "== and refuse a non-colour =="
$TM set -g dark-theme-red 'notacolour' 2>&1; echo "rc=$?"
$TM set -gu dark-theme-red; $TM set -gu light-theme-blue
