// MineflayerExecutor: the music ledger, its listening stations and its cuts (spec-0109 §5.3).

import type { Step } from "../critical-path.ts";
import {
  MusicWatch,
  musicStationsBefore,
  pinnedSoundSources,
  type MusicPlan,
  type StopVerdict,
} from "../music.ts";
import { nextLegWaypoints } from "../waypoints.ts";
import { routeIndexAfter } from "../route.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /** spec-0109 §5.3: adopt the build's music ledger. */
  useMusicPlan(this: MineflayerExecutor, plan: MusicPlan): void {
    this.musicWatch = new MusicWatch(plan, pinnedSoundSources());
  },

  /** Every cut's verdict; empty when the build writes no ledger. */
  musicStopVerdicts(this: MineflayerExecutor): StopVerdict[] {
    return this.musicWatch?.stopVerdicts() ?? [];
  },

  /**
   * Stand at every music station the compiler put on the leg to `step` and
   * read the biome the client holds under the eye there. The walk goes along
   * the leg's own proven waypoints, as a pulse station's does, and the step's
   * own walk resumes from the station.
   */
  async readMusicStations(this: MineflayerExecutor, step: Step): Promise<void> {
    const watch = this.musicWatch;
    if (!watch) return;
    const token =
      "objective" in step && typeof step.objective === "string"
        ? step.objective
        : step.action === "trigger"
          ? step.trigger
          : undefined;
    if (token === undefined) return;
    const done = new Set(watch.stations.map((s) => s.biome));
    const due = musicStationsBefore(watch.plan, token, done);
    if (due.length === 0) return;
    const bot = this.requireBot();
    const pos = "pos" in step ? step.pos : undefined;
    const leg =
      pos && this.waypoints
        ? nextLegWaypoints(this.waypoints.legs, this.legCursor, [pos[0], pos[1], pos[2]])
        : undefined;
    const cells = leg?.matched ? leg.waypoints : undefined;
    let walked = this.legResume?.leg === this.legCursor ? this.legResume.from : 0;
    for (const b of due) {
      const station = b.station;
      if (station === undefined) continue;
      const k = cells ? routeIndexAfter(station.cell, cells) : 0;
      await this.walkTo(
        station.cell,
        0,
        `music station of ${b.id} along the leg to ${token}`,
        false,
        undefined,
        cells && k >= walked ? cells.slice(walked, k) : undefined,
      );
      if (cells && k >= walked) {
        walked = k;
        this.legResume = { leg: this.legCursor, from: k };
      }
      // The eye's block: the cell above the feet, as the ledger reads it.
      const eye = { x: station.cell[0] + 0.5, y: station.cell[1] + 1.5, z: station.cell[2] + 0.5 };
      const world = bot.world as unknown as {
        getColumnAt?: (p: typeof eye) => unknown;
        getBiome?: (p: typeof eye) => number;
      };
      const loaded = typeof world.getColumnAt === "function" && world.getColumnAt(eye) !== undefined;
      const id = loaded && typeof world.getBiome === "function" ? world.getBiome(eye) : undefined;
      const read = id === undefined ? undefined : watch.biomeName(id);
      const r = watch.recordStation(b, read);
      process.stderr.write(
        `[music] ${b.id}: station ${JSON.stringify(station.cell)} — ` +
          (r.read === undefined
            ? `not_read: ${r.notRead ?? "registry"}`
            : r.failure === null
              ? `the client holds ${r.read}, as the ledger says`
              : r.failure) +
          "\n",
      );
    }
  },
};
