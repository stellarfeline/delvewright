scoreboard players set #t_0 dwa.s 0
scoreboard players add #f_0 dwa.s 1
execute if score #clip_0 dwa.s matches 0 if score #f_0 dwa.s matches 60.. run scoreboard players set #f_0 dwa.s 0
execute if score #clip_0 dwa.s matches 1 if score #f_0 dwa.s matches 36.. run scoreboard players set #f_0 dwa.s 35
execute if score #clip_0 dwa.s matches 1 if score #f_0 dwa.s matches 35 run scoreboard players set #done_0 dwa.s 1
execute if score #clip_0 dwa.s matches 2 if score #f_0 dwa.s matches 36.. run scoreboard players set #f_0 dwa.s 35
execute if score #clip_0 dwa.s matches 2 if score #f_0 dwa.s matches 35 run scoreboard players set #done_0 dwa.s 1
execute if score #clip_0 dwa.s matches 3 if score #f_0 dwa.s matches 8.. run scoreboard players set #f_0 dwa.s 7
execute if score #clip_0 dwa.s matches 3 if score #f_0 dwa.s matches 7 run scoreboard players set #done_0 dwa.s 1
execute if score #clip_0 dwa.s matches 4 if score #f_0 dwa.s matches 10.. run scoreboard players set #f_0 dwa.s 9
execute if score #clip_0 dwa.s matches 4 if score #f_0 dwa.s matches 9 run scoreboard players set #done_0 dwa.s 1
tag @e[tag=dwa_asm_0] add dwa_cur
execute store result storage dwa:m f int 1 run scoreboard players get #f_0 dwa.s
execute store result storage dwa:m c int 1 run scoreboard players get #clip_0 dwa.s
function dwa:anim/apply with storage dwa:m
tag @e[tag=dwa_asm_0] remove dwa_cur
