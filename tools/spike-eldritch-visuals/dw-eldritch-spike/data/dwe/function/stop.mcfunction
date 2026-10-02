scoreboard players set #on dwe.s 0
kill @e[tag=dwe]
forceload remove 4080 4080 4127 4303
tag @a remove dwe_viewer
tag @a remove dwe_p3
tellraw @s {"text":"[eldritch lab] stopped; blocks stay at x=4096 z=4096.","color":"gray"}
