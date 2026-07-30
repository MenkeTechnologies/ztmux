# message-format: the status message is placed into a format as #{message}
# rather than drawn directly, so the format decides how it is wrapped.
#
# display-message writes to the status line of an attached client, which a
# detached parity session has none of; -p prints to stdout instead and does not
# exercise the format. What is observable without a client is the option itself
# and, through show-options, that the default is the one next-3.7 ships.
$TM show-options -g message-format

# It is a session option, so it is settable per session and inherits.
$TM set-option -g message-format '#[fg=red]<#{message}>'
$TM show-options -g message-format
$TM new-session -d -s other
$TM show-options -t other message-format
$TM set-option -t other message-format 'session: #{message}'
$TM show-options -t other message-format
$TM show-options -g message-format
$TM set-option -t other -u message-format
$TM show-options -t other message-format
$TM set-option -g -u message-format
$TM show-options -g message-format

# #{command_prompt} is what the default format branches on to pick between
# message-style and message-command-style; it is defined for both.
$TM display-message -p '#{?command_prompt,prompt,message}'
