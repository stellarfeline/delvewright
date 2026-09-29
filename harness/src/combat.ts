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
import type { Vec3Tuple } from "./critical-path.ts";
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
  /**
   * What the campaign DECLARES stands here, phrased as questions the live bodies
   * can be asked, plus the staged removal that follows the reading. Required —
   * see {@link MusterPlan}; a build too old to state it cannot have its wave
   * verified or cleared, and failing loudly beats a silent skip.
   */
  readonly muster: MusterPlan;
  /** The compiler-emitted tag-census probe for this wave. The names
   * come from the plan and are never re-derived here: `safe_local` is a compiler
   * naming rule, and reimplementing it in the harness is the downstream folklore
   * CLAUDE.md forbids. Required — a build too old to state them cannot be
   * measured by tag, and failing loudly beats silently measuring by silhouette. */
  readonly census: CensusProbe;
}

/**
 * One entity kind the wave declares, and the identity questions the probe puts to
 * a body of it.
 *
 * `facts` is ordered and bit `i` of a body's mask is `facts[i]`. The order is the
 * compiler's (`crates/delvec/src/compiler/muster.rs`) and the harness never
 * re-derives it — the same rule the census function names follow.
 */
export interface MusterType {
  readonly entity: string;
  readonly facts: readonly string[];
  /** Facts the wave declares that did not fit in the mask, so a short mask
   * cannot read as a passing check. */
  readonly droppedFacts: readonly string[];
  readonly readsAttackDamage: boolean;
  readonly readsFollowRange: boolean;
}

/** One declared stack, as the multiset comparison expects to find it. */
export interface MusterProfile {
  readonly typeIndex: number;
  readonly count: number;
  readonly mask: number;
  readonly label: string;
  readonly maxHealth?: number;
  readonly attackDamage?: number;
  readonly movementSpeed?: number;
  readonly followRange?: number;
  readonly armorAtLeast: number;
  readonly armorToughnessAtLeast: number;
}

/**
 * The wave's declaration, plus the two staging functions that follow the reading.
 *
 * `probe` reads the live bodies. `strike` fells one of them, credited to the
 * party — staging, never a fight, and `player_attack by @p` rather than `kill`
 * because the wiring under test (`on_kill`, the countdown, a declared drop) pays
 * on a player's kill and a removal crediting nobody would skip it silently.
 * `chip` wounds the whole wave without felling anything, which is the only thing
 * the die-retry stage's "mid-fight" death needs the wave to be.
 */
export interface MusterPlan {
  readonly probe: string;
  readonly strike: string;
  readonly chip: string;
  /** Fixed-point scale every reading crosses the chat channel at. */
  readonly scale: number;
  /** What a holder reads where the probe deliberately did not ask. */
  readonly unread: number;
  /** Bodies the wave seats in total. */
  readonly bodies: number;
  /** Declared facts this probe puts a question to — the binding count. */
  readonly checked: number;
  readonly types: readonly MusterType[];
  readonly profiles: readonly MusterProfile[];
}

/**
 * The muster block. Required, with no default: a wave the harness cannot read is
 * a wave it cannot verify OR clear, and the only fallback is for the harness to
 * write its own copy of the declaration — which is the thing this block exists to
 * delete.
 */
