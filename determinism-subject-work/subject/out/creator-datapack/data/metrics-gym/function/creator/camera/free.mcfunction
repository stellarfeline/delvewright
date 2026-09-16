scoreboard players reset @s dw.free
execute unless score @s dw.fid matches 1.. run function metrics-gym:creator/camera/free_id
scoreboard players operation #fid dw.cm = @s dw.fid
tag @e[type=minecraft:marker,tag=dw_free_this] remove dw_free_this
scoreboard players add @e[type=minecraft:marker,tag=dw_free_home] dw.fid 0
execute as @e[type=minecraft:marker,tag=dw_free_home] if score @s dw.fid = #fid dw.cm run tag @s add dw_free_this
scoreboard players set #back dw.cm 0
execute if entity @s[tag=dw_free] if entity @e[type=minecraft:marker,tag=dw_free_this,limit=1] run scoreboard players set #back dw.cm 1
execute if score #back dw.cm matches 1 run function metrics-gym:creator/camera/free_back
execute if score #back dw.cm matches 0 run function metrics-gym:creator/camera/free_leave
tag @e[type=minecraft:marker,tag=dw_free_this] remove dw_free_this
