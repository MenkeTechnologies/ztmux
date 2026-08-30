# refresh-client -C takes a WIDTHxHEIGHT (or `-C W,H`) size; malformed sizes are
# rejected by the argument parser before any client is needed.
$TM refresh-client -C nonsense 2>&1; echo "rc=$?"
$TM refresh-client -C 80x 2>&1; echo "rc=$?"
$TM refresh-client -C 0x0 2>&1; echo "rc=$?"
$TM refresh-client -A 'bad' 2>&1; echo "rc=$?"