function parseMusterPlan(v: unknown, pointer: string): MusterPlan {
  if (!isRecord(v)) throw new CombatPlanParseError(pointer, "expected an object");
  const str = (key: string): string => {
    const x = v[key];
    if (typeof x !== "string" || x.length === 0) {
      throw new CombatPlanParseError(`${pointer}/${key}`, "expected a non-empty function id");
    }
    return x;
  };
  const int = (key: string): number => {
    const x = v[key];
    if (!Number.isInteger(x)) {
      throw new CombatPlanParseError(`${pointer}/${key}`, "expected an integer");
    }
    return x as number;
  };
  const scale = int("scale");
  if (scale <= 0) throw new CombatPlanParseError(`${pointer}/scale`, "expected a positive integer");
  const rawTypes = v["types"];
  if (!Array.isArray(rawTypes) || rawTypes.length === 0) {
    throw new CombatPlanParseError(`${pointer}/types`, "a wave with no entity kinds is not a wave");
  }
  const types = rawTypes.map((t, i): MusterType => {
    const tp = `${pointer}/types/${i}`;
    if (!isRecord(t)) throw new CombatPlanParseError(tp, "expected an object");
    const entity = t["entity"];
    if (typeof entity !== "string" || entity.length === 0) {
      throw new CombatPlanParseError(`${tp}/entity`, "expected a non-empty entity id");
    }
    const facts = t["facts"];
    if (!Array.isArray(facts) || facts.some((f) => typeof f !== "string")) {
      throw new CombatPlanParseError(`${tp}/facts`, "expected an array of strings");
    }
    const dropped = t["dropped_facts"];
    if (!Array.isArray(dropped) || dropped.some((f) => typeof f !== "string")) {
      throw new CombatPlanParseError(`${tp}/dropped_facts`, "expected an array of strings");
    }
    return {
      entity,
      facts: facts as string[],
      droppedFacts: dropped as string[],
      readsAttackDamage: t["reads_attack_damage"] === true,
      readsFollowRange: t["reads_follow_range"] === true,
    };
  });
  const rawProfiles = v["profiles"];
  if (!Array.isArray(rawProfiles) || rawProfiles.length === 0) {
    throw new CombatPlanParseError(`${pointer}/profiles`, "a wave with no stacks is not a wave");
  }
  const num = (o: Record<string, unknown>, key: string, pp: string): number | undefined => {
    const x = o[key];
    if (x === null || x === undefined) return undefined;
    if (typeof x !== "number" || !Number.isFinite(x)) {
      throw new CombatPlanParseError(`${pp}/${key}`, "expected a finite number or null");
    }
    return x;
  };
  const profiles = rawProfiles.map((pr, i): MusterProfile => {
    const pp = `${pointer}/profiles/${i}`;
    if (!isRecord(pr)) throw new CombatPlanParseError(pp, "expected an object");
    const typeIndex = pr["type"];
    if (!Number.isInteger(typeIndex) || (typeIndex as number) < 0 || (typeIndex as number) >= types.length) {
      throw new CombatPlanParseError(`${pp}/type`, "expected an index into `types`");
    }
    const count = pr["count"];
    if (!Number.isInteger(count) || (count as number) <= 0) {
      throw new CombatPlanParseError(`${pp}/count`, "expected a positive integer");
    }
    const mask = pr["mask"];
    if (!Number.isInteger(mask) || (mask as number) < 0) {
      throw new CombatPlanParseError(`${pp}/mask`, "expected a non-negative integer");
    }
    const label = pr["label"];
    if (typeof label !== "string" || label.length === 0) {
      throw new CombatPlanParseError(`${pp}/label`, "expected a non-empty string");
    }
    return {
      typeIndex: typeIndex as number,
      count: count as number,
      mask: mask as number,
      label,
      maxHealth: num(pr, "max_health", pp),
      attackDamage: num(pr, "attack_damage", pp),
      movementSpeed: num(pr, "movement_speed", pp),
      followRange: num(pr, "follow_range", pp),
      armorAtLeast: num(pr, "armor_at_least", pp) ?? 0,
      armorToughnessAtLeast: num(pr, "armor_toughness_at_least", pp) ?? 0,
    };
  });
  return {
    probe: str("probe"),
    strike: str("strike"),
    chip: str("chip"),
    scale,
    unread: int("unread"),
    bodies: int("bodies"),
    checked: int("checked"),
    types,
    profiles,
  };
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
 * A run-back (spec-0016 §1): a `respawns_on_rest` wave the path already cleared,
 * re-seated by a rest the path performs, standing within its aggro radius of the
 * leg to `before`. The compiler exports it as an encounter because it is one — a
 * player walking that leg meets the fight again — and spec-0023 §3 runs every
 * encounter under a labelled assist and everything between fights clean.
 */
export interface RunBack {
  readonly wave: string;
  /** The `kill` objective that cleared it the first time. */
  readonly objective: string;
  /** The bonfire whose rest re-seats it. */
  readonly bonfire: number;
  /** The token (`obj/…` or `trigger/…`) of the step whose leg re-crosses it. */
  readonly before: string;
  readonly tier: EncounterTier;
  readonly pos: Vec3Tuple;
  readonly count: number;
  /** The aggro radius the crossing was measured at, and where it happened. */
  readonly radius: number;
  readonly crossing: Vec3Tuple;
  readonly distance: number;
  /** Which exported paths carry it (`critical-path` or a branch slug). */
  readonly paths: readonly string[];
}

/**
 * The run-backs due before the step named `before`: those whose bonfire this
 * run rested at AFTER it last cleared the wave. A run-back is only a fight when
 * both halves happened on this walk — the wave was beaten, and a rest put it back
 * — and once fought it is down again until the next rest, which is why the
 * caller records a fought run-back as a fresh clearance.
 *
 * `clearedAt` / `restedAt` map a wave / bonfire to the step index it last
 * happened at.
 */
export function dueRunBacks(
  runBacks: readonly RunBack[],
  before: string,
  clearedAt: ReadonlyMap<string, number>,
  restedAt: ReadonlyMap<number, number>,
): RunBack[] {
  return runBacks.filter((r) => {
    if (r.before !== before) return false;
    const cleared = clearedAt.get(r.wave);
    const rested = restedAt.get(r.bonfire);
    return cleared !== undefined && rested !== undefined && rested > cleared;
  });
}

/**
 * A death-respawn at a bonfire fires that fire's rest hooks (spec-0016 §1), so
 * it re-seats every `respawns_on_rest` wave exactly as a rest does. The party
 * returns to the fire it last rested at — the latest entry of `restedAt` — so
 * that bonfire is recorded as rested at `step`. No rest yet means the respawn is
 * at world spawn and re-seats nothing a run-back is keyed to.
 */
export function respawnReseats(restedAt: Map<number, number>, step: number): void {
  let last: number | undefined;
  let lastAt = -1;
  for (const [b, at] of restedAt) {
    if (at > lastAt) {
      last = b;
      lastAt = at;
    }
  }
  if (last !== undefined) restedAt.set(last, step);
}

function parseRunBacks(v: unknown, pointer: string): readonly RunBack[] {
  // Required, with no default: a plan that cannot say whether a rest puts a fight
  // back beside the path would have the ladder walk that leg as empty — which is
  // the defect this field exists to end.
  if (!Array.isArray(v)) throw new CombatPlanParseError(pointer, "expected an array");
  return v.map((r, i): RunBack => {
    const p = `${pointer}/${i}`;
    if (!isRecord(r)) throw new CombatPlanParseError(p, "expected an object");
    for (const key of ["wave", "objective", "before"] as const) {
      if (typeof r[key] !== "string" || (r[key] as string).length === 0) {
        throw new CombatPlanParseError(`${p}/${key}`, "expected a non-empty string");
      }
    }
    for (const key of ["bonfire", "count"] as const) {
      if (!Number.isInteger(r[key])) throw new CombatPlanParseError(`${p}/${key}`, "expected an integer");
    }
    for (const key of ["radius", "distance"] as const) {
      if (typeof r[key] !== "number" || !Number.isFinite(r[key])) {
        throw new CombatPlanParseError(`${p}/${key}`, "expected a finite number");
      }
    }
    const tier = r["tier"];
    if (typeof tier !== "string" || !ENCOUNTER_TIERS.includes(tier as EncounterTier)) {
      throw new CombatPlanParseError(`${p}/tier`, `expected one of ${ENCOUNTER_TIERS.join("|")}`);
    }
    const paths = r["paths"];
    if (!Array.isArray(paths) || paths.length === 0 || paths.some((x) => typeof x !== "string")) {
      throw new CombatPlanParseError(`${p}/paths`, "expected a non-empty array of path labels");
    }
    return {
      wave: r["wave"] as string,
      objective: r["objective"] as string,
      bonfire: r["bonfire"] as number,
      before: r["before"] as string,
      tier: tier as EncounterTier,
      pos: requirePos(r["pos"], `${p}/pos`),
      count: r["count"] as number,
      radius: r["radius"] as number,
      crossing: requirePos(r["crossing"], `${p}/crossing`),
      distance: r["distance"] as number,
      paths: paths as string[],
    };
  });
}

/** The parsed combat plan. */
export interface CombatPlan {
  readonly version: string;
  readonly campaignId: string;
  /** The declared world difficulty the run is verified AT (spec-0023 §3). */
  readonly difficulty: string;
  readonly encounters: readonly Encounter[];
  /** Re-seated fights the path walks past again after a rest. */
  readonly runBacks: readonly RunBack[];
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
  if (typeof version !== "string" || version.length === 0) {
    throw new CombatPlanParseError("/version", "must name the dsl_version the campaign was built at");
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
      muster: parseMusterPlan(e["muster"], `${p}/muster`),
    };
  });
  return {
    version,
    campaignId,
    difficulty,
    encounters,
    runBacks: parseRunBacks(raw["run_backs"], "/run_backs"),
  };
}

