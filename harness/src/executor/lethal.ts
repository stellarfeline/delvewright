// MineflayerExecutor: the death loop: a lethal volume's trials.

import type { KillStep, Vec3Tuple } from "../critical-path.ts";
import { BotDeathError, formatDeathPos } from "../death.ts";
import { reseatedWaves, type Encounter } from "../combat.ts";
import {
  bodyInVolume,
  entryCellOf,
  overFootprint,
  sinkBudgetMs,
  wayInCandidates,
  firstWayIn,
  inBox,
  volumeGateVerdict,
  volumeIsStaged,
  lethalStepCost,
  nearLip,
  openLethalTrial,
  seatAtRespawn,
  stakesDropped,
  tableAnchor,
  termKey,
  type Box,
  type DeathPlan,
  type LethalTrial,
} from "../death-loop.ts";
import { delay } from "./connection.ts";
import { LEDGER_POLL_MS } from "./score.ts";
import { GATE_DASH_TICK_MS } from "./timed-gate.ts";
import { Movements } from "./walk.ts";
import type { MineflayerExecutor } from "../executor.ts";

// --- the death loop ---------------------------------------------
/**
 * How long to wait, after stepping into a declared lethal volume, for the player
 * to actually die. A volume's driver runs in the campaign `tick`, so the kill is
 * one tick away; this is a generous ceiling on "one tick", not a guess at how long
 * dying takes.
 */
const LETHAL_DEATH_TIMEOUT_MS = 10_000;

/** How long to wait for the collected stake's hardware to be retired. */
const MARKER_RETIRE_TIMEOUT_MS = 5_000;

/**
 * How long to wait, at the anchor, for the hardware the death promised to be on
 * the client.
 *
 * A separate wait from {@link MARKER_RETIRE_TIMEOUT_MS} because it answers a
 * different question, and it exists because the question was being answered by a
 * PROXY. The read used to follow `awaitEntitySettle`, which waits for the count
 * of tracked non-player entities to stop changing — and a count can be perfectly
 * steady at one while the stake's own pair (an `interaction` and the glowing
 * `item_display` that stands with it) is still on its way over the wire. The
 * trial then reported "no recovery stake stands at the anchor" about a stake that
 * did, intermittently, on whichever volume the run happened to reach first.
 *
 * An intermittent red is an under-specified test: the thing to wait for is the
 * object the death promised, not a proxy for the client being quiet. A stake that
 * is really absent still reds — after this deadline, with the same sentence.
 */
