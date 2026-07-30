# display-popup's -w/-h/-x/-y take absolute cells, percentages or position
# keywords, all resolved through the same code the menu uses. The flag string
# itself changed recently (-k and -N were missing, and an internal flag had to
# move out of 0x4), so every documented form is exercised here: accepted ones
# reach the "no current client" error, rejected ones print usage.
for spec in "-w 10 -h 5" "-w 50% -h 50%" "-w 100% -h 100%" "-w 0 -h 0" \
            "-x 0 -y 0" "-x R -y P" "-x W -y S" "-x C -y C" \
            "-w 10 -h 5 -x 3 -y 4" "-B" "-C" "-E" "-EE" "-e FOO=bar" \
            "-d /tmp" "-s fg=red" "-S fg=blue" "-T title" "-b rounded"; do
  # shellcheck disable=SC2086
  echo "$spec => $($TM display-popup $spec 2>&1)"
done
# Malformed values and unknown flags.
for spec in "-w abc" "-h abc" "-x zz" "-y zz" "-w -5" "-Q"; do
  # shellcheck disable=SC2086
  echo "$spec => $($TM display-popup $spec 2>&1)"
done
# -C closes an open popup, which with no client is still not an error.
$TM display-popup -C 2>&1
