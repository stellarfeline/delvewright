// MineflayerExecutor: mustering a wave.

import type { Vec3Tuple } from "../critical-path.ts";
import { type Encounter } from "../combat.ts";
import {
  parseMusterBody,
  parseMusterSummary,
  verifyMuster,
  type MusterVerdict,
} from "../muster.ts";
import { isWaveBody } from "../wave.ts";
import { delay } from "./connection.ts";
import { SCORE_POLL_MS } from "./score.ts";
import { CENSUS_TIMEOUT_MS } from "./wave.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Read the wave's live bodies and check them against what the campaign declared.
   *
   * The one part of a combat step that is a verification. Server-side throughout —
   * the probe walks the wave's own tag — so it needs no proximity, no line of sight
   * and nothing of the bot but the right to run a function.
   */
  async musterWave(this: MineflayerExecutor, enc: Encounter, provisional = false): Promise<void> {
    const bot = this.requireBot();
    const before = this.musterSeq;
    // The seating this reading is OF: taken before the probe is called, so a
    // re-seat that lands while the answer is in flight leaves the wave owed.
    const epoch = this.seatEpoch;
    bot.chat(`/function ${enc.muster.probe}`);
    // What the party has felled of this seating, asked in the same breath: a
    // body the muster cannot read because the party already took it is a
    // different fact from a body the server never seated, and only the census
    // (`#wcred_<wave>`, zeroed by `spawn_<wave>`) can tell them apart.
    const credited = (await this.census(enc))?.summary.credited;
    const deadline = Date.now() + CENSUS_TIMEOUT_MS;
    for (;;) {
      const sum = this.musterSummaries.get(enc.wave);
      if (sum && sum.seq > before) {
        const verdict = verifyMuster(
          enc.muster,
          sum,
          this.musterBodies.get(sum.seq) ?? [],
          credited,
        );
        this.musters.set(enc.wave, verdict);
        // A reading of nothing at a step's open is provisional: an approach
        // trigger seats its wave when the party walks in, so the kill step reads
        // again from the anchor. Until then the reading stays owed (a body that
        // comes to the bot is read by the damage handlers) and nothing is logged.
        if (provisional && sum.tagged === 0) {
          process.stderr.write(
            `[muster] ${enc.wave}: nothing of this wave is standing at the step's open, of ` +
              `${verdict.declared} declared — reading it again from its anchor\n`,
          );
          return;
        }
        for (const f of verdict.failures) this.musterFailureLog.push(`${enc.wave}: ${f}`);
        this.musteredEpoch.set(enc.wave, epoch);
        process.stderr.write(
          `[muster] ${enc.wave}: read ${verdict.read}/${verdict.declared} declared body/bodies, ` +
            `${verdict.matched} matching their declaration over ${verdict.checked} checked ` +
            `fact(s)\n`,
        );
        for (const failure of verdict.failures) {
          process.stderr.write(`[muster] ${enc.wave}: FAILED — ${failure}\n`);
        }
        for (const finding of verdict.findings) {
          process.stderr.write(`[muster] ${enc.wave}: ${finding}\n`);
        }
        return;
      }
      if (Date.now() >= deadline) break;
      await delay(SCORE_POLL_MS);
    }
    this.musters.set(enc.wave, {
      wave: enc.wave,
      checked: enc.muster.checked,
      read: 0,
      declared: enc.muster.bodies,
      matched: 0,
      failures: [],
      findings: [
        `${enc.muster.probe} did not answer within ${CENSUS_TIMEOUT_MS}ms, so nothing about ` +
          `this wave's bodies was read — the ${enc.muster.checked} declared fact(s) it would ` +
          `have checked are unverified`,
      ],
    });
    process.stderr.write(`[muster] ${enc.wave}: the probe did not answer\n`);
  },

  /**
   * Which planned wave a body is of, asked of the SERVER: the census of each
   * candidate wave, by its tag, matched to where the body stands.
   *
   * The body is loaded (it has just hit the bot), so its own census line exists
   * wherever it has wandered. Candidates are the waves a reading or a die-retry
   * stake could be riding on — the rest cannot change what happens to the body —
   * nearest anchor first, and the first match ends the search. `undefined` when no
   * candidate's census places a body there: an ambusher, an actor, a wave with
   * nothing owed.
   */
  async waveOfBody(this: MineflayerExecutor, pos: Vec3Tuple): Promise<Encounter | undefined> {
    const candidates = (this.combatPlan?.encounters ?? []).filter(
      (enc) => this.protectedWave?.wave === enc.wave || this.readingOwed(enc),
    );
    const dist = (enc: Encounter): number =>
      Math.hypot(pos[0] - enc.pos[0], pos[1] - enc.pos[1], pos[2] - enc.pos[2]);
    // Asked together: each census is one function call answered on the next tick,
    // and the body is hitting the bot while the answers come back.
    const answers = await Promise.all(candidates.map((enc) => this.census(enc)));
    const matched = candidates.filter((enc, i) => {
      const census = answers[i];
      return census !== undefined && isWaveBody({ pos, census: census.mobs });
    });
    return matched.sort((a, b) => dist(a) - dist(b))[0];
  },

  /**
   * Does the run still owe a reading of this wave's CURRENT seating?
   *
   * A wave being cleared owes nothing — it has been read and is on its way out. A
   * cleared wave owes nothing until the delve could have put it back (its
   * run-back is then a reading still owed). Otherwise it is owed until a muster
   * has read it, and for a `respawns_on_rest` wave, read since the last rest or
   * respawn: that is a new seating, and a reading of the old one says nothing
   * about it.
   */
  readingOwed(this: MineflayerExecutor, enc: Encounter): boolean {
    if (this.clearing === enc.wave) return false;
    const cleared = this.clearedEpoch.get(enc.wave);
    if (cleared !== undefined && (!enc.respawnsOnRest || cleared === this.seatEpoch)) return false;
    const read = this.musteredEpoch.get(enc.wave);
    if (read === undefined) return true;
    if (!enc.respawnsOnRest) return false;
    return read !== this.seatEpoch;
  },

  /**
   * Read a wave's muster now, if its current seating is still owed a reading —
   * the kill step and the run-back both open with this. A seating a damage
   * handler has already read (a body came to the bot before the step did) is NOT
   * read a second time: the bot has since removed the body that came, and a
   * second reading would count the run's own removal as a body the server never
   * seated.
   */
  async musterUnlessRead(this: MineflayerExecutor, enc: Encounter, provisional = false): Promise<void> {
    const pending = this.earlyMusters.get(enc.wave);
    if (pending) await pending;
    if (!this.readingOwed(enc)) {
      process.stderr.write(
        `[muster] ${enc.wave}: this seating was already read (a body of it came to the bot ` +
          `before its step did) — not read again\n`,
      );
      return;
    }
    // Registered like an early reading, so a body that hits the bot while the
    // step's own muster is in flight waits for it instead of reading again.
    const run = this.musterWave(enc, provisional);
    this.earlyMusters.set(enc.wave, run);
    try {
      await run;
    } finally {
      if (this.earlyMusters.get(enc.wave) === run) this.earlyMusters.delete(enc.wave);
    }
  },

  /**
   * The damage handlers' reading of a wave a body of which has come to the bot:
   * the same muster the step would take, with the anchor's chunk held for it as
   * the step holds it. One reading per wave however many of its bodies hit at once.
   */
  async musterEarly(this: MineflayerExecutor, enc: Encounter): Promise<void> {
    const pending = this.earlyMusters.get(enc.wave);
    if (pending) return pending;
    const run = (async (): Promise<void> => {
      await this.holdChunk(enc.pos, true);
      try {
        if (this.readingOwed(enc)) await this.musterWave(enc);
      } finally {
        await this.holdChunk(enc.pos, false);
      }
    })();
    this.earlyMusters.set(enc.wave, run);
    try {
      await run;
    } finally {
      if (this.earlyMusters.get(enc.wave) === run) this.earlyMusters.delete(enc.wave);
    }
  },

  /** What each wave's muster established. Read by the run report. */
  waveMusters(this: MineflayerExecutor): ReadonlyMap<string, MusterVerdict> {
    return this.musters;
  },

  /** Every failure any muster reading of this run produced, never only the latest's. */
  musterFailures(this: MineflayerExecutor): readonly string[] {
    return this.musterFailureLog;
  },

  /**
   * Buffer a census line. Mob lines arrive before the summary that closes them
   * (one atomic function call, in emission order), so by the time a summary is
   * observed its mobs are already collected under the same sequence number.
   */
  /**
   * Buffer a muster line. Body readings arrive before the summary that closes them
   * (one atomic function call, in emission order), so by the time a summary is
   * observed its bodies are already collected under the same sequence number.
   */
  observeMuster(this: MineflayerExecutor, message: string): void {
    const body = parseMusterBody(message);
    if (body) {
      if (body.campaignId !== this.campaignId) return;
      const at = this.musterBodies.get(body.seq) ?? [];
      at.push(body);
      this.musterBodies.set(body.seq, at);
      return;
    }
    const summary = parseMusterSummary(message);
    if (!summary || summary.campaignId !== this.campaignId) return;
    this.musterSummaries.set(summary.wave, summary);
    this.musterSeq = Math.max(this.musterSeq, summary.seq);
  },
};
