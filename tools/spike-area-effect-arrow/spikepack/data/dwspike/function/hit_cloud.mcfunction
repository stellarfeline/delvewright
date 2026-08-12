# SPIKE PROBE — the splash-potion-style half: a lingering area effect at impact.
scoreboard players add #cloud_hits dw.sig 1
summon minecraft:area_effect_cloud ~ ~ ~ {Tags:["dwspike_cloud"],Radius:3.5f,RadiusPerTick:0.0f,RadiusOnUse:0.0f,Duration:200,WaitTime:0,ReapplicationDelay:10,potion_contents:{custom_effects:[{id:"minecraft:instant_damage",amplifier:0,duration:1},{id:"minecraft:glowing",amplifier:0,duration:200}]}}
