#> The Metrics Gym: the class trigger is one-shot — a second `/trigger dw.class` cannot re-class or warp
# @dummy
# @timeout 100

function metrics-gym:setup
scoreboard players reset @s dw.class
scoreboard players reset @s dw.classed
execute as @s run function metrics-gym:class_arm
execute store success score #cls_arm1 dw.sys run trigger dw.class set 1
assert score #cls_arm1 dw.sys matches 1
function metrics-gym:class_apply_measurer
execute store success score #cls_taken dw.sys if score @s dw.classed matches 1
assert score #cls_taken dw.sys matches 1
execute store success score #cls_left dw.sys if score @s dw.class matches -2147483648..
assert score #cls_left dw.sys matches 0
tp @s 37 64 5
execute store result score #cls_x dw.sys run data get entity @s Pos[0] 1
assert score #cls_x dw.sys matches 37
execute as @s run function metrics-gym:class_arm
execute store success score #cls_arm2 dw.sys run trigger dw.class set 1
assert score #cls_arm2 dw.sys matches 0
execute store success score #cls_left2 dw.sys if score @s dw.class matches -2147483648..
assert score #cls_left2 dw.sys matches 0
execute store success score #cls_still dw.sys if score @s dw.classed matches 1
assert score #cls_still dw.sys matches 1
execute store result score #cls_x2 dw.sys run data get entity @s Pos[0] 1
assert score #cls_x2 dw.sys matches 37
