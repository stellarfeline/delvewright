scoreboard players set #party dw.o_reach_the_far_end 1
tellraw @a {"color":"dark_gray","text":"[dw:complete metrics-gym obj/reach-the-far-end]"}
tellraw @a {"color":"green","fallback":"Objective complete: %s","translate":"delve.metrics-gym.delvewright.ui.objective.complete","with":[{"color":"white","fallback":"Walk to the far end","translate":"delve.metrics-gym.obj.walk-the-ladder.reach-the-far-end.title"}]}
playsound minecraft:entity.experience_orb.pickup player @a
kill @e[tag=dw_r_reach_the_far_end]
function metrics-gym:check_q_walk_the_ladder
