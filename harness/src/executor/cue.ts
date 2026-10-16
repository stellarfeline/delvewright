// MineflayerExecutor: the quest cues — what the bot heard, where it stood, the verdict.

import { judgeCues, type CueJudgement, type CueLedger } from "../cues.ts";
import type { MineflayerExecutor } from "../executor.ts";

const bare = (s: string): string => s.replace(/^minecraft:/, "");

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /** Adopt the build's cue ledger; from here every cue packet and marker is logged. */
  useCueLedger(this: MineflayerExecutor, ledger: CueLedger): void {
    this.cueLedger = ledger;
  },

  /** Log a completion marker of this run's campaign, with where the bot stood. */
  observeCueMarker(this: MineflayerExecutor, token: string): void {
    if (!this.cueLedger) return;
    const p = this.bot?.entity?.position;
    this.cueLog.push({ kind: "marker", token, bot: p ? [p.x, p.y, p.z] : null });
  },

  /** Log a sound packet naming one of the ledger's cues, with where the bot stood. */
  observeCueSound(
    this: MineflayerExecutor,
    name: string,
    pos: { x: number; y: number; z: number },
    volume: number,
    pitch: number,
  ): void {
    const ledger = this.cueLedger;
    if (!ledger || !Object.values(ledger.cues).some((c) => bare(c) === bare(name))) return;
    const p = this.bot?.entity?.position;
    this.cueLog.push({
      kind: "sound",
      name,
      pos: [pos.x, pos.y, pos.z],
      volume,
      pitch,
      bot: p ? [p.x, p.y, p.z] : null,
    });
  },

  /** The verdict over everything logged so far; `undefined` without a ledger. */
  cueJudgement(this: MineflayerExecutor): CueJudgement | undefined {
    return this.cueLedger ? judgeCues(this.cueLedger, this.cueLog) : undefined;
  },
};
