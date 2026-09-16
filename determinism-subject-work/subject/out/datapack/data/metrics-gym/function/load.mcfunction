scoreboard objectives add dw.sys dummy
execute unless score #init dw.sys matches 1 run function metrics-gym:setup
