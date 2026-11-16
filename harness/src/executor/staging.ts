// MineflayerExecutor: staging a body away, and refunding its blows.

import type { Vec3Tuple } from "../critical-path.ts";
import { INSTANT_HEALTH_UNIT } from "../sustain.ts";
import { delay } from "./connection.ts";
import { isRejection } from "../rejection.ts";
import { CENSUS_MATCH_RADIUS, isWaveBody } from "../wave.ts";
import type { MineflayerExecutor } from "../executor.ts";

/**
 * How hard a staged blow hits.
 *
 * Larger than any health a delve can declare once armour and resistance have taken
 * their cut, so one blow is one body and the removal is never a slow exchange
 * somebody could mistake for a fight.
 */
const STAGED_BLOW = 100_000;

/**
 * One body the HARNESS took out of the delve, and why.
 *
 * Every entry here is the harness acting, never the delve behaving, and the run
 * artifact prints them under their own heading for exactly that reason: a reader
 * must be able to tell what the campaign's own machinery did from what was staged
 * so the run could go on reading it.
 */
export interface StagedRemoval {
  /** The body's kind, or the wave's id for a whole-wave act. */
  readonly kind: string;
  /** What the harness was doing, in its own words. */
  readonly why: string;
  /** False when the server refused the blow or the body could not be named. */
  readonly performed: boolean;
  /** The refusal, or what stopped it. Present exactly when `performed` is false. */
  readonly detail?: string;
}

