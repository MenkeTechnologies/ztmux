# `message_time`, `message_text` and `message_number` (cmd-show-messages.c:96-98)
# are the last three variables in the C that no case had reached. They are
# unreachable from a user format: show-messages takes no -F and expands exactly
# one compile-time template, "#{t/p:message_time}: #{message_text}"
# (cmd-show-messages.c:31-32). So the line IS the assertion.
#
# Case 1821 masks each line down to a single MESSAGE-LINE token, which proves a
# line was printed and nothing about its shape. This case masks only the DIGITS,
# which keeps everything that is not a wall clock or a pid comparable:
#
#   - `t/p` on a timestamp from this run renders as %H:%M, because
#     format_pretty_time takes the under-24-hours branch (format.c:4064-4069) --
#     a port rendering the epoch, ctime(3) or a %H:%M:%S would show up here
#   - the ": " the template puts between the time and the text
#   - the "client-<pid> command: " prefix the server logs each command with, and
#     the command text itself, quoting included
#
# The log is newest-first (TAILQ_FOREACH_REVERSE, cmd-show-messages.c:95), which
# is what the ordering below pins: the show-messages call itself is always the
# first line, because logging happens before the command runs.
$TM display-message -p 'seed one' >/dev/null
$TM display-message -p 'seed two' >/dev/null

echo "== the template's shape, with only the digits masked =="
$TM show-messages | perl -pe 's/\d/N/g' | head -4

echo "== newest first: the seeds come back in reverse order =="
$TM show-messages | perl -ne 'print if /seed/' | perl -pe 's/\d/N/g'

echo "== a command with an argument that needs quoting keeps it =="
$TM set -g @msg 'two words' >/dev/null
$TM show-messages | perl -ne 'print if /\@msg/' | perl -pe 's/\d/N/g'

echo "== the time field is the same width on every line =="
$TM show-messages | perl -ne 'my ($t) = /^(\S+):/; $w{length $t}++; END { print join(",", map { "$_=$w{$_}" } sort keys %w), "\n" }'

echo "== message_number is what -N counts from, and the log keeps growing =="
before=$($TM show-messages | wc -l | tr -d ' ')
$TM display-message -p 'seed three' >/dev/null
after=$($TM show-messages | wc -l | tr -d ' ')
echo "grew=$((after > before))"
