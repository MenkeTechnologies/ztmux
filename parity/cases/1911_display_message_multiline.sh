# A format may expand to more than one line; display-message -p prints them all,
# and the message log keeps the text as one entry.
$TM display-message -p 'first#{l:
}second'
echo "---"
$TM display-message -p '#{l:a
b
c}'
echo "---"
$TM set -g @multi 'x
y'
$TM display-message -p '#{@multi}'
echo "--- as a single option value:"
$TM show -gv @multi | wc -l | tr -d ' '
$TM set -gu @multi
