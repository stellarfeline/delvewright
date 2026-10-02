execute as @e[tag=dwe_watcher] at @s facing entity @p eyes run tp @s ~ ~ ~ ~ ~
execute as @e[tag=dwe_watcher] at @s if entity @a[distance=..4.5] run function dwe:living/vanish
execute unless entity @a[x=4087,y=64,z=4279,dx=18,dy=6,dz=20] run return 0
scoreboard players add #hb dwe.s 1
execute if score #hb dwe.s matches 24 run playsound minecraft:entity.warden.heartbeat hostile @a[x=4087,y=64,z=4279,dx=18,dy=6,dz=20] 4096 65 4297 1.6 0.9
execute if score #hb dwe.s matches 24 run particle minecraft:sculk_charge_pop 4096 64.2 4289 7 0 9 0 25
execute if score #hb dwe.s matches 24.. run scoreboard players set #hb dwe.s 0
scoreboard players add #amb dwe.s 1
execute if score #amb dwe.s matches 300.. as @a[x=4087,y=64,z=4279,dx=18,dy=6,dz=20] at @s rotated ~ 0 positioned ^ ^ ^-5 run playsound minecraft:ambient.cave ambient @s ~ ~ ~ 1 0.7
execute if score #amb dwe.s matches 300.. run scoreboard players set #amb dwe.s 0
