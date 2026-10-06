scoreboard objectives add dwa.s dummy
scoreboard players set #on dwa.s 0
kill @e[tag=dwa]
forceload add 4072 4072 4326 4140
scoreboard players set #wait dwa.s 0
schedule function dwa:build_wait 5t
