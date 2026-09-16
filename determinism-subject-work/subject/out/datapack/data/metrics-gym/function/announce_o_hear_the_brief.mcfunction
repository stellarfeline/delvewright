tellraw @a {"bold":true,"color":"yellow","fallback":"New objective: %s","translate":"delve.metrics-gym.delvewright.ui.objective.new","with":[{"bold":false,"color":"gold","fallback":"Hear the brief","translate":"delve.metrics-gym.obj.walk-the-ladder.hear-the-brief.title"}]}
tellraw @a {"color":"gray","fallback":"The Invigilator stands in the smallest bay.","italic":true,"translate":"delve.metrics-gym.obj.walk-the-ladder.hear-the-brief.hint"}
playsound minecraft:block.note_block.pling player @a
scoreboard players set #party dw.ann_hear_the_brief 1
