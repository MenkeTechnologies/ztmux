# An option value can hold quotes, spaces and newlines; show -v gives back
# exactly what was stored and show (without -v) quotes it for re-parsing.
$TM set -g @spaced 'two words'
$TM set -g @quoted 'has "double" and '"'"'single'"'"''
$TM set -g @newline "$(printf 'first\nsecond')"
for o in @spaced @quoted @newline; do
  printf '%s -v: [%s]\n' "$o" "$($TM show -gv "$o")"
  printf '%s    : [%s]\n' "$o" "$($TM show -g "$o")"
done
echo "== and an empty value =="
$TM set -g @empty ''
printf '@empty -v: [%s]\n' "$($TM show -gv @empty)"
printf '@empty   : [%s]\n' "$($TM show -g @empty)"
for o in @spaced @quoted @newline @empty; do $TM set -gu "$o"; done
