# SPIKE PROBE — the command-driven area-damage alternative.
# `minecraft:explode` centres on the impact point, which is INSIDE the block the
# arrow hit, so its own damage collapses to the fully-occluded floor of 1.0.
# `/damage` in a radius has no line-of-sight term and no orientation, so this
# measures what the engine would get by driving the damage itself from the same
# `hit_block` event.
scoreboard players add #dmg_hits dw.sig 1
execute as @e[type=!minecraft:arrow,distance=..2] run damage @s 12 minecraft:explosion
execute as @e[type=!minecraft:arrow,distance=2..4] run damage @s 6 minecraft:explosion
particle minecraft:explosion_emitter ~ ~ ~ 0 0 0 0 1
playsound minecraft:entity.generic.explode master @a ~ ~ ~ 2 1
