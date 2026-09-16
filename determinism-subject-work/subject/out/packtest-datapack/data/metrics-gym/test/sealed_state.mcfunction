#> The Metrics Gym: environment sealed on boot (spec-0002)
# @dummy
# @timeout 100

function metrics-gym:setup
# time set noon -> daytime 6000 (the sole sealing command with a
# vanilla read-back path; gamerules are asserted at compile time).
execute store result score #sealtime_sealed dw.sys run time query daytime
assert score #sealtime_sealed dw.sys matches 6000
