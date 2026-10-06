# The two objectives the rig reads: dwsl.f is the gate (the stand-in for dw.f_<flag>), dwsl.s the readings.
scoreboard objectives add dwsl.f dummy
scoreboard objectives add dwsl.s dummy
scoreboard players set #party dwsl.f 0
scoreboard players set #killtick dwsl.s -1
scoreboard players set #settick dwsl.s -1
scoreboard players set #kills dwsl.s 0
