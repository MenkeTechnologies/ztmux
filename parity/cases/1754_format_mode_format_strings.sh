# The *_mode_format formats carry the mode-format option text for the modes that
# have one; with no client in a mode they are the option's default value.
$TM display-message -p 'buffer=[#{buffer_mode_format}]' | perl -pe 's/\[.+\]/[NONEMPTY]/'
$TM display-message -p 'tree=[#{tree_mode_format}]' | perl -pe 's/\[.+\]/[NONEMPTY]/'
$TM display-message -p 'client=[#{client_mode_format}]' | perl -pe 's/\[.+\]/[NONEMPTY]/'
echo "== they follow their options =="
$TM setw -g mode-format-buffer 'B:#{buffer_name}' 2>/dev/null || true
$TM display-message -p 'after set: [#{buffer_mode_format}]' | perl -pe 's/\[.+\]/[NONEMPTY]/'
