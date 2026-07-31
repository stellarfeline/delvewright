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

Campaigns do not live in this repo (CLAUDE.md forbidden zone) — they live in the
**`delvewright-campaigns` git repo** (symlinked at `campaigns/`, real path
`../delvewright-campaigns/`; override: `$DELVEWRIGHT_CAMPAIGNS_DIR`). Create
`campaigns/campaigns/<campaign-id>/` with the six stage JSONs and a
`GENERATION.md` (prompt verbatim, date, dsl_version, decisions made); build
output goes beside them (gitignored there). After validation passes, **commit the
campaign in that repo** (conventional message; do not push unless asked). The DSL documents are the artifact
of record: the delve must be rebuildable byte-identically from them without any
LLM (ADR-0006/0012).

## The loop

For each stage in order — world → npcs → classes → quest-plan → quests → dialogue:

1. `cargo run -q -p delvewright-compiler --bin delvec -- schema --stage <n>` —
   generate AGAINST the live schema, never from memory.
2. Write the stage JSON. Craft constraints:
   - Areas: prefer `prefab_pool` (stone-keep tileset) for real layouts; check
     `campaigns/prefabs/pools.json` + prefab metadata for available pools/anchors/lighting
     profiles. Respect the lighting contract — darkness only as declared design
     with a mitigation the quest DAG provides.
   - NPCs: personas per schema (archetype/speech_style/motivation required);
     honor them in every stage-6 line. Dialogue: branching options; flavor NPCs
     get real trees too.
   - Pace to `target_minutes`; no grind (forbidden zone); mandatory-only quests.
   - Objectives (v0.3): author `title` (short player-facing name, e.g. "Unbar the
     Deep Gate") and `hint` (one-line location/direction guidance, e.g. "The
     barred door stands in the corner room by the entrance hall") for **every
     non-`talk-to` objective**. The compiler surfaces them in-game when the
     objective activates (chat + sound); without them the player gets no guidance
     and cannot find interact/collect/reach targets. `talk-to` objectives may omit
     them (the NPC dialog is self-explanatory).
3. `delvec validate <campaign-dir>` — fix by diagnostic code (DW####; see
   `crates/dsl/README.md` + `crates/compiler/README.md` tables). Loop until clean.
   Three failed repairs on the same code → stop and think about the design instead
   of patching syntax.
4. Interactive mode: present a 3–6 line summary of the stage; wait.

### Localization stage (only when the prompt asks for other languages)

If the prompt requests one or more languages — or the user prompts in a
non-English language **and asks for localized in-game text** (中文文本 etc.) — add a
**final generation stage after `dialogue`**, once the English campaign is complete:

1. Declare the codes in `world.json`: `"languages": ["zh-cn", …]` (BCP-47-style;
   `en` is implicit/canonical and is **never** listed). Stage docs stay English.
2. `delvec schema` has no l10n stage; get the exact key inventory by writing the
   sidecar and letting `delvec validate` tell you what is missing/orphan
   (`DW0180`/`DW0181`). Author `l10n/<code>.json`:
   `{ dsl_version, campaign_id, kind: "l10n", lang: "<code>", content: { <key>: … } }`.
3. **Translate FROM the finished English** (never author a language natively) —
   honor each NPC's `persona.speech_style` in the target language, and keep a
   Minecraft-appropriate register. Cover the inventory **exactly**.
4. Re-`validate` until zero `DW0180`/`DW0181`. The default build stays English;
   `delvec build --lang <code>` emits the localized delve (same layout, strings
   swapped; `critical-path.json` is language-neutral so the ladder is unchanged).

Then:

5. `delvec analyze <campaign-dir>` — reachability/deadlock/dark-mitigation. Fix in
   the DSL (never by weakening the campaign; a dead quest is a design bug).
6. `delvec build <campaign-dir> -o <workspace>/out` — must exit 0.
7. Machine validation ladder — **delegate to a `sonnet` subagent** (owner policy
   2026-07-30: execution is mechanical, no creativity needed; also keeps long
   server logs out of the authoring context). Spawn an Agent
   (`subagent_type: general-purpose`, `model: sonnet`) instructed to, from repo
   root (docker required):
   - copy/point `validation/delve-output` at the build output
   - `EULA=TRUE docker compose -f validation/compose.yaml --profile packtest up --exit-code-from packtest`
   - `EULA=TRUE docker compose -f validation/compose.yaml --profile validate up --build --abort-on-container-exit --exit-code-from bot`
   - tear down containers, and report ONLY: per-command exit codes, failed
     PackTest names, the bot's failed step (if any), and ≤20 relevant log lines.
   Both must exit 0. Re-runs after fixes go through the same subagent. A red bot
   run = **you** fix the campaign in the DSL (repair judgment stays with the
   authoring agent; or report a compiler bug — never hand-edit compiler output).
8. Report to the user: campaign summary, playtime estimate, validation results,
   and the two commands they care about:
   - play: `EULA=TRUE docker compose -f validation/compose.yaml --profile play up`
   - playtest with notes: same with `--profile playtest` (+ `CREATOR_NAME=<mc name>`)

## Hard rules

- Persist the DSL workspace before validation, not after — a crash must never
  lose the campaign.
- Every player-visible string in the **stage docs stays English** — always. Other
  languages are delivered as `l10n/<code>.json` sidecars (the Localization stage
  above), never by writing non-English into the stage docs. Owner prompts in
  Chinese still yield English stage docs; add a `zh-cn` sidecar only when she asks
  for localized in-game text (中文文本).
- Homages: original text only, cultural reference never asset ingestion
  (ADR-0007).
- If a mechanic the prompt wants has no DSL verb, do NOT fake it with adjacent
  verbs silently — tell the user what's missing and offer the closest authorable
  alternative (spec change requests go to the planning session, not this skill).
