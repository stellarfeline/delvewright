execute unless score #on dwl.s matches 1 run return 0
execute if score #released dwl.s matches 0 as @a[x=4095,y=64,z=4218,dx=2,dy=2,dz=0] at @s run function dwl:loop
execute if score #thick dwl.s matches 0 as @a[x=4094,y=150,z=4300,dx=4,dy=0,dz=4] run function dwl:catch
execute if score #thick dwl.s matches 1 as @a[x=4094,y=150,z=4300,dx=4,dy=2,dz=4] run function dwl:catch