/**
 * The census probe block. Required: measuring a wave by tag is how
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
 * How far a `kill` step got with one encounter.
 *
 * The run artifact states this per encounter because silence is otherwise
 * unreadable: an encounter the run never reached and an encounter whose muster
 * found nothing produce the same empty findings list, and only this tells them
 * apart.
 *
 * `mustered` is the measurement — the live bodies were read against the
 * declaration. `cleared` is what follows it, and it is STAGING: the wave is
 * removed by an attributed command so the run can go on to the wiring the kill
 * drives. Neither phase is a claim about whether the fight can be won; nothing
 * the ladder does at a combat step is.
 */
export const ENCOUNTER_PHASES = [
  "not-reached",
  "die-retry",
  "mustered",
  "cleared",
] as const;
export type EncounterPhase = (typeof ENCOUNTER_PHASES)[number];

/**
 * Who felled the bodies of a fight the floor gate is judging.
 *
 * The gate's whole claim is *the unassisted bot beat this cold*, and a delve is
 * full of things that kill a mob without any bot in it: a fall, a lethal volume,
 * a trap, another mob. The engine's own gallery does exactly that — `wave/muster`
 * seats three bodies within a stride of `lethal/east-pit` and a fall — and the
 * gate reported an `elite` beaten on the first attempt over a cohort in which the
 * bot had confirmed ONE kill while two withered and fell. The advisory was not
 * merely noisy: it advised the author to make an encounter harder on the strength
 * of a fight the world had mostly fought.
 *
 * This is the fight's attribution, and it belongs to the fight rather than to
 * either gate: a wave and an unleashed actor are both bodies somebody felled, and
 * both gates are the same claim about the same thing. A run that cannot say who
 * felled them says so — {@link unattributed} — rather than leaving the reader to
 * read silence as a clean win.
 */
