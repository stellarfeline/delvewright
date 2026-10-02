particle minecraft:portal ~ ~1 ~ 0.3 1 0.3 0.6 90
playsound minecraft:entity.enderman.teleport hostile @a ~ ~ ~ 1 0.6
scoreboard players add #wpos dwe.s 1
execute if score #wpos dwe.s matches 3.. run scoreboard players set #wpos dwe.s 0
execute if score #wpos dwe.s matches 0 run tp @s 4090.5 64 4296.5
execute if score #wpos dwe.s matches 1 run tp @s 4102.5 64 4281.5
execute if score #wpos dwe.s matches 2 run tp @s 4089.5 64 4283.5
