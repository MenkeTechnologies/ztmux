# Every style directive style_parse accepts, set and read back.
#
# A style is the one place a user's config reaches deep into the drawing code,
# and a rejected directive fails the WHOLE option -- `status-style` keeps its old
# value and the config line errors. Three directives were rejected by the port
# and accepted upstream, so any config using them broke at load:
#
#   * `set-default` (STYLE_DEFAULT_SET, style.c:112). Unlike push/pop it moves
#     the BASE, so everything after it -- including a later `pop-default` --
#     resolves against the new cell (format-draw.c:865-870).
#   * `link=URI` (style.c:276-284), the OSC 8 hyperlink directive.
#   * `nolink` (style.c:246-247), which clears it again.
#
# The list is written out in full rather than testing only those three: the
# point is that the accept/reject SET matches, and a directive silently gaining
# or losing acceptance is exactly what went unnoticed here.
# A style option stores the string verbatim, so `show` reads back what was set
# -- what discriminates here is the rc and the `invalid style:` message, not the
# value. style_tostring's own round trip is covered by the unit tests in
# src/ported/style.rs and by what format_draw actually emits (case 1555).
echo "== accept/reject, with the stored value alongside =="
for d in default ignore noignore push-default pop-default set-default \
         'fg=red' 'bg=blue' 'us=green' 'us=#123456' 'fill=red' \
         none noattr nobold nolink nodim \
         'align=left' 'align=centre' 'align=right' 'align=absolute-centre' \
         'list=on' 'list=focus' 'list=left-marker' 'list=right-marker' nolist \
         'range=left' 'range=right' 'range=window|1' 'range=user|x' 'range=control|3' \
         'width=10' 'width=50%' 'pad=2' \
         'link=http://example.com/a' 'link=' \
         bold italics underscore blink reverse hidden strikethrough dim; do
  out=$($TM set -g status-style "$d" 2>&1); rc=$?
  printf '%-26s rc=%s out=[%s] val=[%s]\n' "$d" "$rc" "$out" "$($TM show -gv status-style)"
  $TM set -gu status-style
done

echo "== combinations: order, overwrite and clearing =="
for d in 'fg=red,link=http://a,bold' \
         'link=http://a,nolink' \
         'link=http://a,link=http://b' \
         'link=http://a,default' \
         'set-default,fg=red' \
         'push-default,set-default,pop-default'; do
  $TM set -g status-style "$d" 2>&1
  printf '%-34s -> [%s]\n' "$d" "$($TM show -gv status-style)"
  $TM set -gu status-style
done

echo "== repeated and distinct URIs both survive a set/show cycle =="
# Whether the two `http://a` styles SHARE one hyperlink entry is not visible
# here -- the option holds the string, not the id. Case 1555 reads the id off
# the wire, where sharing is observable.
$TM set -g status-style 'link=http://a'
a1=$($TM show -gv status-style)
$TM set -g status-style 'link=http://a'
a2=$($TM show -gv status-style)
$TM set -g status-style 'link=http://b'
b1=$($TM show -gv status-style)
printf 'a1=[%s] a2=[%s] b1=[%s] same=%s\n' "$a1" "$a2" "$b1" "$([ "$a1" = "$a2" ] && echo yes || echo no)"
$TM set -gu status-style

echo "== a rejected directive leaves the option untouched =="
$TM set -g status-style 'fg=red'
$TM set -g status-style 'fg=green,nosuchdirective' 2>&1
printf 'after failed set: [%s]\n' "$($TM show -gv status-style)"
$TM set -gu status-style

echo "== and it works the same in a pane-border-format style, not just status =="
$TM set -g pane-border-format '#[link=http://x,set-default]#{pane_index}'
$TM show -gv pane-border-format
$TM set -gu pane-border-format
