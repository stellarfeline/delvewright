#> The Metrics Gym: `/trigger dw.cam` stamps a standing player's eye and rotation (spec-0069)
# @dummy
# @timeout 100

gamemode adventure @s
tp @s 3.5 300 3.5 30 15
await delay 2t
gamemode adventure @s
tp @s 3.5 300 3.5 30 15
scoreboard players set @s dw.cam 1
execute at @s run function metrics-gym:creator/camera/cam
execute store success score #cst_stamp dw.cm if data storage metrics-gym:camera {slot:1,yaw:3000,pitch:1500,in:"air"}
assert score #cst_stamp dw.cm matches 1
execute store result score #cst_wx dw.cm run data get entity @s Pos[0] 1000
execute store result score #cst_gx dw.cm run data get storage metrics-gym:camera x
scoreboard players operation #cst_gx dw.cm -= #cst_wx dw.cm
assert score #cst_gx dw.cm matches -1..1
execute store result score #cst_wy dw.cm run data get entity @s Pos[1] 1000
scoreboard players add #cst_wy dw.cm 1620
execute store result score #cst_gy dw.cm run data get storage metrics-gym:camera y
scoreboard players operation #cst_gy dw.cm -= #cst_wy dw.cm
assert score #cst_gy dw.cm matches -1..1
execute store result score #cst_wz dw.cm run data get entity @s Pos[2] 1000
execute store result score #cst_gz dw.cm run data get storage metrics-gym:camera z
scoreboard players operation #cst_gz dw.cm -= #cst_wz dw.cm
assert score #cst_gz dw.cm matches -1..1
