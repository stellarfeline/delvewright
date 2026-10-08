// MineflayerExecutor: the bot's death: racing it, recovering, respawning, reading the server position.

import type { Bot } from "mineflayer";
import type { Entity } from "prismarine-entity";
import type { Vec3Tuple } from "../critical-path.ts";
import { BotDeathError, likelyDeathCause } from "../death.ts";
import type { NamedEntityDeath } from "../teardown.ts";
import { type RespawnReading } from "../combat.ts";
import { isRejection } from "../rejection.ts";
import { SERVER_LOAD_TIMEOUT_TICKS, type LoadWindow } from "../client-loaded.ts";
import { delay } from "./connection.ts";
import { SCORE_POLL_MS, LEDGER_POLL_MS, SCORE_TRACK_TIMEOUT_MS } from "./score.ts";
import { displayNameOf } from "./wave.ts";
import type { MineflayerExecutor } from "../executor.ts";

/**
 * The longest a respawned player cannot be hurt, in server ticks.
 *
 * On this version the only post-respawn protection is the client-load window
 * (`client-loaded.ts`): the server refuses every damage source until the client
 * sends `player_loaded`, or for {@link SERVER_LOAD_TIMEOUT_TICKS} if it never
 * does. The harness bot sends it when its chunk is loaded, which closes the
 * window within a few ticks (measured: `probe/client-loaded.ts`); this bound is
 * what the wait falls back to when it cannot see that send. `/damage` inside the
 * window answers "Target is invulnerable to the given damage type".
 */
export const RESPAWN_PROTECTION_TICKS = SERVER_LOAD_TIMEOUT_TICKS;

/**
 * Whether the respawn after a death has LANDED, the way a real client knows it:
 * the server's `respawn` packet has arrived, and the client has since reported
 * `player_loaded` for that respawn — which it does only once the server's
 * position for the new life has arrived and the chunk under it is loaded. That
 * is the first moment a player sees where the respawn put them.
 *
 * mineflayer's `spawn` is not that moment. It fires on the first health update
 * above zero after the client last thought itself dead, and nothing ties that
 * update to a respawn. The gallery once read a die-retry respawn at [15,69,21],
 * by the muster where the bot died, with the checkpoint at [5,67,9]; the
 * respawn reading carries the server's own `Pos` beside the client's, so a
 * recurrence names its side. With no wire
 * tracker (a bot adopted by `attachBot`) there is no packet to wait for, and the
 * `spawn` count is all there is.
 */
export function respawnLanded(
  spawned: boolean,
  respawnPacketsSinceDeath: number,
  windowsSinceDeath: readonly LoadWindow[] | undefined,
): boolean {
  if (windowsSinceDeath === undefined) return spawned;
  return (
    respawnPacketsSinceDeath > 0 &&
    windowsSinceDeath.some((w) => w.cause === "respawn" && w.sentAt !== undefined)
  );
}

/** Where {@link MineflayerExecutor.readServerPos} parks the game time it reads (harness-owned). */
const POS_READ_STORAGE = "dw_harness:pos_read";

/** The server's world age as the bot last heard it, or `undefined` before any time packet. */
export function serverAge(bot: Bot | undefined): number | undefined {
  const age = (bot as { time?: { age?: number | null } } | undefined)?.time?.age;
  return typeof age === "number" && Number.isFinite(age) ? age : undefined;
}

/** gap 7 (retry): how long (ms) to wait for the bot to respawn before resuming. */
export const RESPAWN_TIMEOUT_MS = 15_000;

/** How often the respawn wait re-reads the spawn counter. */
export const SPAWN_POLL_MS = 50;

/**
 * How long the bot leaves its own corpse on the death screen before taking the
 * respawn — one human beat (20 server ticks).
 *
 * Not a tuning knob and not a wait for a race to settle: it is the difference
 * between a player and a library. mineflayer answers the death packet in the same
 * event-loop turn, so a default bot is alive again on the next tick and the
 * engine's whole corpse-side death edge is unobservable — the branch is guarded by
 * `if data entity @s {Health:0.0f}`, and it is where `on_death` fires. A vanilla
 * player has to click Respawn, so a corpse always exists for many ticks.
 *
 * Kept small enough that the 15 s respawn budget is untouched. The body is dead
 * for the whole hold, so nothing here forces a second death.
 */
