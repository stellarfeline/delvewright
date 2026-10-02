execute as @a[x=4095,y=63,z=4096,dx=4,dy=3,dz=4,tag=!dw_cutscene] run damage @s 6 minecraft:generic
scoreboard players add #lands dwa.s 1
execute store result score #land_at dwa.s run time query gametime
function dwa:play/0_0
scoreboard players set #sm dwa.s 0