export type FightAttribution =
  | {
      readonly kind: "measured";
      /** Bodies the fight seated. */
      readonly bodies: number;
      /** Of those, still standing when the attempt ended. */
      readonly standing: number;
      /** Deaths vanilla credited to a player. */
      readonly credited: number;
      /** Deaths nothing credited to a player — the world's kills. */
      readonly uncredited: number;
    }
  | { readonly kind: "unattributed"; readonly reason: string };

/**
 * The attribution of a wave attempt, from the compiler's own census.
 *
 * `bodies` is the seating the compiler declared and `spawn_<wave>` wrote;
 * `standing` and `credited` are the server's answer. `uncredited` is the
 * remainder, floored at zero — the three numbers are read at slightly different
 * instants (a census is a round trip), so a body that dies between the two can
 * make the arithmetic transiently negative, and a negative body count is not a
 * fact about anything.
 */
export function waveAttribution(
  bodies: number,
  standing: number,
  credited: number,
): FightAttribution {
  return {
    kind: "measured",
    bodies,
    standing,
    credited,
    uncredited: Math.max(0, bodies - standing - credited),
  };
}

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
 * The gamemode a player must be in for a scripted death to be takeable at all.
 *
 * A spectator is invulnerable, so `/damage` does nothing to one — and the engine
 * says the same thing from the other side: every `damage-players` effect the
 * compiler emits carries `tag=!dw_cutscene`, the tag a cutscene adds to `@a` in the
 * same breath as `gamemode spectator @a`.
 */
export const CONTROLLED_GAMEMODE = "adventure";

