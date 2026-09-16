#> The Metrics Gym: NPC `npc/invigilator` node `dlg/greeting` — every gated option is displayed exactly when its own condition holds
# @dummy
# @timeout 100

function metrics-gym:setup
tag @p add dw_t_dvis_invigilator_greeting
scoreboard players set #party dw.qa_walk_the_ladder 1
scoreboard players set #party dw.o_hear_the_brief 0
execute as @a[tag=dw_t_dvis_invigilator_greeting,limit=1] run function metrics-gym:dmask_invigilator_greeting
execute as @a[tag=dw_t_dvis_invigilator_greeting,limit=1] run scoreboard players operation #dm_invigilator_greeting dw.sys = @s dw.dmask
scoreboard players set #dmhi_invigilator_greeting dw.sys 2
scoreboard players operation #dm_invigilator_greeting dw.sys %= #dmhi_invigilator_greeting dw.sys
scoreboard players set #dmlo_invigilator_greeting dw.sys 1
scoreboard players operation #dm_invigilator_greeting dw.sys /= #dmlo_invigilator_greeting dw.sys
assert score #dm_invigilator_greeting dw.sys matches 1
scoreboard players set #party dw.qa_walk_the_ladder 0
execute as @a[tag=dw_t_dvis_invigilator_greeting,limit=1] run function metrics-gym:dmask_invigilator_greeting
execute as @a[tag=dw_t_dvis_invigilator_greeting,limit=1] run scoreboard players operation #dm_invigilator_greeting dw.sys = @s dw.dmask
scoreboard players set #dmhi_invigilator_greeting dw.sys 2
scoreboard players operation #dm_invigilator_greeting dw.sys %= #dmhi_invigilator_greeting dw.sys
scoreboard players set #dmlo_invigilator_greeting dw.sys 1
scoreboard players operation #dm_invigilator_greeting dw.sys /= #dmlo_invigilator_greeting dw.sys
assert score #dm_invigilator_greeting dw.sys matches 0
scoreboard players set #party dw.qa_walk_the_ladder 1
scoreboard players set #party dw.o_hear_the_brief 1
execute as @a[tag=dw_t_dvis_invigilator_greeting,limit=1] run function metrics-gym:dmask_invigilator_greeting
execute as @a[tag=dw_t_dvis_invigilator_greeting,limit=1] run scoreboard players operation #dm_invigilator_greeting dw.sys = @s dw.dmask
scoreboard players set #dmhi_invigilator_greeting dw.sys 2
scoreboard players operation #dm_invigilator_greeting dw.sys %= #dmhi_invigilator_greeting dw.sys
scoreboard players set #dmlo_invigilator_greeting dw.sys 1
scoreboard players operation #dm_invigilator_greeting dw.sys /= #dmlo_invigilator_greeting dw.sys
assert score #dm_invigilator_greeting dw.sys matches 0
scoreboard players set #party dw.o_hear_the_brief 0
