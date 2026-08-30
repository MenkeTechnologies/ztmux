# The session-group formats count and list the group's members and how many of
# them have a client attached; with no clients anywhere the attached counts are
# zero but the membership is still reported.
$TM new-session -d -s lead -x 80 -y 24
$TM new-session -d -s follow -t lead -x 80 -y 24
$TM new-session -d -s alone -x 80 -y 24
$TM list-sessions -F '#{session_name} size=#{session_group_size} attached=[#{session_group_attached}] many=[#{session_group_many_attached}] list=[#{session_group_list}] attached_list=[#{session_group_attached_list}]' | sort
