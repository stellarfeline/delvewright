# Run by the owner in her world. Tags her as the viewer, loads the site,
# then builds once every chunk of it is loaded (build_wait).
scoreboard objectives add dwe.s dummy
scoreboard players set #on dwe.s 0
tag @a remove dwe_viewer
tag @s add dwe_viewer
kill @e[tag=dwe]
forceload add 4080 4080 4127 4303
scoreboard players set #wait dwe.s 0
tellraw @s {"text":"[eldritch lab] loading the site at x=4096 z=4096 ...","color":"gray"}
schedule function dwe:build_wait 5t
