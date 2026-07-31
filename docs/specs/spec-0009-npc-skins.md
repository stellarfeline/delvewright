# spec-0009: NPC skin pipeline — creation-first, resource-pack delivery

- **Status**: Approved (owner, 2026-07-31, via chat)
- **Context**: mannequin player-model NPCs (spec-0008 §6); ADR-0003 (vanilla-first),
  0006 (determinism), 0010 (OCI packaging), 0013 (license allowlist). All mechanics
  below verified live on 1.21.11 (spike, 2026-07-31).

## Architecture (settled by research + spike)

An NPC skin is an **original 64×64 PNG shipped in a per-delve server resource
pack**, referenced by the mannequin's profile component:
`profile: { texture: "delvewright:npc/<skin-id>", model: "wide"|"slim" }` —
texture path resolves against `assets/delvewright/textures/` in the pack. Pure
vanilla (data component + resource pack), zero external services, offline-safe,
byte-deterministic.

Rejected routes (recorded so nobody retries them): player-name profiles
(online-mode-only Mojang lookup + skin impersonation), base64 texture properties
(clients only honor Mojang-signed textures on Mojang's host — verified: unsigned
arbitrary-URL values are stored but never rendered).

**Creation is the only compliant acquisition route**: skin sites are unlicensed
user uploads (ADR-0013 → unusable). There is no scavenging track for skins.

## Hard facts the implementation must honor (spike-verified)

- Resource-pack `pack_format` for 1.21.11 is **75** (bare field; the ≥81
  `min_format`/`max_format` rule applies to datapacks, not this).
- **`model` is MANDATORY in emitted profiles** — omitted = silently slim; a wide
  skin on a slim model renders distorted. The cast sheet records it per skin;
  the compiler always emits it.
- Serving: the OCI delve serves the pack zip itself (verified pattern: busybox
  httpd sidecar; itzg `RESOURCE_PACK` + `RESOURCE_PACK_SHA1` env → forwarded
  verbatim to clients; the server never downloads the pack). `RESOURCE_PACK_PROMPT`
  must be a JSON text component, not a bare string.
- Default **prompt-only**; `require-resource-pack=true` insta-kicks decliners
  (`multiplayer.requiredTexturePrompt.disconnect`) — acceptable for a fixed
  group, owner-flippable, never the silent default.
- *Open packaging item*: the pack URL must be reachable by the **client** — the
  advertised host:port must be templated at container start (the container
  cannot know its own public address). Lands with the packaging work.

## Workflow: cast sheet → create → preview-verify → admit → bake

1. **Cast sheet** (free from the DSL npcs stage): one row per character —
   skin id, `model`, `hidden_layers`, style brief tied to the persona. The skin
   analogue of spec-0007's demand-sheet minimums; quota = every skinned NPC cast.
2. **Create** (agent, generation-time): layered composition via
   `skinpy-extended` (MIT — part/face/3D-coordinate pixel addressing, no raw
   UV-atlas arithmetic); optional AI draft as a starting point only.
3. **Preview-verify**: headless multi-angle player-model render (skinview3d
   lineage, Node — lives in the harness toolchain; **Nucleation cannot render
   player models**, do not try) → the authoring agent critiques front/back/sides
   and seams, repairs, repeats until pass.
4. **Admit**: catalog card per spec-0007 conventions (`catalog/skin-<id>.json`):
   description, tags (role/style/palette/model), quality, preview paths,
   **license = original** always.
5. **Bake**: the compiler packs approved PNGs into the delve resource pack,
   emits the mannequin profile per NPC, and records the pack sha1 in the build
   manifest (determinism: pack bytes are part of the byte-identity contract).

## Acceptance criteria

- [ ] Compiler emits a resource pack (format 75) + sha1 for any campaign with a
      skinned NPC; build manifest records it; double-build byte-identical.
- [ ] Emitted profiles always carry `model`; a cast-sheet entry without `model`
      is a compile-time DW error.
- [ ] Validation compose serves the pack; a vanilla client joining sees the
      custom skin (owner-verified once; thereafter the handshake + sha1 check is
      the CI proxy).
- [ ] Skin preview renderer runs headless in the harness; a sample skin's
      4-angle set is produced deterministically.
- [ ] Catalog cards exist for every shipped skin, license `original`; CI rejects
      a skinned build whose skin lacks a card.
