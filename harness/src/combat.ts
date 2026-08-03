// Parser + rules for spec-0023's combat verification semantics: the compiler's
// combat plan (`validation/combat-plan.json`), the combat-assist ledger, the
// die-retry trial bookkeeping, and the inverted floor gate.
//
// The spec's ruling, restated because every rule here follows from it: the
// machine no longer asserts that a fight can be WON — human skill is the
// variable the design leaves open, deliberately. It asserts that the fight is
// REACHABLE, RETRIABLE and STRUCTURALLY WINNABLE. So:
//
//   * the ladder's load-bearing combat proof is the DIE-RETRY loop — dying must
//     always be safe (respawn → return → re-engage → complete, with no
//     progression flag lost);
//   * the full playthrough runs at the shipped difficulty but may take a bounded,
//     LABELLED combat assist at each encounter, so a poor fencer of a bot never
//     becomes the ceiling on how hard a delve is allowed to be;
//   * and the one place bot combat still bears teeth is INVERTED — an encounter
//     the content billed `elite`/`boss` that the unassisted bot beats on its
//     first try is reported as too easy for its billing.
//
// Everything in this module is pure (no mineflayer): types, arithmetic, and
// verdicts. The executor supplies the bot.

import { readFile } from "node:fs/promises";
import path from "node:path";
import { SUPPORTED_DSL_VERSIONS, type Vec3Tuple } from "./critical-path.ts";

/** Where the combat plan sits relative to `critical-path.json`. */
const COMBAT_PLAN_SUBPATH = ["validation", "combat-plan.json"] as const;

/** What the content bills an encounter as (compiler-side `EncounterTier`). */
export const ENCOUNTER_TIERS = ["ordinary", "elite", "boss"] as const;
export type EncounterTier = (typeof ENCOUNTER_TIERS)[number];

/**
 * One mandatory encounter, as the compiler proved it: which wave, which
 * objective it completes, which critical-path step index it is, what it is
 * billed as, and — the die-retry stage's whole premise — which checkpoint
 * governs a death at it.
 */
export interface Encounter {
  readonly wave: string;
  readonly objective: string;
  readonly step: number;
  readonly tier: EncounterTier;
  readonly pos: Vec3Tuple;
  readonly count: number;
  readonly respawnsOnRest: boolean;
  /** Absent when the campaign has set no checkpoint by this step (world spawn). */
  readonly checkpoint: Vec3Tuple | undefined;
}

/** The parsed combat plan. */
export interface CombatPlan {
  readonly version: string;
  readonly campaignId: string;
  /** The declared world difficulty the run is verified AT (spec-0023 §3). */
  readonly difficulty: string;
  readonly encounters: readonly Encounter[];
}

