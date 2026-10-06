# Volume A: feet-level box x 100..102, y 1..2, z 100..102 (the stone top is y 0, so a body stands at y 1).
execute if entity @a[x=100,dx=2,y=1,dy=1,z=100,dz=2] store result score #killtick dwsl.s run time query gametime
execute as @a[x=100,dx=2,y=1,dy=1,z=100,dz=2] run function dwsl:kill
# Volume B: the same box shifted +10 in x and ONE COURSE UP (y 2..3), so a body standing at y 1 reaches into it from below.
execute if entity @a[x=110,dx=2,y=2,dy=1,z=100,dz=2] store result score #killtick dwsl.s run time query gametime
execute as @a[x=110,dx=2,y=2,dy=1,z=100,dz=2] run function dwsl:kill
