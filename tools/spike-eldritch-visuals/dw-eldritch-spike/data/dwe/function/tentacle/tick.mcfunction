execute if score #tframe dwe.s matches -1 if entity @a[x=4084,y=60,z=4097,dx=24,dy=20,dz=30] run function dwe:tentacle/wake
execute if score #tframe dwe.s matches 1.. run scoreboard players add #ttick dwe.s 1
execute if score #ttick dwe.s matches 5.. run function dwe:tentacle/advance
