# command-prompt needs a client, and its flags are parsed before that: -I gives
# the initial text, -p the prompts, -T the type.
$TM command-prompt 'display-message x' 2>&1; echo "rc=$?"
$TM command-prompt -I 'initial' 'display-message %%' 2>&1; echo "rc=$?"
$TM command-prompt -p 'one,two' 'display-message %1%2' 2>&1; echo "rc=$?"
$TM command-prompt -T nosuchtype 'display-message x' 2>&1; echo "rc=$?"
