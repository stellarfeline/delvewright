// MineflayerExecutor: the bot's connection: connect, the event handlers, the wait primitive, teardown.

import type { Bot } from "mineflayer";
import type { Entity } from "prismarine-entity";
import type { StageName } from "../report.ts";
import { respawnReseats } from "../combat.ts";
import { createHarnessBot } from "../client-loaded.ts";
import type { PackPush } from "../resource-pack.ts";
import { traceLoadWindows, type LoadWindowRecord, type TracedBot } from "../load-window.ts";
import { serverAge } from "./death.ts";
import { answersTrigger } from "./trigger.ts";
import { pathfinder } from "./walk.ts";
import { holdWalkedClimbableNodes, type ClimbHoldBot } from "../movement.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** Connection + identity for the bot. Sourced from the environment (see below). */
export interface BotConfig {
  readonly host: string;
  readonly port: number;
  readonly username: string;
  /** Pinned per ADR-0009; mineflayer's max supported version is 1.21.11. */
  readonly version: string;
  /** `offline` for local/CI (offline-mode server); `microsoft` for real accounts. */
  readonly auth: "offline" | "microsoft";
}

/** The pinned Minecraft version (ADR-0009). Single source of truth for the harness. */
export const PINNED_MC_VERSION = "1.21.11";

/**
 * Build a {@link BotConfig} from environment variables, with local-testing
 * defaults:
 *   DELVEWRIGHT_MC_HOST      (default `127.0.0.1`)
 *   DELVEWRIGHT_MC_PORT      (default `25565`)
 *   DELVEWRIGHT_BOT_USERNAME (default `delve-bot`)
 *   DELVEWRIGHT_MC_VERSION   (default `1.21.11`, the ADR-0009 pin)
 *   DELVEWRIGHT_MC_AUTH      (`offline` | `microsoft`, default `offline`)
 */
export function botConfigFromEnv(
  env: Record<string, string | undefined> = process.env,
): BotConfig {
  const portRaw = env["DELVEWRIGHT_MC_PORT"] ?? "25565";
  const port = Number.parseInt(portRaw, 10);
  if (!Number.isInteger(port) || port <= 0 || port > 65535) {
    throw new Error(
      `DELVEWRIGHT_MC_PORT must be a valid TCP port, got ${JSON.stringify(portRaw)}`,
    );
  }
  const authRaw = env["DELVEWRIGHT_MC_AUTH"] ?? "offline";
  if (authRaw !== "offline" && authRaw !== "microsoft") {
    throw new Error(
      `DELVEWRIGHT_MC_AUTH must be 'offline' or 'microsoft', got ${JSON.stringify(authRaw)}`,
    );
  }
  return {
    host: env["DELVEWRIGHT_MC_HOST"] ?? "127.0.0.1",
    port,
    username: env["DELVEWRIGHT_BOT_USERNAME"] ?? "delve-bot",
    version: env["DELVEWRIGHT_MC_VERSION"] ?? PINNED_MC_VERSION,
    auth: authRaw,
  };
}

/** Recent chat lines retained for death-cause diagnosis. */
const CHAT_BUFFER = 256;

export function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Reject with a labelled error if `promise` does not settle within `ms`. */
export function withTimeout<T>(promise: Promise<T>, ms: number, what: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  const guard = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new Error(`${what}: timed out after ${ms}ms`)), ms);
  });
  return Promise.race([promise, guard]).finally(() => clearTimeout(timer));
}

export function fmt(p: { x: number; y: number; z: number }): string {
  return `[${p.x.toFixed(1)}, ${p.y.toFixed(1)}, ${p.z.toFixed(1)}]`;
}

/**
 * A server's disconnect reason as one readable line. mineflayer hands it over as
 * whatever the packet carried — a plain string, a JSON text component, or (1.20.3+)
 * an NBT compound — so a translate key or text is dug out where there is one and
 * the raw value is printed otherwise, never dropped.
 */
