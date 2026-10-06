# One function, so the tick stamp and the gate flip land in the same server tick.
execute store result score #settick dwsl.s run time query gametime
scoreboard players set #party dwsl.f 1
