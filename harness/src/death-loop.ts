// The death loop, as the bot tier proves it (task #68; spec-0031 lethal volumes,
// spec-0032 `on_death` + the recovery stake).
//
// ## Why this tier and no other
//
// A PackTest fake player is permanently undamageable — measured independently on
// 2026-08-03 and again on 2026-08-09 — so that tier CANNOT witness a player death.
// Everything the drowned-bell shape rests on (dying, forfeiting, respawning,
// walking back, recovering) therefore has exactly one place it can be proven at
// runtime: a mineflayer bot driving a real client that can really die.
//
// ## The rule this module is written to
//
// **Every assertion is derived from what the campaign PROMISED, never from what
// the emitter happened to write.** The promise arrives as
// `validation/death-plan.json`: the declared boxes, the declared wording, the
// declared forfeit rule, the declared collect policy, and the one thing the
// compiler computed and therefore owes a runtime check on — the recovery stake's
// placement table. The forfeit arithmetic below is spec-0032's rule re-derived
// from its text, not a reading of `stake_forfeit_lines`; an assertion written by
// reading the emitter cannot fail when the emitter is wrong.
//
// Everything here is pure (no mineflayer): types, parsing, arithmetic and
// verdicts. The executor supplies the bot, and it supplies only observations.

import { readFile } from "node:fs/promises";
import path from "node:path";
import { SUPPORTED_DSL_VERSIONS, type Vec3Tuple } from "./critical-path.ts";

/** Where the death plan sits relative to `critical-path.json`. */
const DEATH_PLAN_SUBPATH = ["validation", "death-plan.json"] as const;

/**
 * The `format_version` this harness understands. A build that declares a newer
 * one is REFUSED rather than half-read: a bot that silently ignores a field it
 * does not know reports a green over assertions it never made.
 */
export const SUPPORTED_DEATH_PLAN_FORMAT = 1;

/** An inclusive world-space box. */
export interface Box {
  readonly lo: Vec3Tuple;
  readonly hi: Vec3Tuple;
}

/** A declared lethal volume: a box that kills, and what it says while doing it. */
export interface LethalVolume {
  readonly id: string;
  readonly region: Box;
  /** The canonical English the volume promises the player who dies in it. */
  readonly message: string;
  readonly messageKey: string | undefined;
  readonly damageType: string;
}

/** The declared currency a stake wagers, and the ledger spec-0032 keeps it in. */
export interface Currency {
  readonly state: string;
  /** spec-0032: "Currency is a ledger, not an item" — the objective IS the declaration. */
  readonly objective: string;
  readonly initial: number | undefined;
  readonly scope: string | undefined;
  readonly name: string | undefined;
  readonly nameKey: string | undefined;
}

/** How much of the currency a death takes (spec-0032 `Forfeit`). */
export type ForfeitRule =
  | { readonly kind: "all" }
  | { readonly kind: "none" }
  | { readonly kind: "proportion"; readonly percent: number }
  | { readonly kind: "fixed"; readonly amount: number };

/** A declared recovery stake. */
export interface StakeRule {
  readonly id: string;
  readonly currency: Currency;
  readonly forfeit: ForfeitRule;
  readonly maxLive: number;
  readonly onFull: string;
  readonly collectBy: string;
  readonly collectedMessage: string;
  readonly markerItem: string;
}

/** One respawn seat the placement table is keyed on. */
export interface Seat {
  /** `-1` for the campaign's entry spawn, otherwise the checkpoint index. */
  readonly cp: number;
  readonly label: string;
  readonly cell: Vec3Tuple;
}

/** One death region of the placement table. */
export interface DeathRegion {
  readonly label: string;
  readonly lethal: boolean;
  /** The lethal volume's id when this region IS one — the harness matches by
   * NAME rather than by reproducing the compiler's region ordering. */
  readonly volume: string | undefined;
  readonly region: Box;
}

/** One row: *a death in this region, with this seat in force, leaves its stake here.* */
export interface PlacementRow {
  readonly seat: number;
  readonly region: number;
  readonly anchor: Vec3Tuple;
}

/** What the plan lets this tier examine at all (playtest-methodology rule 1). */
export interface DeathPlanBinding {
  readonly volumes: number;
  readonly onDeathEffects: number;
  readonly stakes: number;
  readonly seats: number;
  readonly rows: number;
  readonly unbound: boolean;
  readonly reason: string | undefined;
}

