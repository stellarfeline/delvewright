// MineflayerExecutor: a kill step: waves, encounters, census, clearing.

import type { KillStep, Step, Vec3Tuple } from "../critical-path.ts";
import {
  dueRunBacks,
  waveAttribution,
  type CombatPlan,
  type Encounter,
  type EncounterPhase,
  type FightAttribution,
  type RunBack,
  type WaveCensus,
} from "../combat.ts";
import { parseCensusMob, parseCensusSummary } from "../markers.ts";
import { nearestIndex, nextLegWaypoints } from "../waypoints.ts";
import { beginCensusWatch, observeCensus, type WaveCensusWatch } from "../wave.ts";
import { STAGED_REPLY_MS } from "./chat.ts";
import { delay } from "./connection.ts";
import { SCORE_POLL_MS } from "./score.ts";
import { REACH_POLL_MS } from "./walk.ts";
import type { MineflayerExecutor } from "../executor.ts";

/**
 * Consecutive unanswered censuses that end a staged clear.
 *
 * A probe that has not answered once in this many round trips is broken, and the
 * step says so instead of spending its whole budget asking again. The step still
 * FAILS — a silent probe is never a cleared wave.
 */
const CENSUS_SILENCE_LIMIT = 6;

/** How long (ms) a `kill` step may run before it is declared failed. */
const KILL_TIMEOUT_MS = 90_000;

/**
 * The slowest full-charge attack speed any vanilla melee weapon has (swings per
 * second: the mace). The swing cadence when the server sent no
 * readable attack speed — see `chargeMs`.
 */
const SLOWEST_VANILLA_ATTACK_SPEED = 0.6;

/**
 * A kill step whose fight is already on the bot — a hostile within this many
 * blocks — is fought where the bot stands rather than after a walk to the wave's
 * anchor (`fightWave`). A melee mob closes this in two seconds.
 */
const FIGHT_HERE_RANGE = 8;

/** One server tick (ms) — the granularity the hands are driven at. */
const TICK_POLL_MS = 50;

type StrikeOutcome = "swung" | "gone" | "out-of-reach";

/**
 * How far from its anchor cell an actor's unleashed body may be and still be
 * recognised as that actor. Generous but local: `unleash-actor` replaces
 * the puppet with a real-AI twin at the same cell, and the twin then MOVES — it
 * charges the bot — so a radius tight enough to be an identity check would lose
 * the fight it just started. Nothing else of that entity type exists nearby: the
 * delve world is sealed (`spawn_mobs false`), so every living body is
 * compiler-summoned.
 */
const ACTOR_MATCH_RADIUS = 24;

/** How long to wait for the unleashed twin to exist and reach entity tracking. */
const ACTOR_SETTLE_MS = 6_000;

/**
 * Budget for one unassisted actor attempt. Shorter than the wave `kill` budget on
 * purpose: nothing downstream waits on this fight, and a bot that has not killed
 * a single body in half a minute of swinging has already answered the only
 * question the floor gate asks.
 */
const ACTOR_FIGHT_TIMEOUT_MS = 45_000;

/**
 * How long (ms) the die-retry stage trades blows before taking its `mid-fight`
 * scripted death (spec-0023 §1). Short on purpose: the point is that the wave has
 * been ENGAGED — some mobs hurt, the fight's state dirty — when the death lands,
 * because that is the state a respawn has to restore. Winning the fight here would
 * defeat the trial.
 */
const MID_FIGHT_MS = 6_000;

/**
 * How long one census may take to come back.
 *
 * A census is a `/function` call whose answer arrives on the chat channel within
 * a tick or two; this is the "the command was refused" deadline, not a settle.
 * The census returns `undefined` on expiry — never a zero, which would read as
 * "the wave is gone" and turn an unopped bot into a false `stranded` verdict.
 */
export const CENSUS_TIMEOUT_MS = 3_000;

/**
 * How often the kill step asks the server whether the wave still stands, while
 * nothing else has prompted it to.
 *
 * The step's terminal condition is the census (`wave.ts`), so this is the
 * background cadence that condition runs on: a census is a `/function` round trip
 * plus a chat line per standing mob, which is cheap at this rate and would be
 * noise at the loop's own 250ms. It is a FLOOR, not a schedule — a guess that the
 * fight is over asks at once, and so does the bot's own tally reaching the wave's
 * declared size, which is exactly when the old code asked.
 */
const WAVE_CENSUS_POLL_MS = 2_000;

/** How many censuses' mob lines stay addressable. Only the newest is ever asked
 * for; the rest are kept so a late line cannot grow the map without bound. */
const CENSUS_HISTORY = 4;

