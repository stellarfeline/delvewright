execute as @e[type=minecraft:marker,tag=dw_free_this,limit=1] at @s if score @s dw.fload matches 1 run forceload remove ~ ~
kill @e[type=minecraft:marker,tag=dw_free_this]
summon minecraft:marker ~ ~ ~ {Tags:["dw_free_home","dw_free_this"]}
tp @e[type=minecraft:marker,tag=dw_free_this,limit=1] ~ ~ ~ ~ ~
scoreboard players operation @e[type=minecraft:marker,tag=dw_free_this,limit=1] dw.fid = #fid dw.cm
execute store result score @e[type=minecraft:marker,tag=dw_free_this,limit=1] dw.fmode run data get entity @s playerGameType
execute store success score #forced dw.cm run forceload query ~ ~
scoreboard players set @e[type=minecraft:marker,tag=dw_free_this,limit=1] dw.fload 0
execute if score #forced dw.cm matches 0 run scoreboard players set @e[type=minecraft:marker,tag=dw_free_this,limit=1] dw.fload 1
execute if score #forced dw.cm matches 0 run forceload add ~ ~
tag @s add dw_free
gamemode spectator @s
