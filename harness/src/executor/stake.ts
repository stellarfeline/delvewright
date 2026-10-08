// MineflayerExecutor: a stake: wagers, bounties, the stake's hardware.

import type { Vec3Tuple } from "../critical-path.ts";
import {
  dropOf,
  gateVerdict,
  markersAt,
  openWager,
  promisedForfeit,
  stagedBalance,
  termKey,
  type DeathPlan,
  type GateTerm,
  type LethalTrial,
  type StakeRule,
} from "../death-loop.ts";
import { type Hitbox } from "../crosshair.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** How far from the table's anchor the marker's own hardware is looked for. */
const MARKER_SEARCH_RADIUS = 4;

/**
 * How close the glowing display must stand to the interaction for the two to be
 * ONE stake. `stk_fill_<s>` summons both at the same position in one function, so
 * this is a tolerance on floating point and on a client's rounding of it, never a
 * search radius: at four blocks any display in the neighbourhood vouched for any
 * interaction in it.
 */
const MARKER_PAIR_RADIUS = 0.5;

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * **What this death promises each stake it names**, read from the campaign's
   * own `on_death` gates against the state in force.
   *
   * A `drop-stake` carries a `when` like every other effect, so the promise is
   * conditional and the bot has to read the condition before it can assert the
   * consequence. Read here, immediately before the walk into the volume — the
   * last moment before the death that a client can observe.
   *
   * Three outcomes, and each is written into the trial rather than folded away: a
   * gate that is open yields a wager the death FORFEITS, one the campaign has
   * shut yields a wager the death KEEPS (with the term that shut it) — both are
   * asserted against the ledger — and one that could not be read is `gateUnread`
   * and is a FAILURE: nothing was established about what this death promised, so
   * nothing may be asserted about what it took.
   */
  async wagerStakes(
    this: MineflayerExecutor,
    plan: DeathPlan,
    trial: LethalTrial,
    candidates: readonly StakeRule[],
  ): Promise<void> {
    const answers = new Map<string, boolean | undefined>();
    for (const stake of candidates) {
      for (const gate of dropOf(plan, stake.id)?.gates ?? []) {
        for (const t of gate.terms) {
          if (answers.has(termKey(t))) continue;
          answers.set(termKey(t), await this.askTerm(t));
        }
      }
    }
    const read = (t: GateTerm): boolean | undefined => answers.get(termKey(t));
    for (const stake of candidates) {
      const drop = dropOf(plan, stake.id);
      if (drop === undefined) continue;
      const verdict = gateVerdict(drop, read);
      if (verdict.kind === "open") trial.wagers.push(openWager(stake));
      else if (verdict.kind === "shut") trial.wagers.push(openWager(stake, verdict.why));
      else trial.gateUnread.push({ stake: stake.id, why: verdict.why });
    }
  },

  /**
   * **Give every wagered datum a known, non-zero balance, and read it back.**
   *
   * Staging, like the health restores, and named in `staged_removals` as a
   * `player` row. The value is `stagedBalance` of the datum's own forfeit rule, so
   * the forfeit, the stake it leaves and the collection are all asserted against
   * a purse that held something; a datum this cannot stage (not a per-player
   * ledger, or the server refused) keeps whatever it held, and a trial whose
   * forfeit is then observable only at zero is refused as UNBOUND by
   * `lethalTrialFailures`.
   */
  /**
   * **Take back the bounties a staged clear paid into a wagered purse.**
   *
   * {@link meetReseatedWaves} removes bodies by attributed blows, and a kill pays
   * its bounty: on vesperhold the walk back's six re-seated waves paid 26 tallow
   * into the purse the death had emptied, and the collection then read `0 → 36`
   * for a stake of 10. Every per-player wager whose ledger moved while the waves
   * were met — and nothing else moves it then, because the bot has not taken a
   * step — is set back to what it read before, named in `staged_removals` as a
   * `player` row.
   */
  async takeBackBounties(
    this: MineflayerExecutor,
    plan: DeathPlan,
    trial: LethalTrial,
    before: ReadonlyMap<string, number | undefined>,
  ): Promise<void> {
    const bot = this.requireBot();
    for (const w of trial.wagers) {
      const want = before.get(w.objective);
      const scope = plan.stakes.find((s) => s.id === w.stake)?.currency.scope ?? "player";
      const now = this.myScore(w.objective);
      if (want === undefined || scope !== "player" || now === want) continue;
      const refusal = await this.refusalOf(`/scoreboard players set @s ${w.objective} ${want}`);
      this.stagedRemovals.push({
        kind: "player",
        why:
          `stake datum restored for ${trial.volume}: \`${w.objective}\` (\`${w.stake}\`) set ` +
          `back from ${now ?? "?"} to ${want} — the bounties the staged clears paid into it`,
        performed: refusal === undefined,
        detail: refusal,
      });
      await this.settledScore(w.objective, want);
      process.stderr.write(
        `[death-loop] ${trial.volume}: \`${w.objective}\` read ${now ?? "?"} after the staged ` +
          `clears; set back to ${want}\n`,
      );
    }
  },

  async stageWagers(this: MineflayerExecutor, plan: DeathPlan, trial: LethalTrial): Promise<void> {
    const bot = this.requireBot();
    for (const w of trial.wagers) {
      if (!(await this.trackScore(w.objective))) continue;
      const stake = plan.stakes.find((s) => s.id === w.stake);
      const scope = stake?.currency.scope ?? "player";
      const want = stagedBalance(w.forfeit);
      if (scope === "player") {
        const refusal = await this.refusalOf(`/scoreboard players set @s ${w.objective} ${want}`);
        this.stagedRemovals.push({
          kind: "player",
          why:
            `stake datum staged for ${trial.volume}: \`${w.objective}\` (\`${w.stake}\`) set to ` +
            `${want}, so the ${w.forfeits ? `declared forfeit (${w.forfeit.kind})` : "death, whose gate keeps it,"} ` +
            `takes ${promisedForfeit(w, want)}`,
          performed: refusal === undefined,
          detail: refusal,
        });
        w.balanceBefore = await this.settledScore(w.objective, want);
      } else {
        w.balanceBefore = this.myScore(w.objective);
      }
      if (w.balanceBefore !== undefined) {
        w.expectedForfeit = promisedForfeit(w, w.balanceBefore);
      }
      process.stderr.write(
        `[death-loop] ${trial.volume}: \`${w.objective}\` holds ${w.balanceBefore ?? "?"} ` +
          `${scope === "player" ? `(staged to ${want})` : `(a ${scope} ledger — not staged)`}; ` +
          `the death should take ${w.expectedForfeit ?? "?"}\n`,
      );
    }
  },

  /**
   * The recovery stake's own hardware standing at `anchor`: the `interaction` box
   * a player right-clicks, provided the glowing `item_display` that says there is
   * something here is standing with it.
   *
   * Both halves are required, and that is the assertion rather than a convenience:
   * spec-0032 declares the stake to be an interaction for the hitbox AND a glowing
   * item display for the rendering, so an invisible hitbox is a stake no player
   * would ever find, not a stake that happens to render oddly.
   *
   * WHICH of them is the stake is {@link markersAt}'s rule, not this method's —
   * the executor supplies the observation and the pure module decides, so the
   * reading can be put to a test without a server in front of it.
   *
   * **Every one of them, not the nearest.** A death leaves ONE place, so a second
   * marker standing at the anchor is a defect the run has to be able to state; and
   * a client cannot state it by acquiring the nearest, because coincident boxes
   * have no nearest — the pick is an exact tie the server resolves by iteration
   * order. Counting is the only observation available from this side.
   */
  stakeHardwareAt(this: MineflayerExecutor, anchor: Vec3Tuple): Hitbox[] {
    const bot = this.bot;
    if (!bot) return [];
    const displays = Object.values(bot.entities).flatMap((e) =>
      e?.position && e.name === "item_display"
        ? [{ name: "item_display", position: { x: e.position.x, y: e.position.y, z: e.position.z } }]
        : [],
    );
    return markersAt(
      this.hitboxesNear(anchor, MARKER_SEARCH_RADIUS),
      displays,
      anchor,
      MARKER_SEARCH_RADIUS,
      MARKER_PAIR_RADIUS,
    );
  },
};