/**
 * Self-defense (souls ladder, the-drowned-bell): how long (ms) the bot may spend
 * killing a stalker that interrupted a NAVIGATION leg before it gives up, reports, and
 * resumes walking. A delve mob dies in a handful of swings; this window is many times
 * that, so it only ever expires on something the bot genuinely cannot kill (an
 * Invulnerable actor, a mob it cannot reach) — and even then the leg continues, so the
 * budget can never turn a content problem into a navigation failure.
 */
const DEFEND_BUDGET_MS = 12_000;

/**
 * How often (ms) a walking leg re-checks whether a stalker has latched onto the bot.
 * A backstop only: the check also runs on the damage event itself, so a mob that hits
 * the bot is reacted to on the packet rather than up to a poll later. It matters — a
 * Hollow Gate-Warder swinging an iron axe takes ~7 of the bot's 20 hit points per hit
 * on `easy`, so three hits is the whole margin.
 */
const THREAT_POLL_MS = 200;

/**
 * An entity's custom name as plain text, or `undefined` when it has none.
 *
 * mineflayer surfaces a custom name in several shapes across versions — a plain
 * string, a chat component with `toString`, or `{ text }` — so this reads all of
 * them and gives up quietly rather than throwing. Used only to PREFER the right
 * body among candidates of the same entity type; identity never rests on
 * it, because a client cannot read the entity tag the compiler actually uses.
 *
 * **i18n v2 (spec-0029).** An authored name now ships as
 * `{"translate": "<l10n key>", "fallback": "<English source>"}`, so the `text`
 * field is gone. `fallback` is read explicitly and FIRST among the component
 * shapes: it is by construction the English source the campaign document holds,
 * which is exactly the string the plan's `actors[].name` carries — so the
 * preference heuristic keeps matching, rather than depending on whether the
 * installed prismarine-chat resolves an unknown translate key to its fallback or
 * to the raw key. How often it actually binds is MEASURED, not assumed:
 * {@link NamePreference} counts every candidate-preference decision and how many
 * had a usable name, and the run report prints both.
 */
export function displayNameOf(e: unknown): string | undefined {
  const ent = e as { displayName?: unknown; customName?: unknown };
  for (const raw of [ent.customName, ent.displayName]) {
    if (typeof raw === "string" && raw.length > 0) return raw;
    if (raw && typeof raw === "object") {
      const o = raw as { text?: unknown; fallback?: unknown; toString?: () => string };
      if (typeof o.fallback === "string" && o.fallback.length > 0) return o.fallback;
      if (typeof o.text === "string" && o.text.length > 0) return o.text;
      const s = typeof o.toString === "function" ? o.toString() : "";
      if (s && s !== "[object Object]") return s;
    }
  }
  return undefined;
}

/**
 * How often the same-type candidate preference actually had a name to
 * prefer by — the binding count spec-0029 requires the bot run to state rather
 * than assume.
 *
 * `decisions` counts every time the bot chose among candidate bodies for a named
 * actor; `withUsableName` counts the ones where at least one candidate carried a
 * readable custom name. A run whose `decisions` is 0 examined nothing and is
 * `unbound` — a finding, not a pass (CLAUDE.md, playtest-methodology.md rule 1).
 * A run with decisions but `withUsableName: 0` is the specific regression
 * spec-0029 asks to watch for: translate components rendering as keys the
 * heuristic cannot match.
 */
export interface NamePreference {
  readonly decisions: number;
  readonly withUsableName: number;
  readonly candidates: number;
  readonly namedCandidates: number;
}

/**
 * Vanilla shapes that are never a combat target, whatever campaign is running.
 *
 * Everything here is a fact about MINECRAFT and about the bot's own situation: a
 * player is not a monster, a dropped item is not a body, a display entity has no
 * health. No campaign can make any of them a fight, so no campaign has to say so.
 *
 * **What is deliberately NOT here**: which bodies THIS delve stages as NPCs. That
 * used to be `"mannequin"` and `"villager"`, written down beside the vanilla
 * shapes as though it were the same kind of fact. It is not — it is a statement
 * about what the compiler summons an NPC as, and an author who bodies a
 * quest-giver as a zombie gets a quest-giver the bot beats to death. The delve now
 * states its own cast in `critical-path.json`'s `non_combatants`, which
 * {@link isWaveMob} takes as an argument; see {@link Executor.useNonCombatants}.
 */
const NON_WAVE_ENTITIES = new Set<string>([
  "player",
  "interaction",
  "item",
  "experience_orb",
  "arrow",
  "spectral_arrow",
  "armor_stand",
  "marker",
  "text_display",
  "block_display",
  "item_display",
  "area_effect_cloud",
  "item_frame",
  "glow_item_frame",
  "painting",
  "leash_knot",
  "fishing_bobber",
]);

