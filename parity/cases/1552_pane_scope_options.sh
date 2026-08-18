# `set -p` on the three border options that are pane-scoped upstream.
#
# options-table.c gives pane-active-border-style, pane-border-lines and
# pane-border-style the scope OPTIONS_TABLE_WINDOW|OPTIONS_TABLE_PANE. The port's
# table had plain OPTIONS_TABLE_WINDOW for exactly those three, so
# options_scope_from_name (options.c:900, the combined `case WINDOW|PANE:` label)
# never took the `-p` branch for them: `set -p` silently wrote the WINDOW option
# instead, changing the border on EVERY pane in the window rather than the one
# named. Nothing errored, which is what made it survive -- the command succeeded
# and the wrong pane changed.
#
# The rest of the table is checked by unit test rather than here; this case pins
# the three that had drifted, plus the inheritance behaviour that makes the scope
# observable at all.
$TM split-window -d -t 0 'sleep 300'
$TM split-window -d -t 0 'sleep 300'

echo "== set -p writes the PANE option, not the window one =="
for pair in 'pane-border-lines heavy' 'pane-border-style fg=red' 'pane-active-border-style fg=blue'; do
  set -- $pair
  o="$1"; v="$2"
  $TM set -p -t 0.1 "$o" "$v"
  printf '%s: rc=%s\n' "$o" "$?"
  # The named pane has it; its siblings do not; the window does not.
  printf '  pane 0.1 = [%s]\n' "$($TM show -p -t 0.1 -v "$o")"
  printf '  pane 0.0 = [%s]\n' "$($TM show -p -t 0.0 -v "$o")"
  printf '  pane 0.2 = [%s]\n' "$($TM show -p -t 0.2 -v "$o")"
  printf '  window   = [%s]\n' "$($TM show -w -v "$o")"
done

echo "== -A resolves through the parent chain, so every pane still answers =="
for o in pane-border-lines pane-border-style pane-active-border-style; do
  printf '%-26s 0.0=[%s] 0.1=[%s]\n' "$o" \
    "$($TM show -p -A -t 0.0 -v "$o")" "$($TM show -p -A -t 0.1 -v "$o")"
done

echo "== the pane option overrides a window value set afterwards =="
$TM set -w pane-border-lines double
printf 'window=[%s] pane0.1=[%s] pane0.1(-A)=[%s] pane0.0(-A)=[%s]\n' \
  "$($TM show -w -v pane-border-lines)" \
  "$($TM show -p -t 0.1 -v pane-border-lines)" \
  "$($TM show -p -A -t 0.1 -v pane-border-lines)" \
  "$($TM show -p -A -t 0.0 -v pane-border-lines)"

echo "== unset falls back to the window value =="
$TM set -pu -t 0.1 pane-border-lines
printf 'pane0.1=[%s] pane0.1(-A)=[%s]\n' \
  "$($TM show -p -t 0.1 -v pane-border-lines)" \
  "$($TM show -p -A -t 0.1 -v pane-border-lines)"

echo "== a plain pane listing shows only what the pane itself holds =="
$TM show -p -t 0.1

echo "== a window-only option IGNORES -p and writes the window =="
# Not an error: options_scope_from_name only consults `-p` under the combined
# WINDOW|PANE label, so for a plain-WINDOW option it falls through to the window
# branch and succeeds. Pinned because the fix above must not turn this into a
# rejection -- the difference between the two labels is the whole bug.
for o in main-pane-width window-status-format; do
  out=$($TM set -p -t 0.1 "$o" 5 2>&1); rc=$?
  printf '%-22s rc=%s out=[%s] window=[%s] pane=[%s]\n' \
    "$o" "$rc" "$out" "$($TM show -w -v "$o")" "$($TM show -p -t 0.1 -v "$o")"
done

echo "== and a genuinely pane-scoped neighbour is unaffected =="
$TM set -p -t 0.1 allow-rename on
printf 'allow-rename 0.1=[%s] 0.0=[%s]\n' \
  "$($TM show -p -t 0.1 -v allow-rename)" "$($TM show -p -t 0.0 -v allow-rename)"
