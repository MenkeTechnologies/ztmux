# The command run by run-shell is a format string expanded against the target,
# so #{...} in it sees the session and window.
$TM run-shell 'echo window=#{window_name} panes=#{window_panes}'; echo "rc=$?"
$TM run-shell -t base 'echo target=#{window_name}'; echo "rc=$?"
