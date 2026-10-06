execute store result score #x0 dwl.s run data get entity @s Pos[0] 1000
execute store result score #mx0 dwl.s run data get entity @s Motion[0] 1000
execute store result score #y0 dwl.s run data get entity @s Pos[1] 1000
execute store result score #my0 dwl.s run data get entity @s Motion[1] 1000
execute store result score #z0 dwl.s run data get entity @s Pos[2] 1000
execute store result score #mz0 dwl.s run data get entity @s Motion[2] 1000
execute store result score #yaw0 dwl.s run data get entity @s Rotation[0] 100
execute store result score #pitch0 dwl.s run data get entity @s Rotation[1] 100
tp @s ~ ~ ~-12
execute store result score #x1 dwl.s run data get entity @s Pos[0] 1000
execute store result score #mx1 dwl.s run data get entity @s Motion[0] 1000
execute store result score #y1 dwl.s run data get entity @s Pos[1] 1000
execute store result score #my1 dwl.s run data get entity @s Motion[1] 1000
execute store result score #z1 dwl.s run data get entity @s Pos[2] 1000
execute store result score #mz1 dwl.s run data get entity @s Motion[2] 1000
execute store result score #yaw1 dwl.s run data get entity @s Rotation[0] 100
execute store result score #pitch1 dwl.s run data get entity @s Rotation[1] 100
scoreboard players add #moves dwl.s 1
execute store result score #move_at dwl.s run time query gametime