/**
 * The registry categories whose members are LIVING bodies — the only things a
 * melee swing is ever meant for.
 *
 * mineflayer sets `entity.type` from the pinned version's minecraft-data
 * `entities[].type`, so this is the registry's own statement, not a guess from a
 * silhouette. Everything outside it is `projectile`, `other` (boats, minecarts,
 * displays, falling blocks, TNT, evoker fangs, end crystals…), `orb`, `player`,
 * `global` or `object`.
 *
 * Why a category and not the height proxy this replaced: vanilla does not merely
 * ignore a swing at a non-body — `ServerGamePacketListenerImpl` DISCONNECTS the
 * player for attacking an item, an experience orb, itself, or any non-redirectable
 * `AbstractArrow` ("Attempting to attack an invalid entity"). A thrown `trident`
 * is an `AbstractArrow`, half a block tall, and lay beside the bot after a
 * Drowned Chorister threw it; the height rule passed it, the bot swung, and the
 * server kicked it mid-trade at vesperhold's choir. A deny-list of names cannot
 * keep up with every projectile the registry has; the category can.
 */
const LIVING_CATEGORIES = new Set<string>([
  "hostile",
  "mob",
  "animal",
  "passive",
  "ambient",
  "water_creature",
  "living",
]);

/**
 * True if `e` is something the bot could swing at: not the bot, not a vanilla
 * non-body, not one of the kinds THIS delve stages as an NPC, and a LIVING entity
 * by the pinned registry's own category ({@link LIVING_CATEGORIES}). Excluded
 * names are matched by `name`; living-ness is read off `type`, which mineflayer
 * fills from the same registry the server runs.
 *
 * `nonCombatants` is the delve's own cast statement, read off
 * `critical-path.json` — required, never defaulted. Passing an empty set is a
 * legitimate answer (a campaign with no NPCs states exactly that, and says so in
 * its binding count); omitting the argument is not possible, which is the point.
 *
 * **This is a TARGETING predicate, never a measurement one**. The bot
 * can only attack what its client can see, so picking a swing target by shape is
 * the only thing it could do — but ANSWERING a question about the wave that way
 * is how the drowned bell's ambush husks were reported as wave mobs a re-seat had
 * failed to remove. Every count that reaches a verdict or the run report
 * now comes from the compiler's tag census instead; nothing here may be used to
 * decide what is standing at an encounter.
 */
export function isWaveMob(e: unknown, self: unknown, nonCombatants: ReadonlySet<string>): boolean {
  if (!e || e === self) return false;
  const ent = e as { name?: string; type?: string };
  const name = ent.name ?? "";
  if (name === "" || NON_WAVE_ENTITIES.has(name) || nonCombatants.has(name)) return false;
  return isLivingBody(e);
}

/**
 * Is `e` a living body by the pinned registry's category? The one rule every
 * swing passes through ({@link Executor.swing}), so no caller — wave loop,
 * self-defense, actor fight, mid-fight trade — can hand the server an entity it
 * disconnects the player for attacking.
 */
