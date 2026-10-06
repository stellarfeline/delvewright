scoreboard players add #hits dwa.s 1
execute store result score #hit_at dwa.s run time query gametime
data remove entity @s attack
execute if score #hits dwa.s matches 5 run function dwa:play/0_2
