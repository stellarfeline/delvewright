ARG BASE=ghcr.io/stellarfeline/delvewright-base@sha256:3e7db2562b492dbf442568a327d361547628c98c04a7cb68218c8dde6abdd1de
FROM ${BASE}
COPY datapack /dw/datapack
ENV TYPE=VANILLA VERSION=1.21.11 DATAPACKS=/dw/datapack \
    MODE=creative DIFFICULTY=peaceful \
    LEVEL_TYPE=minecraft:flat \
    GENERATOR_SETTINGS="{\"biome\":\"minecraft:the_void\",\"layers\":[]}" \
    GENERATE_STRUCTURES=false SPAWN_MONSTERS=false ONLINE_MODE=FALSE \
    ENABLE_RCON=true RCON_PASSWORD=dw MEMORY=2G
