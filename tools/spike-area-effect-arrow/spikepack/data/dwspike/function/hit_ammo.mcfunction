# SPIKE PROBE — can the effect branch on the AMMUNITION's identity?
# The enchantment lives on the BOW; this asks whether the arrow entity still
# carries the shot arrow ITEM's components at impact, so a generic launcher
# enchantment can distinguish scavenged bomb-arrows from ordinary ones.
scoreboard players add #ammo_hits dw.sig 1
execute if data entity @s {item:{components:{"minecraft:custom_data":{dw_bomb:1b}}}} run scoreboard players add #ammo_matched dw.sig 1
execute if data entity @s {item:{components:{"minecraft:custom_data":{dw_bomb:1b}}}} run summon minecraft:marker ~ ~ ~ {Tags:["dwspike_ammo"]}
