execute if score #init dw.sys matches 1 unless score #placed dw.sys matches 1 run function metrics-gym:place_all
execute if score #init dw.sys matches 1 unless score #placed dw.sys matches 1 run function metrics-gym:place_verify
execute if score #placed dw.sys matches 1 as @a[tag=!dw_joined] run function metrics-gym:join_place
execute as @a run function metrics-gym:class_arm
scoreboard players enable @a dw.dlg_invigilator
execute as @a unless score @s dw.classed matches 1 unless score @s dw.dlg_shown matches 1 run function metrics-gym:show_class
execute as @a[scores={dw.class=1}] unless score @s dw.classed matches 1 run function metrics-gym:class_apply_measurer
execute as @a[scores={dw.dlg_invigilator=1}] run function metrics-gym:dlg_invigilator_1
execute as @a[scores={dw.dlg_invigilator=2}] run function metrics-gym:dlg_invigilator_2
execute as @a[scores={dw.dlg_invigilator=3}] run function metrics-gym:dlg_invigilator_3
execute if score #party dw.qa_walk_the_ladder matches 1 unless score #party dw.o_hear_the_brief matches 1 unless score #party dw.ann_hear_the_brief matches 1 run function metrics-gym:announce_o_hear_the_brief
execute if score #party dw.qa_walk_the_ladder matches 1 if score #party dw.o_hear_the_brief matches 1 unless score #party dw.o_reach_the_far_end matches 1 unless score #party dw.ann_reach_the_far_end matches 1 run function metrics-gym:announce_o_reach_the_far_end
execute if score #party dw.qa_walk_the_ladder matches 1 if score #party dw.o_hear_the_brief matches 1 unless score #party dw.o_reach_the_far_end matches 1 unless score #act_reach_the_far_end dw.sys matches 1 run function metrics-gym:activate_o_reach_the_far_end
execute as @a if score #party dw.qa_walk_the_ladder matches 1 if score #party dw.o_hear_the_brief matches 1 unless score #party dw.o_reach_the_far_end matches 1 if entity @s[x=422,dx=6,y=61,dy=6,z=10,dz=6] run function metrics-gym:complete_o_reach_the_far_end
