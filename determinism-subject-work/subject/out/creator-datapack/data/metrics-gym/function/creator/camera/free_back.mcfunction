execute store result score #mode dw.cm run scoreboard players get @e[type=minecraft:marker,tag=dw_free_this,limit=1] dw.fmode
execute if score #mode dw.cm matches 0 run gamemode survival @s
execute if score #mode dw.cm matches 1 run gamemode creative @s
execute if score #mode dw.cm matches 2 run gamemode adventure @s
execute if score #mode dw.cm matches 3 run gamemode spectator @s
tp @s @e[type=minecraft:marker,tag=dw_free_this,limit=1]
execute as @e[type=minecraft:marker,tag=dw_free_this,limit=1] at @s if score @s dw.fload matches 1 run forceload remove ~ ~
kill @e[type=minecraft:marker,tag=dw_free_this]
tag @s remove dw_free
