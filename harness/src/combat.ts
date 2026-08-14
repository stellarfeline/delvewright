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
import type { CensusMob, CensusSummary } from "./markers.ts";

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
  /** The compiler-emitted tag-census probe for this wave (task #123). The names
   * come from the plan and are never re-derived here: `safe_local` is a compiler
   * naming rule, and reimplementing it in the harness is the downstream folklore
   * CLAUDE.md forbids. Required — a build too old to state them cannot be
   * measured by tag, and failing loudly beats silently measuring by silhouette. */
  readonly census: CensusProbe;
}

/** The three functions the ladder calls to measure one wave by tag. */
export interface CensusProbe {
  /** Counts the wave's standing mobs and states the totals on the chat channel. */
  readonly census: string;
  /** Stamps this life's mobs, so the next census can name the survivors. */
  readonly brand: string;
  /** Clears the stamp. */
  readonly unbrand: string;
}

/**
 * One beat that stages or unleashes an actor, as the plan states it (compiler
 * `ActorBeat`, #222).
 *
 * This is what makes an actor fight *schedulable* at all. A wave encounter has a
 * `kill` step on the critical path, so the bot already knows when the fight
 * starts; an actor fight starts because something completed, or because a player
 * struck, used or walked into something — and that "something" is only stated
 * here. `site` is the half that decides whether the run can reach it: an
 * `objective` beat fires when the path completes that objective, while a
 * `trigger` beat is player-initiated and has no position in the quest DAG.
 */
export interface ActorBeat {
  readonly site: "trigger" | "quest" | "objective" | "trap";
  /** The owning trigger / quest / trap id. */
  readonly owner: string;
  /** The objective, when the site is a quest's `on_objective_complete`. */
  readonly objective?: string;
  /** JSON pointer to the effect, so a report line can name it exactly. */
  readonly path: string;
  /** Trigger sites: the event kind (`approach`/`strike`/`use`/`strike-npc`). */
  readonly on?: string;
  /** Trigger sites: the anchor watched. */
  readonly at?: string;
  /** `strike-npc` triggers: the NPC whose body is the target. */
  readonly npc?: string;
}

/**
 * Whether the compiler believes the inverted floor gate can measure a fight, and
 * — when it cannot — the reason, in the author's own terms.
 *
 * Carried verbatim into the run report. The whole point of the ledger (#222) is
 * that **silence must not read as a pass**: an encounter nobody fought and an
 * encounter fought and lost produce the same empty findings list, and only this
 * tells them apart.
 */
export interface FloorCoverage {
  readonly covered: boolean;
  readonly reason?: string;
}

/** One tier-declaring stage-5 actor, as the validation ladder sees it (#222). */
export interface ActorEncounter {
  readonly actor: string;
  /** The vanilla entity puppeted and unleashed (`minecraft:wither_skeleton`). */
  readonly entity: string;
  readonly name?: string;
  readonly tier: EncounterTier;
  readonly anchor: string;
  /** The anchor resolved to a world cell — where the bot walks to fight it. */
  readonly pos: Vec3Tuple | undefined;
  readonly tag: string;
  readonly vulnerable: boolean;
  readonly spawnedBy: readonly ActorBeat[];
  readonly unleashedBy: readonly ActorBeat[];
  readonly floorGate: FloorCoverage;
  /** Declared `max_health`, when the actor overrides it — report context only. */
  readonly maxHealth: number | undefined;
}

/** One line of the compiler's floor-gate ledger. */
export interface FloorLedgerEntry {
  readonly kind: string;
  readonly id: string;
  /**
   * The declared tier — `undefined` for an **untiered hostile** (task #121): an
   * actor the campaign unleashes on the party without billing the fight at all.
   * It is always a `not_covered` entry, because nothing declared what the gate
   * was supposed to hold it to.
   */
  readonly tier?: EncounterTier;
  /** Present exactly on a not-covered entry. */
  readonly reason?: string;
}

/**
 * A gate's own statement of what it bound to (playtest-methodology.md rule 1):
 * how many objects it examined, and — when that count is zero — why, in the
 * compiler's own words. A ledger that matched zero objects and one that
 * matched several and found them all fine look identical to a reader who is
 * not counting; this is the field that tells them apart without the reader
 * having to notice an empty array. `reason` is present exactly when `unbound`
 * is `true` — a bound gate carries no reason to explain.
 */
export interface BindingCount {
  readonly examined: number;
  readonly unbound: boolean;
  readonly reason?: string;
}

/**
 * The floor-gate ledger: every encounter billed `elite`/`boss` plus every
 * untiered hostile actor, split into what the gate covers and what it cannot.
 *
 * `present: false` means the build carried NO ledger (a plan from a delvec older
 * than #222) — deliberately distinct from a present-but-empty ledger, because
 * "this campaign bills nothing hard" and "this build cannot tell you" are
 * different facts and only one of them is reassuring. `binding` is undefined
 * for the same reason `present` can be false: a plan from a delvec older than
 * the binding-count fields simply does not carry one.
 */
