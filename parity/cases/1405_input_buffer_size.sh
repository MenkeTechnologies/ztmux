# input-buffer-size: default, set, and minimum enforcement.
$TM show-options -sg input-buffer-size
$TM set-option -s input-buffer-size 2000000
$TM show-options -sg input-buffer-size
$TM set-option -s input-buffer-size 100
