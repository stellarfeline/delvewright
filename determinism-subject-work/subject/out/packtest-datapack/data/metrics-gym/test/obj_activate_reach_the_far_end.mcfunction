#> The Metrics Gym: activating objective `obj/reach-the-far-end` places its own affordance
# @dummy
# @timeout 100

function metrics-gym:setup
execute store result score #fx0_reach_the_far_end dw.sys if entity @e[tag=dw_r_reach_the_far_end]
function metrics-gym:activate_o_reach_the_far_end
execute store result score #fx1_reach_the_far_end dw.sys if entity @e[tag=dw_r_reach_the_far_end]
scoreboard players operation #fx1_reach_the_far_end dw.sys -= #fx0_reach_the_far_end dw.sys
assert score #fx1_reach_the_far_end dw.sys matches 1
kill @e[tag=dw_r_reach_the_far_end,limit=1]
