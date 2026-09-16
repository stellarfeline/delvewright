scoreboard players set #party dw.campaign 1
advancement grant @a only metrics-gym:campaign_complete
tellraw @a [{"color":"gold","fallback":"%s — complete.","translate":"delve.metrics-gym.delvewright.ui.campaign.complete","with":[{"fallback":"The Metrics Gym","translate":"delve.metrics-gym.world.title"}]},{"text":"\n"},{"color":"gray","fallback":"A Delvewright delve.","translate":"delve.metrics-gym.delvewright.ui.campaign.signature"}]
title @a title {"bold":true,"color":"gold","fallback":"Delve Complete","translate":"delve.metrics-gym.delvewright.ui.campaign.banner"}
title @a subtitle {"color":"yellow","fallback":"The Metrics Gym","translate":"delve.metrics-gym.world.title"}
playsound minecraft:ui.toast.challenge_complete player @a
tellraw @a {"color":"dark_gray","text":"[dw:complete metrics-gym campaign]"}
