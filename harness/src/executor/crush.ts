// MineflayerExecutor: a crush gate's death, walked (spec-0016 §4 addendum).

import type { Vec3Tuple } from "../critical-path.ts";
import {
  atSeat,
  crushDueAt,
  governingSeats,
  openCrushTrial,
  type CrushTrial,
} from "../crush-death.ts";
import {
  bodyInVolume,
  gateVerdict,
  seatAtRespawn,
  termKey,
  type CrushGate,
  type DeathPlan,
} from "../death-loop.ts";
import { cycleMs, insideGate } from "../timed-gate.ts";
import type { TimedGate } from "../waypoints.ts";
import { delay } from "./connection.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** Re-aim cadence while the bot keeps its feet in the closing cells. */
const HOLD_TICK_MS = 50;

/** Squared horizontal distance from the stand cell's centre the hold tolerates. */
const HOLD_SLACK_SQ = 0.2 * 0.2;

/**
 * Margin past one full cycle that the hold waits for the closing tick: the
 * window wait already began at an open edge, so the close lands within the open
 * half; this only absorbs server tick jitter.
 */
const HOLD_MARGIN_MS = 5_000;

/** The gate as the timed-gate machinery reads one. */
function asTimedGate(g: CrushGate): TimedGate {
  return {
    id: g.id,
    min: g.region.lo,
    max: g.region.hi,
    block: "",
    openTicks: g.openTicks,
    closedTicks: g.closedTicks,
    phase: g.phase,
    crush: true,
  };
}

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /** Every crush trial this run made, and what it observed. */
  crushTrialsMade(this: MineflayerExecutor): readonly CrushTrial[] {
    return this.crushTrials;
  },

  /** Arm the crush trials at the critical path's step boundaries. */
  armCrushOnPath(this: MineflayerExecutor): void {
    this.crushOnPath = true;
  },

  /**
   * The crush gates due at the boundary after exported step `index`: not yet
   * tried, and due by {@link crushDueAt}. Reachability is asked by the trial.
   */
  crushGatesDue(this: MineflayerExecutor, index: number): CrushGate[] {
    const plan = this.deathPlan;
    if (!this.crushOnPath || plan === undefined) return [];
    return plan.crushGates.filter((g) => !this.crushTried.has(g.id) && crushDueAt(g, index));
  },

  /**
   * Every crush gate no step boundary walked, after the proven path; `boundary`
   * is the path's last exported step. A gate that cannot be reached here is
   * that gate's failure, never a skip.
   */
  async runCrushDeaths(this: MineflayerExecutor, boundary: number): Promise<void> {
    const plan = this.deathPlan;
    if (plan === undefined) return;
    const stage = this.stageNow;
    this.stageNow = "crush-death";
    try {
      for (const g of plan.crushGates) {
        if (this.crushTried.has(g.id)) continue;
        await this.crushTrial(plan, g, boundary, "after the critical path", true);
      }
      if (this.death !== undefined) await this.recoverFromDeath();
    } finally {
      this.stageNow = stage;
    }
  },

  /**
   * **One crush gate's death.** Walk to a mouth, wait for a fresh window, step
   * into the closing cells and keep the feet there until the closing tick; then
   * read where the death happened and where the respawn put the party, against
   * the seats that govern a death at this boundary ({@link governingSeats}).
   *
   * `mustRun` is set after the path: there the trial is the gate's last chance,
   * so a gate no mouth of which can be reached is recorded as abandoned. At a
   * step boundary such a gate is left for a later boundary instead.
   */
  async crushTrial(
    this: MineflayerExecutor,
    plan: DeathPlan,
    gate: CrushGate,
    boundary: number,
    at: string,
    mustRun: boolean,
  ): Promise<void> {
    const bot = this.requireBot();
    if (this.death !== undefined) await this.recoverFromDeath();
    // A disarmed gate's clock is stopped for good: nothing is left to crush.
    if (gate.gate.terms.length > 0) {
      const answers = new Map<string, boolean | undefined>();
      for (const t of gate.gate.terms) {
        if (!answers.has(termKey(t))) answers.set(termKey(t), await this.askTerm(t));
      }
      const verdict = gateVerdict({ stake: gate.id, gates: [gate.gate] }, (t) =>
        answers.get(termKey(t)),
      );
      if (verdict.kind !== "open") {
        if (!mustRun && verdict.kind === "shut") return;
        this.crushTried.add(gate.id);
        const trial = openCrushTrial(gate, boundary, at);
        if (verdict.kind === "shut") trial.notLiveAtTrial = verdict.why;
        else trial.abandoned = `whether the gate's clock still runs could not be read — ${verdict.why}`;
        this.crushTrials.push(trial);
        return;
      }
    }
    // The mouth is asked for with every crush region costed like a lethal
    // volume, so the probe never answers "reachable" by a walk through a gate.
    const saved = this.lethalBoxes;
    this.lethalBoxes = [...saved, ...plan.crushGates.map((g) => g.region)];
    const approach = (() => {
      try {
        return gate.approaches.find((a) => this.reachableFromHere(a.mouth));
      } finally {
        this.lethalBoxes = saved;
      }
    })();
    if (approach === undefined && !mustRun) return;
    this.crushTried.add(gate.id);
    const trial = openCrushTrial(gate, boundary, at);
    this.crushTrials.push(trial);
    trial.expectedSeats = governingSeats(plan.seats, boundary).map(
      (i) => `${plan.seats[i]!.label} [${plan.seats[i]!.cell.join(", ")}]`,
    );
    if (approach === undefined) {
      trial.abandoned =
        gate.approaches.length === 0
          ? "the build lists no side a body can step into the gate from"
          : `none of its ${gate.approaches.length} mouth(s) can be reached from ` +
            `[${(this.feetCell() ?? []).join(", ")}]`;
      return;
    }
    trial.approach = approach;
    process.stderr.write(
      `[crush] ${gate.id} ${at}: walking to the mouth [${approach.mouth.join(", ")}] to stand ` +
        `in [${approach.stand.join(", ")}] on the closing tick; a death here returns the party ` +
        `to ${trial.expectedSeats.join(" or ")}\n`,
    );
    const deathsBefore = this.deathSeq;
    const fault = await this.walkPastReseated(
      `crush approach to ${gate.id}`,
      approach.mouth,
      "the gate mouth",
    );
    if (this.deathSeq !== deathsBefore) {
      trial.abandoned = `the bot died on the walk to the mouth, at ${
        this.lastDeath?.position ? `[${this.lastDeath.position.join(", ")}]` : "?"
      }`;
      return;
    }
    if (fault !== undefined) {
      trial.abandoned = fault.why;
      return;
    }
    const tg = asTimedGate(gate);
    const observed = await this.waitForGateWindow([tg], approach.mouth, approach.stand);
    if (!observed) {
      trial.abandoned = "the gate's closed→open edge could not be observed from its mouth";
      return;
    }
    // In, and held. The hold is raw control (the timed-gate machinery's own
    // mechanism): face the stand cell's centre and walk while off it.
    const deadline = Date.now() + cycleMs(tg) + HOLD_MARGIN_MS;
    try {
      while (this.deathSeq === deathsBefore && Date.now() < deadline) {
        const feet = this.feetCell();
        if (feet !== undefined && insideGate(feet, tg)) trial.enteredRegion = true;
        const p = bot.entity.position;
        const dx = approach.stand[0] + 0.5 - p.x;
        const dz = approach.stand[2] + 0.5 - p.z;
        if (dx * dx + dz * dz > HOLD_SLACK_SQ) {
          try {
            await bot.lookAt(p.offset(dx, 0, dz), true);
          } catch {
            // best effort — a look failure must not end the hold
          }
          bot.setControlState("forward", true);
        } else {
          bot.setControlState("forward", false);
        }
        await delay(HOLD_TICK_MS);
      }
    } finally {
      bot.clearControlStates();
    }
    trial.died = this.deathSeq > deathsBefore;
    if (!trial.died) {
      process.stderr.write(
        `[crush] ${gate.id}: ${trial.enteredRegion ? "stood in the closing cells and is alive" : "never got into the closing cells"}\n`,
      );
      return;
    }
    const pos = (this.death ?? this.lastDeath)?.position;
    trial.deathPos = pos ? ([...pos] as unknown as Vec3Tuple) : undefined;
    trial.credited = trial.deathPos !== undefined && bodyInVolume(trial.deathPos, gate.region);
    if (trial.deathPos !== undefined && bodyInVolume(trial.deathPos, gate.region)) {
      trial.enteredRegion = true;
    }
    await this.recoverFromDeath();
    const p = bot.entity?.position;
    trial.respawnPos = p ? [p.x, p.y, p.z] : undefined;
    if (trial.respawnPos !== undefined) {
      const seat = seatAtRespawn(plan.seats, trial.respawnPos);
      trial.respawnSeat = seat === undefined ? undefined : plan.seats[seat]!.label;
      const respawn = trial.respawnPos;
      trial.respawnedAtGoverning = governingSeats(plan.seats, boundary).some((i) =>
        atSeat(respawn, plan.seats[i]!),
      );
    }
    process.stderr.write(
      `[crush] ${gate.id}: died at ${
        trial.deathPos ? `[${trial.deathPos.map((n) => n.toFixed(2)).join(", ")}]` : "?"
      } (${trial.credited ? "in" : "OUTSIDE"} the closing cells); respawned at ${
        trial.respawnPos ? `[${trial.respawnPos.map((n) => n.toFixed(2)).join(", ")}]` : "?"
      } (${trial.respawnSeat ?? "no declared seat"}) — ${
        trial.respawnedAtGoverning ? "the governing seat" : "NOT the governing seat"
      }\n`,
    );
  },
};
