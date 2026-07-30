# Delvewright validation bot (spec-0003): the mineflayer harness, containerized so
# the `validate` compose profile / CI runs it exactly as locally. Reads a
# critical-path.json and drives the run; exits 0 on pass, 1 on any failure.
#
# Node 24 runs the harness's .ts sources directly (native type stripping), matching
# `npm start` locally — no build step, no emitted JS.
FROM node:24-bookworm-slim

# git + build tools for any mineflayer transitive native/optional deps.
RUN apt-get update \
 && apt-get install -y --no-install-recommends python3 make g++ ca-certificates \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Install deps against the committed lockfile (reproducible).
COPY package.json package-lock.json ./
RUN npm ci

COPY tsconfig.json ./
COPY src ./src

# The critical-path.json is mounted at /delve/critical-path.json by compose/CI.
ENV DELVEWRIGHT_MC_HOST=server \
    DELVEWRIGHT_MC_PORT=25565 \
    DELVEWRIGHT_MC_VERSION=1.21.11 \
    DELVEWRIGHT_MC_AUTH=offline
ENTRYPOINT ["node", "src/run.ts"]
CMD ["/delve/critical-path.json"]
