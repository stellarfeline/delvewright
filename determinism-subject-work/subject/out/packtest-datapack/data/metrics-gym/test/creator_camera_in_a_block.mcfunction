#> The Metrics Gym: `/trigger dw.cam` stamps a spectator's eye inside a block, and refuses nothing (spec-0069)
# @dummy
# @timeout 100

gamemode spectator @s
tp @s 3.5 300 3.5 -90 -30
await delay 2t
gamemode spectator @s
setblock 3 301 3 minecraft:stone
tp @s 3.5 300 3.5 -90 -30
scoreboard players set @s dw.cam 7
execute at @s run function metrics-gym:creator/camera/cam
execute store success score #cwl_stamp dw.cm if data storage metrics-gym:camera {slot:7,yaw:-9000,pitch:-3000,in:"block"}
assert score #cwl_stamp dw.cm matches 1
execute store result score #cwl_wx dw.cm run data get entity @s Pos[0] 1000
execute store result score #cwl_gx dw.cm run data get storage metrics-gym:camera x
scoreboard players operation #cwl_gx dw.cm -= #cwl_wx dw.cm
assert score #cwl_gx dw.cm matches -1..1
execute store result score #cwl_wy dw.cm run data get entity @s Pos[1] 1000
scoreboard players add #cwl_wy dw.cm 1620
execute store result score #cwl_gy dw.cm run data get storage metrics-gym:camera y
scoreboard players operation #cwl_gy dw.cm -= #cwl_wy dw.cm
assert score #cwl_gy dw.cm matches -1..1
execute store result score #cwl_wz dw.cm run data get entity @s Pos[2] 1000
execute store result score #cwl_gz dw.cm run data get storage metrics-gym:camera z
scoreboard players operation #cwl_gz dw.cm -= #cwl_wz dw.cm
assert score #cwl_gz dw.cm matches -1..1
setblock 3 301 3 minecraft:air
gamemode adventure @s
