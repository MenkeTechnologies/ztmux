# list-commands with a name prints just that command's usage line. The name goes
# through cmd_find (cmd-list-commands.c:95), so an abbreviation resolves and an
# unknown or ambiguous name reports cmd_find's error. The full listing is not
# counted here: ztmux's table carries its own extension commands.
$TM list-commands new-window
$TM list-commands -F '#{command_list_name}|#{command_list_alias}|#{command_list_usage}' kill-pane
echo "== an abbreviation =="
$TM list-commands -F '#{command_list_name}' new-w
echo "== an alias =="
$TM list-commands -F '#{command_list_name}' neww
echo "== an unknown name =="
$TM list-commands nosuchcommand 2>&1; echo "rc=$?"
echo "== an ambiguous abbreviation =="
$TM list-commands -F '#{command_list_name}' ne 2>&1; echo "rc=$?"
