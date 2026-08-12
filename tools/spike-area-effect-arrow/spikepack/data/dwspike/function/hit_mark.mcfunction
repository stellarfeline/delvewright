# SPIKE PROBE — measures the EXECUTION CONTEXT of `minecraft:run_function`
# reached from an enchantment's `minecraft:hit_block` effect. Measurement only:
# nothing here is an emission path and the compiler never sees it.
scoreboard players add #mark_hits dw.sig 1
# position context of the function itself
summon minecraft:marker ~ ~ ~ {Tags:["dwspike_ctx"]}
# is there an executor at all, and what is it?
execute if entity @s run scoreboard players set #mark_has_exec dw.sig 1
execute if entity @s[type=minecraft:arrow] run scoreboard players set #mark_exec_arrow dw.sig 1
execute if entity @s[type=minecraft:player] run scoreboard players set #mark_exec_player dw.sig 1
execute as @s run tag @s add dwspike_exec
# position context of the executor, for comparison with the function's own
execute at @s run summon minecraft:marker ~ ~ ~ {Tags:["dwspike_execpos"]}
