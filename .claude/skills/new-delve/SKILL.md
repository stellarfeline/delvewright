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

### Showcase mode (thin prompts)

When the prompt pins down little — a one-line theme, no detailed brief — treat it
as a **SHOWCASE** brief: a thin prompt is creative license, and the goal is to
deliberately exercise the breadth of the currently-supported feature set so a
stranger playing the result discovers what the engine can do (this is our primary
marketing artifact). Do NOT hardcode a feature list here — v0.4 is landing soon and
any list would rot; instead **query the live schema** (`delvec schema --stage <n>`
across stages) for the available verbs/effects, then aim to include, wherever the
story can carry them coherently: multi-area transport as a narrative beat,
flag-gated dialogue consequences, real props / set-dressing, at least one tuned
combat or stealth encounter, narration beats, and varied NPC presentation. **Rule
of thumb: coherence and pacing always win over feature count** — never bolt on a
mechanic the story can't motivate. A **detailed brief is the opposite**: honor
exactly what it pins down and showcase nothing extra.

## Execution architecture (delegation + models)

The **main (authoring) agent** does HIGH-LEVEL creative work ONLY: theme, beats,
personas, quest-plan intent, the stage summaries, ALL user interaction, and
visual-review judgment. The **mechanical writing of each stage's JSON and the
`delvec validate` repair loop** are delegated to a **dev subagent** — hand it the
creative brief for the stage plus the schema command (`delvec schema --stage <n>`);
it returns valid stage JSON and a short summary of the choices it made, which you
fold into your stage summary. The **validation ladder** stays on a **test
subagent** (step 7).

Model policy for subagents: **dev subagents run `opus`; test / validation subagents
run `sonnet`.** A subagent must **NEVER run a higher tier than the main agent
itself** — if you are running on a lower tier, clamp every subagent down to your own
model (e.g. main agent on `sonnet` → all subagents `sonnet`).

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
2. **Delegate the mechanical write + validate repair loop (steps 2–3) to a dev
   subagent** (see *Execution architecture*): hand it this stage's creative brief +
   the schema command; it returns valid JSON and a summary of choices. The brief you
   hand it carries these craft constraints:
   - Areas: prefer `prefab_pool` (stone-keep tileset) for real layouts; check
     `campaigns/prefabs/pools.json` + prefab metadata for available pools/anchors/lighting
     profiles. Respect the lighting contract — darkness only as declared design
     with a mitigation the quest DAG provides.
   - NPCs: personas per schema (archetype/speech_style/motivation required);
     honor them in every stage-6 line. Dialogue: branching options; flavor NPCs
     get real trees too.
   - Pace to `target_minutes`; no grind (forbidden zone); mandatory-only quests.
   - Objectives (v0.3): author `title` (short player-facing name, e.g. "Unbar the
     Deep Gate") and `hint` (one-line location/direction guidance, e.g. "Past the
     entrance hall, take the left passage to the barred door") for **every
     non-`talk-to` objective**. The compiler surfaces them in-game when the
     objective activates (chat + sound); without them the player gets no guidance
     and cannot find interact/collect/reach targets. For `talk-to`, `title`+`hint`
     are **REQUIRED whenever the target NPC is not already visible from where the
     previous objective completed** (a different room, down a corridor, across an
     area) — the player otherwise gets a silent objective and wanders. Only omit
     them when the NPC is in plain sight of where the player just was; read the "may
     omit" allowance narrowly (playtest lesson: an off-screen NPC 60 blocks away
     through an unfamiliar cave left the player with no guidance at all).
   - Hint wording: give landmark-relative directions from places the player already
     knows (the entrance hall, the gate, a named NPC) — never room-shape jargon
     ("corner room", "L-shaped hall") or solver-internal terms (anchor/piece/socket
     ids).
3. `delvec validate <campaign-dir>` — fix by diagnostic code (DW####; see
   `crates/dsl/README.md` + `crates/compiler/README.md` tables). Loop until clean.
   Three failed repairs on the same code → stop and think about the design instead
   of patching syntax.
4. Interactive mode: present a 3–6 line summary of the stage; wait.

### Supported techniques

Load-bearing patterns proven on real runs — reuse rather than rediscover:

