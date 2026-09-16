function metrics-gym:dmask_invigilator_greeting
execute if score @s dw.dmask matches 0 run dialog show @s metrics-gym:invigilator_greeting__m0
execute if score @s dw.dmask matches 1 run dialog show @s metrics-gym:invigilator_greeting__m1
