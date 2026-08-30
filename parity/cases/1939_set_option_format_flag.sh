# -F expands the VALUE as a format before it is stored (cmd-set-option.c:125-128),
# and the option NAME is expanded whether or not -F was given, because the first
# argument always goes through format_single_from_target (cmd-set-option.c:101).
$TM set -g '@plain' 'windows=#{session_windows}'
echo "without -F: [$($TM show -gv '@plain')]"
$TM set -gF '@expanded' 'windows=#{session_windows}'
echo "with -F:    [$($TM show -gv '@expanded')]"
$TM set -g '@name-#{session_name}' stored
echo "the name was expanded too: [$($TM show -gv '@name-0')]"
echo "== set-environment -F =="
$TM set-environment -g -F ZT_ENV 'session=#{session_name}'
echo "env: [$($TM show-environment -g ZT_ENV)]"
$TM set-environment -g ZT_PLAIN 'session=#{session_name}'
echo "env without -F: [$($TM show-environment -g ZT_PLAIN)]"
echo "== -F with a value that is not a format is left alone =="
$TM set -gF '@literal' 'no braces here'
echo "[$($TM show -gv '@literal')]"