export interface FloorLedger {
  readonly present: boolean;
  readonly covered: readonly FloorLedgerEntry[];
  readonly notCovered: readonly FloorLedgerEntry[];
  readonly binding?: BindingCount;
}

/** The parsed combat plan. */
export interface CombatPlan {
  readonly version: string;
  readonly campaignId: string;
  /** The declared world difficulty the run is verified AT (spec-0023 §3). */
  readonly difficulty: string;
  readonly encounters: readonly Encounter[];
  /** Tier-declaring stage-5 actors — the other shape an elite takes (#222). */
  readonly actors: readonly ActorEncounter[];
  /** The compiler's coverage ledger, printed verbatim in the run report. */
  readonly floorGate: FloorLedger;
  /**
   * `actors[]`'s own binding count: how many actors this build's tier
   * machinery tracked at all (any declared tier, `ordinary` included).
   * Undefined for a plan from a delvec that predates it — see
   * {@link FloorLedger.binding}.
   */
  readonly actorsGate?: BindingCount;
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
      census: parseCensusProbe(e["census"], `${p}/census`),
    };
  });
  return {
    version,
    campaignId,
    difficulty,
    encounters,
    actors: parseActors(raw["actors"], "/actors"),
    floorGate: parseFloorLedger(raw["floor_gate"], "/floor_gate"),
    actorsGate: parseBindingCount(raw["actors_gate"], "/actors_gate"),
  };
}

/**
 * A `{examined, unbound, reason?}` block (playtest-methodology.md rule 1).
 * `undefined` input parses as `undefined` — a plan from a delvec that predates
 * these fields carries no binding count, and that is a different fact from a
 * present one that happens to be zero.
 */
function parseBindingCount(v: unknown, pointer: string): BindingCount | undefined {
  if (v === undefined) return undefined;
  if (!isRecord(v)) throw new CombatPlanParseError(pointer, "expected an object");
  const examined = v["examined"];
  if (!Number.isInteger(examined) || (examined as number) < 0) {
    throw new CombatPlanParseError(`${pointer}/examined`, "expected a non-negative integer");
  }
  const unbound = v["unbound"];
  if (typeof unbound !== "boolean") {
    throw new CombatPlanParseError(`${pointer}/unbound`, "expected a boolean");
  }
  if (unbound !== (examined === 0)) {
    throw new CombatPlanParseError(pointer, "`unbound` must be exactly `examined === 0`");
  }
  // A zero binding without its reason is the exact silence this field exists
  // to end, so it is a parse error rather than a quietly-absent explanation.
  if (unbound && (typeof v["reason"] !== "string" || (v["reason"] as string).length === 0)) {
    throw new CombatPlanParseError(`${pointer}/reason`, "an unbound gate must state why");
  }
  return unbound
    ? { examined: examined as number, unbound: true, reason: v["reason"] as string }
    : { examined: examined as number, unbound: false };
}

/**
 * The census probe block (task #123). Required: measuring a wave by tag is how
 * the ladder counts anything at all now, so a plan that cannot name its census
 * functions is a plan this harness must refuse rather than quietly fall back to
 * counting silhouettes.
 */
function parseCensusProbe(v: unknown, pointer: string): CensusProbe {
  if (!isRecord(v)) throw new CombatPlanParseError(pointer, "expected an object");
  for (const key of ["census", "brand", "unbrand"] as const) {
    if (typeof v[key] !== "string" || (v[key] as string).length === 0) {
      throw new CombatPlanParseError(`${pointer}/${key}`, "expected a non-empty function id");
    }
  }
  return {
    census: v["census"] as string,
    brand: v["brand"] as string,
    unbrand: v["unbrand"] as string,
  };
}

function requireTier(v: unknown, pointer: string): EncounterTier {
  if (typeof v !== "string" || !ENCOUNTER_TIERS.includes(v as EncounterTier)) {
    throw new CombatPlanParseError(pointer, `expected one of ${ENCOUNTER_TIERS.join("|")}`);
  }
  return v as EncounterTier;
}

function requireString(o: Record<string, unknown>, key: string, pointer: string): string {
  const v = o[key];
  if (typeof v !== "string" || v.length === 0) {
    throw new CombatPlanParseError(`${pointer}/${key}`, "expected a non-empty string");
  }
  return v;
}

function optionalString(o: Record<string, unknown>, key: string, pointer: string): string | undefined {
  const v = o[key];
  if (v === undefined) return undefined;
  if (typeof v !== "string" || v.length === 0) {
    throw new CombatPlanParseError(`${pointer}/${key}`, "expected a non-empty string when present");
  }
  return v;
}

const BEAT_SITES = ["trigger", "quest", "objective", "trap"] as const;