export class CombatPlanParseError extends Error {
  override readonly name = "CombatPlanParseError";
  readonly pointer: string;
  constructor(pointer: string, detail: string) {
    super(`combat plan invalid at ${pointer || "/"}: ${detail}`);
    this.pointer = pointer;
  }
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function requirePos(v: unknown, pointer: string): Vec3Tuple {
  if (!Array.isArray(v) || v.length !== 3) {
    throw new CombatPlanParseError(pointer, "expected a 3-element position array");
  }
  const out = v.map((n, i) => {
    if (typeof n !== "number" || !Number.isFinite(n)) {
      throw new CombatPlanParseError(`${pointer}/${i}`, "expected a finite number");
    }
    return n;
  });
  return [out[0]!, out[1]!, out[2]!];
}

/** Parse a combat plan document (pure — the file read is the caller's). */
export function parseCombatPlan(raw: unknown): CombatPlan {
  if (!isRecord(raw)) throw new CombatPlanParseError("", "expected an object");
  const version = raw["version"];
  if (typeof version !== "string" || !SUPPORTED_DSL_VERSIONS.includes(version as never)) {
    throw new CombatPlanParseError("/version", `unsupported dsl_version ${String(version)}`);
  }
  const campaignId = raw["campaign_id"];
  if (typeof campaignId !== "string" || campaignId.length === 0) {
    throw new CombatPlanParseError("/campaign_id", "expected a non-empty string");
  }
  const difficulty = raw["difficulty"];
  if (typeof difficulty !== "string" || difficulty.length === 0) {
    throw new CombatPlanParseError("/difficulty", "expected a non-empty string");
  }
  const list = raw["encounters"];
  if (!Array.isArray(list)) throw new CombatPlanParseError("/encounters", "expected an array");
  const encounters = list.map((e, i): Encounter => {
    const p = `/encounters/${i}`;
    if (!isRecord(e)) throw new CombatPlanParseError(p, "expected an object");
    const tier = e["tier"];
    if (typeof tier !== "string" || !ENCOUNTER_TIERS.includes(tier as EncounterTier)) {
      throw new CombatPlanParseError(`${p}/tier`, `expected one of ${ENCOUNTER_TIERS.join("|")}`);
    }
    for (const key of ["wave", "objective"] as const) {
      if (typeof e[key] !== "string" || (e[key] as string).length === 0) {
        throw new CombatPlanParseError(`${p}/${key}`, "expected a non-empty string");
      }
    }
    for (const key of ["step", "count"] as const) {
      if (!Number.isInteger(e[key])) {
        throw new CombatPlanParseError(`${p}/${key}`, "expected an integer");
      }
    }
    return {
      wave: e["wave"] as string,
      objective: e["objective"] as string,
      step: e["step"] as number,
      tier: tier as EncounterTier,
      pos: requirePos(e["pos"], `${p}/pos`),
      count: e["count"] as number,
      respawnsOnRest: e["respawns_on_rest"] === true,
      checkpoint:
        e["checkpoint"] === undefined ? undefined : requirePos(e["checkpoint"], `${p}/checkpoint`),
    };
  });
  return { version, campaignId, difficulty, encounters };
}

/** Read the combat plan beside `criticalPathPath`; `undefined` when absent (a
 * campaign with no mandatory combat emits none, and that is not an error). */
export async function loadCombatPlanForCriticalPath(
  criticalPathPath: string,
): Promise<CombatPlan | undefined> {
  const p = path.join(path.dirname(criticalPathPath), ...COMBAT_PLAN_SUBPATH);
  let text: string;
  try {
    text = await readFile(p, "utf8");
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return undefined;
    throw err;
  }
  return parseCombatPlan(JSON.parse(text) as unknown);
}

// ---------------------------------------------------------------------------
// Combat assist (spec-0023 §3)
// ---------------------------------------------------------------------------

/**
 * Resistance amplifier the assist grants (amplifier 2 = Resistance III = 60%
 * incoming-damage reduction).
 *
 * Deliberately NOT amplifier 4, which is total immunity: an invulnerable bot
 * would stop proving anything about the fight at all — a wave that cannot damage
 * it would read exactly like a wave that can. 60% is the smallest reduction that
 * reliably survives a souls-tuned stack's opening exchange while still leaving
 * the encounter able to kill a bot that never fights back.
 */
export const ASSIST_AMPLIFIER = 2;

/** How long one assist window lasts, in seconds. Bounded by construction: the
 * effect expires on its own even if the harness crashes before clearing it. */
export const ASSIST_SECONDS = 60;

/** The same window in ticks — the unit the run report states (spec-0023 §3
 * requires every window be named with its encounter id and ticks). */
export const ASSIST_TICKS = ASSIST_SECONDS * 20;

/** The vanilla command that opens an assist window on the acting bot. */
export function assistCommand(
  amplifier: number = ASSIST_AMPLIFIER,
  seconds: number = ASSIST_SECONDS,
): string {
  return `/effect give @s minecraft:resistance ${seconds} ${amplifier} true`;
}

/** The vanilla command that closes it. Always issued, even on a failed fight —
 * an assist that outlives its encounter would silently help the next one. */
export function assistClearCommand(): string {
  return "/effect clear @s minecraft:resistance";
}

/**
 * How an encounter is approached.
 *
 * `unassisted-first` is the inverted floor gate in action: a fight the content
 * BILLED as elite/boss gets one honest, unassisted attempt, because whether the
 * bot wins that attempt is the measurement. An `ordinary` encounter carries no
 * such billing, so there is nothing to measure and the assist is applied from
 * the start.
 */
export function assistPolicy(enc: Encounter): "unassisted-first" | "assisted" {
  return enc.tier === "ordinary" ? "assisted" : "unassisted-first";
}

/**
 * How far `kill()` got with one encounter.
 *
 * The run artifact states this per encounter because an EMPTY `assist_windows`
 * array is otherwise unreadable: spec-0023 takes NO assist during the die-retry
 * stage (the whole point there is to die) and none on the first attempt at a
 * billed `elite`/`boss` (the inverted floor gate needs one honest unassisted
 * try). So zero windows is entirely possible per policy — and, before this
 * field existed, indistinguishable from an assist mechanism that was never
 * wired at all (task #102, the-drowned-bell round 3).
 */
export const ENCOUNTER_PHASES = [
  "not-reached",
  "die-retry",
  "unassisted",
  "assisted",
  "cleared",
] as const;
export type EncounterPhase = (typeof ENCOUNTER_PHASES)[number];

/** One opened (and, normally, closed) assist window, as the run report states it. */
export interface AssistWindow {
  readonly encounter: string;
  readonly wave: string;
  readonly tier: EncounterTier;
  readonly amplifier: number;
  readonly ticks: number;
  readonly openedAtMs: number;
  closedAtMs?: number;
  /** Why the assist was taken — "policy" or "after an unassisted attempt failed". */
  readonly reason: string;
}

/** The ledger the run report is built from. Every window is recorded, opened or
 * not closed; a window the harness failed to close is a finding, not a silence. */
export class AssistLedger {
  private readonly opened: AssistWindow[] = [];