const MARKER_PLACE_TIMEOUT_MS = 10_000;

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  useDeathPlan(this: MineflayerExecutor, plan: DeathPlan): void {
    this.deathPlan = plan;
    // The DECLARED regions, not the keep-out boxes, and the difference is a
    // defect a clean auto-merge produced rather than a preference.
    // `applyLethalExclusion` widens whatever it is handed, by the harness's own
    // mirror of the server's rule (`volumeReachesCell`); the plan's `keep_out`
    // is the compiler's answer to the same widening, already applied. Handing
    // the second to the first widens twice and excludes a shell the compiler
    // never refused — walkable ground the bot would then report as no path.
    // One box, one widening. `keep_out` stays the contract's own answer and is
    // held against `bodyInVolume` by `death-loop.test.ts`; which of the two
    // rules the NAVIGATOR should read is one question, and it is answered here
    // by the wider of the two — it excludes everything the keep-out box does,
    // plus the course at `hi.y + 1`, so nothing this branch proved is lost.
    //
    // spec-0088: only the volumes live from world-load are excluded for the whole
    // run. A volume live from a story stage is excluded per walk leg, by asking
    // its gate before the leg — a leg walked before the flip crosses the cells
    // the proof crossed, and a leg after goes round them.
    this.unstagedBoxes = plan.volumes.filter((v) => !volumeIsStaged(v)).map((v) => v.region);
    this.stagedVolumes = plan.volumes.filter(volumeIsStaged);
    this.lethalBoxes = [...this.unstagedBoxes, ...this.stagedVolumes.map((v) => v.region)];
  },

  /**
   * **Which staged volumes are live for the next walk leg** (spec-0088 §8): put
   * every term of each staged volume's gate to the server through the one
   * term-asking the stake trial uses ({@link askTerm}), and exclude the region
   * of every volume whose gate reads open. A term the server answers neither way
   * excludes the region — the conservative direction — and a `[lethal]` line
   * says so.
   */
  async refreshStagedExclusion(this: MineflayerExecutor): Promise<void> {
    if (this.stagedVolumes.length === 0) return;
    const answers = new Map<string, boolean | undefined>();
    const live: Box[] = [];
    const states: string[] = [];
    for (const v of this.stagedVolumes) {
      for (const t of v.gate.terms) {
        if (!answers.has(termKey(t))) answers.set(termKey(t), await this.askTerm(t));
      }
      const verdict = volumeGateVerdict(v, (t) => answers.get(termKey(t)));
      if (verdict.kind === "unread") {
        process.stderr.write(
          `[lethal] ${v.id}: ${verdict.why}; excluded from this leg as if live\n`,
        );
      }
      if (verdict.kind !== "shut") live.push(v.region);
      states.push(`${v.id} ${verdict.kind === "shut" ? "shut" : verdict.kind === "open" ? "live" : "unread"}`);
    }
    this.lethalBoxes = [...this.unstagedBoxes, ...live];
    // The binding of the per-leg exclusion, stated on every leg it ran for.
    process.stderr.write(
      `[lethal] before the walk to ${this.walkLabel}: ${states.join(", ")} — ` +
        `${live.length} of ${this.stagedVolumes.length} staged volume(s) excluded\n`,
    );
  },

  /** Every walk into a lethal volume this run made, and what it observed. */
  deathLoopTrials(this: MineflayerExecutor): readonly LethalTrial[] {
    return this.lethalTrials;
  },

  /** How many of {@link deathLoopTrials} ran to their own end. */
  deathLoopTrialsFinished(this: MineflayerExecutor): number {
    return this.lethalTrialsFinished;
  },

  /** Why the stage did not run, when it did not. `undefined` means it ran. */
  deathLoopSkipReason(this: MineflayerExecutor): string | undefined {
    return this.deathLoopSkip;
  },

  /**
   * Make every cell a declared lethal volume can KILL IN impassable to the
   * pathfinder — which is not the same set as the cells inside the volume.
   *
   * The server adjudicates the volume against a body's whole hitbox, so it kills
   * a player whose feet cell is one outside the box ({@link volumeReachesCell},
   * the cell-shaped reading of {@link bodyInVolume}). Excluding only the box left
   * the shell of cells the volume still reaches open to every walk, and the bot
   * duly died on one: the gallery's east-pit trial opened with the bot parked at
   * `[2, 65, 5]` and it was killed at `[3.85, 65.00, 5.14]` by the WEST pit,
   * which the stage then had to report against the east pit's declared volume.
   */
  applyLethalExclusion(this: MineflayerExecutor, movements: InstanceType<typeof Movements>): void {
    if (this.lethalExclusionSuspended || this.lethalBoxes.length === 0) return;
    const boxes = this.lethalBoxes;
    movements.exclusionAreasStep.push((block): number => lethalStepCost(block, boxes));
  },

  /**
   * **The stage.** Walk into every declared lethal volume, die there, and assert
   * every consequence the campaign promised from what was OBSERVED.
   *
   * One trial per declared volume, and the loop is self-restoring: collecting the
   * stake puts the purse back, so the next volume's trial starts from a full
   * ledger rather than from a zero the previous death left.
   */
  async runDeathLoop(this: MineflayerExecutor): Promise<void> {
    this.stageNow = "death-loop";
    const plan = this.deathPlan;
    if (plan === undefined) {
      this.deathLoopSkip = "this build ships no validation/death-plan.json";
      return;
    }
    if (plan.binding.unbound) {
      this.deathLoopSkip = plan.binding.reason ?? "the build's death plan binds to nothing";
      return;
    }
    try {
      for (const volume of plan.volumes) {
        await this.lethalTrial(plan, volume);
        this.lethalTrialsFinished += 1;
      }
    } finally {
      this.clearScoreDisplay();
    }
  },

  /**
   * **A death-loop walk — the approach to a volume's near lip, or the walk back
   * to the place a death left — meeting what stands on the way the critical path
   * meets an encounter.**
   *
   * Neither is a leg the compiler measured, and the deaths before each (the
   * die-retry stage's before the approach, the trial's own before the walk back)
   * re-seat every `respawns_on_rest` wave. Every wave the delve has put back since
   * this run cleared it ({@link reseatedWaves}) is read if its seating is owed a
   * reading and staged away, by {@link meetReseatedWaves}, BEFORE the walk: the
   * damage handlers alone meet a body only once it has hit, and on vesperhold
   * `wave/unremembered-guard` killed the bot on the approach and took it to 6.3
   * health on the walk back.
   *
   * A death on the way is returned as nothing — the caller counts deaths and
   * judges where it happened. A wave that could not be staged away is
   * `staging`; a destination the pathfinder could not reach is `walk`.
   */
  async walkPastReseated(
    this: MineflayerExecutor,
    label: string,
    dest: Vec3Tuple | undefined,
    destName: string,
    purse?: { readonly plan: DeathPlan; readonly trial: LethalTrial },
  ): Promise<{ readonly kind: "staging" | "walk"; readonly why: string } | undefined> {
    const deaths = this.deathSeq;
    const ledger = new Map(
      (purse?.trial.wagers ?? []).map((w) => [w.objective, this.myScore(w.objective)] as const),
    );
    try {
      await this.meetReseatedWaves(label);
      if (purse !== undefined) await this.takeBackBounties(purse.plan, purse.trial, ledger);
    } catch (err) {
      if (err instanceof BotDeathError) return undefined;
      return {
        kind: "staging",
        why:
          `a wave the delve put back could not be staged away before the walk: ` +
          `${err instanceof Error ? err.message : String(err)}`,
      };
    }
    if (this.deathSeq !== deaths || dest === undefined) return undefined;
    try {
      await this.walkTo(dest, 1, label);
    } catch (err) {
      if (!(err instanceof BotDeathError)) {
        return {
          kind: "walk",
          why:
            `${destName} [${dest.join(", ")}] could not be reached: ` +
            `${err instanceof Error ? err.message : String(err)}`,
        };
      }
    }
    return undefined;
  },

  /**
   * The walk back from the respawn seat to the place a death left, through
   * {@link walkPastReseated} — the death that respawned the bot re-seated every
   * `respawns_on_rest` wave. `false` when it did not arrive; a death on the way or
   * a wave it could not stage away is the trial's `walkBackFailure`.
   */
  async walkBackToStake(
    this: MineflayerExecutor,
    plan: DeathPlan,
    volume: string,
    trial: LethalTrial,
    anchor: Vec3Tuple,
  ): Promise<boolean> {
    const deathsBack = this.deathSeq;
    const back = await this.walkPastReseated(
      `death-loop walk back to the place at the near lip`,
      anchor,
      "the place at the near lip",
      { plan, trial },
    );
    if (back !== undefined || this.deathSeq !== deathsBack) {
      const died = this.deathSeq !== deathsBack ? this.lastDeath : undefined;
      if (died !== undefined || back?.kind === "staging") {
        // The walk back met something on the way, which says nothing about where
        // the stake was placed: its own failure, never "the walk back could not
        // be made".
        trial.walkBackFailure =
          died !== undefined
            ? `the bot died at ${formatDeathPos(died.position)} on the walk back from the ` +
              `respawn seat to [${anchor.join(", ")}]` +
              `${died.likelyCause ? ` (${died.likelyCause})` : ""}`
            : back!.why;
      }
      process.stderr.write(
        `[death-loop] ${volume}: the walk back to [${anchor.join(", ")}] failed: ` +
          `${trial.walkBackFailure ?? back?.why ?? "?"}\n`,
      );
      return false;
    }
    trial.walkedBack = true;
    return true;
  },

  /**
   * Read and stage away every wave the delve has put back since this run cleared
   * it — the critical path's own acts on an encounter ({@link musterUnlessRead},
   * then {@link stageClear}), with the anchor's chunk held as the kill step holds
   * it, because these anchors are not where the bot stands. A wave whose census
   * answers that nothing of it stands is recorded as down and not read. Returns
   * the waves met.
   */
  async meetReseatedWaves(this: MineflayerExecutor, label: string): Promise<string[]> {
    const plan = this.combatPlan;
    if (!plan) return [];
    const met: string[] = [];
    // Nearest the bot first: a wave is staged while the bot stands still, and the
    // nearest one is the one that walks to it meanwhile — on vesperhold a Guard hit
    // the bot while five farther waves were staged ahead of it in plan order.
    // `sort` is stable, so equal distances keep the plan's order (ADR-0006).
    const here = this.feetCell() ?? [0, 0, 0];
    const dist = (e: Encounter): number =>
      Math.hypot(e.pos[0] - here[0], e.pos[1] - here[1], e.pos[2] - here[2]);
    const waves = reseatedWaves(plan.encounters, this.clearedEpoch, this.seatEpoch).sort(
      (a, b) => dist(a) - dist(b),
    );
    for (const enc of waves) {
      process.stderr.write(
        `[staged] ${label}: \`${enc.wave}\` has been put back since this run cleared it — ` +
          `reading it and staging it away before the walk\n`,
      );
      const fight: KillStep = {
        action: "kill",
        objective: enc.objective,
        wave: enc.wave,
        pos: enc.pos,
        tag: "",
        count: enc.count,
      };
      await this.holdChunk(enc.pos, true);
      try {
        // The seating count says a respawn COULD have put the wave back, and a
        // respawn at a checkpoint that is no fire puts back nothing: the gallery's
        // death loop respawns at `anchor/lectern`, and a muster taken there read
        // 0 of 3 and filed the declaration as unverified. The census asks the
        // server whether anything of the wave stands before anything is read.
        const standing = (await this.census(enc))?.summary.present;
        if (standing === 0) {
          process.stderr.write(
            `[staged] ${label}: nothing of \`${enc.wave}\` stands — no seating to read or stage\n`,
          );
        } else {
          await this.musterUnlessRead(enc);
          await this.stageClear(fight, enc);
          met.push(enc.wave);
        }
      } finally {
        await this.holdChunk(enc.pos, false);
      }
      this.recordCleared(enc.wave);
    }
    return met;
  },

  /** One volume: approach, step in, die, and assert the aftermath. */
  async lethalTrial(this: MineflayerExecutor, plan: DeathPlan, volume: DeathPlan["volumes"][number]): Promise<void> {
    const bot = this.requireBot();
    // A trial NEVER opens over an unrecovered death. `stepInto` rethrows the death
    // latch on its first line, so a bot still lying dead from the previous trial's
    // walk back never takes a step, never dies again, and the trial then reports
    // that the bot stood in this volume and survived it — a verdict about a delve,
    // produced by the harness leaving its own bot on the death screen. The
    // previous trial's `deathPos` would also be read as this one's.
    if (this.death !== undefined) {
      process.stderr.write(
        `[death-loop] ${volume.id}: the bot was still dead when this trial opened — ` +
          `recovering before the approach, because a corpse cannot walk into anything\n`,
      );
      await this.recoverFromDeath();
    }
    // spec-0088: a volume live from a story stage is entered only when its gate
    // reads open. Shut, the trial records why and is not entered; unread, it
    // establishes nothing, and that is the trial's failure.
    if (volumeIsStaged(volume)) {
      const answers = new Map<string, boolean | undefined>();
      for (const t of volume.gate.terms) {
        if (!answers.has(termKey(t))) answers.set(termKey(t), await this.askTerm(t));
      }
      const verdict = volumeGateVerdict(volume, (t) => answers.get(termKey(t)));
      if (verdict.kind !== "open") {
        const trial = openLethalTrial(volume, volume.region.lo, []);
        if (verdict.kind === "shut") {
          trial.notLiveAtTrial = verdict.why;
          process.stderr.write(
            `[death-loop] ${volume.id}: not live at trial — ${verdict.why}; not entered\n`,
          );
        } else {
          trial.abandoned = `whether this staged volume is live could not be read — ${verdict.why}`;
        }
        this.lethalTrials.push(trial);
        return;
      }
    }
    const here = this.feetCell() ?? [0, 0, 0];
    const entryCell = entryCellOf(volume.region, here, (c) => this.bodyCanOccupy(c));
    // EVERY stake this death is supposed to leave. `on_death`'s own declaration
    // decides which — never "the first one declared" — and all of them are
    // asserted, because a death that forfeits four datums promises four things and
    // leaves them at one place.
    const candidates = stakesDropped(plan);
    if (entryCell === undefined) {
      // Every cell of the declared box is filled by a block. That is a finding
      // about the campaign — nothing can ever die in this volume — and it is
      // stated as one rather than by driving at a wall for ten seconds.
      const trial = openLethalTrial(volume, volume.region.lo, []);
      trial.abandoned =
        `no cell of the declared volume [${volume.region.lo.join(", ")}]..` +
        `[${volume.region.hi.join(", ")}] can hold a body: every one of them is filled by a ` +
        `block, so nothing can ever be inside this volume for it to kill`;
      this.lethalTrials.push(trial);
      return;
    }
    const trial = openLethalTrial(volume, entryCell, []);
    this.lethalTrials.push(trial);
    // What this death PROMISES each stake: each `drop-stake` carries its own
    // `when`, so a stake under an open gate is forfeited and one under a shut gate
    // is kept — and both are asserted.
    await this.wagerStakes(plan, trial, candidates);
    for (const w of trial.wagers.filter((x) => !x.forfeits)) {
      process.stderr.write(
        `[death-loop] ${volume.id}: \`${w.stake}\` is KEPT by this death — ${w.keptBecause}; ` +
          `its ledger is asserted unchanged across the death\n`,
      );
    }
    for (const g of trial.gateUnread) {
      process.stderr.write(
        `[death-loop] ${volume.id}: \`${g.stake}\` — ${g.why}; this trial cannot assert it\n`,
      );
    }
    // The near lip: the cell the placement table already proved is the reachable
    // point nearest this volume. Nothing new is computed — it is the anchor a
    // death here would leave its stake at, which is the same question as "where
    // does a player stand next to this".
    const lip = nearLip(plan, volume.id);
    process.stderr.write(
      `[death-loop] ${volume.id}: standing at [${here.join(", ")}]; walking into ` +
        `[${entryCell.join(", ")}] to die there via the near lip ` +
        `${lip ? `[${lip.join(", ")}]` : "(none — the build declares no placement row)"}` +
        `${
          trial.wagers.length > 0
            ? `; expecting ${trial.wagers.length} wager(s): ${trial.wagers
                .map((w) => `${w.stake} on ${w.objective} (${w.forfeits ? "forfeit" : "kept"})`)
                .join(", ")}`
            : ""
        }\n`,
    );

    // --- the ledger before, for EVERY datum this death takes ----------------
    // Staged to a known balance, so the forfeit takes something (see
    // `stagedBalance`), and staged again at the lip below: a body the approach
    // stages away pays its bounty into the same purse.
    await this.stageWagers(plan, trial);

    // --- the walk toward the volume ----------------------------------------
    // Armed BEFORE the approach, not between the approach and the step in. The
    // observation is "the player entered the box and died", and which of the two
    // legs delivered them there is the harness's business, not the delve's: the
    // approach ends one block from a hazard, and a pathfinder that overshoots by a
    // block has still produced exactly the event under test. Attributing that to
    // "the lip could not be reached" is how a real, correct death got reported as
    // an infrastructure fault on this stage's first live run.
    //
    // The guard that keeps it honest is the POSITION check below: a death anywhere
    // outside the declared box is not this volume's death and is not credited.
    this.wordWatch = { needle: volume.message, seen: false };
    const deathsBefore = this.deathSeq;
    let navFault: string | undefined;
    const fault = await this.walkPastReseated(
      `death-loop approach to ${volume.id}`,
      lip,
      "the near lip",
    );
    if (fault?.kind === "staging") {
      // A wave on the way that could not be removed: walking on into it is how the
      // bot died here before, so the trial stops and says why.
      this.wordWatch = undefined;
      trial.approachFailure = fault.why;
      return;
    }
    if (fault?.kind === "walk") navFault = fault.why;
    // Every death from here to the step in is the approach's — unless the body is
    // in the volume's reach, which is the event under test (see above).
    const approachDeaths = this.deathSeq;
    if (this.bodyInside(volume.region)) trial.enteredVolume = true;
    if (navFault === undefined && this.deathSeq === deathsBefore) {
      await this.stageWagers(plan, trial);
    }
    // The one leg of the whole run that is ALLOWED into the hazard — skipped when
    // the approach already delivered the death.
    if (navFault === undefined && this.deathSeq === deathsBefore) {
      this.lethalExclusionSuspended = true;
      try {
        const walkIn = await this.stepInto(volume.region, entryCell, trial);
        if (walkIn === "blocked" && this.deathSeq === deathsBefore) {
          // A death on the way to a way in ends the search: it is the trial's
          // death to judge, and no further walk in follows it.
          await this.jumpInApproach(volume.region, volume.id, () =>
            this.deathSeq === deathsBefore
              ? this.stepInto(volume.region, entryCell, trial)
              : Promise.resolve("died" as const),
          );
        }
      } catch (err) {
        if (!(err instanceof BotDeathError)) {
          navFault =
            `the walk into [${entryCell.join(", ")}] failed for a reason that is not a death: ` +
            `${err instanceof Error ? err.message : String(err)}`;
        }
      } finally {
        this.lethalExclusionSuspended = false;
        // Whatever ended the walk in, a gate it opened does not stay open.
        await this.restoreOpenedGates(volume.id);
      }
    }

    // --- the death ---------------------------------------------------------
    const observed =
      navFault !== undefined
        ? false
        : await this.waitFor(
            () => this.deathSeq > deathsBefore,
            LETHAL_DEATH_TIMEOUT_MS,
            LEDGER_POLL_MS,
          ).catch((err: unknown) => {
            if (err instanceof BotDeathError) return true;
            throw err;
          });
    trial.deathPos = this.death?.position ? [...this.death.position] : undefined;
    trial.wordingSeen = this.wordWatch?.seen === true;
    this.wordWatch = undefined;
    // Credited when the volume's own selector would have matched the body —
    // `bodyInVolume`, the server's rule, asked of the exact position. That is
    // the certain case in both directions, so it neither invents a kill nor
    // disowns one, and it is a sharper answer than this branch's first one
    // (`inBox(deathPos, keepOut)`), which asked whether SOME position in the
    // body's cell could have met the volume.
    const inside = trial.deathPos !== undefined && bodyInVolume(trial.deathPos, volume.region);
    // …and whether the body was ever IN the volume is a different question,
    // asked of the feet.
    //
    // **The two came apart the moment credit stopped meaning cell containment,
    // and the inference `enteredVolume = inside` did not.** A body killed from
    // one cell out from a face is matched by the selector and never had its feet
    // in the hole — so reading `inside` here reports it as having stood in the
    // volume, which is exactly the distinction
    // {@link LethalTrial.enteredVolume} was added to make (*the volume did not
    // kill what was in it* against *nothing was ever in it*). The walk sets it
    // from `feetInside(volume.region)`, a cell test, and so does this: the block
    // a body is in is the FLOOR of its position.
    if (observed && trial.deathPos !== undefined) {
      const feet = trial.deathPos.map(Math.floor) as unknown as Vec3Tuple;
      if (inBox(feet, volume.region)) trial.enteredVolume = true;
    }
    trial.died = observed && inside;
    if (navFault !== undefined) {
      trial.abandoned = navFault;
      return;
    }
    if (observed && !inside && approachDeaths > deathsBefore) {
      const died = this.lastDeath;
      trial.approachFailure =
        `the bot died at ${formatDeathPos(died?.position)} on its way to ` +
        `${lip ? `the near lip [${lip.join(", ")}]` : "the volume"}, outside the reach of the ` +
        `declared volume [${volume.region.lo.join(", ")}]..[${volume.region.hi.join(", ")}]` +
        `${died?.likelyCause ? ` (${died.likelyCause})` : ""}`;
      return;
    }
    if (observed && !inside) {
      trial.abandoned =
        `the bot died at ${formatDeathPos(trial.deathPos)}, which is OUTSIDE the reach of ` +
        `the declared volume [${volume.region.lo.join(", ")}]..[${volume.region.hi.join(", ")}] ` +
        `— a body there is not one this volume's own selector can match, so that death is ` +
        `not this volume's kill and is not credited as one`;
      return;
    }
    if (!trial.died) {
      // Nothing downstream is meaningful, and every field stays at its honest
      // default so the report cannot read as if it had checked them.
      process.stderr.write(
        trial.enteredVolume
          ? `[death-loop] ${volume.id}: the bot is standing in the volume and is still alive\n`
          : `[death-loop] ${volume.id}: the bot never got its feet inside the volume, so the ` +
            `volume was not exercised — this says nothing about whether it kills\n`,
      );
      return;
    }
    process.stderr.write(
      `[death-loop] ${volume.id}: died at ${formatDeathPos(trial.deathPos)}` +
        `; the volume's own line ${trial.wordingSeen ? "reached" : "did NOT reach"} the player\n`,
    );

    // --- the respawn -------------------------------------------------------
    await this.recoverFromDeath();
    const p = bot.entity?.position;
    trial.respawnPos = p ? [p.x, p.y, p.z] : undefined;
    const seat =
      trial.respawnPos === undefined ? undefined : seatAtRespawn(plan.seats, trial.respawnPos);
    trial.respawnSeat = seat === undefined ? undefined : plan.seats[seat]?.label;
    if (seat !== undefined) {
      trial.expectedAnchor = tableAnchor(plan, seat, volume.id);
    }
    process.stderr.write(
      `[death-loop] ${volume.id}: respawned at ` +
        `${trial.respawnPos ? `[${trial.respawnPos.map((n) => n.toFixed(2)).join(", ")}]` : "?"} ` +
        `(${trial.respawnSeat ?? "NO declared seat"})\n`,
    );

    if (trial.wagers.length === 0) return;

    // --- the forfeit, for every datum — and the keep, for every kept one -----
    for (const w of trial.wagers) {
      if (w.balanceBefore !== undefined) {
        w.balanceAfterDeath = await this.settledScore(
          w.objective,
          w.balanceBefore - (w.expectedForfeit ?? 0),
        );
      }
    }
    // A death that keeps every datum leaves no stake anywhere: nothing to walk
    // back to, and nothing to collect.
    if (!trial.wagers.some((w) => w.forfeits)) return;

    // --- the walk back, and the stake at the end of it ----------------------
    const anchor = trial.expectedAnchor;
    if (anchor === undefined) return;
    if (!(await this.walkBackToStake(plan, volume.id, trial, anchor))) return;
    await this.awaitEntitySettle();
    // Wait for the hardware this death PROMISED, not for the client to go quiet.
    // See {@link MARKER_PLACE_TIMEOUT_MS}: the settle is a proxy and it can be
    // satisfied while the stake's pair is still in flight.
    await this.waitFor(
      () => this.stakeHardwareAt(anchor).length > 0,
      MARKER_PLACE_TIMEOUT_MS,
      LEDGER_POLL_MS,
    );
    // Every marker standing at the anchor, not the nearest: a death leaves ONE
    // place, and two coincident boxes have no nearest — the pick is an exact tie.
    const markers = this.stakeHardwareAt(anchor);
    trial.markersFound = markers.length;
    const marker = markers[0];
    trial.markerPos = marker ? [marker.position.x, marker.position.y, marker.position.z] : undefined;
    if (!marker) return;
    if (markers.length > 1) {
      process.stderr.write(
        `[death-loop] ${volume.id}: ${markers.length} recovery-stake markers stand at ` +
          `[${anchor.join(", ")}] — one death leaves ONE place, and coincident interaction ` +
          `boxes are a ray-pick tie the client resolves by iteration order\n`,
      );
    }

    // --- the collection, twice in one tick ---------------------------------
    // AC6 is *idempotent under a double right-click in one tick*, so the two
    // clicks are issued back to back in one event-loop turn — the client's own way
    // of putting two `use_entity` packets on the wire inside one server tick — and
    // the assertion is on the OUTCOME: the purse must end at exactly what it held
    // before the death, never twice the stake.
    //
    // **Stated as the limitation it is** (playtest-methodology rule 1: a gate must
    // not claim more than it bound). Whether the server ADJUDICATED both clicks in
    // one tick is not observable from a client, and measurement says it usually
    // does not: the collect rides an interaction advancement, and vanilla grants an
    // advancement at most once per tick, so the second packet is normally absorbed.
    // `collect_clicks` therefore counts packets SENT, not collections adjudicated,
    // and this exercise is best-effort. What is NOT best-effort is the outcome it
    // is checked against — a purse that grew twice, or a marker still standing
    // afterwards, is caught either way, and the second is what actually caught a
    // deliberately non-idempotent `stk_take` in the red demonstration.
    const target = bot.entities[marker.id];
    if (target) {
      const eye = bot.entity.position;
      try {
        await bot.lookAt(
          eye.offset(
            marker.position.x - eye.x,
            marker.position.y - eye.y,
            marker.position.z - eye.z,
          ),
          true,
        );
      } catch {
        // a look failure must not be reported as a collection failure
      }
      const clicks = [bot.activateEntity(target), bot.activateEntity(target)];
      trial.collectClicks = clicks.length;
      await Promise.allSettled(clicks);
    }
    // EVERY datum comes back from the one press: the place is offered to every
    // stake the death left a wager at.
    for (const w of trial.wagers) {
      w.balanceAfterCollect =
        w.balanceBefore === undefined
          ? this.myScore(w.objective)
          : await this.settledScore(w.objective, w.balanceBefore);
    }
    trial.markerRetired = await this.waitFor(
      () => this.stakeHardwareAt(anchor).length === 0,
      MARKER_RETIRE_TIMEOUT_MS,
      LEDGER_POLL_MS,
    );
    process.stderr.write(
      `[death-loop] ${volume.id}: collected — ${trial.wagers
        .map((w) => `${w.stake} ${w.balanceAfterDeath ?? "?"} → ${w.balanceAfterCollect ?? "?"}`)
        .join("; ")}; marker ${trial.markerRetired ? "retired" : "STILL STANDING"}\n`,
    );
  },

  /**
   * Put the bot into `box` the way a player meets it, having already walked to
   * its lip: walk in until nothing is underfoot over the volume, then let go.
   *
   * The pathfinder cannot be asked for this: its nearest goal is a range of one
   * block, so it parks the bot beside a one-cell hazard and calls it arrived. Raw
   * forward drive is the mechanism this file already trusts against exact cells
   * (the timed-gate dash, the unstick burst) and it is what a player pressing W
   * does — **until the body is over the volume with nothing under it**
   * ({@link overFootprint}, and not on the ground). From there a player who does
   * nothing is carried in by the game: a pit is fallen into, and a submerged
   * volume is sunk into, at {@link SINK_BLOCKS_PER_TICK}. Driving on is not what
   * a player does, and in water it can lift the body: a horizontal collision is
   * the climb-out-of-water impulse. The volume's own selector is what then kills
   * the body; nothing here moves it but the game.
   *
   * A released body that lands on something outside the volume — the rim it
   * overhung as it stepped down (measured on the gallery's west pit: released at
   * y 69.92 mid-step, it came to rest at 69.00 on the lip's edge) — is standing
   * again, and it walks on, as a player at the edge of a hole would.
   *
   * Throws whatever the walk threw — including the {@link BotDeathError} that is
   * the whole point. It also RECORDS what it saw: the drive can run its deadline
   * out with the body still outside the box — a wall in the way, a cell no body
   * fits in — and it returns normally when it does, so the only thing separating
   * "the volume did not kill what was in it" from "nothing ever got in" is the
   * flag.
   */
  async stepInto(
    this: MineflayerExecutor,
    box: Box,
    cell: Vec3Tuple,
    trial: LethalTrial,
  ): Promise<"entered" | "released" | "blocked"> {
    const bot = this.requireBot();
    const inside = (): boolean => {
      if (!this.bodyInside(box)) return false;
      trial.enteredVolume = true;
      return true;
    };
    if (inside()) return "entered";
    const released = (): boolean => {
      const p = bot.entity.position;
      return !bot.entity.onGround && overFootprint([p.x, p.y, p.z], box);
    };
    // The pathfinder's own rule for what a walk may open (a non-iron gate), read
    // off the Movements the approach just walked with, so the walk in and every
    // other walk agree on it.
    const openable: ReadonlySet<number> =
      (bot.pathfinder as { movements?: { openable?: ReadonlySet<number> } }).movements
        ?.openable ?? new Set<number>();
    const opened = new Map<string, number>();
    // The drive's own deadline counts DRIVING time only: a body carried in by
    // the game is on the sink's clock, not this one.
    let driveLeft = LETHAL_DEATH_TIMEOUT_MS;
    for (;;) {
      const until = Date.now() + driveLeft;
      try {
        while (Date.now() < until && !inside() && !released()) {
          if (this.death) throw this.death;
          await this.openGateAhead(cell, openable, opened, trial.volume);
          const p = bot.entity.position;
          try {
            await bot.lookAt(p.offset(cell[0] + 0.5 - p.x, 0, cell[2] + 0.5 - p.z), true);
          } catch {
            // best effort — a look failure must not abort the step
          }
          bot.setControlState("forward", true);
          await delay(GATE_DASH_TICK_MS);
        }
      } finally {
        bot.clearControlStates();
      }
      driveLeft = until - Date.now();
      if (!inside() && !released()) {
        const p = bot.entity.position;
        process.stderr.write(
          `[death-loop] ${trial.volume}: the walk in ended at ` +
            `[${p.x.toFixed(2)}, ${p.y.toFixed(2)}, ${p.z.toFixed(2)}] ` +
            `(${bot.entity.onGround ? "standing" : "off the ground"}) with the body neither in ` +
            `the volume nor over it\n`,
        );
      }
      if (inside()) return "entered";
      if (!released()) return "blocked";
      // A body that lands on something outside the volume — a rim it overhung
      // when it stepped down, a ledge in the shaft — is standing again, and a
      // player standing at the edge of a hole walks on.
      if (!(await this.sinkInto(box, trial, inside))) return inside() ? "entered" : "released";
      if (driveLeft <= 0) return "blocked";
    }
  },

  /**
   * **When walking straight in is blocked, get to a place a player jumps to,
   * and walk in from there.**
   *
   * The placement table's lip is the reachable cell nearest the volume by
   * WALKING, and a hazard can be one a player reaches only with a jump:
   * vesperhold's well is entered over a dry cut in front of its curb, onto a
   * sill the choir cannot reach, and the lip is the floor of that cut. From
   * there a straight drive meets a sill a block and a half up. A player climbs
   * back out and jumps across; the pathfinder, which jumps gaps the way a
   * player does, is asked for that: the standable cells within a few blocks
   * that are nearer the volume than the body is, outside what the volume can
   * reach, smallest climb first ({@link wayInCandidates}), EVERY one tried in
   * turn until the walk in from one is not blocked ({@link firstWayIn}). The
   * walk to a candidate is an ordinary walk, with the hazard excluded; the walk
   * in from it (`walkIn`) runs with the exclusion suspended, as the first did.
   */
  async jumpInApproach(
    this: MineflayerExecutor,
    box: Box,
    volume: string,
    walkIn: () => Promise<"entered" | "released" | "blocked" | "died">,
  ): Promise<"entered" | "released" | "blocked" | "died"> {
    const bot = this.requireBot();
    const feet = this.feetCell();
    if (!feet) return "blocked";
    const candidates = wayInCandidates(feet, box, (c) => {
      if (!this.bodyCanOccupy(c)) return false;
      const p = bot.entity.position;
      const below = bot.blockAt(p.offset(c[0] - p.x, c[1] - 1 - p.y, c[2] - p.z));
      return below !== null && below.boundingBox === "block";
    });
    process.stderr.write(
      `[death-loop] ${volume}: the walk in is blocked at [${feet.join(", ")}]; ` +
        `${candidates.length} way-in cell(s) nearer the volume, smallest climb first: ` +
        `${candidates.map((c) => `[${c.join(", ")}]`).join(" ")}\n`,
    );
    const outcome = await firstWayIn(
      candidates,
      async (c) => {
        process.stderr.write(
          `[death-loop] ${volume}: asking the pathfinder for [${c.join(", ")}], ` +
            `${c[1] - feet[1]} course(s) up\n`,
        );
        this.lethalExclusionSuspended = false;
        try {
          await this.walkTo(c, 1, `death-loop way in to ${volume}`);
          return true;
        } catch (err) {
          if (err instanceof BotDeathError) throw err;
          process.stderr.write(
            `[death-loop] ${volume}: [${c.join(", ")}] could not be reached: ` +
              `${err instanceof Error ? err.message : String(err)}\n`,
          );
          return false;
        } finally {
          this.lethalExclusionSuspended = true;
        }
      },
      async (c) => {
        const r = await walkIn();
        if (r === "blocked") {
          process.stderr.write(
            `[death-loop] ${volume}: the walk in from [${c.join(", ")}] is blocked too\n`,
          );
        }
        return r;
      },
    );
    process.stderr.write(
      `[death-loop] ${volume}: way in ` +
        (outcome.from
          ? `[${outcome.from.join(", ")}] (${outcome.result})`
          : `not found (${outcome.result})`) +
        `; ${outcome.tried} of ${candidates.length} tried, ${outcome.reached} reached\n`,
    );
    return outcome.result;
  },

  /**
   * Open a closed gate standing between the body and `cell`, the way a player
   * walking in does: a right-click, which adventure mode permits.
   *
   * vesperhold's well is ringed by a wall whose one opening is a dark oak fence
   * gate — the very cell the placement table names as the well's near lip — and
   * it stands closed when the world starts. The pathfinder opens a gate on a path
   * it plans; the walk in is raw drive, so it has to do the same itself, by the
   * same rule (`openable`: the pathfinder's own set). Each cell is used at most
   * once a second, because using an open gate closes it again.
   */
  async openGateAhead(
    this: MineflayerExecutor,
    cell: Vec3Tuple,
    openable: ReadonlySet<number>,
    opened: Map<string, number>,
    volume: string,
  ): Promise<void> {
    const bot = this.requireBot();
    const p = bot.entity.position;
    const dx = cell[0] + 0.5 - p.x;
    const dz = cell[2] + 0.5 - p.z;
    const len = Math.hypot(dx, dz);
    if (len < 1e-6) return;
    for (const reach of [0.5, 1.0]) {
      for (const dy of [0, 1]) {
        const block = bot.blockAt(p.offset((dx / len) * reach, dy, (dz / len) * reach));
        if (!block || !openable.has(block.type)) continue;
        if ((block.getProperties() as { open?: unknown }).open !== false) continue;
        const key = `${block.position.x},${block.position.y},${block.position.z}`;
        if (Date.now() - (opened.get(key) ?? 0) < 1_000) continue;
        opened.set(key, Date.now());
        const before = block.getProperties() as Record<string, unknown>;
        try {
          await bot.activateBlock(block);
          this.gatesOpenedByTrial.push({
            pos: [block.position.x, block.position.y, block.position.z],
            state:
              `minecraft:${block.name}[` +
              Object.entries(before)
                .map(([k, v]) => `${k}=${String(v)}`)
                .join(",") +
              `]`,
          });
          process.stderr.write(
            `[death-loop] ${volume}: opened the closed ${block.name} at [${key}] in the way in\n`,
          );
        } catch (err) {
          process.stderr.write(
            `[death-loop] ${volume}: could not open the ${block.name} at [${key}]: ` +
              `${err instanceof Error ? err.message : String(err)}\n`,
          );
        }
        return;
      }
    }
  },

  /**
   * **Put back every gate the walk in opened**, in the state it stood in — by
   * command, named in `staged_removals` as staging, read by the shared rejection
   * rule.
   *
   * The walk in opens a closed gate the way a player does, and a player who then
   * dies in the hazard leaves it open. That is the harness changing the world on
   * its own account: on vesperhold the well's gate, left open by the death loop,
   * let the drowned choir — re-seated by the bot's own death — walk into the
   * lethal well it cannot otherwise reach (a chorister drowned 23 s after the
   * trial's death, and the choir's next reading would have read 3 of 4). Called
   * the moment the body is released over the volume, when the gate is already
   * behind it, and again when the trial ends however it ends.
   */
  async restoreOpenedGates(this: MineflayerExecutor, volume: string): Promise<void> {
    const bot = this.bot;
    if (!bot) return;
    while (this.gatesOpenedByTrial.length > 0) {
      const g = this.gatesOpenedByTrial.shift()!;
      const refusal = await this.refusalOf(`/setblock ${g.pos[0]} ${g.pos[1]} ${g.pos[2]} ${g.state}`);
      this.stagedRemovals.push({
        kind: "world",
        why:
          `${volume}: the gate the walk in opened at [${g.pos.join(", ")}] put back as it stood ` +
          `(${g.state})`,
        performed: refusal === undefined,
        detail: refusal,
      });
      process.stderr.write(
        `[death-loop] ${volume}: put back the gate at [${g.pos.join(", ")}] as it stood` +
          `${refusal === undefined ? "" : ` — REFUSED: ${refusal}`}\n`,
      );
    }
  },

  /**
   * **Let go over the volume, and wait for the game to carry the body in.**
   *
   * Every control is already released; this only watches. The wait is bounded by
   * the measured descent ({@link sinkBudgetMs}) over the distance from the feet to
   * the volume's top face, and it ends early when the body gets in, dies, or comes
   * to rest on something outside the volume — the rim it overhung as it stepped
   * down, a ledge, a floor over the hazard. Returns whether it came to rest, so
   * the caller can walk on from there.
   */
  async sinkInto(this: MineflayerExecutor, box: Box, trial: LethalTrial, inside: () => boolean): Promise<boolean> {
    const bot = this.requireBot();
    const from = bot.entity.position.clone();
    await this.restoreOpenedGates(trial.volume);
    const depth = from.y - (box.hi[1] + 1);
    const budget = sinkBudgetMs(depth);
    process.stderr.write(
      `[death-loop] ${trial.volume}: released over the volume at ` +
        `[${from.x.toFixed(2)}, ${from.y.toFixed(2)}, ${from.z.toFixed(2)}]` +
        `${(bot.entity as { isInWater?: boolean }).isInWater ? " in water" : ""}, ${Math.max(0, depth).toFixed(2)} block(s) ` +
        `above it; every control let go, allowing ${budget}ms for the game to carry the body in\n`,
    );
    const deadline = Date.now() + budget;
    while (Date.now() < deadline) {
      if (this.death) throw this.death;
      if (inside()) return false;
      if (bot.entity.onGround) break;
      await delay(GATE_DASH_TICK_MS);
    }
    const p = bot.entity.position;
    const rest = bot.entity.onGround;
    process.stderr.write(
      `[death-loop] ${trial.volume}: the released body ` +
        `${rest ? "came to rest" : "was still moving"} at ` +
        `[${p.x.toFixed(2)}, ${p.y.toFixed(2)}, ${p.z.toFixed(2)}] without entering the volume` +
        `${rest ? " — standing again, so it walks on" : ""}\n`,
    );
    return rest;
  },

  /**
   * Whether the volume `box` would match this body right now — the SERVER's rule
   * ({@link bodyInVolume}), asked of the bot's exact position.
   *
   * It asked whether the floored feet CELL was in the box, which is a different
   * question and a narrower one: the emitted selector intersects a 0.6-wide
   * hitbox against the region `[lo, hi + 1]`, so a body 0.2 blocks outside the
   * face is one the volume kills and one this used to call outside. Saying
   * "entered" of exactly the bodies the volume can act on is what makes
   * `enteredVolume` mean anything, and it also ends {@link stepInto}'s drive at
   * the moment the hazard can reach the bot rather than a third of a block late.
   */
  bodyInside(this: MineflayerExecutor, box: Box): boolean {
    const p = this.bot?.entity?.position;
    return p !== undefined && bodyInVolume([p.x, p.y, p.z], box);
  },

  /**
   * Whether a body could BE in `cell` — its own cell and the one above it clear.
   *
   * Deliberately weaker than {@link stanceStandable}: a lethal volume is often a
   * hole, and falling into one is exactly how a player meets it, so demanding
   * solid support underfoot would rule out the cells the volume is made of. What
   * it does rule out is a cell filled by a block, which no walk can ever reach.
   *
   * Block-shape based (`boundingBox`), like {@link gateOpen}, so it stays right
   * for whatever the campaign built with. A cell whose chunk is not loaded reads
   * as occupiable: the conservative direction here is to keep a candidate the
   * approach can then be measured against, never to silently narrow the box.
   */
  bodyCanOccupy(this: MineflayerExecutor, cell: Vec3Tuple): boolean {
    const bot = this.bot;
    if (!bot?.entity) return true;
    const p = bot.entity.position;
    const at = (dy: number) => bot.blockAt(p.offset(cell[0] - p.x, cell[1] + dy - p.y, cell[2] - p.z));
    const feet = at(0);
    const head = at(1);
    if (!feet || !head) return true;
    return feet.boundingBox === "empty" && head.boundingBox === "empty";
  },
};