/** What the server said of one body against the story-body statement. */
export interface StoryVerdict {
  /** Each matching row, as `of: reads`; empty when the body is no story body. */
  readonly of: readonly string[];
  /** Whether the body carries the wave tag of the `kill` step under way. */
  readonly ofFight: boolean;
}

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Take one body out of the delve, credited to the bot.
   *
   * **This is staging.** It is not a fight, not self-defence and not a measurement,
   * and the run artifact names every one of these so no reader can mistake it for
   * something the delve did. The ladder verifies mechanism — that a wave spawned as
   * declared, that a kill pays what it says it pays, that dying is safe — and none
   * of that requires the bot to survive an exchange or to win one.
   *
   * `player_attack by <bot>` rather than `kill`: a removal that credits nobody
   * skips every piece of wiring that pays on a player's kill (`on_kill`, the wave
   * countdown, a declared drop), and a step that then reads green has verified
   * machinery that never ran. The blow is dealt by UUID, which is the only handle
   * the client has on a specific body that a server selector also accepts.
   */
  async stageAway(this: MineflayerExecutor, id: number, why: string, name?: string): Promise<void> {
    if (this.sneaking) return;
    // spec-0132 §5.2: a pursued step is walked beside a body the party
    // escapes and never fights, so nothing is removed on it.
    if (this.pursuitRefuses(`staging ${name ?? this.bot?.entities[id]?.name ?? "?"}#${id} away (${why})`)) {
      return;
    }
    if (this.stagedIds.has(id) || this.stagingInFlight.has(id)) return;
    const bot = this.bot;
    if (!bot?.entity) return;
    const body = bot.entities[id];
    const uuid = (body as { uuid?: string } | undefined)?.uuid;
    const kind = name ?? body?.name ?? "?";
    // **Outside a scripted death, a body never kills the bot.** A hostile that
    // engages the bot is an enemy and is removed; the only question is what the
    // run must READ before it does, and the answer is decided by which wave the
    // body is of — asked of the server, by tag, never guessed from where it
    // stands. Two cases need a reading first:
    //
    //   * a wave whose current seating the run has not read (its step is still
    //     ahead, or a rest put a cleared wave back for a run-back). The muster's
    //     facts do not depend on where the body stands, so the wave is read where
    //     it is and THEN the body is removed. Leaving it standing instead was
    //     measured twice on vesperhold: an Unremembered Guard that walked out to
    //     meet the bot killed it on a `reach` step; a Wall Archer that had wandered
    //     33 blocks off its anchor — outside the radius the old rule guessed
    //     membership by — was staged unread, and the run-back's muster then
    //     reported the campaign had seated two of three;
    //   * the wave the die-retry stage is proving. See `dieRetryHolds`. Whether
    //     the body is of it is asked of the server BY UUID: does it wear the
    //     step's tag. A census position within a few blocks of where the client
    //     sees the body is a candidate, not a match — on the town's castle yard it
    //     found no body of the protected wave three times in three runs, and the
    //     body the stage was proving was removed.
    if (body?.position) {
      const here: Vec3Tuple = [body.position.x, body.position.y, body.position.z];
      this.stagingInFlight.add(id);
      try {
        const protect = this.protectedWave;
        const enc = protect === undefined ? undefined : this.encounterFor(protect.wave);
        const wears = protect === undefined || enc === undefined ? false : await this.bodyWears(uuid, protect.tag);
        if (enc !== undefined && wears !== false) {
          // How much of the wave stands, asked of the server now: whether this
          // removal would be the one that clears it is the fact the hold turns on.
          const census = await this.census(enc);
          const hold = this.dieRetryHolds(enc, census?.summary.present);
          if (hold !== undefined) {
            const asked = wears === true ? "" : ` (whether it wears \`${protect!.tag}\` went unread: ${wears})`;
            process.stderr.write(`[staged] ${kind}#${id} stands with \`${enc.wave}\`${asked}: ${hold}\n`);
            // What the position match this replaced would have said, so a run
            // records where the census put the body against where the client did.
            if (census !== undefined && !isWaveBody({ pos: here, census: census.mobs })) {
              process.stderr.write(
                `[staged] ${kind}#${id}: no census body of \`${enc.wave}\` stands within ` +
                  `${CENSUS_MATCH_RADIUS} blocks of where the client sees it ` +
                  `(${here.map((v) => v.toFixed(1)).join(", ")}; census seq ${census.summary.seq}: ` +
                  `${census.mobs.map((m) => `[${m.pos.join(", ")}]`).join(" ") || "no body lines"})\n`,
              );
            }
            return;
          }
        } else {
          const of = await this.waveOfBody(here);
          if (of !== undefined && this.readingOwed(of)) {
            process.stderr.write(
              `[staged] ${kind}#${id} is of \`${of.wave}\`, whose current seating this run has ` +
                `not read — reading it where it stands, then removing the body\n`,
            );
            await this.musterEarly(of);
          }
        }
      } finally {
        this.stagingInFlight.delete(id);
      }
      if (this.stagedIds.has(id)) return;
    }
    // The delve's own statement of what is never a combat target. A body on that
    // list is never removed, whatever it appears to have done — the cast is the
    // one thing a harness may not edit.
    if (this.nonCombatants?.has(kind)) {
      process.stderr.write(
        `[staged] ${kind}#${id} is on the delve's \`non_combatants\` list, so it is left ` +
          `standing whatever hit the bot\n`,
      );
      return;
    }
    // A body the server has already announced dead is not standing, so there is
    // nothing to remove: what hit the bot was a shot it loosed before it fell.
    // `/damage` on it is refused ("Target is invulnerable to the given damage
    // type": a dying body takes no damage), and that refusal is not the body
    // standing. Its blow is still refunded.
    if (this.deadBodies.has(id)) {
      process.stderr.write(
        `[staged] ${kind}#${id}: ${why}, but the server had already announced its death — ` +
          `nothing to remove\n`,
      );
      await this.refundBlows(kind, id);
      return;
    }
    // **A story body is never harmed by the harness** (format 14). Its death
    // writes what the story reads — an `on_kill`, a census, the cast — so a
    // removal here would be the harness choosing an outcome the path does not
    // perform: on the town, a goat that rammed the bot during the goats'
    // window was struck, its `on_kill` failed the task, and the expiry the
    // step waited for never came. Asked of the server by UUID against the
    // compiler's own selectors; the body stays, and its blows are given back,
    // the one defence that leaves the story as the path walks it. The path's
    // own `kill` of the body's wave is its act, so a body of that wave is
    // struck as before.
    if (uuid && !this.stagingInFlight.has(id)) {
      this.stagingInFlight.add(id);
      let story: StoryVerdict;
      try {
        story = await this.storyVerdict(uuid);
      } finally {
        this.stagingInFlight.delete(id);
      }
      if (story.of.length > 0 && !story.ofFight) {
        this.storyBodiesSpared.push({ kind, id, of: story.of });
        process.stderr.write(
          `[staged] ${kind}#${id}: ${why}, but it is a story body (${story.of.join("; ")}) — left ` +
            `standing, its blows refunded\n`,
        );
        await this.refundBlows(kind, id, "left standing as a story body");
        return;
      }
    }
    this.stagedIds.add(id);
    if (!uuid) {
      this.stagedRemovals.push({ kind, why, performed: false, detail: "the client has no UUID for it" });
      process.stderr.write(
        `[staged] ${kind}#${id}: ${why} — but the client has no UUID for it, so it cannot be ` +
          `named to the server; left standing\n`,
      );
      return;
    }
    const command = `/damage ${uuid} ${STAGED_BLOW} minecraft:player_attack by ${bot.username}`;
    process.stderr.write(`[staged] ${kind}#${id} removed: ${why}\n`);
    // Every command's response is read (CLAUDE.md). A refused `/damage` leaves the
    // body standing, and a run that did not look would report the removal anyway.
    const refusal = await this.refusalOf(command);
    // The body fell between the hit and the blow. The server announces a death
    // before it answers any later command, so by the time this refusal is read the
    // announcement has arrived if the death is why the blow was refused.
    const fellFirst = refusal !== undefined && this.deadBodies.has(id);
    this.stagedRemovals.push({
      kind,
      why,
      performed: refusal === undefined,
      detail: fellFirst ? `the body died before the blow landed — ${refusal}` : refusal,
    });
    if (fellFirst) {
      process.stderr.write(
        `[staged] ${kind}#${id}: the blow was refused because the body had already died — ${refusal}\n`,
      );
      await this.refundBlows(kind, id);
      return;
    }
    if (refusal !== undefined) {
      process.stderr.write(`[staged] ${kind}#${id}: the server refused the blow — ${refusal}\n`);
      return;
    }
    await this.refundBlows(kind, id);
  },

  /**
   * **Which story-body rows match this body, and whether it is of the `kill`
   * step under way**, asked of the server by UUID ({@link askBody}): one row of
   * the compiler's `story_bodies` per question, and one for {@link fightTag}. An
   * unread answer is read as a story body — the direction that harms nothing.
   */
  async storyVerdict(this: MineflayerExecutor, uuid: string): Promise<StoryVerdict> {
    const rows = this.requireStoryBodies();
    if (rows.length === 0) return { of: [], ofFight: false };
    const selects = rows.map((r) => r.select);
    if (this.fightTag !== undefined) selects.push(`tag=${this.fightTag}`);
    const answer = await this.askBody(uuid, selects);
    if (typeof answer === "string") {
      return { of: [`unread (${answer}), so treated as story`], ofFight: false };
    }
    const of = rows.filter((_, i) => answer[i]).map((r) => `${r.of}: ${r.reads.join(", ")}`);
    return { of, ofFight: this.fightTag !== undefined && answer[rows.length] === true };
  },

  /**
   * Whether the body wears `tag`, asked of the server by UUID ({@link askBody}):
   * `true` or `false` as it answered, or why it could not be read — no UUID, an
   * empty tag, a reply that never closed, a refusal.
   */
  async bodyWears(this: MineflayerExecutor, uuid: string | undefined, tag: string): Promise<boolean | string> {
    if (!uuid) return "the client has no UUID for it";
    if (tag === "") return "the step names no tag";
    const answer = await this.askBody(uuid, [`tag=${tag}`]);
    return typeof answer === "string" ? answer : answer[0] === true;
  },

  /**
   * **Ask the server which of `selects` one body matches, by UUID**: one
   * bracket, one conditional `tellraw` per `@s[<select>]`. Each answer is
   * `true` exactly when its line came back. A string when nothing can be read:
   * the reply never closed, or the server refused a question.
   */
  async askBody(this: MineflayerExecutor, uuid: string, selects: readonly string[]): Promise<boolean[] | string> {
    const bot = this.requireBot();
    const serial = ++this.bodyAsks;
    const mark = (i: number): string => `[dw:body ${serial} ${i}]`;
    const commands = selects.map(
      (select, i) =>
        `/execute as ${uuid} if entity @s[${select}] run tellraw ${bot.username} ` +
        JSON.stringify({ text: mark(i) }),
    );
    const reply = await this.bracketed(commands);
    const refused = reply.lines.find((l) => isRejection(l));
    if (!reply.answered) return "the reply never closed";
    if (refused !== undefined) return `refused — ${refused}`;
    const said = new Set(reply.lines.map((l) => l.trim()));
    return selects.map((_, i) => said.has(mark(i)));
  },

  /**
   * Staging rows that act on a body of the delve — not the ones that act on the
   * bot's own health (`kind: "player"`: a refund, or {@link holdFullHealth}).
   */
  bodiesStaged(this: MineflayerExecutor): number {
    return this.stagedRemovals.filter((r) => r.kind !== "player").length;
  },

  /**
   * Undo what a body the run has just removed did to the bot.
   *
   * Removing a body on its first blow is not enough on its own: a leg through
   * three re-seated waves lets each of their bodies land one blow before it goes,
   * and on vesperhold's die-retry return leg a Hired Knife, a pillager, a Wall
   * Archer and one Guard each did, one after another, until a second Guard's
   * first swing was the killing one — the bot slain by a body a second after the
   * run had begun removing its wave. An enemy killed outright is an enemy whose
   * blows did not land, so the health the server named that body as taking is
   * given back.
   *
   * Only that body's own, attributed blows, rounded UP to what vanilla's instant
   * health can give (4 × 2^amplifier). Rounded down, each removal leaked up to
   * four points — measured on the next run: Guards landing 7.3 were refunded 4,
   * the bot sank to 9.3 over six removals, and a Drowned Precentor's trident then
   * killed it on the death-loop approach. Rounded up, a removal can give back at
   * most 3.9 points more than its body took, and that bound is the whole of what
   * a refund can hide. A fall, a lethal volume or any damage the server named no
   * body for is never refunded, so the delve's own hazards keep their reach. Each
   * effect is read by the shared rejection rule and named in `staged_removals`.
   */
  async refundBlows(
    this: MineflayerExecutor,
    kind: string,
    id: number,
    fate = "removed",
  ): Promise<void> {
    const dealt = this.damageBy.get(id) ?? 0;
    this.damageBy.delete(id);
    const bot = this.bot;
    if (!bot || this.death) return;
    if (dealt <= 0) return;
    if (this.pursuitRefuses(`refunding ${dealt.toFixed(1)} health ${kind}#${id} took`)) return;
    // On a walk leg the bot is already held at full health, whoever dealt the
    // blow — a refund on top of that gives back nothing and would read as if it had.
    if (this.walkLegs > 0) return;
    const refunded = Math.ceil(dealt / INSTANT_HEALTH_UNIT);
    let units = refunded;
    for (let amp = 0; units > 0; amp += 1, units >>= 1) {
      if ((units & 1) === 0) continue;
      const refusal = await this.refusalOf(`/effect give @s minecraft:instant_health 1 ${amp} true`);
      const heal = INSTANT_HEALTH_UNIT << amp;
      this.stagedRemovals.push({
        kind: "player",
        why:
          `refund: ${kind}#${id} was ${fate} after its blows took ${dealt.toFixed(1)} health; ` +
          `${heal} of it given back with instant health ${amp + 1}`,
        performed: refusal === undefined,
        detail: refusal,
      });
      if (refusal !== undefined) {
        process.stderr.write(`[staged] refund for ${kind}#${id} refused — ${refusal}\n`);
        return;
      }
    }
    process.stderr.write(
      `[staged] ${kind}#${id}: its blows took ${dealt.toFixed(1)} health; refunded ` +
        `${refunded * INSTANT_HEALTH_UNIT} (health now ` +
        `${bot.health.toFixed(1)})\n`,
    );
  },

  /**
   * **Finish every staging act in flight**, bounded by `timeoutMs`, and say how
   * many were still running. Called once, before the run report is built.
   *
   * On vesperhold a Guard staged away on the death loop's walk back fired the
   * drowned choir's muster; the stage then ended, the report was written, and
   * the reading — 3 of 4, one chorister had drowned — was never recorded.
   */
  async settleStaging(this: MineflayerExecutor, timeoutMs: number): Promise<{ unfinished: number; waitedMs: number }> {
    // Closed FIRST, so the set can only shrink: before this, a body that hit the
    // bot while the wait was running started a task the wait had not
    // snapshotted — the wait returned when the snapshot settled (about a second,
    // on vesperhold) and reported the late task as "still unfinished 20000ms
    // after the last stage", which it was not.
    this.stagingClosed = true;
    const start = Date.now();
    while (this.stagingTasks.size > 0 && Date.now() - start < timeoutMs) {
      await Promise.race([Promise.allSettled([...this.stagingTasks]), delay(timeoutMs - (Date.now() - start))]);
    }
    return { unfinished: this.stagingTasks.size, waitedMs: Date.now() - start };
  },

  /** Every body this run took out of the delve by command. */
  stagedBodies(this: MineflayerExecutor): readonly StagedRemoval[] {
    return this.stagedRemovals;
  },
};