  open(enc: Encounter, reason: string, nowMs: number): AssistWindow {
    const w: AssistWindow = {
      encounter: enc.objective,
      wave: enc.wave,
      tier: enc.tier,
      amplifier: ASSIST_AMPLIFIER,
      ticks: ASSIST_TICKS,
      openedAtMs: nowMs,
      reason,
    };
    this.opened.push(w);
    return w;
  }

  close(w: AssistWindow, nowMs: number): void {
    w.closedAtMs = nowMs;
  }

  windows(): readonly AssistWindow[] {
    return this.opened;
  }

  /** Windows the harness opened and never closed — a bug in the harness, and one
   * the report must show rather than swallow. */
  leaked(): readonly AssistWindow[] {
    return this.opened.filter((w) => w.closedAtMs === undefined);
  }
}

// ---------------------------------------------------------------------------
// The inverted floor gate (spec-0023 "bot as difficulty FLOOR")
// ---------------------------------------------------------------------------

/** The outcome of the unassisted attempt at a billed encounter. */
export interface UnassistedOutcome {
  readonly attempted: boolean;
  readonly won: boolean;
}

/**
 * The floor finding, or `undefined` when there is nothing to say.
 *
 * WARNING tier by construction — it returns prose, never a failure. A fight the
 * bot beats cold is a design signal for the author, and spec-0023 is explicit
 * that content decides. Ordinary encounters carry no expectation at all, so they
 * never produce a finding however easily they fall.
 */
export function floorFinding(
  enc: Encounter,
  outcome: UnassistedOutcome,
): string | undefined {
  if (enc.tier === "ordinary") return undefined;
  if (!outcome.attempted || !outcome.won) return undefined;
  return (
    `${enc.wave} is billed \`${enc.tier}\` and the UNASSISTED bot beat it on its first ` +
    `attempt. The bot is a poor fencer by design — a fight it wins cold is very ` +
    `likely too easy to carry that billing in a souls delve. Advisory: raise the ` +
    `stack, or drop the tier to \`ordinary\`.`
  );
}

// ---------------------------------------------------------------------------
// The die-retry ladder stage (spec-0023 §1) — the load-bearing combat proof
// ---------------------------------------------------------------------------

/** Scripted deaths per encounter. spec-0023's default: one at first contact, one
 * mid-fight, because the two exercise different re-seat state. */
export const DIE_RETRY_DEATHS = 2;

/** When in the fight a scripted death is taken. */
export const DEATH_PHASES = ["first-contact", "mid-fight"] as const;
export type DeathPhase = (typeof DEATH_PHASES)[number];

/** The phases for `n` scripted deaths, cycling the two shapes. */
export function deathPhases(n: number = DIE_RETRY_DEATHS): DeathPhase[] {
  return Array.from({ length: n }, (_, i) => DEATH_PHASES[i % DEATH_PHASES.length]!);
}

/** The vanilla command the harness kills itself with. `/damage` rather than
 * `/kill` so the death runs the ordinary damage path a player's death runs —
 * `/kill` bypasses damage handling entirely and would prove a loop no player can
 * take. */
export function scriptedDeathCommand(): string {
  return "/damage @s 1000 minecraft:generic";
}

/**
 * How far from the governing checkpoint a respawn may land and still count.
 *
 * Vanilla's respawn search moves a player off an obstructed spawn point, so an
 * exact match would be a false red; 8 blocks is loose enough to absorb that and
 * far tighter than the distance to any other checkpoint a delve would set.
 */
export const RESPAWN_RADIUS = 8;

/** Did the bot come back where the campaign said it would? */
export function respawnedAtCheckpoint(
  pos: Vec3Tuple,
  checkpoint: Vec3Tuple | undefined,
  radius: number = RESPAWN_RADIUS,
): boolean {
  // No declared checkpoint yet → the world spawn governs, which the harness has
  // no independent statement of. Not a finding: there is nothing to contradict.
  if (checkpoint === undefined) return true;
  const dx = pos[0] - checkpoint[0];
  const dy = pos[1] - checkpoint[1];
  const dz = pos[2] - checkpoint[2];
  return Math.sqrt(dx * dx + dy * dy + dz * dz) <= radius;
}

/**
 * What the harness found when it walked back to the encounter.
 *
 * The stage's sacred property is that **dying is safe for PROGRESSION** — not
 * that the fight must still be standing there. A wave the party already beat
 * before dying is not a broken retry loop; it is a won fight staying won, which
 * is exactly what a player who dies to the last mob's parting hit experiences.
 * Reading "no hostile present" as a uniform red made the verdict depend on
 * whether the bot's timed melee happened to finish the wave — the same fixture
 * went red then green on consecutive runs (task #102 follow-up; planner ruling
 * 2026-08-03).
 *
 * The distinction that actually matters is whether the party can still FINISH:
 *
 *   * `re-engaged`           — hostiles are there again. The fight is retriable.
 *   * `cleared-before-retry` — nothing left to fight, and the encounter's
 *     objective is COMPLETE. The death cost nothing; progression is intact.
 *   * `stranded`             — nothing left to fight and the objective is NOT
 *     complete. The fight can neither be finished nor re-fought: a soft lock,
 *     and precisely what this stage exists to catch.
 *
 * `unproven` is the opening value: the loop never got far enough to look.
 */
export const RETRY_OUTCOMES = [
  "unproven",
  "re-engaged",
  "cleared-before-retry",
  "stranded",
] as const;
export type RetryOutcome = (typeof RETRY_OUTCOMES)[number];

/** Decide the outcome from the two observations that determine it. */
export function retryOutcome(waveMobPresent: boolean, objectiveComplete: boolean): RetryOutcome {
  if (waveMobPresent) return "re-engaged";
  return objectiveComplete ? "cleared-before-retry" : "stranded";
}

/** One scripted death and everything proved about the loop it opened. */
export interface DeathTrial {
  readonly encounter: string;
  readonly wave: string;
  readonly attempt: number;
  readonly phase: DeathPhase;
  /** What the harness found waiting for it at the end of the loop. */
  readonly outcome: RetryOutcome;
  /** The death message the loop opened with, when the server broadcast one. */
  readonly cause: string | undefined;
  /** Where the bot respawned. */
  readonly respawnPos: Vec3Tuple | undefined;
  /** Did it respawn at the governing checkpoint? */
  readonly atCheckpoint: boolean;
  /** Did it walk back to the encounter? */
  readonly returned: boolean;
  /** Raw observation behind {@link outcome}: was a wave mob standing there again? */
  readonly reEngaged: boolean;
  /** Raw observation behind {@link outcome}: is the encounter's objective complete? */
  readonly objectiveComplete: boolean;
  /** Objectives that were complete before the death and are still complete after. */
  readonly objectivesIntact: boolean;
  /** Objectives that were complete before the death and were NOT after. */
  readonly lostObjectives: readonly string[];
  /** Did the loop run all the way to its verdict? A trial the run abandoned
   * half-way proves nothing about respawn, return or re-engagement. */
  readonly completed: boolean;
  /** Why it did not, when `completed` is false. */
  readonly abortedWith: string | undefined;
}

/**
 * The same record while the harness is still filling it in.
 *
 * A trial is entered in the ledger the MOMENT the harness commits to dying, not
 * when the loop reaches its verdict, and every fact is written as it is learned.
 * A death that happened and went unrecorded is the one thing this artifact must
 * never do: the-drowned-bell round 3 shipped a report with `die_retry: []` and
 * `passed: true` beside a log line naming the death it had just taken (task
 * #102).
 */
export type DeathTrialRecord = { -readonly [K in keyof DeathTrial]: DeathTrial[K] };

/** Open a trial: the record as it exists between the death command and the first
 * fact learned about the loop. Nothing is assumed proved — every verdict field
 * starts at its FAILING value, so a record abandoned here reads red, not green. */
export function openTrial(enc: Encounter, attempt: number, phase: DeathPhase): DeathTrialRecord {
  return {
    encounter: enc.objective,
    wave: enc.wave,
    attempt,
    phase,
    outcome: "unproven",
    cause: undefined,
    respawnPos: undefined,
    atCheckpoint: false,
    returned: false,
    reEngaged: false,
    objectiveComplete: false,
    objectivesIntact: true,
    lostObjectives: [],
    completed: false,
    abortedWith: undefined,
  };
}

/** The verdict on one trial: a red run, or nothing. */
export function trialVerdict(t: DeathTrial): string | undefined {
  if (!t.completed) {
    return (
      `${t.wave} death ${t.attempt} (${t.phase}): the retry loop was ABANDONED before it ` +
      `reached a verdict — ${t.abortedWith ?? "no reason was recorded"}. The bot died and ` +
      `the run ended there, so nothing is known about respawn, return or re-engagement. ` +
      `An unfinished trial is never a passed one.`
    );
  }
  if (!t.atCheckpoint) {
    return (
      `${t.wave} death ${t.attempt} (${t.phase}): respawned at ` +
      `${t.respawnPos ? t.respawnPos.join(",") : "an unknown position"}, which is not the ` +
      `checkpoint governing this encounter. Dying must always be safe — an unpredictable ` +
      `respawn point is the one thing a souls delve cannot ship.`
    );
  }
  if (!t.returned) {
    return (
      `${t.wave} death ${t.attempt} (${t.phase}): the route from the respawn back to the ` +
      `encounter is not walkable. The retry loop is broken: the party can die but not ` +
      `try again.`
    );
  }
  if (!t.objectivesIntact) {
    return (
      `${t.wave} death ${t.attempt} (${t.phase}): dying LOST completed progress ` +
      `(${t.lostObjectives.join(", ")}). Progress is kept across death by contract ` +
      `(spec-0016 §1) — this is state corruption, not difficulty.`
    );
  }
  if (t.outcome === "stranded") {
    return (
      `${t.wave} death ${t.attempt} (${t.phase}): after the walk back there was no hostile ` +
      `left to fight AND \`${t.encounter}\` is still incomplete. The encounter can neither ` +
      `be finished nor re-fought, so a party that dies here is STRANDED — a soft lock, not ` +
      `difficulty. (A wave that does not re-seat is legitimate; a wave that vanishes with ` +
      `its objective unfinished is not.)`
    );
  }
  if (t.outcome === "unproven") {
    return (
      `${t.wave} death ${t.attempt} (${t.phase}): the loop finished without establishing ` +
      `whether the encounter could be re-engaged or was already cleared. Nothing was ` +
      `proved about the retry loop, so nothing is passed.`
    );
  }
  // `re-engaged` (the fight is retriable) and `cleared-before-retry` (the fight
  // was already won and the objective survived the death) are both the loop
  // WORKING: in each case a party that dies here can still finish the delve.
  return undefined;
}

/** Every finding across a stage's trials, in order. */
export function dieRetryFindings(trials: readonly DeathTrial[]): string[] {
  return trials.map(trialVerdict).filter((v): v is string => v !== undefined);
}

/**
 * Whether the stage actually PROVED what it claims, encounter by encounter.
 *
 * The per-trial verdicts above can only judge trials that exist. The failure
 * mode they cannot see is silence: a stage that engaged an encounter and
 * recorded nothing produced an empty `die_retry` array, and an empty array of
 * findings, and therefore read `passed: true` — which is exactly how a run that
 * aborted at its first scripted death reported a green die-retry stage (task
 * #102). Coverage closes that: the stage owes `expected` COMPLETED trials for
 * every encounter the compiler put in the plan, and anything less is a stage
 * that did not prove its property, whatever else went right.
 */
export function dieRetryCoverageFailures(
  plan: readonly Encounter[],
  engagedWaves: ReadonlySet<string>,
  trials: readonly DeathTrial[],
  expected: number = DIE_RETRY_DEATHS,
): string[] {
  const out: string[] = [];
  for (const enc of plan) {
    const recorded = trials.filter((t) => t.wave === enc.wave);
    const proved = recorded.filter((t) => t.completed).length;
    if (proved >= expected) continue;
    if (engagedWaves.has(enc.wave)) {
      out.push(
        `${enc.wave}: the die-retry stage ENGAGED this encounter but proved only ` +
          `${proved}/${expected} scripted death(s) (${recorded.length} recorded). ` +
          `"Dying is always safe" is unproven here, so the stage has not passed.`,
      );
    } else {
      out.push(
        `${enc.wave}: the die-retry stage never reached this encounter — the run ended ` +
          `first. Its retry loop is unproven, so the stage cannot report a pass for it.`,
      );
    }
  }
  return out;
}
