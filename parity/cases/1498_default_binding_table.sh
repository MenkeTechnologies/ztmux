# The WHOLE default binding table, diffed against next-3.7's.
#
# This is the check the port did not have. The anti-drift gate compares function
# NAMES; a default binding is DATA, so nothing ever compared the ~283 binding
# strings against key-bindings.c, and five of them had drifted -- a wrong command
# on MouseDown1Status, a missing #{alternate_on} on WheelUpPane, a spurious -O on
# five menus, a hand-written session menu, and 16 bindings missing the -- before
# their argument. Case 1497 pins nineteen keys by name; this pins every other one.
#
# It compares what list-keys prints, not the source text, so it is a diff of what
# each binary actually PARSED. Cosmetic transcription differences that parse to
# the same command list -- `resize-pane -R 5` versus `{ resize-pane -R 5 }`, a
# -N note the port words differently -- are canonicalised away by the round trip,
# and only real differences in the resulting command survive.
#
# Keys listed in SKIP below are excluded because they are known to differ, each
# for a reason recorded elsewhere. The 32 command-prompt -P keys that used to sit
# here left the list when the in-pane prompt was ported (cases 1506/1507). Every OTHER key must match byte-for-byte, so a
# new divergence on any of them fails this case. Shrinking SKIP is the point: a
# key leaves the list when the feature behind it lands.
#
# The output is SORTED, which is a real weakening and not a formatting choice.
# list-keys walks each table in key-code order, and ztmux's keyc enum is the flat
# sequential one where the C's is type-shifted, so the two orders differ inside a
# table -- WheelUpPane and WheelDownPane come out swapped, and the mouse block
# sits at a different offset. That ordering is a consequence of the encoding
# (task: migrate to the type-shifted scheme) and is not something this case can
# fix, so it compares the SET of bindings and their commands, not their order.
# When the encoding migrates, drop the sort and this case gets stricter.
$TM list-keys | sort | perl -e '
my @skip = (
    # ztmux extensions: bindings the C does not have at all (dashboard, graph,
    # doctor, stats, switcher, tree, watch, scrollback editor, pane sync,
    # floating-pane toggle and the border context menu).
    "prefix C-d", "prefix C-f", "prefix C-s", "prefix e", "prefix G",
    "prefix H", "prefix I", "prefix R", "prefix S", "prefix T",
    "prefix W", "prefix y", "prefix +", "root MouseDown3Border",

    # The scrollbar and control mouse locations the six-location keyc table
    # cannot name. Covered by parity/known_gaps/mouse_scrollbar_locations.sh.
    "root MouseDown1ScrollbarUp", "root MouseDown1ScrollbarDown",
    "root MouseDrag1ScrollbarSlider",
    "root MouseDown1Control8", "root MouseDown1Control9",

    # The pane context menu: ztmux adds entries to it (stack, tab bar, floating
    # pane, open URL). Verified a strict superset -- it omits nothing the C has,
    # which case 1497 and the Paste row restored in this commit series cover.
    "prefix >", "root MouseDown3Pane", "root M-MouseDown3Pane",
);
my %skip = map { $_ => 1 } @skip;
my $n = 0;
while (<STDIN>) {
    chomp; s/[ \t]+/ /g; s/^ //; s/ $//;
    next unless length;
    my @f = split / /;
    my $tk = "";
    for my $i (0 .. $#f) {
        if ($f[$i] eq "-T") { $tk = "$f[$i+1] $f[$i+2]"; last }
    }
    next if $skip{$tk};
    $n++;
    print "$_\n";
}
# The count is part of the comparison: if one binary silently stops emitting a
# whole table, the surviving lines could still all match without it.
print "compared $n bindings\n";
'