export function disconnectReason(reason: unknown): string {
  if (typeof reason === "string") {
    try {
      return disconnectReason(JSON.parse(reason));
    } catch {
      return reason;
    }
  }
  const pick = (o: unknown): string | undefined => {
    if (o === null || typeof o !== "object") return typeof o === "string" ? o : undefined;
    const r = o as Record<string, unknown>;
    // NBT: { type: "compound", value: { translate: { type: "string", value } } }
    if (r["type"] !== undefined && "value" in r) return pick(r["value"]);
    return pick(r["translate"]) ?? pick(r["text"]) ?? pick(r["fallback"]);
  };
  const found = pick(reason);
  if (found !== undefined && found !== "") return found;
  return JSON.stringify(reason) ?? String(reason);
}

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /** Connect and resolve once the bot has spawned into the world. */
  async connect(this: MineflayerExecutor): Promise<void> {
    const { bot, loaded, pack } = createHarnessBot({
      host: this.config.host,
      port: this.config.port,
      username: this.config.username,
      version: this.config.version,
      auth: this.config.auth,
      // mineflayer's auto-respawn answers the death packet in the SAME
      // event-loop turn it arrives in, so the bot is alive again on the very next
      // server tick. No human is: the death screen requires a click, and the
      // engine's death edge is specified ON THE CORPSE (spec-0031: the
      // `deathCount` edge is armed pre-respawn, and `cp_respawn_check` reads it
      // with `if data entity @s {Health:0.0f}`). A corpse that never exists for a
      // whole tick makes the whole `on_death` branch unreachable — and a validator
      // that cannot observe a mechanism because of its own client library is a
      // validator that will report the mechanism green forever.
      //
      // So the respawn is taken MANUALLY, one human beat later ({@link
      // DEATH_SCREEN_HOLD_MS}). Nothing else changes: every existing wait is
      // counted off `spawnSeq`, which still rises exactly once per respawn.
      respawn: false,
    });
    this.bot = bot;
    this.clientLoaded = loaded;
    this.packState = pack;
    // Every gamemode change is written down with where the body is and which
    // step is under way: a cutscene's control window is otherwise invisible
    // in a run's log, and a stranding reads as a path failure.
    {
      let seen: string | undefined;
      bot.on("game", () => {
        const mode = bot.game?.gameMode as string | undefined;
        if (mode === seen) return;
        process.stderr.write(
          `[gamemode] ${seen ?? "?"} -> ${mode ?? "?"} at ` +
            `${bot.entity ? fmt(bot.entity.position) : "?"} during step ${this.currentStep}\n`,
        );
        seen = mode;
      });
    }
    // Installed in the turn the bot is created, before its `login` can arrive:
    // the join is the first window it has to see.
    this.loadTracer = traceLoadWindows(bot as unknown as TracedBot, {
      stage: () => this.stageNow,
      step: () => this.currentStep,
      lethal: () => this.lethalBoxes,
    });
    bot.loadPlugin(pathfinder);
    // The pathfinder aims a ladder cell a block above the feet; a body walking
    // along the panel's face never rises to it (spec-0099). See the rule.
    holdWalkedClimbableNodes(bot as unknown as ClimbHoldBot, (line) => process.stderr.write(line));
    this.installHandlers(bot);

    await new Promise<void>((resolve, reject) => {
      const onSpawn = (): void => {
        cleanup();
        resolve();
      };
      const onError = (err: Error): void => {
        cleanup();
        reject(err);
      };
      const onEnd = (reason: string): void => {
        cleanup();
        reject(new Error(`bot disconnected before spawn: ${reason}`));
      };
      const onKicked = (reason: string): void => {
        cleanup();
        reject(new Error(`bot kicked before spawn: ${reason}`));
      };
      const cleanup = (): void => {
        bot.removeListener("spawn", onSpawn);
        bot.removeListener("error", onError);
        bot.removeListener("end", onEnd);
        bot.removeListener("kicked", onKicked);
      };
      bot.once("spawn", onSpawn);
      bot.once("error", onError);
      bot.once("end", onEnd);
      bot.once("kicked", onKicked);
    });
    // Entity-tracker settle race (2026-08-06 island triage): `bot.entities` is empty
    // for a few seconds after spawn while world-persisted entities' packets are still
    // arriving. Waiting here, once, before the run does anything with `bot.entities`
    // (the crosshair sweep foremost) is cheaper and more honest than teaching every
    // caller of `hitboxesNear` to guess whether an empty read means "nothing there"
    // or "not yet told" — see entity-settle.ts.
    await this.awaitEntitySettle();
  },

  requireBot(this: MineflayerExecutor): Bot {
    if (!this.bot) {
      throw new Error("executor is not connected; call connect() first");
    }
    if (this.lostConnection !== undefined) {
      throw new Error(
        `the server disconnected the bot (${this.lostConnection}) — nothing after that ` +
          `moment was performed or observed`,
      );
    }
    return this.bot;
  },

  /**
   * Wire the always-on listeners: completion-marker + chat-ring capture, and the
   * death handler. Called from {@link connect} and from {@link attachBot} (tests).
   */
  installHandlers(this: MineflayerExecutor, bot: Bot): void {
    // Capture completion markers from the moment we connect: an objective's marker
    // is broadcast the instant its score flips — usually DURING the step's walk,
    // before the executor gets to wait for it — and the campaign marker lands during
    // the last objective step, so both must be buffered as they arrive. The same
    // stream feeds the recent-chat ring the death diagnostic mines for a cause.
    bot.on("messagestr", (message: string) => {
      // A bracket marker is protocol, not speech: nothing else reads it.
      if (this.brackets.observe(message)) return;
      this.observeMarker(message);
      this.observeCensus(message);
      this.observeMuster(message);
      if (this.wordWatch && message.includes(this.wordWatch.needle)) {
        this.wordWatch.seen = true;
      }
      if (this.trigger && answersTrigger(message, this.trigger.objective)) {
        this.trigger.lines.push(message);
      }
      this.recentChat.push(message);
      this.chatSeen += 1;
      if (this.recentChat.length > CHAT_BUFFER) {
        this.recentChat.shift();
      }
    });
    // The scoreboard ledgers, straight off the wire. Both packets are
    // read because 1.21.11 split "set a score" and "clear a score" into two — a
    // reader that watches only the first reports a cleared purse as its last known
    // value, which is the one direction a currency assertion must never drift in.
    this.installScoreObserver(bot);
    bot.on("death", () => this.onDeath());
    // A mid-run disconnect is recorded the moment it happens (see lostConnection).
    // `close()` detaches the bot before it ends it, so our own quit never lands here.
    bot.on("kicked", (reason: unknown) => {
      if (this.bot !== bot) return;
      this.lostConnection ??= `kicked: ${disconnectReason(reason)}`;
      process.stderr.write(`[connection] ${this.lostConnection}\n`);
    });
    bot.on("end", (reason: unknown) => {
      if (this.bot !== bot) return;
      this.lostConnection ??= `connection ended: ${disconnectReason(reason)}`;
      process.stderr.write(`[connection] ${this.lostConnection}\n`);
    });
    // Scripted-teardown death classification (2026-08-06 island triage): `entityDead`
    // fires on the LivingEntity death status packet, while the entity's last known
    // position is still readable — unlike `entityGone`, which also fires for an
    // ordinary despawn (out of render distance, dimension change) and says nothing
    // about a death. Only named bodies are actors the compiler ever tears down or a
    // story fight ever names; an unnamed mob's death is not this run report's concern.
    bot.on("entityDead", (entity: Entity) => {
      if (entity) this.deadBodies.add(entity.id);
      this.onNamedEntityDeath(entity);
    });
    // Counted from connect, so a respawn is never missed by a listener armed too
    // late (see recoverFromDeath).
    bot.on("respawn", () => {
      this.respawnPackets += 1;
    });
    bot.on("spawn", () => {
      this.spawnSeq += 1;
      // A respawn after the first join is a death-respawn at the last-rested
      // bonfire, which fires the rest's own hooks (spec-0016 §1): every
      // `respawns_on_rest` wave is back, exactly as after a rest.
      if (this.spawnSeq > 1) {
        respawnReseats(this.restedAt, this.currentStep);
        this.seatEpoch += 1;
        this.lastSpawnAt = Date.now();
        this.lastSpawnAge = serverAge(bot);
      }
    });
    // Self-defense attribution (souls ladder). PRIMARY channel: mineflayer 4.37 turns
    // the 1.20+ `damage_event` packet into `entityHurt(entity, source)`, where `source`
    // is the entity the server names as responsible (`sourceCauseId`). When the hurt
    // entity is the bot, that source IS the attacker — no guessing needed.
    bot.on("entityHurt", (entity: Entity, source?: Entity) => {
      if (!entity || entity.id !== bot.entity?.id) return;
      this.onBotDamaged(source?.id);
    });
    // FALLBACK: `sourceCauseId` is 0 when the server names no responsible entity (and
    // the lookup misses if that entity is not tracked client-side), so a hit can arrive
    // with no source. A health DROP with no fresh attribution is then blamed on the
    // nearest hostile in melee reach — and on nothing at all if none is close, so a
    // trap or a fall never makes the bot swing at a bystander.
    bot.on("health", () => this.onHealthUpdate());
    // gap 8: mineflayer applies a server position packet to
    // `bot.entity.position` and THEN emits `forcedMove` (lib/plugins/physics.js).
    // A large horizontal jump is the compiler's cross-area `teleport` landing; stop
    // the pathfinder so any path/goal computed in the old area is dropped rather than
    // fought or resumed across the void (the "No path to the goal!" / "Path was
    // stopped" race documented in the nobodys-cave gap-8 field notes).
    bot.on("forcedMove", () => this.onForcedMove());
    bot.on("physicsTick", () => {
      const p = bot.entity?.position;
      if (p) this.tickPos = { x: p.x, y: p.y, z: p.z };
    });
    // spec-0100 §4.7: the sculk family's ear, from connect — a sensor click is a
    // block update, a shriek a `world_event` 3007, and a darkness effect or a
    // warden fails the run.
    bot.on("blockUpdate", (_old: unknown, block: unknown) => {
      const b = block as
        | { position?: { x: number; y: number; z: number }; getProperties?: () => Record<string, unknown> }
        | null;
      if (!b?.position || typeof b.getProperties !== "function") return;
      this.sculk.onBlockUpdate([b.position.x, b.position.y, b.position.z], b.getProperties(), Date.now());
    });
    const client = (bot as unknown as { _client?: { on: (e: string, f: (p: unknown) => void) => void } })
      ._client;
    client?.on("world_event", (packet: unknown) => {
      const p = packet as { effectId?: number; location?: { x: number; y: number; z: number } };
      if (typeof p.effectId !== "number" || !p.location) return;
      this.sculk.onWorldEvent(p.effectId, [p.location.x, p.location.y, p.location.z], Date.now());
    });
    bot.on("entityEffect", (entity: Entity, effect: { id: number }) => {
      if (!entity || entity.id !== bot.entity?.id) return;
      const registry = (bot as unknown as { registry?: { effects?: Record<number, { name?: string }> } })
        .registry;
      this.sculk.onSelfEffect(registry?.effects?.[effect.id]?.name);
    });
    bot.on("entitySpawn", (entity: Entity) => {
      this.sculk.onSpawn(entity?.name);
    });
  },

  /**
   * Abandon whatever the pathfinder is doing, leaving it in a state the NEXT `goto`
   * can actually use — the synchronous half of {@link NavigationOwner}, which is
   * what the death handler and the forced-move handler need.
   *
   * The stop/`setGoal(null)` pairing, and why it is a pairing, live on
   * {@link NavigationOwner.stopNow}. What matters here: the cancelled trip stays
   * registered with the owner, so its `GoalChanged`/`PathStopped` rejection is still
   * observed when it arrives, and the next hop waits for it before setting a goal.
   */
  stopPathfinding(this: MineflayerExecutor): void {
    this.nav.stopNow();
  },

  /**
   * Adopt the critical path's campaign id and step count. Markers are scoped to this
   * campaign, so a marker from other content can never satisfy a step. Called by the
   * entrypoint before the run starts.
   */
  useCampaign(this: MineflayerExecutor, campaignId: string): void {
    this.campaignId = campaignId;
  },

  /** Sequencer hook: the run has moved on to step `index`. Attribution only. */
  beginStep(this: MineflayerExecutor, index: number): void {
    this.currentStep = index;
    // A trigger echo belongs to the step that sent it. Dropping it here keeps a
    // later step's timeout from quoting the previous step's `/trigger` as if it
    // were its own — a diagnostic that names the wrong command is worse than none.
    this.trigger = undefined;
  },

  /**
   * Test/advanced seam: adopt an already-created (or fake) bot and install the same
   * handlers `connect()` wires, without the network path. Unit tests use this to
   * drive death/cutscene behaviour against a mocked bot.
   */
  attachBot(this: MineflayerExecutor, bot: Bot): void {
    this.bot = bot;
    this.installHandlers(bot);
  },

  /**
   * The death recorded so far, if any. Diagnostic accessor (also lets tests assert the
   * captured position/cause after a simulated death).
   */
  /**
   * The labelled stage the executor is inside right now — what a crash report has
   * to say to stop a harness fault reading as a content verdict on whichever stage
   * happened to be next.
   */
  /** Every resource pack the server pushed to this bot — see resource-pack.ts. */
  resourcePackPushes(this: MineflayerExecutor): readonly PackPush[] {
    return this.packState?.pushes() ?? [];
  },

  /** Every load window this bot opened, in order — see load-window.ts. */
  loadWindows(this: MineflayerExecutor): readonly LoadWindowRecord[] {
    return this.loadTracer?.windows() ?? [];
  },

  currentStage(this: MineflayerExecutor): StageName {
    return this.stageNow;
  },

  /** Disconnect the bot, if connected. Safe to call more than once. */
  close(this: MineflayerExecutor): void {
    if (this.bot) {
      const bot = this.bot;
      this.bot = undefined;
      bot.end();
    }
  },

  /**
   * Poll `predicate` every `pollMs` until it holds (→ true) or `timeoutMs` elapses
   * (→ false). Death-aware: throws the recorded {@link BotDeathError} the moment the
   * bot dies, so a transport/footing wait never outlives a death. A pure timing helper
   * — no game logic.
   */
  async waitFor(
    this: MineflayerExecutor,
    predicate: () => boolean,
    timeoutMs: number,
    pollMs: number,
  ): Promise<boolean> {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      if (this.death) throw this.death;
      if (predicate()) return true;
      if (Date.now() >= deadline) return false;
      await delay(pollMs);
    }
  },
};
