# Saving into a directory that does not exist is an error naming the reason;
# loading a file that does not exist likewise. Strip the path, keep the message.
$TM set-buffer -b b 'x'
$TM save-buffer -b b /nonexistent-dir-ztpar/out.txt 2>&1 | perl -pe 's{/\S+/}{PATH/}'; echo "rc=${PIPESTATUS[0]}"
$TM load-buffer /nonexistent-dir-ztpar/in.txt 2>&1 | perl -pe 's{/\S+/}{PATH/}'; echo "rc=${PIPESTATUS[0]}"
$TM save-buffer -b nosuchbuffer /dev/null 2>&1; echo "rc=$?"
