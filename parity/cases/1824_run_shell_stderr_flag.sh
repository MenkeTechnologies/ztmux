# run-shell -E shows the command's standard error as well as its output; without
# it the error output is dropped (cmd-run-shell.c:170).
$TM run-shell 'printf "to-stdout\n"; printf "to-stderr\n" >&2'; echo "without -E rc=$?"
$TM run-shell -E 'printf "to-stdout\n"; printf "to-stderr\n" >&2'; echo "with -E rc=$?"
echo "== a command that only writes to stderr =="
$TM run-shell 'printf "quiet\n" >&2'; echo "rc=$?"
$TM run-shell -E 'printf "loud\n" >&2'; echo "rc=$?"
