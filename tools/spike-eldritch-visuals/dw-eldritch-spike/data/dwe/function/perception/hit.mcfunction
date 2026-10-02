scoreboard players set #p3 dwe.s 1
tag @s add dwe_p3
effect give @s minecraft:darkness 8 0 true
effect give @s minecraft:nausea 10 0 true
particle minecraft:elder_guardian ~ ~ ~ 0 0 0 0 1 force @s
playsound minecraft:entity.elder_guardian.curse hostile @s ~ ~ ~ 1 0.7
playsound minecraft:entity.warden.heartbeat hostile @s ~ ~ ~ 1 0.6
schedule function dwe:perception/second 50t
