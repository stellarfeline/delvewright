#> The Metrics Gym: dialogue trigger re-arms without a tick (singleplayer pause parity)
# @dummy
# @timeout 100

function metrics-gym:setup
tag @p add dw_t_rearm
# The per-tick re-enable, run ONCE. Nothing below runs the tick
# function again: that suppression IS the integrated server's
# pause-menu tick freeze, which a dedicated server never enters.
scoreboard players enable @a[tag=dw_t_rearm,limit=1] dw.dlg_invigilator
execute as @a[tag=dw_t_rearm,limit=1] run trigger dw.dlg_invigilator set 2
assert score @a[tag=dw_t_rearm,limit=1] dw.dlg_invigilator matches 2
# The tick's dispatch, hand-run: the handler consumes (and locks) the
# trigger, then must re-arm it itself.
execute as @a[tag=dw_t_rearm,limit=1] run function metrics-gym:dlg_invigilator_2
# Second use, still with no tick in between. If the handler did not
# re-arm, vanilla rejects this and the score stays unset.
execute as @a[tag=dw_t_rearm,limit=1] run trigger dw.dlg_invigilator set 2
assert score @a[tag=dw_t_rearm,limit=1] dw.dlg_invigilator matches 2
