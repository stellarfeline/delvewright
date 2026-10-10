// MineflayerExecutor: the pulse plan, its stations and its verdicts (spec-0102 §5.3).

import type { Step } from "../critical-path.ts";
import {
  isBeat,
  judgeListening,
  namesSound,
  judgeSilent,
  pulseVerdicts,
  standTicks,
  stationsBefore,
  type HeardSound,
  type PulsePlan,
  type PulseVerdict,
} from "../pulse.ts";
import { nearestIndex, nextLegWaypoints } from "../waypoints.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /** spec-0102 §5.3: adopt the build's pulses and their stations. */
  usePulsePlan(this: MineflayerExecutor, plan: PulsePlan): void {
    this.pulsePlan = plan;
  },

  /** Every pulse's verdict; empty when the build declares no pulse. */
  pulseVerdicts(this: MineflayerExecutor): PulseVerdict[] {
    return this.pulsePlan ? pulseVerdicts(this.pulsePlan, this.pulseRecorded) : [];
  },

  /**
   * Anything the path owes before a step's own action: the run-back fights
   * its leg meets (spec-0016 §1), then the pulse stations on it (spec-0102).
   */
  async beforeStep(this: MineflayerExecutor, step: Step): Promise<void> {
    await this.runBacksBefore(step);
    await this.listenAtPulseStations(step);
  },

  /**
   * Stand at every pulse station the compiler put on the leg to `step`, in
   * the order the leg reaches them, and record what was heard there. The walk
   * goes along the leg's own proven waypoints, as a run-back's approach does,
   * and the step's own walk resumes from the station.
   */
  async listenAtPulseStations(this: MineflayerExecutor, step: Step): Promise<void> {
    const plan = this.pulsePlan;
    if (!plan) return;
    const token =
      "objective" in step && typeof step.objective === "string"
        ? step.objective
        : step.action === "trigger"
          ? step.trigger
          : undefined;
    if (token === undefined) return;
    const due = stationsBefore(plan, token, new Set(this.pulseRecorded.keys()));
    if (due.length === 0) return;
    const bot = this.requireBot();
    const pos = "pos" in step ? step.pos : undefined;
    const leg =
      pos && this.waypoints
        ? nextLegWaypoints(this.waypoints.legs, this.legCursor, [pos[0], pos[1], pos[2]])
        : undefined;
    const cells = leg?.matched ? leg.waypoints : undefined;
    const along = (c: readonly [number, number, number]): number =>
      cells ? nearestIndex(cells, c) : 0;
    let walked = this.legResume?.leg === this.legCursor ? this.legResume.from : 0;
    for (const d of [...due].sort((a, b) => along(a.station.cell) - along(b.station.cell))) {
      const k = along(d.station.cell);
      await this.walkTo(
        d.station.cell,
        0,
        `${d.kind} station of ${d.pulse.id} along the leg to ${token}`,
        false,
        undefined,
        cells && k >= walked ? cells.slice(walked, k) : undefined,
      );
      if (cells && k >= walked) {
        walked = k;
        this.legResume = { leg: this.legCursor, from: k };
      }
      const heard: HeardSound[] = [];
      const listener = (
        name: string,
        p: { x: number; y: number; z: number },
        volume: number,
        pitch: number,
      ): void => {
        heard.push({ name, pos: [p.x, p.y, p.z], volume, pitch });
      };
      bot.on("soundEffectHeard", listener);
      try {
        await bot.waitForTicks(standTicks(d.pulse));
      } finally {
        bot.removeListener("soundEffectHeard", listener);
      }
      const failure =
        d.kind === "listening" ? judgeListening(d.pulse, heard) : judgeSilent(d.pulse, heard);
      this.pulseRecorded.set(d.key, failure);
      const named = heard.filter((h) => namesSound(d.pulse, h)).length;
      const beats = heard.filter((h) => isBeat(d.pulse, h)).length;
      this.pulseStations.push({
        pulse: d.pulse.id,
        kind: d.kind,
        cell: d.station.cell,
        ticks: standTicks(d.pulse),
        named,
        beats,
        failure: failure ?? null,
      });
      process.stderr.write(
        `[pulse] ${d.pulse.id}: ${d.kind} station ${JSON.stringify(d.station.cell)} — ` +
          `${beats} beat(s) as written, ${named} naming the sound, over ` +
          `${standTicks(d.pulse)} ticks: ${failure ?? "as the compiler wrote it"}\n`,
      );
    }
  },
};
