// The crush death, as the bot tier walks it (spec-0016 §4 addendum).
//
// A `crush: true` timed gate kills a body standing in its region on the closing
// tick. The build's death plan (`validation/death-plan.json`, format 6) lists
// every such gate with its clock, the sides a body can step in from, and the
// exported step after which its death is walked with the mouth walked to
// (`trial_at`, `trial_mouth`); every respawn seat carries the step after which
// it is the seat a death returns to. The bot stands in each gate's closing
// cells once, at that boundary, dies there, and comes back at the seat that
// governs a death at that point of the run.
//
// Everything here is pure: types, the governing-seat rule, verdicts and the
// stage row. The executor supplies only observations.

import type { Vec3Tuple } from "./critical-path.ts";
import type { StageResult } from "./report.ts";
import type { CrushGate, DeathPlan, GateApproach, Seat } from "./death-loop.ts";

/** How far a respawn may land from a seat's cell and still be that seat. */
export const SEAT_TOLERANCE = 1.5;

/**
 * **The seats that govern a death after exported step `boundary`**: the seat
 * whose `inForceAfter` is the latest at or before the boundary (every seat
 * sharing that step — two checkpoints one beat fires are one answer the path
 * cannot split), plus every seat a trigger sets, which may be in force wherever
 * the party struck it. Indices into `seats`, in plan order.
 */
export function governingSeats(seats: readonly Seat[], boundary: number): number[] {
  let latest = Number.NEGATIVE_INFINITY;
  for (const s of seats) {
    if (s.inForceAfter !== null && s.inForceAfter <= boundary) {
      latest = Math.max(latest, s.inForceAfter);
    }
  }
  const out: number[] = [];
  for (const [i, s] of seats.entries()) {
    if (s.inForceAfter === null || s.inForceAfter === latest) out.push(i);
  }
  return out;
}

/** Whether `pos` is a respawn at seat `seat` (vanilla lands at `cell + 0.5`). */
export function atSeat(pos: Vec3Tuple, seat: Seat, tolerance = SEAT_TOLERANCE): boolean {
  const d = Math.hypot(
    pos[0] - (seat.cell[0] + 0.5),
    pos[1] - seat.cell[1],
    pos[2] - (seat.cell[2] + 0.5),
  );
  return d <= tolerance;
}

/**
 * Whether a crush gate's death is walked at the boundary after exported step
 * `boundary`: exactly at the one boundary the build names (`trialAt`), the
 * first proven leg whose start reaches a mouth, and at no other. A gate the
 * build names no boundary for is walked after the path. The run never chooses
 * the boundary by what its own pathfinder finds live: that answer moves with
 * the clock of every timed gate in the way and with the client's timing.
 */
export function crushDueAt(gate: CrushGate, boundary: number): boolean {
  return gate.trialAt !== undefined && boundary === gate.trialAt;
}

/** One walk into one crush gate's closing cells, and what was observed of it. */
export interface CrushTrial {
  readonly gate: string;
  /** Where in the run the trial ran: after which exported step, or after the path. */
  readonly at: string;
  /** The exported step after which it ran. */
  readonly boundary: number;
  /** The exported step after which the build says it runs (`trialAt`), if it names one. */
  readonly named: number | undefined;
  /** The side the bot stepped in from. */
  approach: GateApproach | undefined;
  /** Labels of the seats that govern a death here ({@link governingSeats}). */
  expectedSeats: string[];
  /** Whether the bot's feet were OBSERVED in the region while the gate was open. */
  enteredRegion: boolean;
  died: boolean;
  deathPos: Vec3Tuple | undefined;
  /** Whether the closing tick's own selector would have matched the body. */
  credited: boolean;
  respawnPos: Vec3Tuple | undefined;
  /** The seat the respawn matched, of any declared seat. */
  respawnSeat: string | undefined;
  /** Whether the respawn matched one of {@link expectedSeats}. */
  respawnedAtGoverning: boolean;
  /** Why the gate was not entered: its disarm had stopped the clock. */
  notLiveAtTrial: string | undefined;
  /** A step that could not be attempted at all, with the reason. */
  abandoned: string | undefined;
}

/** A fresh trial record. */
export function openCrushTrial(gate: CrushGate, boundary: number, at: string): CrushTrial {
  return {
    gate: gate.id,
    at,
    boundary,
    named: gate.trialAt,
    approach: undefined,
    expectedSeats: [],
    enteredRegion: false,
    died: false,
    deathPos: undefined,
    credited: false,
    respawnPos: undefined,
    respawnSeat: undefined,
    respawnedAtGoverning: false,
    notLiveAtTrial: undefined,
    abandoned: undefined,
  };
}

const fmtPos = (p: Vec3Tuple | undefined): string =>
  p === undefined ? "an unknown position" : `[${p.map((n) => n.toFixed(2)).join(", ")}]`;