const DEATH_SCREEN_HOLD_MS = 1_000;

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Death handler: record where and (best-effort) why the bot died, stop any
   * in-flight pathfinding, and fire the death waiters so the current long-running
   * step rejects promptly with a {@link BotDeathError} instead of hanging.
   */
  onDeath(this: MineflayerExecutor): void {
    if (this.death) return; // already recorded this death
    const bot = this.bot;
    let position: readonly [number, number, number] | undefined;
    const p = bot?.entity?.position;
    if (p) {
      // EXACT, never rounded. `Math.round` here produced a triple that is neither
      // a position nor the cell the body was in, and the death-loop stage read it
      // as a cell: a kill at `z = 4.6` (cell 4, inside the volume) rounded to 5
      // and was reported as a death outside the box that killed it. Whoever wants
      // a cell floors; whoever wants "the volume's selector matched this body"
      // asks `bodyInVolume`.
      position = [p.x, p.y, p.z];
    }
    const cause = likelyDeathCause(this.recentChat, bot?.username ?? "");
    const err = new BotDeathError(position, cause);
    this.death = err;
    this.lastDeath = err;
    this.deathSeq += 1;
    this.spawnSeqAtDeath = this.spawnSeq;
    this.respawnPacketsAtDeath = this.respawnPackets;
    this.windowsAtDeath = this.clientLoaded?.windows().length ?? 0;
    this.stopPathfinding();
    for (const waiter of this.deathWaiters) {
      waiter(err);
    }
    this.deathWaiters.clear();
    // The respawn a player takes, not the one a library takes for them (see the
    // `respawn: false` note in `connect`). Held one human beat so the corpse
    // exists for at least one server tick — which is where the engine's death
    // edge lives.
    const takeRespawn = (): void => {
      try {
        // The death screen's button, pressed once the respawn has not already
        // happened. mineflayer's own `respawn()` refuses whenever it believes the
        // bot alive, and a health update above zero on the corpse is enough to make
        // it believe that — so a bot with a wire asks the packet count instead.
        if (this.clientLoaded === undefined) {
          bot?.respawn();
        } else if (this.respawnPackets === this.respawnPacketsAtDeath && bot) {
          const payload = bot.supportFeature("respawnIsPayload") ? { payload: 0 } : { actionId: 0 };
          bot._client.write("client_command", payload);
        }
      } catch {
        // A failed respawn is not lost: `recoverFromDeath` bounds its own wait and
        // reports a missing respawn loudly.
      }
    };
    setTimeout(takeRespawn, DEATH_SCREEN_HOLD_MS).unref?.();
  },

  /**
   * Record a named entity's death (raw — not yet classified scripted-teardown vs
   * combat; see teardown.ts). Every other body dying near the bot every fight is
   * silent about here on purpose: only a NAMED body is an actor the compiler's
   * `despawn-actor` can tear down or a tiered fight can lose, and the island's
   * report-legibility gap was specifically about those.
   */
  onNamedEntityDeath(this: MineflayerExecutor, entity: Entity | undefined): void {
    if (!entity || entity.id === this.bot?.entity?.id) return; // the bot's own death is onDeath's
    const label = displayNameOf(entity);
    if (label === undefined || label === "") return;
    const p = entity.position;
    if (!p) return;
    this.namedEntityDeathLog.push({
      name: label,
      entityId: entity.id,
      position: [Math.floor(p.x), Math.floor(p.y), Math.floor(p.z)],
    });
  },

  /**
   * Race the bot dying against the operation `start` returns: resolves/rejects with
   * that operation, but rejects with the {@link BotDeathError} the instant a death is
   * observed (the underlying op keeps running but the pathfinder is already stopped
   * in {@link onDeath}). Used to abort the ~60s `pathfinder.goto` wait the moment the
   * bot dies.
   *
   * `start` is a THUNK, not a promise, and that is the whole of it: when a death is
   * already latched this returns without ever calling it, so there is no operation
   * to leave unobserved. Taking a promise instead meant the caller had already
   * built one by the time the early return fired — and a `pathfinder.goto` built and
   * then dropped rejects with `GoalChanged` the next time ANY goal is set, with no
   * handler attached, which is a fatal unhandled rejection under Node's default. A
   * boss that killed the bot mid-trade killed the whole run that way, three seconds
   * after the die-retry stage set its re-approach goal.
   */
  raceDeath<T>(this: MineflayerExecutor, start: () => Promise<T>): Promise<T> {
    if (this.death) return Promise.reject(this.death);
    const op = start();
    return new Promise<T>((resolve, reject) => {
      let settled = false;
      const onDeath = (err: BotDeathError): void => {
        if (settled) return;
        settled = true;
        reject(err);
      };
      this.deathWaiters.add(onDeath);
      op.then(
        (value) => {
          if (settled) return;
          settled = true;
          this.deathWaiters.delete(onDeath);
          resolve(value);
        },
        (err: unknown) => {
          if (settled) return;
          settled = true;
          this.deathWaiters.delete(onDeath);
          reject(err);
        },
      );
    });
  },

  deathDiagnostic(this: MineflayerExecutor): BotDeathError | undefined {
    return this.death;
  },

  /**
   * gap 7 (retry path): ready the bot to resume after a death — wait for it to respawn,
   * then clear the death latch so subsequent steps run against the live bot again. The
   * sequencer re-runs `select-class` afterwards (respawn drops class state).
   */
  async recoverFromDeath(this: MineflayerExecutor): Promise<void> {
    this.requireBot();
    // COUNTED, not listened for. mineflayer auto-respawns within a few dozen ms of
    // the death, which is sooner than a caller polling the death latch can arm a
    // `once("spawn")` — so the old listener routinely missed the respawn it was
    // waiting for and burned the whole 15s timeout before "resuming anyway". Free
    // before spec-0023; on the die-retry stage it is 15s per scripted death, two
    // per encounter, straight out of the run budget (observed live on
    // the keep-trial fixture). A counter cannot miss an event that already fired.
    const deadline = Date.now() + RESPAWN_TIMEOUT_MS;
    let respawned = false;
    while (Date.now() < deadline) {
      if (
        respawnLanded(
          this.spawnSeq > this.spawnSeqAtDeath,
          this.respawnPackets - this.respawnPacketsAtDeath,
          this.clientLoaded?.windows().slice(this.windowsAtDeath),
        )
      ) {
        respawned = true;
        break;
      }
      await delay(SPAWN_POLL_MS);
    }
    if (!respawned) {
      // Best effort: proceed anyway — the re-select-class teleport re-establishes a
      // known position regardless.
      process.stderr.write(
        `[death] no respawn observed within ${RESPAWN_TIMEOUT_MS}ms; resuming anyway\n`,
      );
    }
    this.death = undefined;
    // A respawn starts a new life: the bodies staged away in the old one are gone,
    // and their client ids must not keep a new body from being staged away.
    this.stagedIds.clear();
    this.lastHealth = undefined;
  },

  /** Every named-entity death this run observed, raw and unclassified — the
   * entrypoint classifies each scripted-teardown-vs-combat for the run report. */
  namedEntityDeaths(this: MineflayerExecutor): readonly NamedEntityDeath[] {
    return this.namedEntityDeathLog;
  },

  /**
   * Wait for a death NEWER than `seq` — the harness's own scripted one.
   *
   * Deliberately not {@link waitFor}, which THROWS the recorded
   * {@link BotDeathError} the instant one exists. That is right everywhere else
   * in the harness (a death mid-step is a failure to surface fast) and fatal
   * here, where the death IS the step: `waitFor(() => this.death !== undefined)`
   * threw on the very condition it was asked to wait for, so no die-retry trial
   * could ever complete and the harness's own scripted death was reported as the
   * content killing the bot (the-drowned-bell round 3).
   */
  async awaitDeathAfter(this: MineflayerExecutor, seq: number, timeoutMs: number): Promise<boolean> {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      if (this.deathSeq > seq) return true;
      // A bot the server dropped can never die; say so rather than time out.
      this.requireBot();
      if (Date.now() >= deadline) return false;
      await delay(SCORE_POLL_MS);
    }
  },

  /**
   * Wait out a death, note WHERE the bot came back, and ready it to fight again
   * **without moving it**.
   *
   * The re-arm does NOT replay `select-class`. The premise that "a respawn drops
   * class state" is false, and the replay is destructive (the-drowned-bell run
   * five):
   *
   *   * false, because the compiler seals `gamerule keep_inventory true` in every
   *     build — the kit survives the death — and class state lives in scoreboard
   *     values and player tags, which survive it too;
   *   * destructive, because `class_apply_<class>` ends in
   *     `teleport @s <campaign entry point>` and the `dw.class` trigger was, at the
   *     time, re-enabled for every player on every tick. Chatting it again therefore
   *     teleported the bot off the checkpoint it had just respawned on, back to the
   *     start of the delve — 150 blocks and eight levels away on the bell — and the
   *     "walk back to the encounter" leg then measured a route no dying player ever
   *     walks. Every trial recorded a truthful `respawn_pos` at the bonfire and then
   *     immediately made it a lie.
   *
   * The compiler now seals that warp shut: the class trigger is armed only
   * for a player who has not classed, so a replay would fail rather than teleport.
   * The rule here is unchanged and does not lean on it — a harness that re-classes
   * is stating something false about the run whether or not the pack lets it.
   *
   * So: measure the position, then re-equip what the player kept. Nothing here may
   * move the bot — the respawn point IS the thing under test.
   */
  async respawnAndRearm(this: MineflayerExecutor): Promise<{
    pos: Vec3Tuple | undefined;
    kitKept: boolean;
    /** Settles once the server has answered; sent at the client reading, awaited
     * by whoever records it, so asking never delays the bot. */
    reading: Promise<RespawnReading>;
  }> {
    const bot = this.requireBot();
    await this.recoverFromDeath();
    const p = bot.entity?.position;
    const pos: Vec3Tuple | undefined =
      p === undefined ? undefined : [Math.floor(p.x), Math.floor(p.y), Math.floor(p.z)];
    const client = {
      pos,
      age: serverAge(bot),
      respawnPackets: this.respawnPackets - this.respawnPacketsAtDeath,
    };
    const reading = this.readServerPos().then((server) => ({ client, server }));
    const kitKept = await this.rearmAfterRespawn();
    return { pos, kitKept, reading };
  },

  /**
   * The server's own `Pos` for the bot, with the game time it was read at.
   *
   * The delve seals `send_command_feedback false`, so a `data get` or `time query`
   * reply never reaches the bot. The answer is therefore spoken by `tellraw`: the
   * game time is stored first (`execute store … run time query gametime`) and the
   * same `tellraw` prints that tick and the bot's `Pos` NBT, then a second store
   * and `tellraw` print the tick after, so the reading carries the ticks it was
   * taken between. A refusal still reaches the sender with feedback off, and every
   * line is judged by the shared rejection rule; a refused or missing answer is
   * recorded as unread, never guessed.
   */
  async readServerPos(this: MineflayerExecutor): Promise<RespawnReading["server"]> {
    const bot = this.requireBot();
    const serial = ++this.posReads;
    const before = `[dw:pos ${serial} at `;
    const after = `[dw:pos ${serial} after `;
    const from = this.chatMark();
    const store = (key: string): string =>
      `/execute store result storage ${POS_READ_STORAGE} ${key} int 1 run time query gametime`;
    bot.chat(store("t0"));
    bot.chat(
      `/tellraw @s ${JSON.stringify([
        { text: before },
        { nbt: "t0", storage: POS_READ_STORAGE },
        { text: " " },
        { nbt: "Pos", entity: "@s" },
        { text: "]" },
      ])}`,
    );
    bot.chat(store("t1"));
    bot.chat(
      `/tellraw @s ${JSON.stringify([
        { text: after },
        { nbt: "t1", storage: POS_READ_STORAGE },
        { text: "]" },
      ])}`,
    );
    let pos: Vec3Tuple | undefined;
    let tickFrom: number | undefined;
    let tickTo: number | undefined;
    let refused: string | undefined;
    // Not `waitFor`: a death after the commands were sent is not this reading's
    // business, and the answer about the respawn it was sent for still arrives.
    const answered = (): boolean => {
      const lines = this.chatSince(from).lines;
      refused = lines.find((l) => isRejection(l));
      for (const l of lines) {
        if (l.includes(before)) {
          const m =
            /\[dw:pos \d+ at (\d+) \[(-?[\d.]+)d,\s*(-?[\d.]+)d,\s*(-?[\d.]+)d\]\]/.exec(l);
          if (m) {
            tickFrom = Number(m[1]);
            pos = [Number(m[2]), Number(m[3]), Number(m[4])];
          }
        }
        if (l.includes(after)) {
          const m = /\[dw:pos \d+ after (\d+)\]/.exec(l);
          if (m) tickTo = Number(m[1]);
        }
      }
      return refused !== undefined || (pos !== undefined && tickTo !== undefined);
    };
    const deadline = Date.now() + SCORE_TRACK_TIMEOUT_MS;
    while (!answered() && Date.now() < deadline) await delay(LEDGER_POLL_MS);
    if (refused !== undefined) return { unread: `refused: ${refused}` };
    if (pos === undefined || tickFrom === undefined || tickTo === undefined) {
      return {
        unread:
          `no complete answer in ${SCORE_TRACK_TIMEOUT_MS}ms: ` +
          JSON.stringify(this.chatSince(from).lines.filter((l) => l.includes("[dw:pos "))),
      };
    }
    return { pos, tickFrom, tickTo };
  },
};
