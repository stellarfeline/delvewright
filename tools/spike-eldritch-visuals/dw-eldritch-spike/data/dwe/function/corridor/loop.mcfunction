tp @s ~ ~ ~-12
scoreboard players add #loops dwe.s 1
execute if score #loops dwe.s matches 2 run fill 4094 64 4188 4098 67 4278 minecraft:cracked_stone_bricks replace minecraft:stone_bricks
execute if score #loops dwe.s matches 4 run function dwe:corridor/dim
execute if score #loops dwe.s matches 6 at @s rotated ~ 0 positioned ^ ^ ^-8 run playsound minecraft:entity.enderman.stare hostile @s ~ ~ ~ 0.6 0.5
execute store result score #loop_at dwe.s run time query gametime
