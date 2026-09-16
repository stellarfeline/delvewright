#> The Metrics Gym: `/trigger dw.free` leaves the body and returns to it (spec-0069)
# @dummy
# @timeout 100

gamemode adventure @s
tp @s 3.5 300 3.5 45 10
execute store result score #cfr_x dw.cm run data get entity @s Pos[0] 1000
execute store result score #cfr_y dw.cm run data get entity @s Pos[1] 1000
execute store result score #cfr_z dw.cm run data get entity @s Pos[2] 1000
scoreboard players set @s dw.free 1
execute at @s run function metrics-gym:creator/camera/free
assert entity @s[gamemode=spectator]
tp @s 9.5 290 -1.5 200 -40
scoreboard players set @s dw.free 1
execute at @s run function metrics-gym:creator/camera/free
assert entity @s[gamemode=adventure]
execute store result score #cfr_gx dw.cm run data get entity @s Pos[0] 1000
scoreboard players operation #cfr_gx dw.cm -= #cfr_x dw.cm
assert score #cfr_gx dw.cm matches 0
execute store result score #cfr_gy dw.cm run data get entity @s Pos[1] 1000
scoreboard players operation #cfr_gy dw.cm -= #cfr_y dw.cm
assert score #cfr_gy dw.cm matches 0
execute store result score #cfr_gz dw.cm run data get entity @s Pos[2] 1000
scoreboard players operation #cfr_gz dw.cm -= #cfr_z dw.cm
assert score #cfr_gz dw.cm matches 0
execute store result score #cfr_yaw dw.cm run data get entity @s Rotation[0] 100
assert score #cfr_yaw dw.cm matches 4500
scoreboard players operation #cfr_fid dw.cm = @s dw.fid
scoreboard players set #cfr_left dw.cm 0
execute as @e[type=minecraft:marker,tag=dw_free_home] if score @s dw.fid = #cfr_fid dw.cm run scoreboard players add #cfr_left dw.cm 1
assert score #cfr_left dw.cm matches 0
