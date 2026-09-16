# Environment sealing (spec-0002): box garden — nothing left to vanilla chance.
gamerule spawn_mobs false
gamerule advance_time false
gamerule advance_weather false
gamerule fire_spread_radius_around_player 0
gamerule mob_griefing false
gamerule respawn_radius 0
gamerule keep_inventory true
gamerule send_command_feedback false
time set noon
gamerule tnt_explodes false
weather clear
scoreboard objectives add dw.class trigger
scoreboard objectives add dw.classed dummy
scoreboard objectives add dw.dlg_shown dummy
scoreboard objectives add dw.dlg_invigilator trigger
scoreboard objectives add dw.o_hear_the_brief dummy
scoreboard objectives add dw.o_reach_the_far_end dummy
scoreboard objectives add dw.qa_walk_the_ladder dummy
scoreboard objectives add dw.q_walk_the_ladder dummy
scoreboard objectives add dw.campaign dummy
scoreboard objectives add dw.dmask dummy
scoreboard objectives add dw.cast dummy
scoreboard objectives add dw.ann_hear_the_brief dummy
scoreboard objectives add dw.ann_reach_the_far_end dummy
forceload add 3 3 8 8
forceload add 8 3 17 12
forceload add 17 3 50 36
forceload add 50 3 115 68
forceload add 115 3 180 68
forceload add 180 3 309 132
forceload add 309 3 326 20
forceload add 326 3 359 36
forceload add 359 3 368 12
forceload add 368 3 385 20
forceload add 385 3 390 12
forceload add 390 3 395 24
forceload add 395 3 404 24
forceload add 404 3 417 24
forceload add 417 3 434 24
forceload add 8 12 17 21
forceload add 17 36 34 53
forceload add 17 53 34 70
scoreboard players set #placed dw.sys 0
scoreboard players set #init dw.sys 1
