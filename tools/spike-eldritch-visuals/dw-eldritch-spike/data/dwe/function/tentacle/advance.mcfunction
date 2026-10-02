scoreboard players set #ttick dwe.s 0
execute store result storage dwe:t f int 1 run scoreboard players get #tframe dwe.s
function dwe:tentacle/play with storage dwe:t
scoreboard players add #tframe dwe.s 1
execute if score #tframe dwe.s matches 96.. run scoreboard players set #tframe dwe.s 36
