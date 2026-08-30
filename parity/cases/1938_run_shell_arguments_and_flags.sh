# run-shell without -C expands the command as a format, with the extra arguments
# available as #{1}, #{2} and so on (cmd-run-shell.c:135-145); -C runs tmux
# commands instead; -c sets the working directory; -E shows the job's stderr and
# without it stderr is dropped.
$TM run-shell 'printf "plain\n"'
echo "rc=$?"
echo "== the extra arguments arrive as numbered format keys =="
$TM run-shell 'printf "one=#{1} two=#{2}\n"' alpha beta
echo "== a format in the command is expanded from the target =="
$TM run-shell 'printf "session=#{session_name}\n"'
echo "== -C runs tmux commands =="
$TM run-shell -C 'set -g @from-run-shell yes'
echo "option: [$($TM show -gv '@from-run-shell')]"
$TM run-shell -C 'display-message -p set-by-C'
echo "== -c chooses the directory =="
$TM run-shell -c / 'pwd'
echo "== stderr is dropped without -E and shown with it =="
$TM run-shell 'printf "to-stderr\n" >&2'; echo "without -E rc=$?"
$TM run-shell -E 'printf "to-stderr\n" >&2'; echo "with -E rc=$?"
echo "== a failing command reports its exit status =="
$TM run-shell 'exit 3' 2>&1; echo "rc=$?"
echo "== -b returns before the job runs =="
$TM run-shell -b 'printf "background\n"'; echo "-b rc=$?"
$TM run-shell 'printf "sync-after-background\n"'
