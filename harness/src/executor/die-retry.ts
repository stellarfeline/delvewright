// MineflayerExecutor: die-retry: dying to a wave and meeting it again.

import type { KillStep } from "../critical-path.ts";
import { BotDeathError } from "../death.ts";
import {
  CONTROLLED_GAMEMODE,
  deathPhases,
  checkpointPrecondition,
  observationOf,
  openTrial,
  describeStanding,
  respawnedAtCheckpoint,
  retryOutcome,
  scriptedDeathCommand,
  scriptedDeathRefusal,
  type DeathTrial,
  type Encounter,
  type ReengageObservation,
  type RunBack,
} from "../combat.ts";
import { delay } from "./connection.ts";
import { CUTSCENE_POLL_MS } from "./cutscene.ts";
import { RESPAWN_PROTECTION_TICKS, serverAge, RESPAWN_TIMEOUT_MS } from "./death.ts";
import { REACH_POLL_MS } from "./walk.ts";
import { CENSUS_TIMEOUT_MS } from "./wave.ts";
import type { MineflayerExecutor } from "../executor.ts";

/**
 * How often the server sends its world age (`update_time`): every 20 ticks. The
 * age the bot holds may be this far behind the server's, so the window is counted
 * from a reading that may be this far stale.
 */
const TIME_PACKET_TICKS = 20;

/** Bound on the respawn-protection wait, however slowly the server ticks. */
const RESPAWN_PROTECTION_TIMEOUT_MS = 15_000;

/**
 * How long the die-retry re-engage probe waits for the encounter to show itself
 * before concluding nothing is there.
 *
 * The probe used to be ONE instantaneous sample taken the moment the walk back
 * resolved, and that is a sampling bug, not an observation: a client learns about
 * an entity when the server sends it, which takes ticks after arrival —
 * `fightWave` has always slept a second on arrival for exactly this reason. On
 * nobodys-cave-island r14 three demonstrably-alive drowned (feral, follow_range
 * 48, wandered off the anchor after killing the bot) read as "no hostile was
 * there to fight" and reddened both trials of a healthy encounter.
 *
 * Generous on purpose, and it costs nothing on a healthy run: the probe returns
 * the instant the declared wave is standing. The probe asks the SERVER, by tag,
 * so what it settles on is the wave itself — client tracking range does not bound
 * the answer, and nothing standing nearby can enter it.
 */
const REENGAGE_SETTLE_MS = 6_000;

/**
 * How long the re-seat census may settle ({@link Executor.awaitReseat}). The
 * re-seat lands one server tick after the respawn; this only has to outlast the
 * census round-trips around that tick, and stays far below any walk back, so no
 * harm a re-seated wave takes on its own can land inside it.
 */
