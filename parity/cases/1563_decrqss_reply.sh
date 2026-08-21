# DECRQSS: DCS $ q Pt ST -- "what is the current setting of Pt".
#
# The port had no `input_handle_decrqss` at all, so a program inside a pane that
# asked got silence where the reference answered. Silence is the worst outcome:
# a terminal that says nothing leaves the asking program waiting on a reply that
# never comes.
#
# Both answers are pinned. The cursor-style query (DCS $ q SP q ST) is the one
# the C actually answers with a value; everything else gets the "not recognized"
# DCS 0 $ r ST, and SGR (`m`) is the case for that.
#
# The reply is read back the only way this suite can see terminal output: the
# pane runs `cat`, so whatever the server writes to the pane arrives on the
# program's stdin and is echoed straight back onto the screen, where
# capture-pane can read it. `\044` is `$`, kept literal through the shell.
#
# Primary DA is deliberately NOT probed here: the C's reply depends on whether
# it was built with ENABLE_SIXEL (input.c:1562-1566), so it discriminates the
# build, not the port.

probe() {
  $TM kill-window -t probe 2>/dev/null
  $TM new-window -d -n probe "sh -c 'sleep 1; printf \"$1\"; cat'"
  sleep 3
  $TM capture-pane -p -t probe | grep -v '^$' | head -1 | cat -v
  $TM kill-window -t probe 2>/dev/null
}

echo "== cursor style: DCS \$ q SP q ST =="
probe '\033P\044q q\033\\\\'

echo "== unrecognized (SGR): DCS \$ q m ST =="
probe '\033P\044qm\033\\\\'
