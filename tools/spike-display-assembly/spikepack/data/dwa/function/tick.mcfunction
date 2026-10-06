execute unless score #on dwa.s matches 1 run return 0
function dwa:anim/tick
execute if score #poll dwa.s matches 1 run function dwa:hit/tick
function dwa:strike/tick