const RESEAT_SETTLE_MS = 3_000;

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /** Every scripted death of the die-retry stage — including the ones whose loop
   * the run abandoned half-way, which is the whole point of recording on death. */
  deathTrials(this: MineflayerExecutor): readonly DeathTrial[] {
    return this.trials;
  },

  /** Waves the die-retry stage entered. A wave here with no completed trial is an
   * unproven retry loop, not a silent pass. */
  dieRetryEngagements(this: MineflayerExecutor): ReadonlySet<string> {
    return this.dieRetryEngaged;
  },

  /** The run-backs this run fought, in order. */
  runBacks(this: MineflayerExecutor): readonly RunBack[] {
    return this.runBacksFought;
  },

  /**
   * The die-retry ladder stage for one encounter (spec-0023 §1): the load-bearing
   * combat proof. In a souls delve the sacred property is not winning — it is that
   * dying is always SAFE. So the harness deliberately dies to each mandatory
   * encounter and proves the whole loop: death → respawn at the governing
   * checkpoint → the route back is walkable → the encounter re-engages → and no
   * completed objective was lost on the way.
   */
  async dieRetryAt(this: MineflayerExecutor, step: KillStep, enc: Encounter): Promise<void> {
    const bot = this.requireBot();
    // Recorded BEFORE the approach walk: from here on, silence about this
    // encounter is a finding. `dieRetryCoverageFailures` turns an engagement with
    // no completed trial into a red stage, so a run that dies on the way in can
    // never report a passed die-retry.
    // PRECONDITION: the loop this stage proves is
    // "death → respawn at the governing checkpoint → walk back". Two ways that
    // premise can be false, and they are different kinds of fact:
    //   * the checkpoint exists but was never ARMED (a bonfire the route walked
    //     past) — the harness's own gap, and every measurement below would
    //     describe it rather than the delve. Red;
    //   * the campaign fires NO checkpoint before this fight at all — a content
    //     fact: every death here is a full restart. Advisory, because the
    //     compiler's retry-cost and checkpoint rules are what judge that.
    // Either way: take NO death. A scripted death would blame the campaign for a
    // proof that was never in a position to be made.
    const precondition = checkpointPrecondition(
      enc,
      this.restSteps,
      this.restedBonfires,
      this.currentStep,
    );
    if (precondition !== undefined) {
      (precondition.reds ? this.preconditionFindings : this.preconditionAdvisories).push(
        precondition.finding,
      );
      this.preconditionWaves.add(enc.wave);
      process.stderr.write(`[die-retry] ${precondition.finding}\n`);
      return;
    }
    this.dieRetryEngaged.add(enc.wave);
    // The approach walks the bot to within 3 blocks of a LIVE encounter. Anything
    // that hits it on the way is staged away by the damage handlers — the stage
    // asks "is dying safe here", not "can this bot survive the walk in", and a bot
    // cut down before it can script its first death answers neither.
    for (let attempt = 1; ; attempt += 1) {
      try {
        await this.walkTo(step.pos, 3, `die-retry approach ${step.wave}`, step.sneak);
        break;
      } catch (err) {
        // A death on the way in is not a verdict on the retry loop, and it is not
        // one this stage may be stopped by: the encounter is live and lethal
        // BECAUSE that is what is being proved safe. Recover and walk it again,
        // twice at most, then let the trial record the abort.
        if (!(err instanceof BotDeathError) || attempt >= 3) throw err;
        process.stderr.write(
          `[die-retry] the approach to ${step.wave} ended in a death; recovering and walking ` +
            `it again (attempt ${attempt + 1} of 3)\n`,
        );
        await this.respawnAndRearm();
      }
    }
    const phases = deathPhases();
    for (const [i, phase] of phases.entries()) {
      const attempt = i + 1;
      // A death still latched from the last loop (the bot was killed for real on
      // the way back) would make the next scripted death resolve instantly and
      // credit a trial that never happened. Clear it first, honestly.
      if (this.death) {
        process.stderr.write(
          `[die-retry] an unscripted death is still pending — recovering from it and walking ` +
            `back to the fight before taking the next scripted one\n`,
        );
        await this.respawnAndRearm();
        // The respawn put the bot at the checkpoint. A death scripted THERE is not a
        // death at this encounter, and the trial would measure the checkpoint.
        await this.walkTo(step.pos, 3, `die-retry re-approach ${step.wave}`, step.sneak);
      }
      // "mid-fight" is a state of the WAVE, not of the bot: bodies below their own
      // `max_health`, which a faithful re-seat must replace. One attributed point
      // of damage to every body puts the wave in exactly that state, with no
      // fencing in it — and, unlike a traded exchange, it does so deterministically
      // and without ever risking the unscripted death that would credit this trial
      // to a life the harness never opened.
      if (phase === "mid-fight") {
        await this.chipWave(enc);
        if (this.death) {
          process.stderr.write(
            `[die-retry] the wave killed the bot while it stood there — recovering before ` +
              `taking the scripted death, so the death this trial records is the one it asked for\n`,
          );
          await this.respawnAndRearm();
          await this.walkTo(step.pos, 3, `die-retry re-approach ${step.wave}`, step.sneak);
        }
      }
      const before = new Set(this.completedObjectives.keys());
      // BRAND the mobs this life fought. A re-seat must replace every
      // one of them; a mob still wearing the brand in the next life IS the chipped
      // survivor a faithful re-seat forbids. The stamp rides the wave's
      // own tag, so it can only ever land on this wave — the mistake the previous
      // id-based baseline made, which branded whatever the client happened to be
      // tracking.
      this.brandWave(enc);
      process.stderr.write(
        `[die-retry] ${step.wave} death ${attempt}/${phases.length} (${phase})\n`,
      );
      // The record exists from the moment the harness commits to dying. Everything
      // below MUTATES it, so however the run ends the artifact still says a death
      // was taken here and what was (and was not) learned from it.
      const trial = openTrial(enc, attempt, phase);
      this.trials.push(trial);
      try {
        // A cutscene cannot be allowed to eat this death. The encounter's own
        // objective completion may start one, and a cutscene's first act is
        // `gamemode spectator @a` — so a trade that finished the wave hands the
        // next scripted death an invulnerable body, `/damage` does nothing, and
        // the stage used to report that as a missing op.
        await this.awaitControlForScriptedDeath(step, enc);
        await this.awaitRespawnProtection(enc);
        const seq = this.deathSeq;
        // What the bot carries INTO the death — the baseline `keep_inventory` is
        // judged against on the way out.
        this.itemsBeforeDeath = bot.inventory.items().length;
        const chatFrom = this.chatMark();
        bot.chat(scriptedDeathCommand());
        if (!(await this.awaitDeathAfter(seq, RESPAWN_TIMEOUT_MS))) {
          // No death followed, so the stage proves nothing and the trial fails —
          // but it fails saying what it SAW. The bot is opped and receives the
          // server's own answer on the chat stream, and its gamemode decides
          // whether the command could ever have worked.
          trial.abortedWith = scriptedDeathRefusal(
            scriptedDeathCommand(),
            RESPAWN_TIMEOUT_MS,
            this.gameModeNow(),
            this.chatSince(chatFrom).lines,
          );
          throw new Error(`die-retry: ${trial.abortedWith}`);
        }
        trial.cause = this.death?.likelyCause;
        const respawn = await this.respawnAndRearm();
        trial.respawnPos = respawn.pos;
        trial.kitKept = respawn.kitKept;
        trial.atCheckpoint = respawnedAtCheckpoint(trial.respawnPos ?? [0, 0, 0], enc.checkpoint);
        process.stderr.write(
          `[die-retry] ${step.wave} death ${attempt}: respawned at ` +
            `${trial.respawnPos ? trial.respawnPos.join(",") : "an unknown position"}` +
            `${trial.atCheckpoint ? "" : ` — NOT the governing checkpoint ${enc.checkpoint?.join(",") ?? "(none)"}`}\n`,
        );
        // Fidelity is read HERE, at the event it guards: the re-seat has just
        // landed (`cp_respawn_fire` runs on the first tick after the respawn) and
        // nothing has touched the new cohort yet. Read after the walk back, the
        // same census also carried everything the wave did to itself on the way
        // and everything the world did to it (vesperhold's choir,
        // drowned in a lethal well) — and blamed the re-seat for both.
        if (enc.respawnsOnRest) {
          const at = await this.awaitReseat(enc);
          trial.reseat = at;
          process.stderr.write(
            `[die-retry] ${step.wave} death ${attempt}: re-seat read ${at.present}/${at.declared} ` +
              `wave mob(s) after ${at.settleMs}ms` +
              `${at.carriedOver > 0 ? `, ${at.carriedOver} carried over from a previous life` : ""}` +
              `${at.healthReadable > 0 ? `, ${at.damaged}/${at.healthReadable} damaged` : ""}\n`,
          );
        }
        // The walk back ends inside the re-seated wave; whether the ROUTE is walkable
        // is the measurement, and anything that hits the bot on it is staged away.
        try {
          await this.walkTo(step.pos, 3, `die-retry return ${step.wave}`, step.sneak);
          trial.returned = true;
        } catch (err) {
          const detail = err instanceof Error ? err.message : String(err);
          // A leg that ended in a death has said nothing about the ROUTE: the bot
          // was killed on it. Recorded as what it was, so the verdict never reads a
          // death on the way back as unwalkable geometry.
          trial.returnFailure = {
            killed: err instanceof BotDeathError,
            detail,
          };
          process.stderr.write(`[die-retry] return leg failed: ${detail}\n`);
        }
        const after = new Set(this.completedObjectives.keys());
        trial.lostObjectives = [...before].filter((o) => !after.has(o));
        trial.objectivesIntact = trial.lostObjectives.length === 0;
        trial.objectiveComplete = this.completedObjectives.has(enc.objective);
        // Two observations, one verdict (see RetryOutcome). A wave mob standing
        // here again means the fight is retriable. Nothing left to fight is only
        // a failure if the encounter's objective is ALSO unfinished — then the
        // party can neither complete it nor re-fight it, which is a soft lock.
        // A wave already beaten before the death is a won fight staying won.
        //
        // Observed ONLY when the bot got back. The probe reads the
        // entities the CLIENT tracks, so a bot standing 150 blocks from the fight
        // is not observing the encounter at all — it is observing wherever it is
        // stuck. Reporting that as `re_engaged` produced the run-five artifact in
        // which one trial said "the route back is not walkable" and "the fight
        // re-engaged" at once. A trial that never returned leaves `re_engaged`
        // false, `reengage` null and its outcome `unproven`: not looked at is not
        // the same fact as looked at and empty, and neither is a pass.
        if (trial.returned) {
          // What the party had felled of this seating when it landed: the bodies
          // it fells from here on met it on the way back, and that IS the fight
          // re-engaging — a re-seating wave's bodies are removed when they hit
          // the bot (see `dieRetryHolds`), and the census credits each one. Only a
          // re-seating wave has a landing reading to count from; a wave that does
          // not re-seat is never removed during the stage, so only what stands
          // answers for it.
          const landed = trial.reseat?.credited;
          const obs = await this.awaitReengage(enc, landed);
          trial.reengage = obs;
          trial.reEngaged = obs.present > 0 || (landed !== undefined && obs.credited > landed);
          trial.outcome = retryOutcome(trial.reEngaged, trial.objectiveComplete);
          process.stderr.write(
            `[die-retry] ${step.wave} death ${attempt}: ${obs.present}/${obs.declared} wave mob(s) ` +
              `after ${obs.settleMs}ms` +
              `${obs.nearest !== undefined ? `, ${obs.nearest.toFixed(1)}–${obs.farthest!.toFixed(1)} blocks from the anchor` : ""}` +
              `${obs.carriedOver > 0 ? `, ${obs.carriedOver} carried over from a previous life` : ""}` +
              `${obs.healthReadable > 0 ? `, ${obs.damaged}/${obs.healthReadable} damaged` : ""}\n`,
          );
        } else {
          process.stderr.write(
            `[die-retry] ${step.wave} death ${attempt}: the bot never got back to the ` +
              `encounter, so re-engagement was NOT observed (outcome stays \`unproven\`)\n`,
          );
        }
        process.stderr.write(
          `[die-retry] ${step.wave} death ${attempt}: ${trial.outcome}` +
            `${trial.outcome === "cleared-before-retry" ? ` (\`${enc.objective}\` was already complete — the death cost no progress)` : ""}\n`,
        );
        const reading = await respawn.reading;
        trial.respawnReading = reading;
        process.stderr.write(
          `[die-retry] ${step.wave} death ${attempt}: respawn readings — client ` +
            `${JSON.stringify(reading.client.pos)} at world age ` +
            `${reading.client.age ?? "unknown"} after ` +
            `${reading.client.respawnPackets} respawn packet(s); server ` +
            ("pos" in reading.server
              ? `${JSON.stringify(reading.server.pos)} between ticks ` +
                `${reading.server.tickFrom} and ${reading.server.tickTo}`
              : `unread (${reading.server.unread})`) +
            `\n`,
        );
        trial.completed = true;
      } catch (err) {
        trial.abortedWith ??= err instanceof Error ? err.message : String(err);
        process.stderr.write(
          `[die-retry] ${step.wave} death ${attempt} loop abandoned: ${trial.abortedWith}\n`,
        );
        throw err;
      } finally {
        // Clear the brand however the trial ended, so the NEXT death brands a
        // clean slate and a stale stamp can never be read as a survivor. In the
        // `finally` because an abandoned trial leaves mobs standing too.
        this.unbrandWave(enc);
      }
    }
  },

  /**
   * Hold until the bot is a body a scripted death can reach — out of the spectator
   * a cutscene put it in.
   *
   * The bound is the campaign's own number, never one invented here: a step whose
   * completion fires a `Cutscene` carries `cutscene_seconds` in
   * `critical-path.json`, and a `kill` step carries it exactly as a walking step
   * does. The sequencer already waits it out AFTER a step; nothing waited it out
   * inside one, which is where the die-retry stage lives — the general mechanism
   * was there and its binding did not reach this caller. The grace on top is the
   * same {@link awaitCutscene} uses.
   *
   * Bounded, and it never fails: a window that outlasts what the build declared is
   * a finding, and the finding is the refusal {@link scriptedDeathRefusal} writes
   * when the death then does not land — which says the gamemode it saw.
   */
  async awaitControlForScriptedDeath(this: MineflayerExecutor, step: KillStep, enc: Encounter): Promise<void> {
    this.requireBot();
    if (this.gameModeNow() === CONTROLLED_GAMEMODE) return;
    const declaredMs = (step.cutsceneSeconds ?? 0) * 1000;
    const budget = declaredMs + this.cutsceneGraceMs;
    const started = Date.now();
    const deadline = started + budget;
    process.stderr.write(
      `[die-retry] ${enc.wave}: the bot is in \`${this.gameModeNow() ?? "?"}\`, not ` +
        `\`${CONTROLLED_GAMEMODE}\` — waiting out the cutscene before scripting a death ` +
        `(${step.cutsceneSeconds ?? 0}s declared + ${this.cutsceneGraceMs}ms grace)\n`,
    );
    while (Date.now() < deadline) {
      if (this.death) throw this.death;
      if (this.gameModeNow() === CONTROLLED_GAMEMODE) {
        process.stderr.write(
          `[die-retry] ${enc.wave}: control returned after ${Date.now() - started}ms\n`,
        );
        return;
      }
      await delay(CUTSCENE_POLL_MS);
    }
    process.stderr.write(
      `[die-retry] ${enc.wave}: still \`${this.gameModeNow() ?? "?"}\` after ${budget}ms — ` +
        `scripting the death anyway, so the refusal below says what was seen rather than ` +
        `this wait swallowing it\n`,
    );
  },

  /**
   * Hold until a respawned body can be hurt again, so a scripted death is not
   * refused inside the client-load window (see {@link RESPAWN_PROTECTION_TICKS}).
   *
   * The window ends when the bot has sent `player_loaded` for this life. Without
   * a tracker to ask (a bot adopted by {@link attachBot}), it is counted in SERVER
   * ticks to the server's own fallback, from the world age the time packets carry:
   * the window is the server's, and a lagging server stretches it in wall-clock time. The
   * respawn's age reading can be up to one time-packet interval stale, so the wait
   * runs until the age has moved the window plus that interval past it. A bot that
   * never heard a time packet waits the window at the nominal 20 ticks a second
   * and says so. Bounded; a window that never closes is left to the refusal the
   * scripted death then reads and reports.
   */
  async awaitRespawnProtection(this: MineflayerExecutor, enc: Encounter): Promise<void> {
    const bot = this.requireBot();
    if (this.lastSpawnAt === undefined) return;
    const needTicks = RESPAWN_PROTECTION_TICKS + TIME_PACKET_TICKS;
    const from = this.lastSpawnAge;
    const deadline = Date.now() + RESPAWN_PROTECTION_TIMEOUT_MS;
    const closed = (): boolean => {
      if (this.clientLoaded?.phase() === "loaded") return true;
      const now = serverAge(bot);
      if (from !== undefined && now !== undefined) return now - from >= needTicks;
      return Date.now() - this.lastSpawnAt! >= needTicks * 50;
    };
    if (closed()) return;
    process.stderr.write(
      `[die-retry] ${enc.wave}: the bot respawned ${Date.now() - this.lastSpawnAt}ms ago and is ` +
        `inside the client-load window (at most ${RESPAWN_PROTECTION_TICKS} ticks) — waiting it out ` +
        `before scripting a death` +
        `${from === undefined ? " (no time packet heard yet, so counted at 20 ticks a second)" : ""}\n`,
    );
    while (Date.now() < deadline) {
      if (this.death) throw this.death;
      if (closed()) return;
      await delay(CUTSCENE_POLL_MS);
    }
    process.stderr.write(
      `[die-retry] ${enc.wave}: the respawn-protection window did not close within ` +
        `${RESPAWN_PROTECTION_TIMEOUT_MS}ms — scripting the death anyway, so the server's answer ` +
        `says what happened\n`,
    );
  },

  /**
   * Why a body of the wave the die-retry stage is proving must be left standing,
   * or `undefined` when it is removed like any other.
   *
   * A wave that re-seats on respawn is removed: the next scripted death brings it
   * back WHOLE, the fidelity verdict is read at that landing, and a body the party
   * fells afterwards is counted as re-engagement by the census's own credit. So
   * nothing the stage measures is lost, and the bot is not killed by the subject of
   * a proof about dying safely — which is what happened on vesperhold's return leg,
   * where a re-seated Guard slew the bot and the trial reported the route back as
   * unwalkable.
   *
   * A wave that does NOT re-seat is different: its bodies persist across both
   * lives, and they are the fight the second life must find again. Removing one
   * would change what the next trial proves, so it stays — the one place a body
   * may still land hits on the bot outside a scripted death, and the log says so
   * every time.
   *
   * A re-seating wave's LAST standing body is held too (`standing`, the census's
   * count at the moment of the hit; unread counts as last). Removing it would
   * CLEAR the encounter, and a clear is the delve's event, not the harness's: it
   * completes the objective and fires its completion, and that completion can act
   * on the anchor while the re-seat lands. On the gallery the last Muster Hand
   * struck the bot as the scripted death landed and was staged away after the bot
   * was already dead; the clear fired `obj/clear-the-muster`'s completion, whose
   * volley shoots from the muster anchor for two seconds, and the re-seat reading
   * found the re-seated Muster Stray five points down — red only on the runs where
   * the bot happened to have staged the rest of the cohort first. Held, the next
   * re-seat removes the body as it removes every other.
   */
  dieRetryHolds(
    this: MineflayerExecutor,
    enc: Encounter,
    standing: number | undefined,
  ): string | undefined {
    if (enc.respawnsOnRest) {
      if (standing !== undefined && standing > 1) return undefined;
      return (
        `the die-retry stage is proving it live and this is the last body of it standing ` +
        `(${standing ?? "an unread count"}); removing it would clear the encounter and fire ` +
        `its completion between the death and the re-seat reading — left standing, for the ` +
        `next re-seat to remove`
      );
    }
    return (
      `the die-retry stage is proving it live and it does not re-seat, so its bodies ` +
      `persist across both lives and are the fight the next life must find — left standing`
    );
  },

  /**
   * Wait for the encounter to show itself, then describe what came back.
   *
   * Returns the moment the declared wave is standing, so a healthy run pays
   * nothing; otherwise it settles for {@link REENGAGE_SETTLE_MS} before
   * concluding. A single instantaneous sample was the island-r14 false negative:
   * entity tracking lags arrival by ticks, and three living drowned read as an
   * empty room.
   */
  async awaitReengage(
    this: MineflayerExecutor,
    enc: Encounter,
    landedCredited?: number,
  ): Promise<ReengageObservation> {
    const started = Date.now();
    const deadline = started + REENGAGE_SETTLE_MS;
    let census = await this.census(enc);
    for (;;) {
      // Enough is accounted for to answer every question this observation feeds:
      // standing, or felled by the party since the re-seat landed.
      if (
        census &&
        census.summary.present +
          (landedCredited === undefined
            ? 0
            : Math.max(0, census.summary.credited - landedCredited)) >=
          enc.count
      ) {
        break;
      }
      if (Date.now() >= deadline) break;
      await delay(REACH_POLL_MS);
      census = (await this.census(enc)) ?? census;
    }
    if (!census) {
      // No census came back at all. That is a broken probe, not an empty room, and
      // the run must say so rather than report a `stranded` the delve never caused.
      throw new Error(
        `die-retry: the wave census \`${enc.census.census}\` never answered within ` +
          `${CENSUS_TIMEOUT_MS}ms — the bot must be opped to call it`,
      );
    }
    return observationOf(census, enc.count, enc.pos, Date.now() - started);
  },

  /**
   * The census the moment a re-seat has landed.
   *
   * The re-seat runs on the server tick after the respawn, and a census can be
   * answered before that tick. A reading taken too early still shows the branded
   * cohort of the life that just ended (or nothing, if the party had cleared it),
   * so this settles until the census shows what only a completed re-seat can: no
   * branded body standing and the declared count present. Bounded; a re-seat that
   * never gets there is exactly what the fidelity verdict then reports, from the
   * last reading.
   */
  async awaitReseat(this: MineflayerExecutor, enc: Encounter): Promise<ReengageObservation> {
    const started = Date.now();
    const deadline = started + RESEAT_SETTLE_MS;
    let census = await this.census(enc);
    for (;;) {
      if (census && census.summary.branded === 0 && census.summary.present >= enc.count) break;
      if (Date.now() >= deadline) break;
      await delay(REACH_POLL_MS);
      census = (await this.census(enc)) ?? census;
    }
    if (!census) {
      throw new Error(
        `die-retry: the wave census \`${enc.census.census}\` never answered at the re-seat ` +
          `within ${CENSUS_TIMEOUT_MS}ms — the bot must be opped to call it`,
      );
    }
    // A wound at the landing is what the fidelity verdict reds on; the census has
    // each body's health, and the size of the wound is the one fact that can say
    // what dealt it (an arrow still in flight from the last life, the world, a
    // re-seat that did not summon whole). Stated so the next occurrence carries it.
    if (census.summary.damaged > 0) {
      process.stderr.write(
        `[die-retry] ${enc.wave}: the re-seat landed with ${census.summary.damaged} body/bodies ` +
          `below full — ${describeStanding(census.mobs)}\n`,
      );
    }
    return observationOf(census, enc.count, enc.pos, Date.now() - started);
  },

  /** Encounters whose scripted deaths were skipped for want of an ARMED
   * checkpoint — the run's own gap, so these red the stage. */
  dieRetryPreconditionFindings(this: MineflayerExecutor): readonly string[] {
    return this.preconditionFindings;
  },

  /** Encounters whose scripted deaths were skipped because the campaign fires no
   * governing checkpoint before them. Advisory: the retry loop there went
   * unproven, and whether that staging is acceptable is the compiler's call, not
   * this stage's. Reported, never graded — and never silent. */
  dieRetryPreconditionAdvisories(this: MineflayerExecutor): readonly string[] {
    return this.preconditionAdvisories;
  },

  /** The waves those findings name. Coverage stays silent about them: the
   * precondition already says why they are unproven, and "never reached this
   * encounter" would be plainly untrue — the bot stood in the room and declined. */
  dieRetryPreconditionWaves(this: MineflayerExecutor): ReadonlySet<string> {
    return this.preconditionWaves;
  },
};