export function isLivingBody(e: unknown): boolean {
  if (!e) return false;
  return LIVING_CATEGORIES.has((e as { type?: string }).type ?? "");
}

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Adopt the compiler's combat plan (spec-0023). With it, a `kill` step becomes a
   * verified ENCOUNTER rather than a fight to be won: the die-retry stage proves
   * dying is safe, the assist windows keep bot fencing skill from capping how hard
   * a delve may be, and a billed `elite`/`boss` gets one honest unassisted attempt
   * so the inverted floor gate has something to measure.
   */
  /**
   * Adopt the delve's cast statement — the kinds nothing may swing at.
   *
   * Called by the entrypoint before the run starts, from the parsed critical
   * path. An empty set is a legitimate value (a campaign with no NPCs); NOT
   * calling this at all is a harness wiring bug, and {@link requireNonCombatants}
   * refuses rather than guessing.
   */
  useNonCombatants(this: MineflayerExecutor, kinds: ReadonlySet<string>): void {
    this.nonCombatants = kinds;
  },

  /** The cast statement, or a refusal naming what is missing. */
  requireNonCombatants(this: MineflayerExecutor): ReadonlySet<string> {
    if (!this.nonCombatants) {
      throw new Error(
        "the executor was never told which entity kinds are never a combat target. That " +
          "statement is `non_combatants` in `critical-path.json` (contract format 4) and the " +
          "entrypoint must hand it over before the run starts — there is no default, because " +
          "the only available default is a list of entity names written in the harness, which " +
          "is right only for the campaigns whose author happened to pick those bodies",
      );
    }
    return this.nonCombatants;
  },

  /**
   * Adopt the compiler's combat plan. With it a `kill` step becomes a verified
   * ENCOUNTER rather than a fight: the wave's live bodies are read against what
   * the campaign declared, the wave is then removed by an attributed command, and
   * the die-retry stage proves that dying to it is safe.
   */
  useCombatPlan(this: MineflayerExecutor, plan: CombatPlan, dieRetry: boolean): void {
    this.combatPlan = plan;
    this.dieRetry = dieRetry;
  },

  /** The objectives the compiled path proves — what decides which actor fights
   * this run can reach at all. Set by the entrypoint before the run starts. */
  usePathObjectives(this: MineflayerExecutor, objectives: Iterable<string>): void {
    this.pathObjectives = new Set(objectives);
  },

  /** How far `kill()` got with `wave`. `not-reached` when the run ended first. */
  encounterPhase(this: MineflayerExecutor, wave: string): EncounterPhase {
    return this.encounterPhases.get(wave) ?? "not-reached";
  },

  /**
   * Who felled `wave`'s bodies, as its last census answered.
   *
   * `unattributed` when no census answered during the fight — which is a fact
   * about the probe, and must never be readable as a clean win.
   */
  waveAttribution(this: MineflayerExecutor, wave: string): FightAttribution {
    return (
      this.waveAttributions.get(wave) ?? {
        kind: "unattributed",
        reason: "no wave census answered during this run's fight at this encounter",
      }
    );
  },

  /** The plan's entry for `wave`, if the campaign declares one. */
  encounterFor(this: MineflayerExecutor, wave: string): Encounter | undefined {
    return this.combatPlan?.encounters.find((e) => e.wave === wave);
  },

  /**
   * The critical path's `kill` step (spec-0023 §1/§3/§4).
   *
   * Order is the whole design. The die-retry stage runs FIRST, while the
   * encounter is still live — dying to a fight already won proves nothing — and
   * only then is the fight taken to completion, unassisted first when the content
   * billed it hard.
   */
  async kill(this: MineflayerExecutor, step: KillStep): Promise<void> {
    await this.killStep(step);
    this.recordCleared(step.wave);
  },

  /**
   * This wave is down in the seating in force — what {@link dueRunBacks} and
   * {@link reseatedWaves} read. One place, for every act that clears a wave.
   */
  recordCleared(this: MineflayerExecutor, wave: string): void {
    this.waveClearedAt.set(wave, this.currentStep);
    this.clearedEpoch.set(wave, this.seatEpoch);
  },

  /**
   * Fight every run-back due before `step` (spec-0016 §1, spec-0023 §3): a wave
   * this run cleared, that a rest it took since has put back beside the leg
   * the step walks. A player walking that leg meets the fight again, so the
   * ladder fights it — under a labelled assist, like every encounter — before
   * the step walks on. What is due is the compiler's `run_backs` filtered by
   * what this walk did (`dueRunBacks`); the harness decides nothing else.
   */
  async runBacksBefore(this: MineflayerExecutor, step: Step): Promise<void> {
    const after = "cutsceneSeconds" in step ? step.cutsceneSeconds : undefined;
    const enRoute = "enRouteCutsceneSeconds" in step ? step.enRouteCutsceneSeconds : undefined;
    this.stepCutsceneAllowanceS =
      after === undefined && enRoute === undefined ? undefined : Math.max(after ?? 0, enRoute ?? 0);
    this.stepLabel =
      `${step.action}` +
      ("objective" in step && typeof step.objective === "string" ? ` ${step.objective}` : "") +
      (step.action === "trigger" ? ` ${step.trigger}` : "");
    const plan = this.combatPlan;
    if (!plan || plan.runBacks.length === 0) return;
    const token =
      "objective" in step && typeof step.objective === "string"
        ? step.objective
        : step.action === "trigger"
          ? step.trigger
          : undefined;
    if (token === undefined) return;
    const due = dueRunBacks(plan.runBacks, token, this.waveClearedAt, this.restedAt);
    if (due.length === 0) return;
    // The leg this step walks, when a proven one is next: the fight is met ON it,
    // at the crossing the compiler measured, so the bot walks the leg's own
    // proven cells up to there, fights, and the step's walk resumes from there.
    // Walking to the wave's anchor from wherever the last step ended is an
    // unproven cross-map walk — measured stranding the bot in the belfry.
    const pos = "pos" in step ? step.pos : undefined;
    const leg =
      pos && this.waypoints
        ? nextLegWaypoints(this.waypoints.legs, this.legCursor, [pos[0], pos[1], pos[2]])
        : undefined;
    const cells = leg?.matched ? leg.waypoints : undefined;
    const along = (rb: RunBack): number => (cells ? nearestIndex(cells, rb.crossing) : 0);
    let walked = 0;
    for (const rb of [...due].sort((a, b) => along(a) - along(b))) {
      // Two rests can put one wave back beside the same leg; it stands there
      // once, so it is fought once.
      if (this.waveClearedAt.get(rb.wave) === this.currentStep) continue;
      if (cells) {
        const k = along(rb);
        if (k >= walked) {
          await this.walkTo(
            cells[k]!,
            1,
            `run-back approach to ${rb.wave} along the leg to ${rb.before}`,
            false,
            undefined,
            cells.slice(walked, k),
          );
          walked = k;
          this.legResume = { leg: this.legCursor, from: k };
        }
      }
      const enc = this.encounterFor(rb.wave);
      const fight: KillStep = {
        action: "kill",
        objective: rb.objective,
        wave: rb.wave,
        pos: rb.pos,
        tag: "",
        count: rb.count,
      };
      process.stderr.write(
        `[run-back] ${rb.wave}: re-seated by the rest at bonfire ${rb.bonfire}, ` +
          `${rb.distance.toFixed(1)} blocks from the leg to ${rb.before} (aggro radius ` +
          `${rb.radius}) — reading and clearing it before the leg\n`,
      );
      if (!enc) {
        throw new Error(
          `run-back ${rb.wave}: the combat plan names the run-back but not the encounter, so ` +
            `the wave can be neither read nor cleared`,
        );
      }
      await this.musterUnlessRead(enc);
      await this.clearWave(fight, enc);
      this.runBacksFought.push(rb);
      this.recordCleared(rb.wave);
    }
  },

  /**
   * The critical path's `kill` step.
   *
   * Three acts, and only the first is a measurement.
   *
   * 1. **Die-retry** (spec-0023 §1), while the encounter is still live — dying to a
   *    fight already over proves nothing.
   * 2. **The muster**: the bodies the server seated are read and checked against
   *    what the campaign declared. This is the part of a combat step that is a
   *    verification, and nothing looked at it before.
   * 3. **The staged clear**: the wave is removed by an attributed command so the
   *    run can go on to the wiring the kill drives — the objective, `on_kill`, the
   *    declared drops, the health bar, the re-seat. The bot does not fight, and
   *    nothing here is a claim about whether the fight can be won.
   */
  async killStep(this: MineflayerExecutor, step: KillStep): Promise<void> {
    const enc = this.encounterFor(step.wave);
    if (!enc) {
      // No fallback. A wave outside the combat plan has no muster to read it with
      // and no `wave_strike_*` to clear it — and the only alternative is for the
      // harness to invent both, which is exactly the downstream folklore the plan
      // exists to end.
      throw new Error(
        `kill ${step.wave}: the combat plan names no encounter for this wave, so there is ` +
          `nothing to read its bodies with and nothing to clear them with`,
      );
    }
    // Both the muster and the staged clear ask the SERVER about entities carrying
    // the wave's tag, and an entity in an unloaded chunk is not there to be asked.
    // The bot's own view distance is not a guarantee at the moment a step opens —
    // and a probe that answers "no bodies" because nobody was looking is the
    // silent zero this repository keeps finding. Hold the anchor's chunk open for
    // the whole step instead of hoping.
    await this.holdChunk(enc.pos, true);
    try {
      // The muster goes FIRST, before anything the harness does to this wave.
      // The cohort it must read is the one the DELVE seated: the die-retry stage
      // kills the bot twice and the wave re-seats around it, and on the gallery it
      // also walks the bodies past a lethal pit — run second, the probe read an
      // empty anchor and reported three declared stacks missing, over a wave that
      // had spawned exactly as declared.
      await this.musterUnlessRead(enc, true);
      // Nothing seated yet: the wave is one an approach trigger seats as the
      // party walks in (the-stranding's wrecks, lice and Marrack's men read 0 on
      // every run, and the bot then fought them unread). Walk the step's route to
      // the anchor as a player does, and read it there; a zero now is a red.
      if (this.readingOwed(enc)) {
        await this.walkTo(
          step.pos,
          3,
          `wave ${step.wave} anchor (nothing seated at the step's open)`,
          step.sneak,
          { objective: step.objective, transport: step.transport },
        );
        await this.musterUnlessRead(enc);
      }
      this.encounterPhases.set(enc.wave, "mustered");
      if (this.dieRetry) {
        this.encounterPhases.set(enc.wave, "die-retry");
        this.stageNow = "die-retry";
        // Until the first census of the stage refines it, everything standing at
        // the anchor is treated as the encounter's: a protected body wrongly left
        // standing costs the run a little health, and an unprotected one costs the
        // measurement.
        this.protectedWave = { wave: enc.wave, census: [{ pos: enc.pos }] };
        try {
          await this.dieRetryAt(step, enc);
        } catch (err) {
          // The die-retry stage has its OWN verdict, and an encounter it engaged
          // without completing its trials already reds it
          // (`dieRetryCoverageFailures`). Letting that failure end the RUN as well
          // suppresses every measurement behind this encounter — the muster of
          // every later wave, the endings, the whole death loop — over a stage
          // that has already said what it found. The trial carries the abort; the
          // run carries on.
          const detail = err instanceof Error ? err.message : String(err);
          process.stderr.write(
            `[die-retry] ${enc.wave}: the stage was abandoned (${detail}) — it reds on its ` +
              `own; the run carries on to what this kill drives\n`,
          );
          if (this.death) await this.respawnAndRearm();
        } finally {
          this.protectedWave = undefined;
          this.stageNow = "critical-path";
        }
      }
      await this.clearWave(step, enc);
      this.encounterPhases.set(enc.wave, "cleared");
    } finally {
      await this.holdChunk(enc.pos, false);
    }
  },

  /**
   * Force-load (or release) the chunk an encounter stands in.
   *
   * Paired: the release is in the caller's `finally`, because a forceload the run
   * leaves behind keeps a chunk ticking for the rest of the session and is the
   * harness quietly changing the world it is measuring.
   */
  async holdChunk(this: MineflayerExecutor, pos: Vec3Tuple, hold: boolean): Promise<void> {
    const bot = this.requireBot();
    // Counted per chunk: the damage handlers can hold a wave's chunk for an early
    // muster while a step holds the same chunk, and the first release must not
    // unload the chunk under the other.
    const key = `${Math.floor(pos[0] / 16)},${Math.floor(pos[2] / 16)}`;
    const holders = this.chunkHolds.get(key) ?? 0;
    if (hold) {
      this.chunkHolds.set(key, holders + 1);
      if (holders > 0) return;
    } else {
      if (holders > 1) {
        this.chunkHolds.set(key, holders - 1);
        return;
      }
      this.chunkHolds.delete(key);
    }
    const verb = hold ? "add" : "remove";
    const refusal = await this.refusalOf(`/forceload ${verb} ${pos[0]} ${pos[2]}`);
    if (refusal !== undefined) {
      process.stderr.write(
        `[kill] forceload ${verb} ${pos[0]} ${pos[2]} was refused — ${refusal}\n`,
      );
    }
  },

  /**
   * Wound every body of the wave without felling one — the die-retry stage's
   * "mid-fight" state, which is a state of the WAVE and not of the bot.
   */
  async chipWave(this: MineflayerExecutor, enc: Encounter): Promise<void> {
    this.requireBot().chat(`/function ${enc.muster.chip}`);
    this.stagedRemovals.push({
      kind: enc.wave,
      why: "die-retry: one attributed point of damage, to put the wave in its mid-fight state",
      performed: true,
    });
    await delay(STAGED_REPLY_MS);
  },

  /**
   * Remove the wave, one attributed body at a time, then walk its anchor.
   *
   * **Staging, then a measurement.** The removal is not a fight and proves nothing
   * about the encounter; what follows it is everything the kill DRIVES — the
   * objective completing, `on_kill` paying, the declared drops dropping, the health
   * bar clearing — and the walk to the anchor, which is the route proof the step
   * has always owed.
   *
   * One body per blow, rather than the whole wave at once, so the countdown, the
   * bar and any per-kill effect are each exercised the number of times the
   * campaign says they should be.
   */
  async clearWave(this: MineflayerExecutor, step: KillStep, enc: Encounter): Promise<void> {
    await this.stageClear(step, enc);
    // The route to the encounter is a mechanism the step owes, and it is read with
    // the wave gone: what it reports on is then the route, not what was standing in
    // it. A run-back walks its own leg and passes no objective here.
    await this.walkTo(
      step.pos,
      3,
      `wave ${step.wave} anchor`,
      step.sneak,
      { objective: step.objective, transport: step.transport },
    );
  },

  /**
   * **The staged clear on its own**: one attributed blow per standing body until
   * the census answers that none stands, refusing a census that never answered and
   * a wave that outlasts {@link KILL_TIMEOUT_MS}. What every act that removes a
   * read wave shares — the kill step, the run-back, the death loop's approach —
   * and nothing else: the walk to the anchor is the kill step's route proof, which
   * a walk that is not that step does not owe.
   */
  async stageClear(this: MineflayerExecutor, step: KillStep, enc: Encounter): Promise<void> {
    const bot = this.requireBot();
    const watch = beginCensusWatch();
    const deadline = Date.now() + KILL_TIMEOUT_MS;
    let struck = 0;
    let silent = 0;
    // The wave has been read; from here it is on its way out, so it stops being
    // an encounter this run still owes a reading and its bodies become stageable
    // like any other. Measured on vesperhold: `wave/walk-ambush`'s last pillager
    // shot the bot dead in the second between two staged blows, and the step
    // reported that the delve had killed the run.
    this.clearing = enc.wave;
    try {
    while (Date.now() < deadline) {
      // A death WHILE the harness is removing a wave is not a verdict on
      // anything: nothing is being fought and nothing is being measured. Recover
      // and carry on clearing, so the run reaches what the kill drives.
      if (this.death) {
        process.stderr.write(
          `[kill ${step.wave}] the bot died while the wave was being staged away; ` +
            `recovering and carrying on\n`,
        );
        await this.respawnAndRearm();
      }
      const standing = await this.pollWaveCensus(step, enc, watch);
      if (standing === undefined) {
        // A probe that has never once answered this step is broken, and waiting
        // out the whole budget on it only delays saying so.
        if (++silent >= CENSUS_SILENCE_LIMIT && watch.answers === 0) break;
        process.stderr.write(
          `[kill ${step.wave}] the wave census did not answer; retrying\n`,
        );
        await delay(REACH_POLL_MS);
        continue;
      }
      silent = 0;
      if (standing === 0) {
        process.stderr.write(
          `[kill ${step.wave}] the wave census reports nothing of ${step.wave} standing after ` +
            `${struck} staged blow(s)\n`,
        );
        break;
      }
      const refusal = await this.refusalOf(`/function ${enc.muster.strike}`);
      struck += 1;
      if (refusal !== undefined) {
        throw new Error(
          `kill ${step.wave}: the staged blow was refused by the server — ${refusal} ` +
            `(${enc.muster.strike}, ${standing} of the wave standing)`,
        );
      }
    }
    } finally {
      this.clearing = undefined;
    }
    if (this.death) await this.respawnAndRearm();
    this.stagedRemovals.push({
      kind: step.wave,
      why: `staged clear: ${struck} attributed blow(s), so the run can read what the kill drives`,
      performed: true,
    });
    const attribution = this.waveAttribution(step.wave);
    if (attribution.kind === "measured" && attribution.uncredited > 0) {
      process.stderr.write(
        `[kill ${step.wave}] ${attribution.uncredited} of ${attribution.bodies} bodies fell with ` +
          `NOBODY credited — everything that pays on a player's kill was skipped for those\n`,
      );
    }
    // A census that never answered is NOT a cleared wave. The terminal condition
    // is the server's answer, and a silent probe has given none: reading its
    // silence as "nothing stands" would let a broken probe pass every encounter
    // in the delve.
    const left = await this.pollWaveCensus(step, enc, watch);
    if (left === undefined) {
      throw new Error(
        `kill ${step.wave}: the wave census (${enc.census.census}) did not answer, so nothing ` +
          `says whether the wave stands — over ${watch.answers} answer(s) this step, after ` +
          `${struck} staged blow(s)`,
      );
    }
    if (left > 0) {
      throw new Error(
        `kill timed out after ${KILL_TIMEOUT_MS}ms: ${left} of wave ${step.wave} still stands ` +
          `after ${struck} staged blow(s) — the census last answered over ${watch.answers} ` +
          `answer(s)${watch.seen ? "" : ", and never once saw the wave exist"}`,
      );
    }
  },

  /**
   * Ask the server what is standing at this encounter — BY TAG.
   *
   * The old probe answered by silhouette: every entity the client tracked, no
   * distance filter, anything taller than half a block. That set is not the wave.
   * On the drowned bell it swept in two ambush husks 57 blocks away at the cistern
   * and whichever neighbouring wave had just been re-seated, so a 2-mob wave read
   * as 4 standing — and because those bystanders were alive on both sides of a
   * scripted death, they were reported as survivors the re-seat had failed to
   * remove. The re-seat was innocent; the ruler was wrong.
   *
   * Only the server can see the wave tag, so the compiler emits the census and
   * this reads it: counts of standing / branded / damaged, plus one line per mob
   * with its position and health. Everything the fidelity verdict consumes is the
   * server's own answer about entities carrying `dw_wave_<id>`, and nothing else
   * can enter it.
   *
   * Returns `undefined` if no answer arrives — never a zero, which would read as
   * "the wave is gone".
   */
  async census(this: MineflayerExecutor, enc: Encounter): Promise<WaveCensus | undefined> {
    const bot = this.requireBot();
    const before = this.censusSeq;
    bot.chat(`/function ${enc.census.census}`);
    const deadline = Date.now() + CENSUS_TIMEOUT_MS;
    for (;;) {
      const sum = this.censusSummaries.get(enc.wave);
      if (sum && sum.seq > before) {
        const mobs = this.censusMobs.get(sum.seq) ?? [];
        // Keep the protected wave's view of where its bodies stand current, so the
        // die-retry stage's own re-seats stay protected as they move.
        if (this.protectedWave?.wave === enc.wave) this.protectedWave.census = mobs;
        return { summary: sum, mobs };
      }
      if (Date.now() >= deadline) return undefined;
      await delay(SCORE_POLL_MS);
    }
  },

  /**
   * How many of the wave stand? Asks the SERVER, by tag, and records the answer.
   *
   * This is the kill step's terminal condition and the source of its attribution,
   * so everything the step decides about the fight comes through here.
   *
   * Every OTHER test in `fightWave` is a guess made from shapes — "a mob the bot
   * hit winked out near the anchor", "everything it engaged is down and nothing
   * hostile is close". None of them can tell a wave mob from any other mob,
   * because the client cannot see the wave tag. On the drowned bell that cost the
   * campaign a whole round: at the belfry the bot killed one of
   * `ambush/the-rafters`' husks, counted it as the Bellkeeper (`confirmed kill:
   * husk#232 (1/1)`), and walked away from a wither skeleton that was still very
   * much alive. `obj/the-keeper` therefore never completed, `quest/the-keeper`
   * never completed, `quest/ring-it-home` was never armed — and the next step's
   * `interact` click was adjudicated against an unarmed quest and spent. The
   * click was the SYMPTOM; this was the cause.
   *
   * The guesses still DRIVE the fight — the bot can only swing at what it can see
   * — and they still prompt a census. What they no longer do is end the step on
   * their own, and neither does the bot's tally of confirmed kills: a body that
   * died where the proximity rule cannot attribute it, or a scripted death that
   * re-seated the wave under that tally, both leave it unable to reach the wave's
   * declared size for the rest of the step.
   *
   * `undefined` when there is nothing to ask (a wave outside the combat plan) or
   * when the census did not answer — never a zero, which would read as "the wave
   * is gone" on a broken probe. The watch is not fed in that case, so a probe that
   * never answers can never clear a fight.
   */
  async pollWaveCensus(
    this: MineflayerExecutor,
    step: KillStep,
    enc: Encounter | undefined,
    watch: WaveCensusWatch,
  ): Promise<number | undefined> {
    if (!enc) return undefined;
    const census = await this.census(enc);
    if (!census) {
      process.stderr.write(
        `[kill ${step.wave}] the wave census did not answer; falling back to what the client ` +
          `can see\n`,
      );
      return undefined;
    }
    observeCensus(watch, {
      present: census.summary.present,
      credited: census.summary.credited,
    });
    // Who felled this cohort. `step.count` is the seating the compiler declared
    // and `spawn_<wave>` wrote; the other two are the server's own answer.
    this.waveAttributions.set(
      step.wave,
      waveAttribution(step.count, census.summary.present, census.summary.credited),
    );
    return census.summary.present;
  },

  /** Stamp this life's wave mobs so the next census can name the survivors. */
  brandWave(this: MineflayerExecutor, enc: Encounter): void {
    this.requireBot().chat(`/function ${enc.census.brand}`);
  },

  /** Clear the stamp, so the next trial brands a clean slate. */
  unbrandWave(this: MineflayerExecutor, enc: Encounter): void {
    this.requireBot().chat(`/function ${enc.census.unbrand}`);
  },

  observeCensus(this: MineflayerExecutor, message: string): void {
    const mob = parseCensusMob(message);
    if (mob) {
      if (mob.campaignId !== this.campaignId) return;
      const at = this.censusMobs.get(mob.seq) ?? [];
      at.push(mob);
      this.censusMobs.set(mob.seq, at);
      // Bounded: only the newest few censuses can still be asked about.
      while (this.censusMobs.size > CENSUS_HISTORY) {
        const oldest = Math.min(...this.censusMobs.keys());
        this.censusMobs.delete(oldest);
      }
      return;
    }
    const sum = parseCensusSummary(message);
    if (!sum || sum.campaignId !== this.campaignId) return;
    this.censusSummaries.set(sum.wave, sum);
    this.censusSeq = Math.max(this.censusSeq, sum.seq);
  },

  /**
   * The measured name-preference binding for the run report (spec-0029). Always
   * reported, including the all-zero shape — a preference nobody exercised is a
   * stated zero binding, never a silent absence.
   */
  namePreference(this: MineflayerExecutor): NamePreference {
    return {
      decisions: this.namePreferenceDecisions,
      withUsableName: this.namePreferenceWithName,
      candidates: this.namePreferenceCandidates,
      namedCandidates: this.namePreferenceNamedCandidates,
    };
  },
};
