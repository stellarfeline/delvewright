#> The Metrics Gym: NPC `npc/invigilator`'s right-click reward runs and re-arms the interaction
# @dummy
# @timeout 100

function metrics-gym:setup
tag @p add dw_t_tlk_invigilator
scoreboard players reset @a[tag=dw_t_tlk_invigilator,limit=1] dw.cast
# Grant the interaction advancement, exactly as a right-click
# does: the record is written.
execute as @a[tag=dw_t_tlk_invigilator,limit=1] run advancement grant @s only metrics-gym:invigilator_interact
execute as @a[tag=dw_t_tlk_invigilator,limit=1] run function metrics-gym:talk_invigilator
execute as @a[tag=dw_t_tlk_invigilator,limit=1] if entity @s[advancements={metrics-gym:invigilator_interact=false}] run scoreboard players set #tlk_invigilator dw.sys 1
assert score #tlk_invigilator dw.sys matches 1
execute as @a[tag=dw_t_tlk_invigilator,limit=1] store success score #cst_invigilator dw.sys if score @s dw.cast matches -2147483648..
assert score #cst_invigilator dw.sys matches 1