/** The parsed death plan. */
export interface DeathPlan {
  readonly version: string;
  readonly formatVersion: number;
  readonly campaignId: string;
  readonly volumes: readonly LethalVolume[];
  readonly onDeathEffects: number;
  readonly dropsStake: readonly string[];
  readonly stakes: readonly StakeRule[];
  readonly seats: readonly Seat[];
  readonly regions: readonly DeathRegion[];
  readonly rows: readonly PlacementRow[];
  readonly binding: DeathPlanBinding;
}

export class DeathPlanParseError extends Error {
  override readonly name = "DeathPlanParseError";
  readonly pointer: string;
  constructor(pointer: string, detail: string) {
    super(`death plan invalid at ${pointer || "/"}: ${detail}`);
    this.pointer = pointer;
  }
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function requireCell(v: unknown, pointer: string): Vec3Tuple {
  if (!Array.isArray(v) || v.length !== 3) {
    throw new DeathPlanParseError(pointer, "expected a 3-element position array");
  }
  const out = v.map((n, i) => {
    if (typeof n !== "number" || !Number.isFinite(n)) {
      throw new DeathPlanParseError(`${pointer}/${i}`, "expected a finite number");
    }
    return n;
  });
  return [out[0]!, out[1]!, out[2]!];
}

function requireBox(v: unknown, pointer: string): Box {
  if (!isRecord(v)) throw new DeathPlanParseError(pointer, "expected an object");
  return {
    lo: requireCell(v["lo"], `${pointer}/lo`),
    hi: requireCell(v["hi"], `${pointer}/hi`),
  };
}

function requireString(v: unknown, pointer: string): string {
  if (typeof v !== "string" || v.length === 0) {
    throw new DeathPlanParseError(pointer, "expected a non-empty string");
  }
  return v;
}

function optionalString(v: unknown, pointer: string): string | undefined {
  if (v === undefined || v === null) return undefined;
  if (typeof v !== "string") throw new DeathPlanParseError(pointer, "expected a string or null");
  return v;
}

function optionalInteger(v: unknown, pointer: string): number | undefined {
  if (v === undefined || v === null) return undefined;
  if (!Number.isInteger(v)) throw new DeathPlanParseError(pointer, "expected an integer or null");
  return v as number;
}

function requireInteger(v: unknown, pointer: string): number {
  if (!Number.isInteger(v)) throw new DeathPlanParseError(pointer, "expected an integer");
  return v as number;
}

function parseForfeit(v: unknown, pointer: string): ForfeitRule {
  if (!isRecord(v)) throw new DeathPlanParseError(pointer, "expected an object");
  const kind = requireString(v["kind"], `${pointer}/kind`);
  switch (kind) {
    case "all":
      return { kind: "all" };
    case "none":
      return { kind: "none" };
    case "proportion": {
      const percent = requireInteger(v["percent"], `${pointer}/percent`);
      if (percent < 0 || percent > 100) {
        throw new DeathPlanParseError(`${pointer}/percent`, "expected 0..=100");
      }
      return { kind: "proportion", percent };
    }
    case "fixed":
      return { kind: "fixed", amount: requireInteger(v["amount"], `${pointer}/amount`) };
    default:
      throw new DeathPlanParseError(
        `${pointer}/kind`,
        `unknown forfeit rule ${JSON.stringify(kind)} — this harness cannot compute what ` +
          `this death is supposed to take, so it must not claim to have checked it`,
      );
  }
}

function parseCurrency(v: unknown, pointer: string): Currency {
  if (!isRecord(v)) throw new DeathPlanParseError(pointer, "expected an object");
  return {
    state: requireString(v["state"], `${pointer}/state`),
    objective: requireString(v["objective"], `${pointer}/objective`),
    initial: optionalInteger(v["initial"], `${pointer}/initial`),
    scope: optionalString(v["scope"], `${pointer}/scope`),
    name: optionalString(v["name"], `${pointer}/name`),
    nameKey: optionalString(v["name_key"], `${pointer}/name_key`),
  };
}

function parseBinding(v: unknown, pointer: string): DeathPlanBinding {
  if (!isRecord(v)) throw new DeathPlanParseError(pointer, "expected an object");
  const n = (key: string): number => {
    const raw = v[key];
    if (!Number.isInteger(raw) || (raw as number) < 0) {
      throw new DeathPlanParseError(`${pointer}/${key}`, "expected a non-negative integer");
    }
    return raw as number;
  };
  const unbound = v["unbound"];
  if (typeof unbound !== "boolean") {
    throw new DeathPlanParseError(`${pointer}/unbound`, "expected a boolean");
  }
  const binding: DeathPlanBinding = {
    volumes: n("lethal_volumes"),
    onDeathEffects: n("on_death_effects"),
    stakes: n("stakes"),
    seats: n("respawn_seats"),
    rows: n("placement_rows"),
    unbound,
    reason: optionalString(v["reason"], `${pointer}/reason`),
  };
  // The compiler's own definition, re-asserted here: a contract that says it
  // binds while carrying no volume or no promised consequence is a contract that
  // would report a vacuous green, and the harness refuses to walk it.
  const derived = binding.volumes === 0 || binding.onDeathEffects === 0;
  if (derived !== unbound) {
    throw new DeathPlanParseError(
      pointer,
      "`unbound` must be exactly `lethal_volumes === 0 || on_death_effects === 0`",
    );
  }
  if (unbound && binding.reason === undefined) {
    throw new DeathPlanParseError(`${pointer}/reason`, "an unbound plan must state why");
  }
  return binding;
}

/** Parse a death plan document (pure — the file read is the caller's). */
export function parseDeathPlan(raw: unknown): DeathPlan {
  if (!isRecord(raw)) throw new DeathPlanParseError("", "expected an object");
  const formatVersion = requireInteger(raw["format_version"], "/format_version");
  if (formatVersion !== SUPPORTED_DEATH_PLAN_FORMAT) {
    throw new DeathPlanParseError(
      "/format_version",
      `this build declares death-plan format ${formatVersion}; this harness understands ` +
        `${SUPPORTED_DEATH_PLAN_FORMAT}. Refusing rather than reading the fields it recognises: ` +
        `a bot that skips a field it does not know reports a pass over assertions it never made`,
    );
  }
  const version = raw["version"];
  if (typeof version !== "string" || !SUPPORTED_DSL_VERSIONS.includes(version as never)) {
    throw new DeathPlanParseError("/version", `unsupported dsl_version ${String(version)}`);
  }
  const volumesRaw = raw["lethal_volumes"];
  if (!Array.isArray(volumesRaw)) {
    throw new DeathPlanParseError("/lethal_volumes", "expected an array");
  }
  const volumes = volumesRaw.map((v, i): LethalVolume => {
    const p = `/lethal_volumes/${i}`;
    if (!isRecord(v)) throw new DeathPlanParseError(p, "expected an object");
    return {
      id: requireString(v["id"], `${p}/id`),
      region: requireBox(v["region"], `${p}/region`),
      message: requireString(v["message"], `${p}/message`),
      messageKey: optionalString(v["message_key"], `${p}/message_key`),
      damageType: requireString(v["damage_type"], `${p}/damage_type`),
    };
  });

  const onDeath = raw["on_death"];
  if (!isRecord(onDeath)) throw new DeathPlanParseError("/on_death", "expected an object");
  const dropsRaw = onDeath["drops_stake"];
  if (!Array.isArray(dropsRaw)) {
    throw new DeathPlanParseError("/on_death/drops_stake", "expected an array");
  }

  const stakesRaw = raw["stakes"];
  if (!Array.isArray(stakesRaw)) throw new DeathPlanParseError("/stakes", "expected an array");
  const stakes = stakesRaw.map((s, i): StakeRule => {
    const p = `/stakes/${i}`;
    if (!isRecord(s)) throw new DeathPlanParseError(p, "expected an object");
    return {
      id: requireString(s["id"], `${p}/id`),
      currency: parseCurrency(s["currency"], `${p}/currency`),
      forfeit: parseForfeit(s["forfeit"], `${p}/forfeit`),
      maxLive: requireInteger(s["max_live"], `${p}/max_live`),
      onFull: requireString(s["on_full"], `${p}/on_full`),
      collectBy: requireString(s["collect_by"], `${p}/collect_by`),
      collectedMessage: requireString(s["collected_message"], `${p}/collected_message`),
      markerItem: requireString(s["marker_item"], `${p}/marker_item`),
    };
  });

  const placement = raw["placement"];
  if (!isRecord(placement)) throw new DeathPlanParseError("/placement", "expected an object");
  const seatsRaw = placement["seats"];
  const regionsRaw = placement["regions"];
  const rowsRaw = placement["rows"];
  if (!Array.isArray(seatsRaw)) {
    throw new DeathPlanParseError("/placement/seats", "expected an array");
  }
  if (!Array.isArray(regionsRaw)) {
    throw new DeathPlanParseError("/placement/regions", "expected an array");
  }
  if (!Array.isArray(rowsRaw)) throw new DeathPlanParseError("/placement/rows", "expected an array");
  const seats = seatsRaw.map((s, i): Seat => {
    const p = `/placement/seats/${i}`;
    if (!isRecord(s)) throw new DeathPlanParseError(p, "expected an object");
    return {
      cp: requireInteger(s["cp"], `${p}/cp`),
      label: requireString(s["label"], `${p}/label`),
      cell: requireCell(s["cell"], `${p}/cell`),
    };
  });
  const regions = regionsRaw.map((r, i): DeathRegion => {
    const p = `/placement/regions/${i}`;
    if (!isRecord(r)) throw new DeathPlanParseError(p, "expected an object");
    const lethal = r["lethal"];
    if (typeof lethal !== "boolean") {
      throw new DeathPlanParseError(`${p}/lethal`, "expected a boolean");
    }
    return {
      label: requireString(r["label"], `${p}/label`),
      lethal,
      volume: optionalString(r["volume"], `${p}/volume`),
      region: requireBox(r["region"], `${p}/region`),
    };
  });
  const rows = rowsRaw.map((r, i): PlacementRow => {
    const p = `/placement/rows/${i}`;
    if (!isRecord(r)) throw new DeathPlanParseError(p, "expected an object");
    return {
      seat: requireInteger(r["seat"], `${p}/seat`),
      region: requireInteger(r["region"], `${p}/region`),
      anchor: requireCell(r["anchor"], `${p}/anchor`),
    };
  });

  return {
    version,
    formatVersion,
    campaignId: requireString(raw["campaign_id"], "/campaign_id"),
    volumes,
    onDeathEffects: requireInteger(onDeath["effects"], "/on_death/effects"),
    dropsStake: dropsRaw.map((d, i) => requireString(d, `/on_death/drops_stake/${i}`)),
    stakes,
    seats,
    regions,
    rows,
    binding: parseBinding(raw["binding"], "/binding"),
  };
}

/** Load the death plan that accompanies `criticalPathFile`, if the build ships one. */
export async function loadDeathPlanForCriticalPath(
  criticalPathFile: string,
): Promise<DeathPlan | undefined> {
  const file = path.join(path.dirname(criticalPathFile), ...DEATH_PLAN_SUBPATH);
  let text: string;
  try {
    text = await readFile(file, "utf8");
  } catch {
    // A campaign that declares no lethal volume, no `on_death` and no stake emits
    // no file at all. Absence is a legitimate "this campaign has no death loop",
    // and it is reported as SKIPPED with that reason — never as a pass.
    return undefined;
  }
  return parseDeathPlan(JSON.parse(text) as unknown);
}

/**
 * **spec-0032's forfeit rule, re-derived from its text.**
 *
 * Deliberately not a reading of the emitted `stk_drop_*` commands: this is the
 * number the campaign PROMISED, so it can disagree with the engine — which is the
 * only way an assertion about the engine can ever fail.
 *
 * The spec's two stated properties are both here: a proportion is *rounded toward
 * zero* (integer arithmetic, ADR-0006), and a fixed forfeit is *capped at the
 * balance so a purse can never go negative*. A negative balance forfeits nothing —
 * a death must never HAND the player money.
 */
export function expectedForfeit(rule: ForfeitRule, balance: number): number {
  const purse = Math.max(balance, 0);
  switch (rule.kind) {
    case "none":
      return 0;
    case "all":
      return purse;
    case "proportion":
      return Math.trunc((purse * rule.percent) / 100);
    case "fixed":
      return Math.min(purse, Math.max(rule.amount, 0));
  }
}

/** Whether `cell` lies inside `box` (inclusive). */
export function inBox(cell: Vec3Tuple, box: Box): boolean {
  return [0, 1, 2].every((i) => box.lo[i]! <= cell[i]! && cell[i]! <= box.hi[i]!);
}

/** Every cell of an inclusive box, in a fixed order. */
export function boxCells(box: Box): Vec3Tuple[] {
  const out: Vec3Tuple[] = [];
  for (let x = box.lo[0]; x <= box.hi[0]; x++) {
    for (let y = box.lo[1]; y <= box.hi[1]; y++) {
      for (let z = box.lo[2]; z <= box.hi[2]; z++) {
        out.push([x, y, z]);
      }
    }
  }
  return out;
}

/**
 * The cell of `box` the bot should walk into to die in it: the one nearest `from`,
 * ties broken lexicographically so the choice is stable across runs.
 *
 * Navigation, not game logic — which cell of a declared box a client approaches is
 * exactly the kind of decision the harness is allowed to make.
 */
export function entryCellOf(box: Box, from: Vec3Tuple): Vec3Tuple {
  const key = (c: Vec3Tuple): readonly number[] => [
    (c[0] - from[0]) ** 2 + (c[1] - from[1]) ** 2 + (c[2] - from[2]) ** 2,
    c[0],
    c[1],
    c[2],
  ];
  return boxCells(box).sort((a, b) => {
    const ka = key(a);
    const kb = key(b);
    for (let i = 0; i < ka.length; i++) {
      if (ka[i]! !== kb[i]!) return ka[i]! - kb[i]!;
    }
    return 0;
  })[0]!;
}

/**
 * The seat whose cell the player actually respawned on.
 *
 * This is how the harness learns which row of the placement table applies WITHOUT
 * reading the engine's `#cp` bookkeeping: the promise is *"respawn puts the player
 * where the checkpoint in force says"*, so the observed respawn position is both
 * the assertion and the key. `undefined` means the player came back somewhere the
 * campaign never declared — itself a finding.
 *
 * `tolerance` absorbs vanilla's own respawn offset (a spawnpoint lands the player
 * at `cell + (0.5, 0.1, 0.5)`), never a whole block of drift.
 */
export function seatAtRespawn(
  seats: readonly Seat[],
  pos: Vec3Tuple,
  tolerance = 1.5,
): number | undefined {
  let best: number | undefined;
  let bestD = Number.POSITIVE_INFINITY;
  for (const [i, s] of seats.entries()) {
    const d = Math.hypot(pos[0] - (s.cell[0] + 0.5), pos[1] - s.cell[1], pos[2] - (s.cell[2] + 0.5));
    if (d <= tolerance && d < bestD) {
      best = i;
      bestD = d;
    }
  }
  return best;
}

/** The placement table's answer for a death in `volumeId` with `seat` in force. */
export function tableAnchor(
  plan: DeathPlan,
  seat: number,
  volumeId: string,
): Vec3Tuple | undefined {
  const region = plan.regions.findIndex((r) => r.lethal && r.volume === volumeId);
  if (region < 0) return undefined;
  return plan.rows.find((r) => r.seat === seat && r.region === region)?.anchor;
}

/** One walk into one lethal volume, and everything that was observed of it. */
export interface LethalTrial {
  readonly volume: string;
  /** The cell the bot walked into. */
  readonly entryCell: Vec3Tuple;
  /** The stake this trial expects the death to leave, if any. */
  readonly stake: string | undefined;
  /** The currency objective the ledger was read from. */
  readonly objective: string | undefined;
  died: boolean;
  deathPos: Vec3Tuple | undefined;
  /** Whether the volume's own promised line reached this player. */
  wordingSeen: boolean;
  balanceBefore: number | undefined;
  balanceAfterDeath: number | undefined;
  expectedForfeit: number | undefined;
  respawnPos: Vec3Tuple | undefined;
  respawnSeat: string | undefined;
  expectedAnchor: Vec3Tuple | undefined;
  markerPos: Vec3Tuple | undefined;
  walkedBack: boolean;
  collectClicks: number;
  balanceAfterCollect: number | undefined;
  markerRetired: boolean;
  /** A step that could not be attempted at all, with the reason. */
  abandoned: string | undefined;
}

/** A fresh trial record for a walk into `volume`. */
export function openLethalTrial(
  volume: LethalVolume,
  entryCell: Vec3Tuple,
  stake: StakeRule | undefined,
): LethalTrial {
  return {
    volume: volume.id,
    entryCell,
    stake: stake?.id,
    objective: stake?.currency.objective,
    died: false,
    deathPos: undefined,
    wordingSeen: false,
    balanceBefore: undefined,
    balanceAfterDeath: undefined,
    expectedForfeit: undefined,
    respawnPos: undefined,
    respawnSeat: undefined,
    expectedAnchor: undefined,
    markerPos: undefined,
    walkedBack: false,
    collectClicks: 0,
    balanceAfterCollect: undefined,
    markerRetired: false,
    abandoned: undefined,
  };
}

/** Distance from a float position to a block cell's centre-of-floor. */
function distToCell(pos: Vec3Tuple, cell: Vec3Tuple): number {
  return Math.hypot(pos[0] - (cell[0] + 0.5), pos[1] - cell[1], pos[2] - (cell[2] + 0.5));
}

/**
 * Turn one trial's observations into the failures they represent.
 *
 * Every clause is a promise the DSL made, stated as the reader will have to act on
 * it. Nothing here consults the emitter, and nothing is conditional on a feature
 * being "wired": a trial that was abandoned says so and is a failure, because a
 * stage that cannot run is not a stage that passed.
 */
export function lethalTrialFailures(t: LethalTrial, markerTolerance = 0.75): string[] {
  const out: string[] = [];
  const where = `[${t.entryCell.join(", ")}]`;
  if (t.abandoned !== undefined) {
    out.push(`${t.volume}: the death loop could not be exercised — ${t.abandoned}`);
    return out;
  }
  if (!t.died) {
    out.push(
      `${t.volume}: the bot stood inside the declared lethal volume at ${where} and did NOT die. ` +
        `A lethal volume is the one thing in the engine whose entire contract is that entering ` +
        `it kills; nothing downstream of the death edge can be true if this is false`,
    );
    return out;
  }
  if (!t.wordingSeen) {
    out.push(
      `${t.volume}: the player died in the volume and the line it PROMISES never reached them. ` +
        `A volume with no words is a player who dies with no idea why — the wording is a ` +
        `required field precisely because there is no default that could be right`,
    );
  }
  if (t.objective !== undefined) {
    if (t.balanceBefore === undefined || t.balanceAfterDeath === undefined) {
      out.push(
        `${t.volume}: the currency ledger \`${t.objective}\` could not be read across the death, ` +
          `so the declared forfeit was never checked — an unread ledger is not a matched one`,
      );
    } else {
      const expected = t.balanceBefore - (t.expectedForfeit ?? 0);
      if (t.balanceAfterDeath !== expected) {
        out.push(
          `${t.volume}: the death took the wrong amount. \`${t.objective}\` was ` +
            `${t.balanceBefore} before and ${t.balanceAfterDeath} after; the campaign's declared ` +
            `forfeit rule says it should be ${expected} (a forfeit of ${t.expectedForfeit ?? 0})`,
        );
      }
    }
  }
  if (t.respawnSeat === undefined) {
    out.push(
      `${t.volume}: the player respawned at ` +
        `${t.respawnPos ? `[${t.respawnPos.map((n) => n.toFixed(2)).join(", ")}]` : "an unknown position"} ` +
        `— not at any respawn seat this campaign declares. \`/spawnpoint\` is only a hint and ` +
        `vanilla silently falls back to world spawn, so a wrong seat here is the softlock the ` +
        `re-seat exists to prevent`,
    );
  }
  if (t.stake === undefined) return out;
  if (t.expectedAnchor === undefined) {
    out.push(
      `${t.volume}: the placement table has NO row for this (death region, respawn seat) pair, ` +
        `so the build never said where the stake should land — the runtime lookup has nothing ` +
        `to be right about`,
    );
    return out;
  }
  const anchor = `[${t.expectedAnchor.join(", ")}]`;
  if (t.markerPos === undefined) {
    out.push(
      `${t.volume}: no recovery stake stands at ${anchor}, the anchor the compile-time ` +
        `placement table chose for this death. The purse was taken and left nowhere`,
    );
    return out;
  }
  // Under three quarters of a block, because a stake placed at the anchor the
  // table chose stands at its exact centre (`stk_put_<n>` positions at
  // `cell + 0.5`, measured drift 0.00) — while the failure this catches is the
  // marker landing on the NEXT cell, one block away. A tolerance of a whole block
  // would let the degenerate "leave it where you fell" branch pass as the
  // projected one, which is the entire difference the placement rule exists for.
  const drift = distToCell(t.markerPos, t.expectedAnchor);
  if (drift > markerTolerance) {
    out.push(
      `${t.volume}: the recovery stake stands at ` +
        `[${t.markerPos.map((n) => n.toFixed(2)).join(", ")}], ${drift.toFixed(2)} blocks from ` +
        `${anchor} — the anchor the compile-time placement table proved reachable and safe. ` +
        `Every proof about where a stake may stand is about that cell, not this one`,
    );
  }
  if (!t.walkedBack) {
    out.push(
      `${t.volume}: the walk back from the respawn seat to the stake at ${anchor} could not be ` +
        `made. The placement rule's whole premise is that the anchor is on the route the player ` +
        `must come back along`,
    );
    return out;
  }
  if (t.balanceAfterCollect === undefined) {
    out.push(`${t.volume}: the ledger could not be read after collecting the stake`);
    return out;
  }
  if (t.balanceBefore !== undefined && t.balanceAfterCollect !== t.balanceBefore) {
    out.push(
      `${t.volume}: collecting the stake did not return exactly what the death took. ` +
        `\`${t.objective}\` was ${t.balanceBefore} before the death and ${t.balanceAfterCollect} ` +
        `after ${t.collectClicks} right-click(s) in one tick; it must be ${t.balanceBefore} — ` +
        `more means the collection is not idempotent, less means the stake short-changed the ` +
        `player`,
    );
  }
  if (!t.markerRetired) {
    out.push(
      `${t.volume}: the stake was collected and its hardware is still standing at ${anchor}. ` +
        `A collected stake that does not vanish is an affordance that answers a click with ` +
        `nothing, forever`,
    );
  }
  return out;
}

/** What the whole stage examined — printed even when it examined nothing. */
export interface DeathLoopBinding {
  /** Lethal volumes the campaign declares. */
  readonly declaredVolumes: number;
  /** Volumes this run actually walked into. */
  readonly volumesEntered: number;
  /** Player deaths this run OBSERVED. */
  readonly deathsObserved: number;
  /** Recovery stakes this run examined at a table anchor. */
  readonly stakesExamined: number;
  /** Respawns matched to a declared seat. */
  readonly seatsMatched: number;
  /** Walk-back legs completed. */
  readonly walksBack: number;
}

/** Count what a set of trials examined. */
export function deathLoopBinding(
  plan: DeathPlan,
  trials: readonly LethalTrial[],
): DeathLoopBinding {
  return {
    declaredVolumes: plan.volumes.length,
    volumesEntered: trials.length,
    deathsObserved: trials.filter((t) => t.died).length,
    stakesExamined: trials.filter((t) => t.markerPos !== undefined).length,
    seatsMatched: trials.filter((t) => t.respawnSeat !== undefined).length,
    walksBack: trials.filter((t) => t.walkedBack).length,
  };
}

/**
 * The findings a binding count is, on its own.
 *
 * playtest-methodology rule 1: **a zero binding is a finding, not a pass.** The
 * island's combat floor gate examined zero enemies for nineteen rounds and was
 * green every time; this is the clause that stops the same thing happening to the
 * one mechanic a souls-shaped delve is entirely made of.
 */
export function deathLoopBindingFailures(b: DeathLoopBinding): string[] {
  const out: string[] = [];
  if (b.volumesEntered === 0) {
    out.push(
      `the death-loop stage entered ZERO of ${b.declaredVolumes} declared lethal volume(s): it ` +
        `examined nothing, and an empty examination is a finding, never a pass`,
    );
    return out;
  }
  if (b.deathsObserved === 0) {
    out.push(
      `the death-loop stage entered ${b.volumesEntered} lethal volume(s) and observed ZERO ` +
        `player deaths — every assertion downstream of the death edge is therefore unbound`,
    );
  }
  return out;
}
