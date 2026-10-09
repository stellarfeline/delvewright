// MineflayerExecutor: the repaint plan and its verdicts.

import { CLIENT_VIEW_DISTANCE } from "../client-loaded.ts";
import { RepaintWatch, type RepaintPlan, type RepaintVerdict } from "../repaint.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  // -------------------------------------------------------------------------
  // The death loop: a real player dies, and every consequence the
  // engine promised is asserted from the observation.
  // -------------------------------------------------------------------------

  /**
   * Adopt the build's death contract. Also hands the declared lethal volumes to
   * the navigator, which has to agree with the compiler that they are impassable.
   */
  /**
   * spec-0080 §5.2: watch every repaint the build performs reach the client —
   * a `chunk_biomes` for each held chunk of its volume, and no `map_chunk`.
   */
  useRepaintPlan(this: MineflayerExecutor, plan: RepaintPlan): void {
    this.repaintWatch = new RepaintWatch(plan, CLIENT_VIEW_DISTANCE);
  },

  /** The repaint verdicts so far; empty when the build repaints nothing. */
  repaintVerdicts(this: MineflayerExecutor): RepaintVerdict[] {
    return this.repaintWatch?.verdicts() ?? [];
  },
};
