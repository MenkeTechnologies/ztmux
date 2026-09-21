# environ_for_session (environ.c:251-286) copies the global environment into a
# new pane's environment and then OVERWRITES four fixed variables on top of it:
# TERM, TERM_PROGRAM, TERM_PROGRAM_VERSION and COLORTERM. Only three were
# ported -- COLORTERM was missing -- so a pane inherited whatever the server was
# started with instead of always being told the terminal is truecolor.
#
# That is invisible wherever the server's own environment already carries
# COLORTERM: a desktop terminal exports it, environ_copy brings it along, and
# the pane looks right by accident. It is not invisible on a Linux CI runner,
# where nothing sets it. There `tmux-256color` carries `colors#0x100` and
# neither `Tc` nor `RGB`, which makes the COLORTERM check at tty-term.c:608-617
# the only route to the RGB feature -- so a nested server's client came up
# without TERM_RGBCOLOURS, `#{client_colours}` answered below 256, and every
# dark-theme-* default resolved to its 8-colour arm. Twelve render cases failed
# on Linux and passed on macOS for exactly that reason.
#
# Seeding a sentinel first is what makes this assertion independent of the
# environment the suite happens to run in: an implementation that merely
# inherits shows the sentinel, one that follows the C shows the overwrite, and
# neither answer depends on what the host exports. The pane's environment is
# only observable from inside the pane -- `show-environment` reads a session's
# environment, not the one a child was spawned with -- so the values are
# printed by the pane's own command and read back with capture-pane.
$TM set-environment -g COLORTERM ztpar-sentinel
$TM set-environment -g TERM ztpar-sentinel
$TM set-environment -g ZTPAR_PLAIN inherited-unchanged

$TM new-window -d -n envwin -- sh -c 'printenv COLORTERM; printenv TERM; printenv TERM_PROGRAM; printenv ZTPAR_PLAIN; sleep 300'

# Poll for the four lines rather than sleeping blind: the case has to finish
# inside the runner's 15s budget, and the pane only needs one scheduling slice.
i=0
while [ $i -lt 100 ]; do
  n=$($TM capture-pane -p -t envwin 2>/dev/null | grep -c .)
  [ "${n:-0}" -ge 4 ] && break
  i=$((i+1)); sleep 0.05
done

echo "== COLORTERM and TERM are overwritten; the sentinel does not survive =="
$TM capture-pane -p -t envwin | sed -n 1,2p
echo "== TERM_PROGRAM is set outright too =="
$TM capture-pane -p -t envwin | sed -n 3p
echo "== a variable with no default keeps the value it inherited =="
$TM capture-pane -p -t envwin | sed -n 4p
