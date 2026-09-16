#> The Metrics Gym: every NPC summon resolves to exactly one entity
# @dummy
# @timeout 100

function metrics-gym:setup
scoreboard players set #placed dw.sys 1
kill @e[tag=dw_npc_invigilator]
function metrics-gym:setup_finish
execute store result score #npc_invigilator dw.sys if entity @e[tag=dw_npc,tag=dw_npc_invigilator]
assert score #npc_invigilator dw.sys matches 1
