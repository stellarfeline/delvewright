execute store result storage metrics-gym:camera slot int 1 run scoreboard players get @s dw.cam
scoreboard players reset @s dw.cam
kill @e[type=minecraft:marker,tag=dw_cam_probe]
execute anchored eyes positioned ^ ^ ^ run summon minecraft:marker ~ ~ ~ {Tags:["dw_cam_probe"]}
execute store result storage metrics-gym:camera x int 1 run data get entity @e[type=minecraft:marker,tag=dw_cam_probe,limit=1] Pos[0] 1000
execute store result storage metrics-gym:camera y int 1 run data get entity @e[type=minecraft:marker,tag=dw_cam_probe,limit=1] Pos[1] 1000
execute store result storage metrics-gym:camera z int 1 run data get entity @e[type=minecraft:marker,tag=dw_cam_probe,limit=1] Pos[2] 1000
execute store result storage metrics-gym:camera yaw int 1 run data get entity @s Rotation[0] 100
execute store result storage metrics-gym:camera pitch int 1 run data get entity @s Rotation[1] 100
data modify storage metrics-gym:camera in set value "air"
execute anchored eyes positioned ^ ^ ^ unless block ~ ~ ~ minecraft:air unless block ~ ~ ~ minecraft:cave_air unless block ~ ~ ~ minecraft:void_air run data modify storage metrics-gym:camera in set value "block"
kill @e[type=minecraft:marker,tag=dw_cam_probe]
function metrics-gym:creator/camera/stamp with storage metrics-gym:camera