/**
 * **Why a scripted death did not land**, said from what was OBSERVED.
 *
 * The stage used to guess — *"was refused (is the bot opped?)"* — and that guess is
 * wrong in the one case that actually happens. An encounter's own objective
 * completion can start a cutscene (`cs_<beat>`), and a cutscene's first two lines
 * are `tag @a add dw_cutscene` and `gamemode spectator @a`. A trade that finishes
 * the wave therefore hands the next scripted death a spectator, `/damage` does
 * nothing to it, and the stage blamed an op seed. The gamemode is right there to be
 * read, and the server's own answer arrives on the chat stream the bot is opped to
 * receive, so the refusal says both instead of naming a cause it never checked.
 *
 * Pure, because a verdict is a reading and readings are testable without a server —
 * and because the wording is the whole deliverable of this repair.
 */
export function scriptedDeathRefusal(
  command: string,
  timeoutMs: number,
  gameMode: string | undefined,
  answers: readonly string[],
): string {
  const said =
    answers.length === 0
      ? "the server answered nothing on the chat stream"
      : `the server answered ${answers.map((l) => `"${l}"`).join(", ")}`;
  const mode = gameMode ?? "a gamemode the client could not read";
  const because =
    gameMode !== undefined && gameMode !== CONTROLLED_GAMEMODE
      ? `the bot was in \`${mode}\`, not \`${CONTROLLED_GAMEMODE}\` — a spectator is ` +
        `invulnerable and \`/damage\` does nothing to one, so this death was never ` +
        `takeable. A cutscene is what puts a player there (\`gamemode spectator @a\`), ` +
        `and the stage waits one out before scripting a death; a window longer than ` +
        `the one the build declares in \`cutscene_seconds\` is the finding`
      : `the bot was in \`${mode}\`, which is the mode a death is takeable in, so the ` +
        `command itself did not take effect — the op seed, the command tree, or the ` +
        `delve's own damage rules are where to look`;
  return (
    `the scripted death never landed within ${timeoutMs}ms — \`${command}\` did ` +
    `nothing and ${said}. ${because}`
  );
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
 * went red then green on consecutive runs.
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
 *  * the fidelity failure: a retry must never let the
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
   * mob or an ambush actor standing nearby can never be mistaken for one. */
  readonly carriedOver: number;
  /** How many had readable health, and how many of those were below full. The
   * census reads `Health` and `max_health` off each mob with vanilla's own
   * commands, so on this path every counted mob is readable — unlike the client
   * view, which is never sent an unmodified max health at all. */
  readonly healthReadable: number;
  readonly damaged: number;
  /**
   * Deaths of this wave the party was credited with SINCE ITS SEATING — the
   * census's own last total, zeroed by `spawn_<wave>` and therefore zeroed by
   * every re-seat.
   *
   * It is here because the walk back is not a walk. The return leg is assisted
   * and the bot DEFENDS itself on it: a re-seated mob that hits the bot twice
   * within four blocks is put down before the leg resumes
   * (`[defend] die-retry return …: zombie#63 is down`), and the settle that
   * follows then counts one body fewer. Without this field the fidelity verdict
   * cannot tell "the re-seat came back short" from "I killed one of them on my
   * way to look", and it reported the second as the first.
   */
  readonly credited: number;
  /** Distance spread from the encounter anchor, for the wandered-mob case. */
  readonly nearest: number | undefined;
  readonly farthest: number | undefined;
  /** How long the probe waited before it settled on this answer. */
  readonly settleMs: number;
}

/**
 * Where, and how hurt, each body a census found standing was — the one reading
 * that tells a fight the bot lost at 85/90 from one it lost at 5/90, and a wave
 * that is merely unfinished from one standing somewhere the fight cannot reach.
 * Empty for an empty census.
 */
export function describeStanding(mobs: readonly CensusMob[]): string {
  return mobs
    .map(
      (m) =>
        `[${m.pos.map((v) => v.toFixed(1)).join(", ")}] at ${m.health.toFixed(1)}/` +
        `${m.maxHealth.toFixed(0)} health`,
    )
    .join("; ");
}

