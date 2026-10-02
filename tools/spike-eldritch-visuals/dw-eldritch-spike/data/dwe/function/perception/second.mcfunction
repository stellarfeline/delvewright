execute as @a[tag=dwe_p3] at @s run effect give @s minecraft:blindness 2 0 true
execute as @a[tag=dwe_p3] at @s run particle minecraft:elder_guardian ~ ~ ~ 0 0 0 0 1 force @s
execute as @a[tag=dwe_p3] at @s run playsound minecraft:entity.elder_guardian.curse hostile @s ~ ~ ~ 1 0.5
schedule function dwe:perception/third 60t
