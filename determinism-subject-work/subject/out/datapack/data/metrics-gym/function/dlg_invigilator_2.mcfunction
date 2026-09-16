scoreboard players reset @s dw.dlg_invigilator
scoreboard players enable @s dw.dlg_invigilator
execute if score #party dw.qa_walk_the_ladder matches 1 unless score #party dw.o_hear_the_brief matches 1 run function metrics-gym:complete_o_hear_the_brief
