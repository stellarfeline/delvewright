advancement revoke @s only metrics-gym:invigilator_interact
function metrics-gym:cast_invigilator
execute if score @s dw.cast matches 0 run function metrics-gym:show_invigilator_greeting
execute if score @s dw.cast matches 1 run function metrics-gym:show_invigilator_greeting