/**
 * Summarize one settled census.
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
    credited: census.summary.credited,
    nearest: distances.length > 0 ? Math.min(...distances) : undefined,
    farthest: distances.length > 0 ? Math.max(...distances) : undefined,
    settleMs,
  };
}

/**
 * The re-seat fidelity verdict for one trial, or `undefined` when the wave came
 * back whole. Only ever consulted for a `respawns_on_rest` wave that re-engaged,
 * and read off {@link DeathTrial.reseat} — the census taken the moment the
 * re-seat landed, before the walk back.
 *
 * Read THEN because that is the event it guards. The walk back takes 25–50 s,
 * and in that time a re-seated wave is not left alone: it aggroes on the bot as
 * it comes, and vesperhold's skeletons shoot one another ("Cliff Watchman was
 * shot by Cliff Watchman"), its spear zombies stab one another ("Stable Groom was
 * killed while fighting Stable Groom"), and its drowned swim into a lethal well
 * ("Drowned Chorister drowned"). Every one of those used to be read as the
 * re-seat's fault. What happens between the re-seat and the return is judged
 * separately, by {@link returnAttritionFinding}.
 *
 * A half-fought wave is REMOVED and regenerated identically; the player comes
 * back full, so the wave does too. Grinding a boss down one swing per life is
 * not a difficulty curve, it is a bug.
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
  // `credited` is zeroed by the re-seat itself (`spawn_<wave>`) and the party is
  // standing at the checkpoint when this is read, so it is 0 in every honest
  // reading; it is still added, so the count can only ever be corrected by a kill
  // vanilla actually credited — which a genuinely short re-seat cannot produce.
  if (obs.present + obs.credited < obs.declared) {
    return (
      `${where}: the re-seated wave came back SHORT — ${obs.present} mob(s) standing` +
      `${obs.credited > 0 ? ` and ${obs.credited} felled by the party since the re-seat` : ""}, ` +
      `${obs.declared} declared. A retry must face the fight the first life faced.`
    );
  }
  // Read the moment the re-seat lands, nothing has touched the cohort yet, so a
  // wound here is one the re-seat brought back — no confound to exempt.
  if (obs.damaged > 0) {
    return (
      `${where}: ${obs.damaged} of the ${obs.healthReadable} wave mob(s) whose health could ` +
      `be read came back BELOW full. The player respawns whole; so must the wave.`
    );
  }
  return undefined;
}

/**
 * What happened to a whole re-seated wave between the re-seat and the party's
 * return, or `undefined` when every body the re-seat brought back is still
 * accounted for.
 *
 * `reseat` is the census at the re-seat; `back` the settled census at the
 * encounter. A body missing from `back` that the party was not credited with died
 * to something that is not the party — a lethal volume, a fall, its own wave's
 * fire — before any player could reach it. That is red: the fight a player walks
 * back into is thinner than the one the campaign declares, and it is thinner on
 * EVERY life, so it is a property of the encounter, not of the retry. Said as what
 * it is, never as a short re-seat — the re-seat has already been read whole.
 */
export function returnAttritionFinding(
  wave: string,
  attempt: number,
  phase: DeathPhase,
  reseat: ReengageObservation,
  back: ReengageObservation,
): string | undefined {
  const lost = reseat.present - (back.present + back.credited);
  if (lost <= 0) return undefined;
  return (
    `${wave} death ${attempt} (${phase}): the re-seat brought back ${reseat.present} of ` +
    `${reseat.declared}, but by the time the party walked back ${back.present} stood and ` +
    `${back.credited} had fallen to the party — ${lost} died to something that is not the ` +
    `party (a lethal volume, a fall, their own wave's fire) before anyone reached them. ` +
    `The encounter kills its own wave: a player meets a thinner fight than the one declared, ` +
    `on every life. Look at where the wave's members can walk from their seat.`
  );
}

/**
 * The health a re-seated wave carried when the party got back to it, stated —
 * never judged — or `undefined` when nothing standing was hurt.
 *
 * The re-seat itself is judged whole or not by {@link reseatFidelityFinding} at
 * the moment it lands. Wounds found at the RETURN were dealt afterwards, by the
 * return leg's own self-defence, by the wave's members hitting one another, or by
 * the world; the census counts credited deaths, never blows, so it cannot say
 * which. That is the ordinary state of a fight a party walks into, and what the
 * first life walked into too, so it is telemetry: stated so a reader can see it,
 * and so a stage that judged nothing at the return is never read as one that
 * judged it all and found it whole.
 */
