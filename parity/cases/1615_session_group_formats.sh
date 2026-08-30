# Sessions created with -t join the target's group: the group name, size and
# member list are formats, and an ungrouped session leaves them empty/zero.
$TM new-session -d -s lead
$TM new-session -d -s follow -t lead
$TM list-sessions -F '#{session_name} group=[#{session_group}] size=[#{session_group_size}] list=[#{session_group_list}]' | sort
