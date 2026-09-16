#> The Metrics Gym: objective completions set dw.campaign (Delvewright mechanism test)
# @dummy
# @timeout 100

function metrics-gym:setup
tag @p add dw_t_camp
function metrics-gym:pt_camp_drive
assert score #party dw.campaign matches 1
