# #{I/f:...}, #{I/c:...} and #{I/e:...} ask about a client's terminal features,
# its terminfo capabilities and its environment (format.c:5290). With no client
# they expand to nothing rather than erroring.
$TM display-message -p 'features: [#{I/f:RGB}]'
$TM display-message -p 'capability: [#{I/c:colors}]'
$TM display-message -p 'environment: [#{I/e:HOME}]'
$TM display-message -p 'no argument: [#{I:x}]'