function parseBeats(v: unknown, pointer: string): ActorBeat[] {
  if (!Array.isArray(v)) throw new CombatPlanParseError(pointer, "expected an array");
  return v.map((b, i): ActorBeat => {
    const p = `${pointer}/${i}`;
    if (!isRecord(b)) throw new CombatPlanParseError(p, "expected an object");
    const site = b["site"];
    if (typeof site !== "string" || !BEAT_SITES.includes(site as never)) {
      throw new CombatPlanParseError(`${p}/site`, `expected one of ${BEAT_SITES.join("|")}`);
    }
    return {
      site: site as ActorBeat["site"],
      owner: requireString(b, "owner", p),
      objective: optionalString(b, "objective", p),
      path: requireString(b, "path", p),
      on: optionalString(b, "on", p),
      at: optionalString(b, "at", p),
      npc: optionalString(b, "npc", p),
    };
  });
}

function parseCoverage(v: unknown, pointer: string): FloorCoverage {
  if (!isRecord(v)) throw new CombatPlanParseError(pointer, "expected an object");
  const covered = v["covered"];
  if (typeof covered !== "boolean") {
    throw new CombatPlanParseError(`${pointer}/covered`, "expected a boolean");
  }
  // A not-covered entry without its reason would be the exact silence the ledger
  // exists to end, so it is a parse error rather than an empty string.
  if (!covered && (typeof v["reason"] !== "string" || (v["reason"] as string).length === 0)) {
    throw new CombatPlanParseError(`${pointer}/reason`, "a not-covered entry must state why");
  }
  return covered ? { covered: true } : { covered: false, reason: v["reason"] as string };
}

/**
 * The plan's `actors[]`. Absent (a plan from a delvec older than #222) parses as
 * an empty list — the run then says the ledger is absent rather than pretending
 * the campaign declares no tiered actor.
 */
function parseActors(v: unknown, pointer: string): ActorEncounter[] {
  if (v === undefined) return [];
  if (!Array.isArray(v)) throw new CombatPlanParseError(pointer, "expected an array");
  return v.map((a, i): ActorEncounter => {
    const p = `${pointer}/${i}`;
    if (!isRecord(a)) throw new CombatPlanParseError(p, "expected an object");
    const attributes = a["attributes"];
    const maxHealth =
      isRecord(attributes) && typeof attributes["max_health"] === "number"
        ? (attributes["max_health"] as number)
        : undefined;
    return {
      actor: requireString(a, "actor", p),
      entity: requireString(a, "entity", p),
      name: optionalString(a, "name", p),
      tier: requireTier(a["tier"], `${p}/tier`),
      anchor: requireString(a, "anchor", p),
      // Absent past DW0325, but the plan types it optional and a missing cell is
      // exactly a "nowhere to walk" skip reason rather than a parse failure.
      pos: a["pos"] === undefined ? undefined : requirePos(a["pos"], `${p}/pos`),
      tag: requireString(a, "tag", p),
      vulnerable: a["vulnerable"] === true,
      spawnedBy: parseBeats(a["spawned_by"], `${p}/spawned_by`),
      unleashedBy: parseBeats(a["unleashed_by"], `${p}/unleashed_by`),
      floorGate: parseCoverage(a["floor_gate"], `${p}/floor_gate`),
      maxHealth,
    };
  });
}

function parseLedgerSide(v: unknown, pointer: string, needReason: boolean): FloorLedgerEntry[] {
  if (!Array.isArray(v)) throw new CombatPlanParseError(pointer, "expected an array");
  return v.map((e, i): FloorLedgerEntry => {
    const p = `${pointer}/${i}`;
    if (!isRecord(e)) throw new CombatPlanParseError(p, "expected an object");
    const reason = optionalString(e, "reason", p);
    if (needReason && reason === undefined) {
      throw new CombatPlanParseError(`${p}/reason`, "a not-covered entry must state why");
    }
    // `tier: null` (or absent) is the compiler saying the entry declares NO
    // tier — the untiered hostile of task #121. Anything else present must
    // still be a real tier, so a typo can never be read as "untiered".
    const tier = e["tier"];
    return {
      kind: requireString(e, "kind", p),
      id: requireString(e, "id", p),
      ...(tier === null || tier === undefined
        ? {}
        : { tier: requireTier(tier, `${p}/tier`) }),
      reason,
    };
  });
}

