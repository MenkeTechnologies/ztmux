# Prompt types: next-3.7 has exactly two, `command` and `search`. The `target`
# and `window-target` types went away when the prompt moved into prompt.c, and
# anything that is not one of the two is rejected as an invalid type.
#
# This is the case for a divergence the suite had no coverage for: ztmux carried
# the older four-value enum, so it printed a history section for `target` and
# accepted `clear-prompt-history -T target` instead of erroring.
for t in command search target window-target bogus ''; do
  printf '== show -T %s\n' "${t:-<empty>}"
  $TM show-prompt-history -T "$t" 2>&1 | head -3
  printf '== clear -T %s: ' "${t:-<empty>}"
  $TM clear-prompt-history -T "$t" 2>&1 | head -1
  echo
done

# Without -T every type is listed, which is how the count of types shows through.
printf '== show all\n'
$TM show-prompt-history 2>&1 | head -8

# The type reaches formats as a string, and is only valid while a prompt is open.
printf '== format: %s\n' "$($TM display-message -p '#{prompt_type}')"

# command-prompt -T takes the same names, so the invalid ones are refused there
# too rather than silently selecting a fallback.
for t in command search target bogus; do
  printf '== command-prompt -T %s: ' "$t"
  $TM command-prompt -T "$t" -p x 'display-message y' 2>&1 | head -1
done
