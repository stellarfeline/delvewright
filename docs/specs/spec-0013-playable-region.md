# spec-0013 — Playable region & ocean horizon (pseudo-open-world staging)

- **Status**: Draft (planner-authored per owner direction, 2026-08-01)
- **Motivation**: owner verdict on nobodys-cave — box-garden maps must be able to
  *look* like a coherent open world (sky, horizon, sea) while containing zero
  content outside the scripted area. Two halves: a scenic horizon, and a
  boundary that returns wanderers to the story.

## DSL surface (stage 1 `world`, v0.6)

| Field | Behavior |
|-------|----------|
| `horizon` (opt) | `"void"` (default, byte-identical to v0.5) \| `"ocean"`. Ocean = superflat generator: bedrock, stone, water layers, **sea level y=62** (areas sit at y=64+, so land reads as islands). Deterministic world config, no structures/mobs. |
| `boundary` (opt) | `{margin?: u16 = 16, message?: string}`. Declares the playable region and its enforcement. `message` → l10n key `world.boundary.message` (default provided). |

## Semantics

- **Region is derived, not authored**: union of all placed-piece AABBs (final
  assembled layout) inflated horizontally by `margin`; unbounded vertically
  upward, floor at the lowest placed block − 8. Derivation makes the "every
  anchor/waypoint is inside" proof structural — nothing to get wrong.
- **Enforcement**: a per-second check; any player outside the region is
  teleported to the **last activated checkpoint** (spec-0012's `dw:cp` storage,
  initialized to spawn at setup) via a macro function, with the boundary
  message (actionbar) and a soft sound cue. No damage, no items lost.
- Out-of-region is **not** death and **not** a diagnostic at runtime — it is
  scenery you can look at but not inhabit.

## Diagnostics

- **DW0320** (validation, exit 1): `horizon: "ocean"` without `boundary` —
  an infinite swimmable sea with no return rule is an authoring error.
- **DW0321** (validation, exit 1): `margin` outside `0..=64`.
- Numbers may shift at implementation if the range collides; the DW gate and
  `docs/reference/compiler.md` are authoritative.

## Emission

- Ocean horizon: `level-type=minecraft:flat` + pinned generator-settings JSON in
  the emitted server config (byte-identical across builds).
- Boundary: setup writes region bounds into `dw:region` storage; a scheduled
  1s clock runs `execute as @a unless entity @s[x=…,dx=…]` → macro tp from
  `dw:cp`, actionbar message, sound.

## Validation / acceptance criteria

1. `horizon: "ocean"` build is byte-identical across double builds; `"void"`
   and absent field produce output identical to v0.5.
2. Negative fixtures assert DW0320 and DW0321 by code.
3. PackTest: a fake player teleported outside the region is returned to the
   last checkpoint within 2s and receives the boundary message; a player
   inside the region is never moved.
4. Critical-path bot run on a boundary-enabled campaign is unaffected (bot
   never leaves the region).

## Non-goals

Invisible walls (motion is never blocked, only reset); per-area sub-regions;
non-rectangular regions; ocean content (boats, swimming legs) — the sea is
backdrop unless a future spec says otherwise.