export function returnHealthNote(
  wave: string,
  attempt: number,
  phase: DeathPhase,
  back: ReengageObservation,
): string | undefined {
  if (back.damaged === 0) return undefined;
  return (
    `${wave} death ${attempt} (${phase}): ${back.damaged} of ${back.healthReadable} ` +
    `readable wave mob(s) stood below full health when the party walked back ` +
    `(${back.credited} felled by the party since the re-seat). The re-seat was read ` +
    `whole when it landed, so these wounds were dealt after it — by the return leg's ` +
    `self-defence, the wave's own members, or the world. Telemetry, not a verdict.`
  );
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
   * longer replays `select-class` (`class_apply_*` teleports).
   */
  readonly respawnPos: Vec3Tuple | undefined;
  /** Did it respawn at the governing checkpoint? Derived from {@link respawnPos}. */
  readonly atCheckpoint: boolean;
  /** Did the kit survive the death? The delve seals `gamerule keep_inventory true`,
   * so a bot that comes back empty-handed found that seal absent — and a player who
   * must re-gear after every death has no cheap retry. */
  readonly kitKept: boolean;
  /** Did it walk back to the encounter, from where it respawned? */
  readonly returned: boolean;
  /**
   * Why the walk back did not arrive, when it did not. `killed` separates a leg
   * that ended in a death — which proves nothing about the route either way —
   * from a leg that could not be walked, which is the retry loop broken.
   */
  readonly returnFailure: { readonly killed: boolean; readonly detail: string } | undefined;
  /**
   * Raw observation behind {@link outcome}: was a wave mob standing there again?
   *
   * **Only observed when {@link returned}**. The probe reads the
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
  /** The census read the moment the re-seat landed, before the walk back — what
   * the fidelity verdict is read from. Only taken for a re-seating wave. */
  readonly reseat: ReengageObservation | undefined;
  /** The settled set at the encounter the outcome was read from. */
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
 * `passed: true` beside a log line naming the death it had just taken.
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
    returnFailure: undefined,
    reEngaged: false,
    objectiveComplete: false,
    reseats: enc.respawnsOnRest,
    reseat: undefined,
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
  if (!t.returned && t.returnFailure?.killed) {
    return (
      `${t.wave} death ${t.attempt} (${t.phase}): the bot was KILLED on the way back from the ` +
      `respawn at ${t.respawnPos ? t.respawnPos.join(",") : "an unknown position"} ` +
      `(${t.returnFailure.detail}). Nothing is proved about the route or the re-engagement — ` +
      `a death on the leg is not a verdict on its geometry — so the trial is unproven, ` +
      `and an unproven trial is never a passed one.`
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
    if (t.reseat === undefined) {
      return (
        `${t.wave} death ${t.attempt} (${t.phase}): the wave re-seats on rest, but the ` +
        `census was never read at the re-seat, so whether it came back whole is unproven.`
      );
    }
    const fidelity = reseatFidelityFinding(t.wave, t.attempt, t.phase, t.reseat);
    if (fidelity !== undefined) return fidelity;
    const attrition = returnAttritionFinding(t.wave, t.attempt, t.phase, t.reseat, t.reengage);
    if (attrition !== undefined) return attrition;
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
 *     it. This is not hypothetical — `fire_step < i` means a checkpoint armed
 *     by the encounter's own kill step is correctly NOT its governing one, and
 *     souls-bonfire's encounter truthfully reports none. Reading it as "armed"
 *     instead is the answer that flatters the campaign.
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
  // BOTH sides of this comparison are EXPORTED path indices: the
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
 * The health each re-seated wave carried when the party got back to it, in order
 * — advisories, never failures ({@link returnHealthNote}).
 */
export function dieRetryFidelityGaps(trials: readonly DeathTrial[]): string[] {
  return trials
    .filter((t) => t.reseats && t.outcome === "re-engaged" && t.reengage !== undefined)
    .map((t) => returnHealthNote(t.wave, t.attempt, t.phase, t.reengage!))
    .filter((v): v is string => v !== undefined);
}

/**
 * Whether the stage actually PROVED what it claims, encounter by encounter.
 *
 * The per-trial verdicts above can only judge trials that exist. The failure
 * mode they cannot see is silence: a stage that engaged an encounter and
 * recorded nothing produced an empty `die_retry` array, and an empty array of
 * findings, and therefore read `passed: true` — which is exactly how a run that
 * aborted at its first scripted death reported a green die-retry stage.
 * Coverage closes that: the stage owes `expected` COMPLETED trials for
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
 * and an encounter that has none is excluded with an advisory — correctly,
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
