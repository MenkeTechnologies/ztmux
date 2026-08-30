# fill-character and scroll-format are window options that hold a format string;
# they round-trip and expand where they are used.
$TM show -gv fill-character
$TM setw -g fill-character '.'
$TM show -gwv fill-character
$TM setw -g fill-character '#{session_name}'
$TM show -gwv fill-character
$TM setw -gu fill-character
$TM show -gwv fill-character
echo "== scroll-format =="
$TM show -gwv scroll-format
$TM setw -g scroll-format 'x#{scroll_position}'
$TM show -gwv scroll-format
$TM setw -gu scroll-format