/** One trial's observations, turned into the failures they represent. */
export function crushTrialFailures(t: CrushTrial): string[] {
  if (t.named !== undefined && t.boundary !== t.named) {
    return [
      `${t.gate}: the trial ran ${t.at} (boundary ${t.boundary}), and the build names the ` +
        `boundary after step ${t.named}: the seat a death returns to is judged at the boundary ` +
        `the build names, never at one the run found`,
    ];
  }
  if (t.notLiveAtTrial !== undefined) return [];
  if (t.abandoned !== undefined) {
    return [`${t.gate}: the crush death could not be walked — ${t.abandoned}`];
  }
  if (!t.died) {
    return [
      t.enteredRegion
        ? `${t.gate}: the bot stood in the gate's closing cells through the closing tick and did ` +
          `NOT die. A crush gate's whole contract is that a body caught in it is killed`
        : `${t.gate}: the bot never got its feet into the gate's closing cells, so the crush was ` +
          `not exercised — the fault is the walk in, not the gate`,
    ];
  }
  const out: string[] = [];
  if (!t.credited) {
    out.push(
      `${t.gate}: the bot died at ${fmtPos(t.deathPos)}, where the closing tick's selector does ` +
        `not reach — that death is not this gate's kill`,
    );
    return out;
  }
  if (!t.respawnedAtGoverning) {
    out.push(
      `${t.gate}: after the crush the player came back at ${fmtPos(t.respawnPos)} ` +
        `(${t.respawnSeat ?? "no declared seat"}), not at the seat that governs a death ${t.at}: ` +
        `${t.expectedSeats.join(" or ") || "none declared"}`,
    );
  }
  return out;
}

/** What the crush half examined, against what the build declares. */
export interface CrushBinding {
  /** Crush gates the build declares. */
  readonly declared: number;
  /** Trials opened over a live gate. */
  readonly entered: number;
  /** Crush deaths walked: a death the closing tick's selector credits. */
  readonly deathsWalked: number;
  /** Of those, respawns at the governing seat. */
  readonly respawnsAtGoverning: number;
  /** Gates whose disarm had stopped the clock when their trial opened. */
  readonly notLive: number;
}

/** Count what a set of trials examined. */
export function crushBinding(plan: DeathPlan, trials: readonly CrushTrial[]): CrushBinding {
  return {
    declared: plan.crushGates.length,
    entered: trials.filter((t) => t.notLiveAtTrial === undefined).length,
    deathsWalked: trials.filter((t) => t.died && t.credited).length,
    respawnsAtGoverning: trials.filter((t) => t.died && t.credited && t.respawnedAtGoverning)
      .length,
    notLive: trials.filter((t) => t.notLiveAtTrial !== undefined).length,
  };
}

/** The binding line the run prints and the stage carries. */
export function crushLine(b: CrushBinding): string {
  return (
    `[crush] walked ${b.deathsWalked} of ${b.declared} declared crush death(s); ` +
    `${b.respawnsAtGoverning} of ${b.deathsWalked} respawned at the governing seat` +
    (b.notLive > 0 ? `; ${b.notLive} not live at trial (disarmed)` : "")
  );
}

/** What the run knows about the crush stage when it writes the report. */
export interface CrushStageInput {
  readonly enabled: boolean;
  readonly disabledReason: string;
  readonly pathProven: boolean;
  readonly interruption: string | undefined;
  readonly plan: DeathPlan | undefined;
  readonly trials: readonly CrushTrial[];
}

/** **The crush stage's row of the run report.** */
export function crushStage(i: CrushStageInput): StageResult {
  if (!i.enabled || i.plan === undefined) {
    return { stage: "crush-death", ran: false, passed: false, findings: [i.disabledReason], failures: [] };
  }
  if (!i.pathProven) {
    // A trial is a whole observation the moment it ends: the body stood in the
    // span on the closing tick and died or did not. Those are judged; coverage
    // is not, because the path's failure ended the run before every gate's turn.
    const failures = i.trials.flatMap((t) => crushTrialFailures(t));
    return {
      stage: "crush-death",
      ran: i.trials.length > 0,
      passed: false,
      findings: [
        `the critical path failed, so the crush stage did not finish: ${i.trials.length} of ` +
          `${i.plan.crushGates.length} declared crush gate(s) were tried before it did, and each ` +
          `trial is judged; the gates not reached are not`,
      ],
      failures,
    };
  }
  const b = crushBinding(i.plan, i.trials);
  const tried = new Set(i.trials.map((t) => t.gate));
  const failures = [
    ...i.plan.crushGates
      .filter((g) => !tried.has(g.id))
      .map((g) => `${g.id} is a declared crush gate and no trial walked its death`),
    ...(i.interruption === undefined
      ? []
      : [`the crush stage was interrupted: ${i.interruption}`]),
    ...(b.declared > 0 && b.deathsWalked === 0 && b.notLive < b.declared
      ? [
          `ZERO of ${b.declared} declared crush death(s) were walked: an empty examination is a ` +
            `finding, never a pass`,
        ]
      : []),
    ...i.trials.flatMap((t) => crushTrialFailures(t)),
  ];
  const notLive = i.trials
    .filter((t) => t.notLiveAtTrial !== undefined)
    .map((t) => `${t.gate} was not live at its trial — ${t.notLiveAtTrial}; it was not entered`);
  return {
    stage: "crush-death",
    ran: true,
    passed: failures.length === 0,
    findings: [crushLine(b), ...notLive],
    failures,
  };
}
