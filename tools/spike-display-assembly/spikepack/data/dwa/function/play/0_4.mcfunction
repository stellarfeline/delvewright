scoreboard players set #clip_0 dwa.s 4
scoreboard players set #f_0 dwa.s -1
scoreboard players operation #t_0 dwa.s = #ft dwa.s
scoreboard players set #done_0 dwa.s 0
execute store result score #play_at dwa.s run time query gametime
