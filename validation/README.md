# validation/

Docker-compose entrypoint for running Delvewright the same way CI and prod do
(ADR-0005, ADR-0008, ADR-0010, spec-0003). "Works on my machine" is defined as
"this compose profile passes". Two profiles:

- **`play`** (available now) — a bare, pinned vanilla **1.21.11** server the owner
  can join by hand from her own client to check a delve at any time.
- **`validate`** (stub, comments in `compose.yaml`) — the full two-layer dynamic
  validation stack (server + PackTest runner + mineflayer bot, exit codes
  propagated to CI). Arrives with the rest of spec-0003 once the compiler emits a
  delve.

## One command (owner)

EULA acceptance is **your** action and is never hardcoded in this repo. Pass it in
your environment (this is Mojang's EULA: https://aka.ms/MinecraftEULA):

```sh
EULA=TRUE docker compose -f validation/compose.yaml --profile play up
```

Then join from a vanilla **1.21.11** client at `localhost:25565`. Stop with
`Ctrl-C`; `... --profile play down` removes the container (the world persists in the
`delvewright-play-data` volume). To join with a real Microsoft account instead of
offline mode, add `ONLINE_MODE=TRUE` (auth mode is still an open spec-0003 decision).

If `EULA` is unset, compose fails fast with a message telling you to set it — by
design.

## What works today vs with M1 integration

**Today (bare pinned server):**

- Boots vanilla 1.21.11 via the `itzg/minecraft-server` image, which downloads the
  pinned server jar at runtime (never baked into a layer — the ADR-0010 EULA-safe
  pattern).
- Empty world generated on first boot. No delve is loaded yet: the compiler
  (spec-0002) does not exist, so there is nothing to mount.
- Port bound to `127.0.0.1` only — never world-reachable.

**Arrives with M1 integration:**

- The compiler's build output (`<out>/`: `manifest.json`, `datapack/`, `server/`,
  `packtest-datapack/`, `critical-path.json`) is mounted into the server so it
  loads the compiled delve. The placeholder mount path is documented (commented) in
  `compose.yaml`: `./delve-output/datapack -> /data/world/datapacks/<campaign-id>`.
- The `validate` profile is filled in: `packtest-runner` (exit code = failed tests)
  and `bot` (the `../harness` mineflayer runner reading `critical-path.json`,
  exiting 0/1). The tooling-mod overlay (PackTest + Fabric) is layered on only at
  compose time and must never leak into the shipped delve image.
- `docker compose --profile validate up` then reproduces CI locally with exit codes
  propagated (spec-0003 acceptance criteria).

## Harness

The mineflayer bot that the `validate` profile runs lives in [`../harness`](../harness).
It parses the compiler's `critical-path.json` contract and drives the run; it
contains **zero** campaign-specific logic (spec-0003). See that directory's code and
tests. Current status: parser + sequencer + a mineflayer executor skeleton
(`reach`/`assert-complete` implemented; `select-class`/`talk-to` stubbed pending the
dialog-vs-`tellraw` interaction-channel decision).
