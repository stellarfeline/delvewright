#> The Metrics Gym: a SCHEDULED function still reaches the party (scheduled-executor contract)
# @dummy
# @timeout 100

function metrics-gym:setup
scoreboard objectives add dw.f_pt_sched_probe dummy
scoreboard players set #party dw.f_pt_sched_probe 0
schedule function metrics-gym:pt_sched_probe 2t
await score #party dw.f_pt_sched_probe matches 1
