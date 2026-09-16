#> The Metrics Gym: class `class/measurer` applies its own kit, tag and entry warp
# @dummy
# @timeout 100

function metrics-gym:setup
scoreboard players reset @s dw.class
scoreboard players reset @s dw.classed
tp @s 37 64 5
execute as @s run function metrics-gym:class_apply_measurer
execute store success score #cap_measurer dw.sys if score @s dw.classed matches 1
assert score #cap_measurer dw.sys matches 1
execute store result score #capx_measurer dw.sys run data get entity @s Pos[0] 1
assert score #capx_measurer dw.sys matches 5
clear @s
tag @s remove dw_class_measurer
scoreboard players reset @s dw.classed
