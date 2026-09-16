#> The Metrics Gym: npc `npc/invigilator`'s cast ladder selects each declared clause's own scene
# @dummy
# @timeout 100

function metrics-gym:setup
tag @p add dw_t_clad_invigilator
# Every phase pins EVERYTHING the ladder reads — quest actives,
# branch flags, datums. The batch is one shared server; a term left
# undriven is decided by whichever sibling template ran last.
# -- no declaring quest has begun: the stage-6 root governs (scene 0) --
scoreboard players set #party dw.qa_walk_the_ladder 0
execute as @a[tag=dw_t_clad_invigilator,limit=1] run function metrics-gym:cast_invigilator
assert score @a[tag=dw_t_clad_invigilator,limit=1] dw.cast matches 0
# -- clause 1: quest `quest/walk-the-ladder` placement 0 -> scene 1 --
scoreboard players set #party dw.qa_walk_the_ladder 1
execute as @a[tag=dw_t_clad_invigilator,limit=1] run function metrics-gym:cast_invigilator
assert score @a[tag=dw_t_clad_invigilator,limit=1] dw.cast matches 1
