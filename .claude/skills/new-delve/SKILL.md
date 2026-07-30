---
name: new-delve
description: Generate a complete playable Minecraft delve from a creative prompt — staged DSL authoring with validation-loop self-repair, deterministic compile, machine validation, joinable output. Use when the user asks to create/generate a new delve or campaign. Args = the creative prompt (theme one-liner or detailed brief).
---

# /new-delve — the Delvewright generation front-end (ADR-0012)

You are authoring a delve campaign as staged DSL documents. You NEVER write
mcfunction, dialogs, or datapack files — `delvec` compiles everything (ADR-0001).
Read `CLAUDE.md` first if you haven't; forbidden zones apply in full.

## Inputs

The user's prompt is a **constraint set over the DSL stages**: honor everything it
pins down (theme, specific levels, plot beats, NPCs, homages); invent the rest.
Ask 2–3 clarifying questions ONLY if the prompt is too thin to pick a theme and a
target length; otherwise proceed.

Mode: default **interactive** (pause after each stage for the user to review a
short summary — not the raw JSON — and confirm/adjust). If the user says "e2e" /
"一口气" / "don't stop", run straight through.

## Campaign workspace (artifact of record — NEVER skip)

Campaigns do not live in the repo (CLAUDE.md forbidden zone). Create
`../delvewright-campaigns/<campaign-id>/` (override: `$DELVEWRIGHT_CAMPAIGNS_DIR`)
containing the six stage JSONs, the build output, and a `GENERATION.md` (prompt
verbatim, date, dsl_version, decisions made). The DSL documents are the artifact
of record: the delve must be rebuildable byte-identically from them without any
LLM (ADR-0006/0012).

## The loop

For each stage in order — world → npcs → classes → quest-plan → quests → dialogue:

1. `cargo run -q -p delvewright-compiler --bin delvec -- schema --stage <n>` —
   generate AGAINST the live schema, never from memory.
2. Write the stage JSON. Craft constraints:
   - Areas: prefer `prefab_pool` (stone-keep tileset) for real layouts; check
     `prefabs/pools.json` + prefab metadata for available pools/anchors/lighting
     profiles. Respect the lighting contract — darkness only as declared design
     with a mitigation the quest DAG provides.
   - NPCs: personas per schema (archetype/speech_style/motivation required);
     honor them in every stage-6 line. Dialogue: branching options; flavor NPCs
     get real trees too.
   - Pace to `target_minutes`; no grind (forbidden zone); mandatory-only quests.
3. `delvec validate <campaign-dir>` — fix by diagnostic code (DW####; see
   `crates/dsl/README.md` + `crates/compiler/README.md` tables). Loop until clean.
   Three failed repairs on the same code → stop and think about the design instead
   of patching syntax.
4. Interactive mode: present a 3–6 line summary of the stage; wait.

Then:

5. `delvec analyze <campaign-dir>` — reachability/deadlock/dark-mitigation. Fix in
   the DSL (never by weakening the campaign; a dead quest is a design bug).
6. `delvec build <campaign-dir> -o <workspace>/out` — must exit 0.
7. Machine validation ladder (from repo root, docker required):
   - copy/point `validation/delve-output` at the build output
   - `EULA=TRUE docker compose -f validation/compose.yaml --profile packtest up --exit-code-from packtest`
   - `EULA=TRUE docker compose -f validation/compose.yaml --profile validate up --build --abort-on-container-exit --exit-code-from bot`
   Both must exit 0. A red bot run = fix the campaign (or report a compiler bug —
   do not hand-edit compiler output, ever).
8. Report to the user: campaign summary, playtime estimate, validation results,
   and the two commands they care about:
   - play: `EULA=TRUE docker compose -f validation/compose.yaml --profile play up`
   - playtest with notes: same with `--profile playtest` (+ `CREATOR_NAME=<mc name>`)

## Hard rules

- Persist the DSL workspace before validation, not after — a crash must never
  lose the campaign.
- Every player-visible string in English unless the prompt requests otherwise
  (owner prompts in Chinese still yield English defaults unless she says 中文文本).
- Homages: original text only, cultural reference never asset ingestion
  (ADR-0007).
- If a mechanic the prompt wants has no DSL verb, do NOT fake it with adjacent
  verbs silently — tell the user what's missing and offer the closest authorable
  alternative (spec change requests go to the planning session, not this skill).
