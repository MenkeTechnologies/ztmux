# Every CHOICE option, and every name in its choice list
# (options-table.c). This is a whole-table comparison rather than another probe:
# the choice LISTS are data, so the anti-drift gate over function names cannot
# see them, and a list that is short by one name is invisible until something
# writes the missing index -- which is exactly how `remain-on-exit` "key" was
# found (it took the server down through options_value_to_string).
#
# Each value is set and read back, so a missing name shows up as an error where
# the reference accepts it, and a value at the wrong index shows up as the wrong
# name coming back. A name that is not in any list is refused by all of them.
#
# destroy-unattached is left out on purpose: with no client attached, setting it
# to anything but "off" takes the session -- and with it the server -- away
# mid-loop. Its four names are pinned by the case that follows this one.
$TM set -g status off
for pair in \
  '-w cursor-style default blinking-block block blinking-underline underline blinking-bar bar' \
  '-g extended-keys off on always' \
  '-g extended-keys-format csi-u xterm' \
  '-g get-clipboard off buffer request both' \
  '-w menu-border-lines single double heavy simple rounded padded none' \
  '-g set-clipboard off external on' \
  '-g theme detect terminal light dark' \
  '-g activity-action none any current other' \
  '-g bell-action none any current other' \
  '-g detach-on-destroy off on no-detached previous next' \
  '-g message-line 0 1 2 3 4' \
  '-g silence-action none any current other' \
  '-g status off on 2 3 4 5' \
  '-g status-justify left centre right absolute-centre' \
  '-g status-keys emacs vi' \
  '-g status-position top bottom' \
  '-g prompt-cursor-style default blinking-block block blinking-underline underline blinking-bar bar' \
  '-g prompt-command-cursor-style default blinking-block block blinking-underline underline blinking-bar bar' \
  '-g visual-activity off on both' \
  '-g visual-bell off on both' \
  '-g visual-silence off on both' \
  '-w allow-passthrough off on all' \
  '-w clock-mode-style 12 24 12-with-seconds 24-with-seconds' \
  '-w copy-mode-line-numbers off default absolute relative hybrid' \
  '-w mode-keys emacs vi' \
  '-w pane-border-indicators off colour arrows both' \
  '-w pane-border-lines single double heavy simple number spaces none' \
  '-w pane-border-status off top bottom top-floating bottom-floating' \
  '-w pane-scrollbars off modal on auto-hide' \
  '-w pane-scrollbars-position right left' \
  '-w popup-border-lines single double heavy simple rounded padded none' \
  '-w remain-on-exit off on failed key' \
  '-w window-size largest smallest manual latest'
do
  set -- $pair
  flag=$1; opt=$2; shift 2
  printf '%s:' "$opt"
  for v in "$@"; do
    if out=$($TM set $flag "$opt" "$v" 2>&1) && [ -z "$out" ]; then
      back=$($TM show $flag -v "$opt" 2>&1)
      [ "$back" = "$v" ] && printf ' %s' "$v" || printf ' %s->%s' "$v" "$back"
    else
      printf ' %s!REFUSED(%s)' "$v" "$out"
    fi
  done
  printf '\n'
done
echo "== a name that is in no list is refused by every choice option =="
for pair in '-g status-justify' '-w mode-keys' '-w window-size' '-g bell-action'; do
  set -- $pair
  printf '%s: ' "$2"
  $TM set "$1" "$2" nosuchchoice 2>&1
  echo "rc=$?"
done
