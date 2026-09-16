scoreboard players reset @s dw.note
execute store result storage metrics-gym:note x int 1 run data get entity @s Pos[0]
execute store result storage metrics-gym:note y int 1 run data get entity @s Pos[1]
execute store result storage metrics-gym:note z int 1 run data get entity @s Pos[2]
data modify storage metrics-gym:note area set value "none"
execute if entity @s[x=3,dx=431,y=63,dy=20,z=3,dz=129] run data modify storage metrics-gym:note area set value "area/site"
data modify storage metrics-gym:note npc set value "none"
execute positioned as @s as @e[tag=dw_npc,sort=nearest,limit=1] if entity @s[tag=dw_npc_invigilator] run data modify storage metrics-gym:note npc set value "npc/invigilator"
execute store result storage metrics-gym:note o_hear_the_brief int 1 run scoreboard players get #party dw.o_hear_the_brief
execute store result storage metrics-gym:note o_reach_the_far_end int 1 run scoreboard players get #party dw.o_reach_the_far_end
function metrics-gym:creator/emit with storage metrics-gym:note
