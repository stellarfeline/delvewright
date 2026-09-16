scoreboard players set #party dw.o_hear_the_brief 1
tellraw @a {"color":"dark_gray","text":"[dw:complete metrics-gym obj/hear-the-brief]"}
tellraw @a {"color":"green","fallback":"Objective complete: %s","translate":"delve.metrics-gym.delvewright.ui.objective.complete","with":[{"color":"white","fallback":"Hear the brief","translate":"delve.metrics-gym.obj.walk-the-ladder.hear-the-brief.title"}]}
playsound minecraft:entity.experience_orb.pickup player @a
function metrics-gym:check_q_walk_the_ladder