/** The plan's `floor_gate`. Absent → `present: false` (see {@link FloorLedger}). */
function parseFloorLedger(v: unknown, pointer: string): FloorLedger {
  if (v === undefined) return { present: false, covered: [], notCovered: [] };
  if (!isRecord(v)) throw new CombatPlanParseError(pointer, "expected an object");
  const covered = parseLedgerSide(v["covered"], `${pointer}/covered`, false);
  const notCovered = parseLedgerSide(v["not_covered"], `${pointer}/not_covered`, true);
  const binding = parseBindingCount(
    v["examined"] === undefined && v["unbound"] === undefined
      ? undefined
      : { examined: v["examined"], unbound: v["unbound"], reason: v["reason"] },
    pointer,
  );
  if (binding !== undefined && binding.examined !== covered.length + notCovered.length) {
    throw new CombatPlanParseError(
      pointer,
      `examined (${binding.examined}) must equal covered.length + not_covered.length ` +
        `(${covered.length + notCovered.length})`,
    );
  }
  return { present: true, covered, notCovered, binding };
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
 * The run artifact states this per encounter because an `assist_windows` array is
 * otherwise unreadable: no assist is taken on the first attempt at a billed
 * `elite`/`boss` (the inverted floor gate needs one honest unassisted try), so a
 * short array is possible per policy — and, before this field existed,
 * indistinguishable from an assist mechanism that was never wired at all (task
 * #102, the-drowned-bell round 3).
 *
 * The die-retry stage DOES take assist windows (task #121), which is a correction
 * rather than a softening. It dies on purpose, but it needs the bot alive long
 * enough to SCHEDULE that death and to walk back afterwards; leaving those
 * segments bare made bot fencing skill decide whether the stage could run at all
 * — the very thing spec-0023 downgraded from gate to telemetry. Every window is
 * still named in the artifact, and the scripted death itself is taken with no
 * assist in force, so it stays unambiguous.
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
// The floor gate on ACTORS (spec-0023's gate, #222's other encounter shape)
// ---------------------------------------------------------------------------

/**
 * Whether this run can honestly measure an actor fight, and — when it cannot —
 * the reason, in the same voice the compiler's ledger uses.
 *
 * `exercise` carries the objective whose completion unleashes the actor: that is
 * the ONLY moment the harness can know a fight starts without inventing one. A
 * `trigger` beat (`strike`, `use`, `approach`, `strike-npc`) is player-initiated
 * and, as `compiler::flow` puts it, has no position in the quest DAG — the
 * campaign does not schedule it, so neither may the bot. Scheduling it anyway
 * would fabricate a fight and then report telemetry about the fabrication.
 */
export type ActorExercise =
  | { readonly kind: "exercise"; readonly afterObjective: string }
  | { readonly kind: "skip"; readonly reason: string };

/**
 * Decide, from the plan alone, whether the bot fights this actor on this run.
 *
 * `pathObjectives` is the set of `obj/<id>` the compiled critical path proves —
 * every step's objective. Pure and total: every actor gets either an exercise or
 * a NAMED skip, because an actor missing from the report entirely is the silence
 * the whole ledger exists to end.
 */
export function actorExercise(
  a: ActorEncounter,
  pathObjectives: ReadonlySet<string>,
): ActorExercise {
  if (a.tier === "ordinary") {
    return {
      kind: "skip",
      reason:
        "billed `ordinary` — the inverted floor gate measures only what the content bills " +
        "`elite`/`boss`, so there is no expectation here to hold it to",
    };
  }
  if (!a.floorGate.covered) {
    // The compiler already decided this and said why; repeating it in our own
    // words would let the two drift.
    return { kind: "skip", reason: `the compiler's floor gate does not cover it: ${a.floorGate.reason}` };
  }
  if (a.pos === undefined) {
    return {
      kind: "skip",
      reason: `the plan resolved no world cell for anchor \`${a.anchor}\`, so there is nowhere to walk`,
    };
  }
  const onPath = a.unleashedBy.find(
    (b) => b.site === "objective" && b.objective !== undefined && pathObjectives.has(b.objective),
  );
  if (onPath?.objective !== undefined) {
    return { kind: "exercise", afterObjective: onPath.objective };
  }
  return { kind: "skip", reason: unleashSkipReason(a) };
}

/** Why an actor the compiler covers is still not fought on THIS run. */
function unleashSkipReason(a: ActorEncounter): string {
  const beats = a.unleashedBy;
  if (beats.length === 0) {
    // Unreachable in practice (no unleash beat ⇒ not covered), kept because a
    // reason must exist for every skip, not for every skip we predicted.
    return "no `unleash-actor` beat is stated in the plan";
  }
  const objectives = beats.flatMap((b) => (b.site === "objective" && b.objective ? [b.objective] : []));
  if (objectives.length > 0) {
    return (
      `unleashed by ${objectives.map((o) => `\`${o}\``).join(", ")}, which the compiled critical ` +
      `path never completes — the fight is off this run's storyline`
    );
  }
  const quests = beats.flatMap((b) => (b.site === "quest" ? [b.owner] : []));
  if (quests.length > 0) {
    return (
      `unleashed when ${quests.map((q) => `\`${q}\``).join(", ")} completes; the critical path ` +
      `names objectives, not quests, so the harness cannot tell when that fires`
    );
  }
  const t = beats[0]!;
  const where = t.at !== undefined ? ` at \`${t.at}\`` : t.npc !== undefined ? ` on \`${t.npc}\`` : "";
  return (
    `unleashed only by an ambient \`${t.on ?? t.site}\` ${t.site} (\`${t.owner}\`${where}): a ` +
    `player-initiated beat with no position in the quest DAG. The campaign does not schedule ` +
    `it, so the bot inventing a moment to fire it would fabricate the fight it then reported on`
  );
}

/** How an actor engagement ended. */
export const ACTOR_OUTCOMES = ["won-first-try", "lost", "timed-out", "body-not-found"] as const;
export type ActorOutcome = (typeof ACTOR_OUTCOMES)[number];

/** One actor fight the run attempted, as the report states it. */
export interface ActorTrial {
  readonly actor: string;
  readonly tier: EncounterTier;
  readonly afterObjective: string;
  readonly outcome: ActorOutcome;
  /** Melee swings landed on the body — the reading key for `body-not-found`. */
  readonly swings: number;
  readonly elapsedMs: number;
  /** What ended it, when something did. */
  readonly detail?: string;
}

/**
 * The floor finding for an actor fight, or `undefined` when there is nothing to
 * say. Same rule and same tier as the wave gate: WARNING, never a failure.
 *
 * A bot that loses is exactly the design — spec-0023 downgraded bot melee
 * competence from gate-critical to telemetry so a souls delve could be as hard
 * as it likes. What is worth saying out loud is the inverse.
 */
export function actorFloorFinding(t: ActorTrial): string | undefined {
  if (t.tier === "ordinary" || t.outcome !== "won-first-try") return undefined;
  return (
    `${t.actor} is billed \`${t.tier}\` and the UNASSISTED bot beat it on its first attempt ` +
    `(${t.swings} swing(s), ${(t.elapsedMs / 1000).toFixed(1)}s after ${t.afterObjective}). The ` +
    `bot is a poor fencer by design — a fight it wins cold is very likely too easy to carry ` +
    `that billing in a souls delve. Advisory: raise the stack, or drop the tier to \`ordinary\`.`
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

/** One completed census of one wave: the summary line and the mob lines it closed. */
export interface WaveCensus {
  readonly summary: CensusSummary;
  readonly mobs: readonly CensusMob[];
}

/**
 * What the bot found when it walked back, as a whole SET rather than a nearest hit.
 *
 * Two failures live here, and they need different evidence:
 *
 *  * the false negative (island r14): the probe used to be a single instantaneous
 *    sample the moment the walk-back resolved. Entity tracking is not
 *    instantaneous — `fightWave` has always slept a second on arrival for exactly
 *    this reason — so three demonstrably-alive drowned read as "no hostile was
 *    there to fight" and the trial went red. The probe now SETTLES.
 *  * the fidelity failure (owner ruling 2026-08-03): a retry must never let the
 *    party chip a wave down across lives. A re-seating wave must come back whole
 *    — the authored count, all-new entities, undamaged — never topped up around
 *    the survivors the last life left standing.
 */
export interface ReengageObservation {
  /** Wave mobs present after the settle. */
  readonly present: number;
  /** What the compiler's plan says the wave holds. */
  readonly declared: number;
  /** Of those present, how many still wear the BRAND applied to this wave's live
   * mobs just before the scripted death. On a re-seating wave every one of these
   * is a survivor that was not cleared — the chipped mob the ruling forbids
   * carrying across a life. Counted server-side by tag, so a neighbouring wave's
   * mob or an ambush actor standing nearby can never be mistaken for one (#230). */
  readonly carriedOver: number;
  /** How many had readable health, and how many of those were below full. The
   * census reads `Health` and `max_health` off each mob with vanilla's own
   * commands, so on this path every counted mob is readable — unlike the client
   * view, which is never sent an unmodified max health at all. */
  readonly healthReadable: number;
  readonly damaged: number;
  /** Distance spread from the encounter anchor, for the wandered-mob case. */
  readonly nearest: number | undefined;
  readonly farthest: number | undefined;
  /** How long the probe waited before it settled on this answer. */
  readonly settleMs: number;
}

/**
 * Summarize one settled census (task #123).
 *
 * Every count here is the SERVER's answer about entities carrying the wave's own
 * tag. The distances are computed here rather than server-side only because
 * scoreboards have no square root; the positions they are computed from are the
 * census's, so they too describe the wave and nothing else.
 */
export function observationOf(
  census: WaveCensus,
  declared: number,
  anchor: Vec3Tuple,
  settleMs: number,
): ReengageObservation {
  const distances = census.mobs.map((m) =>
    Math.sqrt(
      (m.pos[0] - anchor[0]) ** 2 + (m.pos[1] - anchor[1]) ** 2 + (m.pos[2] - anchor[2]) ** 2,
    ),
  );
  return {
    present: census.summary.present,
    declared,
    carriedOver: census.summary.branded,
    healthReadable: census.mobs.length,
    damaged: census.summary.damaged,
    nearest: distances.length > 0 ? Math.min(...distances) : undefined,
    farthest: distances.length > 0 ? Math.max(...distances) : undefined,
    settleMs,
  };
}

/**
 * The re-seat fidelity verdict for one trial, or `undefined` when the wave came
 * back whole. Only ever consulted for a `respawns_on_rest` wave that re-engaged.
 *
 * Owner ruling 2026-08-03: "打一半的怪要移除重新生成一模一样的,玩家满血了怪也满血了,
 * 不能通过每条命砍一刀磨过去" — a half-fought wave is REMOVED and regenerated
 * identically; the player comes back full, so the wave does too. Grinding a boss
 * down one swing per life is not a difficulty curve, it is a bug.
 */
export function reseatFidelityFinding(
  wave: string,
  attempt: number,
  phase: DeathPhase,
  obs: ReengageObservation,
): string | undefined {
  const where = `${wave} death ${attempt} (${phase})`;
  if (obs.carriedOver > 0) {
    return (
      `${where}: ${obs.carriedOver} of the ${obs.present} wave mob(s) standing after the ` +
      `re-seat ${obs.carriedOver === 1 ? "is an entity" : "are entities"} the bot already ` +
      `fought in a previous life. A \`respawns_on_rest\` wave must be REMOVED and ` +
      `re-summoned whole, never topped up around its survivors — otherwise the party ` +
      `grinds it down one swing per death.`
    );
  }
  if (obs.present < obs.declared) {
    return (
      `${where}: the re-seated wave came back SHORT — ${obs.present} mob(s) standing, ` +
      `${obs.declared} declared. A retry must face the fight the first life faced.`
    );
  }
  if (obs.damaged > 0) {
    return (
      `${where}: ${obs.damaged} of the ${obs.healthReadable} wave mob(s) whose health could ` +
      `be read came back BELOW full. The player respawns whole; so must the wave.`
    );
  }
  return undefined;
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
  /**
   * Where the bot respawned — MEASURED (`bot.entity.position` the moment the
   * respawn settled), never the plan's expectation. Nothing between the respawn
   * and this reading may move the bot, which is why the post-death re-arm no
   * longer replays `select-class` (task #120: `class_apply_*` teleports).
   */
  readonly respawnPos: Vec3Tuple | undefined;
  /** Did it respawn at the governing checkpoint? Derived from {@link respawnPos}. */
  readonly atCheckpoint: boolean;
  /** Did the kit survive the death? The delve seals `gamerule keep_inventory true`,
   * so a bot that comes back empty-handed found that seal absent — and a player who
   * must re-gear after every death has no cheap retry (task #120). */
  readonly kitKept: boolean;
  /** Did it walk back to the encounter, from where it respawned? */
  readonly returned: boolean;
  /**
   * Raw observation behind {@link outcome}: was a wave mob standing there again?
   *
   * **Only observed when {@link returned}** (task #120). The probe reads the
   * entities the CLIENT tracks, so a bot that never got back describes the place it
   * is stuck in, not the encounter. `returned: false` therefore forces
   * `reEngaged: false`, `reengage: undefined` and `outcome: "unproven"` — "not
   * looked at" is a different fact from "looked at and empty", and the run-five
   * artifact reported an unwalkable route and a re-engaged fight in the same trial
   * precisely because the two were conflated.
   */
  readonly reEngaged: boolean;
  /** Raw observation behind {@link outcome}: is the encounter's objective complete?
   * Read off the scoreboard, so it is meaningful wherever the bot is standing. */
  readonly objectiveComplete: boolean;
  /** Does this wave re-seat on rest? Only such a wave owes re-seat fidelity. */
  readonly reseats: boolean;
  /** The settled set the outcome and the fidelity verdict were read from. */
  readonly reengage: ReengageObservation | undefined;
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
    kitKept: false,
    returned: false,
    reEngaged: false,
    objectiveComplete: false,
    reseats: enc.respawnsOnRest,
    reengage: undefined,
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
  if (!t.kitKept) {
    return (
      `${t.wave} death ${t.attempt} (${t.phase}): the bot came back EMPTY-HANDED. Every ` +
      `delve seals \`gamerule keep_inventory true\` — dying must never cost the kit — so ` +
      `this is a broken seal, not difficulty: the party would have to re-gear before every ` +
      `retry.`
    );
  }
  if (!t.returned) {
    return (
      `${t.wave} death ${t.attempt} (${t.phase}): the route from the respawn at ` +
      `${t.respawnPos ? t.respawnPos.join(",") : "an unknown position"} back to the ` +
      `encounter is not walkable. The retry loop is broken: the party can die but not ` +
      `try again. (Nothing is reported about re-engagement here — the bot never reached ` +
      `the fight to look at it.)`
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
  // A wave the content declared `respawns_on_rest` owes one more thing: it must
  // come back WHOLE. Checked last, because a stranded party or lost progress is a
  // worse fault than an imperfect re-seat and should be the sentence a reader sees.
  if (t.reseats && t.outcome === "re-engaged" && t.reengage !== undefined) {
    const fidelity = reseatFidelityFinding(t.wave, t.attempt, t.phase, t.reengage);
    if (fidelity !== undefined) return fidelity;
  }
  // `re-engaged` (the fight is retriable) and `cleared-before-retry` (the fight
  // was already won and the objective survived the death) are both the loop
  // WORKING: in each case a party that dies here can still finish the delve.
  return undefined;
}

/** A rest the bot actually performed, as the precondition check reads it. */
export interface PerformedRest {
  readonly bonfire: number;
  readonly anchor: string;
  readonly pos: Vec3Tuple;
  /** Index of the critical-path step that performed it. */
  readonly step: number;
}

/** How close a governing checkpoint has to sit to a bonfire to BE that bonfire. */
export const BONFIRE_MATCH_RADIUS = 2;

function near(a: Vec3Tuple, b: Vec3Tuple, radius: number): boolean {
  return (
    Math.abs(a[0] - b[0]) <= radius &&
    Math.abs(a[1] - b[1]) <= radius &&
    Math.abs(a[2] - b[2]) <= radius
  );
}

/**
 * Why the die-retry stage may not script a death at an encounter.
 *
 * `reds` is the whole reason this is a structure rather than a string. Both gaps
 * stop the scripted death, but they are different KINDS of fact:
 *
 *   * `unarmed` is about the RUN — the proof walked past the fire it was about to
 *     be measured against. That is the harness's own gap, and it reds the stage;
 *   * `no-checkpoint` is about the CONTENT — the campaign fires no checkpoint
 *     before this fight at all, so every death here is a full restart by design.
 *     Whether that is acceptable is a pacing judgement the compiler already owns
 *     (`DW0379` retry cost, the checkpoint proofs `DW0315`/`DW0316`), so the
 *     harness states it and declines to grade it.
 */
export interface CheckpointPreconditionGap {
  readonly kind: "unarmed" | "no-checkpoint";
  readonly finding: string;
  /** Whether this gap makes the die-retry stage RED, or is advisory only. */
  readonly reds: boolean;
}

/**
 * Can the die-retry stage honestly script a death at this encounter?
 *
 * A bonfire arms an affordance and moves nothing until the party rests
 * (spec-0016 §1). The combat plan's `checkpoint` is the last checkpoint the
 * campaign fires strictly BEFORE the encounter — for a bonfire that means armed,
 * not rested. Bell round 3 died into exactly that gap: every fire walked past
 * untouched, both trials respawned at world spawn on the far beach, and a 60s
 * walk-back budget judged the campaign for a loop the proof had never performed.
 *
 * The harness settles it from the two artifacts it already holds: the path's
 * `rest` steps say which checkpoints are bonfires, and the executor knows which
 * of those it performed. Four cases, and only two of them stop the death:
 *
 *   * the governing checkpoint sits on a bonfire the bot rested at → armed,
 *     proceed;
 *   * it matches no bonfire in the path → an ordinary `set-checkpoint`, which
 *     arms itself when its beat fires. Nothing to contradict, proceed;
 *   * it sits on a bonfire the bot walked past → **unarmed**, and a scripted
 *     death here would measure the campaign against a respawn point the player
 *     loop was never performed for. Red;
 *   * the plan names **no governing checkpoint at all** → the campaign fires none
 *     before this fight, so a death respawns at world spawn and the retry loop is
 *     a full restart. Advisory: this is a content fact, and in a souls campaign a
 *     design smell, but the compiler's checkpoint/retry-cost rules are what judge
 *     it. Post-#223 this is no longer hypothetical — `fire_step < i` means a
 *     checkpoint armed by the encounter's own kill step is correctly NOT its
 *     governing one, and souls-bonfire's encounter now truthfully reports none.
 *     Before that fix the same case was silently "armed", which is the answer
 *     that would have flattered the campaign.
 */
export function checkpointPrecondition(
  enc: Encounter,
  bonfires: readonly PerformedRest[],
  rested: ReadonlySet<number>,
  beforeStep: number,
): CheckpointPreconditionGap | undefined {
  if (enc.checkpoint === undefined) {
    return {
      kind: "no-checkpoint",
      reds: false,
      finding:
        `${enc.wave}: die-retry precondition: no governing checkpoint — die-retry cannot ` +
        `prove safe death here. The plan names no checkpoint fired before this encounter, so ` +
        `a death respawns at world spawn and the retry loop is a full restart of the delve. ` +
        `No death was taken. Advisory, not a failure: this is a CONTENT fact about where the ` +
        `campaign puts its rest points, and the compiler's rules own that judgement (DW0379 ` +
        `retry cost, DW0315/DW0316 checkpoint proofs) — the harness only reports that this ` +
        `fight's retry loop went unproven.`,
    };
  }
  // Matched by POSITION, never by step order: what makes a checkpoint unarmed is
  // that nobody has rested at it yet, and that is true whether the path rests
  // there later or never. Order only changes the sentence.
  const fire = bonfires.find((b) => near(b.pos, enc.checkpoint!, BONFIRE_MATCH_RADIUS));
  if (fire === undefined || rested.has(fire.bonfire)) return undefined;
  // BOTH sides of this comparison are EXPORTED path indices (compiler #223): the
  // combat plan's `step` is exported-coordinate now, and `beforeStep` is the index
  // the encounter is executing at. The rest splice inserts steps, so exported and
  // `plan.critical_path` indices drift by one per bonfire — which is exactly why
  // the plan was moved onto the exported system rather than the harness being
  // taught to convert. Nothing here consumes `enc.step`.
  const why =
    fire.step < beforeStep
      ? `the route passed bonfire ${fire.bonfire} (${fire.anchor}) without resting`
      : `the path does not rest at bonfire ${fire.bonfire} (${fire.anchor}) until AFTER ` +
        `this encounter (path step ${fire.step})`;
  return {
    kind: "unarmed",
    reds: true,
    finding:
      `${enc.wave}: die-retry precondition: no checkpoint armed — ${why}, so the governing ` +
      `checkpoint at ${enc.checkpoint.join(",")} has never moved. A bonfire ARMS on arrival ` +
      `and only moves the respawn point when the party RESTS; scripting a death now would ` +
      `measure the delve against world spawn. No death was taken — this is a gap in the ` +
      `proof, not a fault in the encounter.`,
  };
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

// ---------------------------------------------------------------------------
// The die-retry stage's own binding count (playtest-methodology.md rule 1)
// ---------------------------------------------------------------------------

/**
 * What the die-retry stage actually EXAMINED — counted, and stated on every run.
 *
 * ## Why this exists
 *
 * The stage's precondition is an ARMED checkpoint before a mandatory encounter,
 * and an encounter that has none is excluded with an advisory (#223) — correctly,
 * because where a campaign puts its rest points is a content judgement the
 * compiler's rules own, not the bot's. But the arithmetic downstream of that
 * exclusion is `dieRetryCoverageFailures` over an already-emptied list, so a build
 * where EVERY encounter is excluded produces no failures and reports
 * `passed: true`. The stage then reads as "dying is safe here" while having
 * scripted no death at all.
 *
 * That is not hypothetical and it is not rare: measured 2026-08-11 across every
 * campaign and fixture in both repos, **no build exercises a scripted death**.
 * `keep-trial` and `hollow-vigil` field encounters with no checkpoint before them;
 * `nobodys-cave-island` reports zero mandatory encounters; the drowned bell has no
 * stage documents yet. Every one of those runs was green on this stage.
 *
 * This is the same shape as the island's combat floor gate examining zero enemies
 * for nineteen rounds, and it gets the same treatment: the count travels in the
 * run artifact and an unbound stage says so out loud, so nobody has to notice an
 * empty `die_retry` array to learn that nothing was proven.
 */
export interface DieRetryBinding {
  /** Mandatory encounters the compiler put in the plan. */
  readonly declared: number;
  /** Encounters the stage entered. */
  readonly engaged: number;
  /** Scripted deaths actually TAKEN — the number that decides `unbound`. */
  readonly deathsScripted: number;
  /** Of those, how many reached a verdict about the retry loop. */
  readonly trialsCompleted: number;
  /** Encounters excluded because the campaign fires no checkpoint before them. */
  readonly skippedNoCheckpoint: number;
  /** Encounters excluded because the governing checkpoint was never armed. */
  readonly skippedUnarmed: number;
  /** True when no scripted death was taken — nothing about dying was examined. */
  readonly unbound: boolean;
  /** Why it examined nothing. `undefined` exactly when it examined something. */
  readonly reason?: string;
}

/** Count what the die-retry stage examined, and say why when that is nothing. */
export function dieRetryBinding(
  ran: boolean,
  declared: number,
  engagedWaves: ReadonlySet<string>,
  trials: readonly DeathTrial[],
  skippedNoCheckpoint: number,
  skippedUnarmed: number,
): DieRetryBinding {
  const deathsScripted = trials.length;
  const base = {
    declared,
    engaged: engagedWaves.size,
    deathsScripted,
    trialsCompleted: trials.filter((t) => t.completed).length,
    skippedNoCheckpoint,
    skippedUnarmed,
    unbound: deathsScripted === 0,
  };
  if (deathsScripted > 0) return base;
  let reason: string;
  if (!ran) {
    reason =
      "the stage did not run (no combat plan in this build, or DELVEWRIGHT_DIE_RETRY=0) — " +
      "nothing about dying is proven or disproven by this run";
  } else if (declared === 0) {
    reason =
      "this build declares NO mandatory encounter, so the stage had nothing to script a " +
      "death at. A campaign whose fights are not named by a `kill` objective cannot have " +
      "its retry loop proven by this tier at all";
  } else if (skippedNoCheckpoint >= declared) {
    reason =
      `all ${declared} mandatory encounter(s) were excluded because the campaign fires NO ` +
      `checkpoint before them: every death there is a full restart, so the stage took no ` +
      `death and proved nothing. Where the rest points go is a CONTENT judgement ` +
      `(DW0379/DW0315/DW0316), but the consequence for this stage is that it examined zero`;
  } else if (skippedUnarmed > 0) {
    reason =
      `${skippedUnarmed} encounter(s) were excluded because the governing checkpoint was ` +
      `never armed — the run's own gap, not the delve's — and no other encounter yielded a ` +
      `scripted death`;
  } else {
    reason =
      `the stage ran over ${declared} declared encounter(s) and took NO scripted death; ` +
      `the run ended before it reached one`;
  }
  return { ...base, reason };
}
