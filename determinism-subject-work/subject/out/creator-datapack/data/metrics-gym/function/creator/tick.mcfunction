scoreboard players enable @a dw.note
execute as @a[scores={dw.note=1..}] at @s run function metrics-gym:creator/stamp
scoreboard players enable @a dw.cam
scoreboard players enable @a dw.free
execute as @a[scores={dw.cam=1..}] at @s run function metrics-gym:creator/camera/cam
execute as @a[scores={dw.free=1..}] at @s run function metrics-gym:creator/camera/free
