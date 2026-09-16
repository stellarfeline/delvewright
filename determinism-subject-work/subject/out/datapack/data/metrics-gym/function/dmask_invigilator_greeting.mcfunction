scoreboard players set @s dw.dmask 0
execute if score #party dw.qa_walk_the_ladder matches 1 unless score #party dw.o_hear_the_brief matches 1 run scoreboard players add @s dw.dmask 1
