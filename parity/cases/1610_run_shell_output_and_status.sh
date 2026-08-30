# run-shell prints the command's output and reports a non-zero exit status; the
# shell is invoked so redirection and quoting work.
$TM run-shell 'echo hello from run-shell'; echo "rc=$?"
$TM run-shell 'printf "a\nb\n"'; echo "rc=$?"
$TM run-shell 'exit 3' 2>&1; echo "rc=$?"
$TM run-shell 'echo to-stderr >&2'; echo "rc=$?"
