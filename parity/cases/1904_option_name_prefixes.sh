# Option names may be abbreviated to a unique prefix, and an ambiguous one is an
# error naming the candidates.
$TM set -g status-interval 7
echo "full name:   $($TM show -gv status-interval)"
echo "prefix:      $($TM show -gv status-inter 2>&1)"
echo "ambiguous:   $($TM show -gv status- 2>&1)"; echo "rc=$?"
echo "unknown:     $($TM show -gv nosuchoption 2>&1)"; echo "rc=$?"
echo "== setting by prefix =="
$TM set -g status-interv 9 2>&1; echo "rc=$?"
echo "value now:   $($TM show -gv status-interval)"
$TM set -g status- 3 2>&1; echo "ambiguous set rc=$?"
$TM set -gu status-interval
