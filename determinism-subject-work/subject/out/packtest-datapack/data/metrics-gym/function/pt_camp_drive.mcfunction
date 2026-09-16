scoreboard players set #party dw.campaign 0
scoreboard players set #party dw.q_walk_the_ladder 0
scoreboard players set #party dw.qa_walk_the_ladder 0
scoreboard players set #party dw.o_hear_the_brief 0
scoreboard players set #party dw.o_reach_the_far_end 0
scoreboard players set #party dw.qa_walk_the_ladder 1
execute as @a[tag=dw_t_camp,limit=1] run function metrics-gym:complete_o_hear_the_brief
execute as @a[tag=dw_t_camp,limit=1] run function metrics-gym:complete_o_reach_the_far_end
