scoreboard objectives add dwl.s dummy
scoreboard players set #on dwl.s 0
forceload add 4080 4080 4127 4319
function dwl:corridor/build
scoreboard players set #released dwl.s 0
scoreboard players set #moves dwl.s 0
scoreboard players set #thick dwl.s 0
scoreboard players set #caught dwl.s 0
scoreboard players set #on dwl.s 1
