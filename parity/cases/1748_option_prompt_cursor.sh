# The prompt cursor options colour and shape the in-pane prompt's cursor; the
# command form has its own pair.
for o in prompt-cursor-colour prompt-cursor-style prompt-command-cursor-colour prompt-command-cursor-style; do
  printf '%-30s %s\n' "$o" "$($TM show -gv "$o" 2>&1)"
done
$TM set -g prompt-cursor-colour red; $TM show -gv prompt-cursor-colour
$TM set -g prompt-command-cursor-colour '#00ff00'; $TM show -gv prompt-command-cursor-colour
for v in default blinking-block block blinking-underline underline blinking-bar bar; do
  $TM set -g prompt-cursor-style "$v" >/dev/null 2>&1 && printf '%-20s %s\n' "$v" "$($TM show -gv prompt-cursor-style)"
done
$TM set -g prompt-cursor-style nonsense 2>&1; echo "rc=$?"
$TM set -g prompt-cursor-colour notacolour 2>&1; echo "rc=$?"
for o in prompt-cursor-colour prompt-cursor-style prompt-command-cursor-colour prompt-command-cursor-style; do $TM set -gu "$o"; done
