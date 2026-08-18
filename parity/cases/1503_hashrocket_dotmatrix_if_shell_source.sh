# dotmatrix's last line: a conditional local-override include.
#
#   if-shell '[[ -e ~/.tmux.conf.local ]]' 'source-file ~/.tmux.conf.local'
#
# Three separate things have to agree for that to behave the same, and each is
# checked below with the file present and absent:
#
#   1. if-shell runs its condition through a SHELL, so both binaries must pick
#      the same one (the `default-shell` option) -- the `[[` form is a bashism
#      the config relies on, and whether it succeeds is the shell's answer, not
#      tmux's, so the two binaries must get the same answer from it.
#   2. source-file inside if-shell runs deferred, after the condition's job
#      exits, so its options must still land.
#   3. A missing source-file is an error, not a silent skip -- but only when
#      it is sourced directly; if-shell's false branch must not report anything.
d=$(mktemp -d)
printf 'set -g status-left-length 42\nset -g @hr-local sourced\n' >"$d/present.conf"

$TM if-shell "[ -e $d/present.conf ]" "source-file $d/present.conf"
sleep 0.5
$TM show-options -g status-left-length
$TM show-options -gv @hr-local

# The false branch: no such file, and an else-branch that proves it was taken.
$TM if-shell "[ -e $d/missing.conf ]" "source-file $d/missing.conf" "set -g @hr-local else-branch"
sleep 0.5
$TM show-options -gv @hr-local

# The `[[` form dotmatrix actually uses. Whether the job shell supports it is
# the shell's business; both binaries must reach the same verdict from the same
# `default-shell`, so the branch taken is the comparison.
$TM set -g @hr-local unset
$TM if-shell "[[ -e $d/present.conf ]]" "set -g @hr-local bracket-true" "set -g @hr-local bracket-false"
sleep 0.5
$TM show-options -gv @hr-local

# -b (background) is the same code path with the job detached from the queue.
$TM if-shell -b "[ -e $d/present.conf ]" "set -g @hr-local backgrounded"
sleep 0.8
$TM show-options -gv @hr-local

# -F takes a FORMAT instead of a shell command: no job, no shell.
$TM if-shell -F '#{==:#{host},#{host}}' "set -g @hr-local format-true" "set -g @hr-local format-false"
$TM show-options -gv @hr-local

# Sourcing the missing file directly IS an error. The path is stripped so the
# mktemp directory does not leak into the comparison.
$TM source-file "$d/missing.conf" 2>&1 | perl -pe "s{\Q$d\E}{DIR}g"
echo "rc=$?"
rm -rf "$d"