- **`base_entity` accepts any entity id, and NPCs are inert by construction.** Every
  NPC is summoned `NoAI,Invulnerable,Silent,NoGravity,PersistenceRequired` plus a
  separate interaction hitbox, and there is no registry validation on the field — so
  any mob id becomes a talking statue that cannot move or hurt anyone. This is how a
  villager-sized cast can include a giant: e.g. `minecraft:warden` as a Cyclops you
  must slip past. *Caveat:* `Silent:1b` also suppresses that entity's ambient sounds
  (the Warden's heartbeat), and the emitted `VillagerData` tag is inert on a
  non-villager.
- **Multi-area + automatic inter-area transport is a physically enforced point of no
  return.** Placing beats in separate areas (256 blocks apart across void, no
  walkable link) makes the compiler emit a one-way teleport on the objective that
  crosses areas — the player *cannot* walk back, so "the boulder seals the cave" is
  enforced by geometry, not merely asserted. The return trip is the same mechanism
  in reverse.

### Localization stage (only when the prompt asks for other languages)

If the prompt requests one or more languages — or the user prompts in a
non-English language **and asks for localized in-game text** (中文文本 etc.) — add a
**final generation stage after `dialogue`**, once the English campaign is complete:

1. Declare the codes in `world.json`: `"languages": ["zh-cn", …]` (BCP-47-style;
   `en` is implicit/canonical and is **never** listed). Stage docs stay English.
2. **Who translates** — if the repo's `delvewright.toml`/`delvewright.local.toml`
   has an `[i18n]` section AND the env var it names (`api_key_env`) is set, run
   `python3 tools/i18n-translate.py <campaign-dir> --lang <code>` (external LLM
   API; it writes and validates the sidecar for you, then go to step 4).
   Otherwise translate in-agent, steps 3–4. Generation-time only either way —
   shipped delves never call an LLM. See `docs/reference/i18n.md`.
3. In-agent: `delvec l10n-inventory <campaign-dir> --lang <code>` gives the exact
   key inventory as JSON (key, English, speaking NPC, existing translation).
   **Translate FROM the finished English** (never author a language natively) —
   honor each NPC's `persona.speech_style`, keep a Minecraft-appropriate register,
   cover the inventory **exactly**. Write `l10n/<code>.json`:
   `{ dsl_version, campaign_id, kind: "l10n", lang: "<code>", content: { <key>: … } }`.
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
   Both must exit 0. Re-runs after fixes go through the same subagent. On any
   red, **triage before touching anything** (debug doctrine, CLAUDE.md):
   - *Content bug* (your DSL declares something wrong/unreachable/unlit): fix
     the campaign in the DSL. Repair judgment stays with the authoring agent.
   - *Toolchain bug* (compiler/harness/tileset misbehaves on a campaign the
     diagnostics accept): **stop content work and report it** with evidence —
     never hand-edit compiler output, never restructure the campaign to dodge
     the bug, never weaken a check or reroll a seed to get green. A workaround
     that turns a toolchain bug green is itself a quality defect: it ships the
     bug to every future campaign. Escalating is success.
8. Visual review (spec-0003 visual tier) — **you** (the authoring agent, not a
   subagent; visual judgment is the point). The build output already contains
   `render-plan.json` (deterministic shots + per-shot `expect` checklists derived
   from the DSL). Render the per-prefab sets with Nucleation and read them against
   each shot's `expect`:
   - `cargo run -q -p delvewright-render --bin delve-render -- batch campaigns/prefabs -o <workspace>/renders`
     (needs the 1.21.11 client jar via `--textures`/`$DELVEWRIGHT_CLIENT_JAR`;
     skip with a note if unavailable locally).
   - `delve-render fidelity-gate` must exit 0 before trusting any render.
   - Open the exterior/top/interior/anchor PNGs and check each against its
     `expect` line (marker visible? room not dark? NPC faces camera and its name
     is text not JSON? seam clean?). Findings are **DSL-level** — fix the campaign
     (lighting profile, anchor, NPC facing, name string) and rebuild; never
     hand-edit output. (Whole-scene Chunky beauty shots via `delve-render scene`
     stay manual/CI-future.)
9. **Storybook** (spec-0007): write `campaigns/campaigns/<id>/README.md` — the
   reader-facing intro. Background/setting ONLY: premise, lore, public NPC
   introductions (never persona `secret`), classes, playtime, build/play
   commands. NO puzzle solutions, quest structure, or endings. Images (relative
   links into `media/`, small JPEGs): exterior / starting-scene renders only,
   picked from the visual-review set — never interiors or late-game locations.
   Localized `README.<code>.md` per declared language. The render-set images are
   the default — the author may later replace them with hand-crafted shots
   (shaders, staged compositions); media ships with the campaign PR.
10. Report to the user: campaign summary, playtime estimate, validation results,
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
