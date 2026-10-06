execute if score #sm dwa.s matches 0 if entity @a[x=4090,y=63,z=4090,dx=12,dy=4,dz=22] run function dwa:strike/begin
execute if score #sm dwa.s matches 1 if score #done_0 dwa.s matches 1 run function dwa:strike/hold
execute if score #sm dwa.s matches 2 run scoreboard players add #hold dwa.s 1
execute if score #sm dwa.s matches 2 if score #hold dwa.s matches 20.. run function dwa:strike/swing
execute if score #sm dwa.s matches 3 if score #done_0 dwa.s matches 1 run function dwa:strike/land
