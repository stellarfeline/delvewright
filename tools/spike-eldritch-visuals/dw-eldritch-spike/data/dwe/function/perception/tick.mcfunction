execute if score #p3 dwe.s matches 0 as @a[x=4096,y=64,z=4176,dx=0,dy=1,dz=0] at @s run function dwe:perception/hit
execute if score #p3 dwe.s matches 1 unless entity @a[x=4096.5,y=64,z=4176.5,distance=..5] run scoreboard players set #p3 dwe.s 0
