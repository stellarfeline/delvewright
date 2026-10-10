// mineflayer-backed StepExecutor. Connects a headless bot to a pinned 1.21.11
// server and drives the critical path against the amended bot-interaction
// contract (spec-0002, 2026-07-30).
//
// Interaction channel (settled): Minecraft 1.21.6+ routes NPC dialogue / class
// selection through the server-driven dialog system. mineflayer 4.37.x exposes no
// high-level dialog API and cannot reliably emit a dialog button click, so the
// compiler emits every dialog button as a `run_command` firing a `/trigger`, and
// the bot drives the same outcome by chatting that exact command (`bot.chat`).
// select-class / talk-to therefore just send `step.command`; talk-to walks to the
// NPC first (realism + reach mechanics that some dialogs gate on).

import type { Bot } from "mineflayer";
import type { Vec3Tuple } from "./critical-path.ts";
import type { StepExecutor } from "./sequencer.ts";
import { BotDeathError } from "./death.ts";
import { NavigationOwner } from "./navigation.ts";
import type { NamedEntityDeath } from "./teardown.ts";
import type { StageName } from "./report.ts";
import {
  type CombatPlan,
  type DeathTrialRecord,
  type EncounterPhase,
  type FightAttribution,
  type PerformedRest,
  type RunBack,
} from "./combat.ts";
import { type MusterBody, type MusterSummary, type MusterVerdict } from "./muster.ts";
import { ReplyBrackets } from "./command-reply.ts";
import { type Box, type DeathPlan, type LethalTrial } from "./death-loop.ts";
import { type ClientLoadedState } from "./client-loaded.ts";
import type { ResourcePackState } from "./resource-pack.ts";
import { type LoadWindowTracer } from "./load-window.ts";
import { type CensusMob, type CensusSummary } from "./markers.ts";
import { RepaintWatch } from "./repaint.ts";
import { SculkEar } from "./sculk.ts";
import type { PulsePlan } from "./pulse.ts";
import { type Waypoints } from "./waypoints.ts";
import type { BotConfig } from "./executor/connection.ts";
import { LOOP_CROSS_TIMEOUT_MS } from "./executor/loop.ts";
import { ENTITY_SETTLE_TIMEOUT_MS } from "./executor/settle.ts";
import type { StagedRemoval } from "./executor/staging.ts";
import type { TriggerEcho } from "./executor/trigger.ts";
import { methods as connectionMethods } from "./executor/connection.ts";
import { methods as chatMethods } from "./executor/chat.ts";
import { methods as objectiveMethods } from "./executor/objective.ts";
import { methods as triggerMethods } from "./executor/trigger.ts";
import { methods as interactMethods } from "./executor/interact.ts";
import { methods as classMethods } from "./executor/class.ts";
import { methods as talkMethods } from "./executor/talk.ts";
import { methods as walkMethods } from "./executor/walk.ts";
import { methods as timedGateMethods } from "./executor/timed-gate.ts";
import { methods as climbMethods } from "./executor/climb.ts";
import { methods as cutsceneMethods } from "./executor/cutscene.ts";
import { methods as transportMethods } from "./executor/transport.ts";
import { methods as loopMethods } from "./executor/loop.ts";
import { methods as sustainMethods } from "./executor/sustain.ts";
import { methods as deathMethods } from "./executor/death.ts";
import { methods as repaintMethods } from "./executor/repaint.ts";
import { methods as pulseMethods } from "./executor/pulse.ts";
import { methods as scoreMethods } from "./executor/score.ts";
import { methods as lethalMethods } from "./executor/lethal.ts";
import { methods as stakeMethods } from "./executor/stake.ts";
import { methods as waveMethods } from "./executor/wave.ts";
import { methods as dieRetryMethods } from "./executor/die-retry.ts";
import { methods as stagingMethods } from "./executor/staging.ts";
import { methods as musterMethods } from "./executor/muster.ts";
import { methods as restMethods } from "./executor/rest.ts";
import { methods as witnessMethods } from "./executor/witness.ts";
import { methods as watchMethods } from "./executor/watch.ts";
import type { WatchLedger } from "./watch.ts";
import { methods as crosshairMethods } from "./executor/crosshair.ts";
import { methods as collectMethods } from "./executor/collect.ts";
import { methods as settleMethods } from "./executor/settle.ts";

export {
  type BotConfig,
  PINNED_MC_VERSION,
  botConfigFromEnv,
  disconnectReason,
} from "./executor/connection.ts";
export { RESPAWN_PROTECTION_TICKS, respawnLanded } from "./executor/death.ts";
export { completionWindowMs } from "./executor/objective.ts";
export { type StagedRemoval } from "./executor/staging.ts";
export { type GateAssist } from "./executor/timed-gate.ts";
export {
  type TriggerEcho,
  swallowedTriggerVerdict,
  triggerObjective,
  answersTrigger,
} from "./executor/trigger.ts";
export { type Unstick, type LegSettled, replayLegWithRecovery } from "./executor/walk.ts";
export { displayNameOf, type NamePreference, isWaveMob, isLivingBody } from "./executor/wave.ts";

/**
 * A mineflayer-backed executor. Construct, `await connect()`, then hand it to
 * `runSequence`. `close()` disconnects the bot. Not reusable across servers.
 */
export class MineflayerExecutor implements StepExecutor {
  readonly config: BotConfig;
  bot: Bot | undefined;
  /**
   * The campaign whose markers this run accepts (from `critical-path.json`).
   * Markers naming any other campaign are ignored — a completion belonging to other
   * content can never satisfy this run's steps.
   */
  campaignId: string | undefined;
  /**
   * Objective ids whose anchored completion marker has arrived, and the 0-based
   * step index that was executing when it did. Buffered from connect, because an
   * objective often completes DURING its step's walk (before the executor gets to
   * wait for it) and campaign completion lands during the last objective step.
   */
  readonly completedObjectives = new Map<string, number>();
  /** The repaint ledger (spec-0080 §5.2), when the build repaints anything. */
  repaintWatch: RepaintWatch | undefined;
  /** The sculk family's ear (spec-0100 §4.7): sensor clicks, shrieks, darkness
   * on the bot and warden spawns, from connect. */
  readonly sculk = new SculkEar();
  /** The pulses and their stations (spec-0102 §5.3), when the build declares any. */
  pulsePlan: PulsePlan | undefined;
  /** Station key → `undefined` when the station judged as owed, else the failure. */
  readonly pulseRecorded = new Map<string, string | undefined>();
  /** Every station stood at, with what it heard — the run report's `pulse_stations`. */
  readonly pulseStations: import("./pulse.ts").StationRecord[] = [];
  /**
   * How many times each marker token has been broadcast this run. A repeatable
   * trigger broadcasts its marker every time it fires, and a hit count on the
   * path is N trigger steps (spec-0082 §10) — so the second and later steps owe
   * a FRESH arrival, which the first-arrival map above cannot tell apart.
   */
  readonly markerArrivals = new Map<string, number>();
  /** How many times each trigger step has been performed this run. */
  readonly triggerPerformances = new Map<string, number>();
  /**
   * The step index at which the campaign-completion marker arrived, if it has.
   * Endgame discipline: campaign completion belongs to the LAST objective step; its
   * arrival any earlier means the path is incoherent (a branch completed the
   * campaign while steps remained) and the run is failed on the spot rather than
   * marching through hollow remaining steps.
   */
  campaignCompleteAtStep: number | undefined;
  /** The step index currently executing, for marker attribution. */
  currentStep = -1;
  /**
   * The cutscene allowance of the step under way, in seconds: the larger of
   * its `cutscene_seconds` and `en_route_cutscene_seconds`, or `undefined` when
   * it declares neither. Set by {@link beforeStep}; read where a walk finds
   * control taken ({@link awaitControlEnRoute}).
   */
  stepCutsceneAllowanceS: number | undefined;
  stepLabel = "";
  /**
   * gap 7 (death): set once when the bot dies; long waits race against it so a death
   * fails FAST with a diagnostic instead of respawning and pathfinding across the void.
   */
  death: BotDeathError | undefined;
  /**
   * Why the server dropped the connection mid-run, once it has. Every act after
   * a disconnect is a no-op against a socket that is gone — `bot.chat` sends
   * nothing and the chat stream answers nothing — so an unrecorded kick turns
   * into whatever the NEXT wait times out on. At vesperhold's choir that was a
   * scripted death "that never landed", blamed on the op seed, when the server
   * had kicked the bot three seconds earlier. {@link requireBot} throws this.
   */
  lostConnection: string | undefined;
  /**
   * The one owner of the pathfinder's goal. Every trip is issued and collected
   * through it, so a hop this executor walks away from (a death, a timeout, a
   * stalker winning the race) still has its rejection read — see navigation.ts.
   */
  readonly nav = new NavigationOwner(() => this.bot?.pathfinder);
  /**
   * Which labelled stage the executor is inside RIGHT NOW. Read by the crash
   * reporter, which has no other way to know: the ladder's stages are assembled
   * from the outside once the run is over, and a process that dies mid-run never
   * gets there. Only the boundaries that a crash could plausibly sit inside are
   * marked — a crash anywhere else is on the critical path by construction.
   */
  stageNow: StageName = "critical-path";
  /**
   * How many deaths this run has observed. The die-retry stage waits for a FRESH
   * death rather than for `this.death` to be set, so a leftover latch (the bot
   * died on the way back from the last one) can never be mistaken for the next
   * scripted death and credited as a trial that never happened.
   */
  deathSeq = 0;
  /** The most recent death, kept after its recovery clears {@link death}: what a
   * caller that counts deaths with {@link deathSeq} reads to say where one was. */
  lastDeath: BotDeathError | undefined;
  /** Every stretch the server held the bot unhurtable (see load-window.ts). */
  loadTracer: LoadWindowTracer | undefined;
  /** The bot's `player_loaded` tracker; `undefined` for a bot adopted by {@link attachBot}. */
  clientLoaded: ClientLoadedState | undefined;
  /** The resource packs the server pushed (spec-0084 §11); `undefined` for an adopted bot. */
  packState: ResourcePackState | undefined;
  /** How many `spawn` events this run has seen (login, then every respawn). */
  spawnSeq = 0;
  /** {@link spawnSeq} at the moment of the last death — the respawn wait watches
   * for a spawn NEWER than this, so a respawn that beats the wait is never lost. */
  spawnSeqAtDeath = 0;
  /** `respawn` packets this run has received, and the count at the last death. */
  respawnPackets = 0;
  respawnPacketsAtDeath = 0;
  /** How many load windows the wire tracker had opened at the last death. */
  windowsAtDeath = 0;
  /** Which reply belongs to which command. See {@link refusalOf}. */
  readonly brackets = new ReplyBrackets();
  /** The record's watching bodies and what the walk found about them (spec-0101 §5.4). */
  watch: WatchLedger | undefined;
  /**
   * Every entity the server has announced dead (the entity-event death status),
   * by id. A body in its death throes is still in the client's entity table, and a
   * shot it loosed before it fell can still land on the bot.
   */
  readonly deadBodies = new Set<number>();
  /** Serial for {@link readServerPos}'s answer markers. */
  posReads = 0;
  /** One-shot callbacks armed by {@link raceDeath}, fired on death. */
  readonly deathWaiters = new Set<(err: BotDeathError) => void>();
  /** Ring buffer of recent chat lines, mined for the death-cause message. */
  readonly recentChat: string[] = [];
  /**
   * **How many chat lines this run has seen** — the ring's index space, and the
   * only safe way to read "everything said since I sent that command".
   *
   * `const from = this.recentChat.length` is not that, and the difference is a
   * silent pass. The ring is bounded, so once it is full its `length` never
   * changes again: `from` is the cap, `slice(from)` is `[]` forever, and a caller
   * that reads a command's reply that way finds nothing however loudly the server
   * answered. Measured here — a `/say` the server logged and broadcast was read by
   * this executor as silence, and the three refusal readers that mark the ring the
   * same way had been looking at `[]` for the whole back half of every run since
   * the ring was introduced. A command whose response nobody reads cannot fail.
   *
   * So a reader marks {@link chatMark} and reads {@link chatSince}, which also
   * says how many lines the ring DROPPED between the two — an answer that was
   * evicted is not an answer that never came.
   */
  chatSeen = 0;
  /**
   * One exact line the run is currently watching for, and whether it has
   * arrived. Armed around a single act (walking into a lethal volume) rather than
   * mined out of {@link recentChat}, because that ring holds sixteen lines and a
   * death broadcasts several — a wording assertion that can be pushed out of a ring
   * is an intermittent test, which is an under-specified one.
   */
  wordWatch: { readonly needle: string; seen: boolean } | undefined;
  /**
   * The scoreboard ledgers this run is observing, `objective → entry →
   * value`, fed straight off the wire.
   *
   * **Why the raw packet and not `bot.scoreboards`.** mineflayer 4.37's scoreboard
   * plugin gates every score update on `packet.action === 0`, and 1.21.11 has no
   * `action` field on `scoreboard_score` at all (it was split out into
   * `reset_score`), so its model never updates on the pinned version. The packet
   * itself decodes perfectly — `{itemName, scoreName, value}` — so the harness
   * reads it directly. This is observation, not game logic: no delve score is ever
   * written from here.
   */
  readonly scores = new Map<string, Map<string, number>>();
  /** Which display slot the harness put each tracked objective in. */
  readonly trackedSlots = new Map<string, string>();
  /**
   * The lethal volumes the build declares, as impassable boxes for the
   * PATHFINDER. The compiler already treats them as impassable in every route
   * proof; without the same fact here the bot walks its way back from a death
   * straight through the hazard it just died in, and a run intermittently dies
   * twice for reasons no content author could reproduce.
   *
   * These are each volume’s `keep_out` box and NOT its `region`: the volume
   * kills on hitbox intersection, so the cell beside its face is a cell the bot
   * dies standing in. Excluding only the region is what put the bot on
   * `[12, 65, 21]` — one cell east of a pit — and killed it there twice, with
   * every compile-time proof green.
   */
  lethalBoxes: readonly Box[] = [];
  /** The regions of the volumes live from world-load — always excluded. */
  unstagedBoxes: readonly Box[] = [];
  /**
   * The volumes live from a story stage (spec-0088): excluded per walk leg, by
   * asking their gate's terms before the leg ({@link refreshStagedExclusion}).
   */
  stagedVolumes: readonly DeathPlan["volumes"][number][] = [];
  /** Suspended for exactly one walk: the deliberate step INTO a volume. */
  lethalExclusionSuspended = false;
  /** Every walk into a lethal volume this run made, and what it observed. */
  readonly lethalTrials: LethalTrial[] = [];
  /** Why the death-loop stage did not run, when it did not. */
  deathLoopSkip: string | undefined;
  /** Trials of the death loop that ran to their own end — not cut off mid-way. */
  lethalTrialsFinished = 0;
  /** Serial number of the last gate term asked — see {@link askTerm}. */
  gateAsks = 0;
  /**
   * The build's death contract — what the campaign PROMISES a death does.
   * Absent → a run in which nothing about dying is asserted at all (which is
   * the gap this stage exists to close).
   */
  deathPlan: DeathPlan | undefined;
  /** The `/trigger` the current step sent, and the server's answer to it.
   * Replaced by each new trigger; see {@link swallowedTriggerVerdict}. */
  trigger: TriggerEcho | undefined;
  /**
   * gap 8: the bot position captured at the previous server-forced move
   * (`forcedMove`). A forced move whose horizontal delta from this reaches
   * {@link TRANSPORT_JUMP_BLOCKS} is a cross-area teleport; used to reset the
   * pathfinder so a path computed in the old area cannot survive the jump.
   */
  lastForcedPos: { x: number; y: number; z: number } | undefined;
  /**
   * spec-0086 §6: where the bot stood at the last physics tick — the position a
   * server-forced move's delta is measured from. mineflayer applies the position
   * packet between ticks and then emits `forcedMove`, so this is the position the
   * move started from.
   */
  tickPos: { x: number; y: number; z: number } | undefined;
  /** The loops the path exercises, by id → offset (spec-0086 §6). A forced move
   * during a plain walk whose delta is one of these fails that walk. */
  loopOffsets = new Map<string, Vec3Tuple>();
  /** While a loop step crosses: every forced move's delta, in order. */
  loopWatch: { deltas: Vec3Tuple[] } | undefined;
  /** A forced move during a plain walk that equals a loop's offset: the proof
   * and the game disagree about the loop's gate. Thrown by the walk it ended. */
  loopFault: Error | undefined;
  /** Grace (ms) added onto a cutscene's declared length before giving up. */
  readonly cutsceneGraceMs: number;
  /** Hard ceiling (ms) on the post-spawn entity-settle wait. Overridable
   * (`DELVEWRIGHT_ENTITY_SETTLE_TIMEOUT_MS`) so a test can shorten the give-up
   * path without waiting out the production default. */
  readonly entitySettleTimeoutMs: number;
  /** spec-0086 §6: how long one loop crossing may take. Overridable
   * (`DELVEWRIGHT_LOOP_CROSS_TIMEOUT_MS`) so a test can reach the give-up path
   * without waiting out the production default. */
  readonly loopCrossTimeoutMs: number;
  /**
   * The compiler's proven per-leg critical-path waypoints (keyed by
   * destination anchor). When a walked step's target has a leg here, `walkTo`
   * replays it as successive nearby goals so each mineflayer A* solve is trivial —
   * instead of one distant goal that strands the bot on a large open winding cave.
   * Absent → the original single distant-goal behavior (fallback). Compiler-proven
   * navigation data, not a route the harness computes.
   */
  waypoints: Waypoints | undefined;
  /**
   * The delve's own statement of which entity kinds are never a combat target,
   * off `critical-path.json` (format 4). There is deliberately no default: the
   * only fallback available is a set of entity names living in this file, which
   * is the thing the field exists to delete. The path parser refuses a document
   * without it, and {@link requireNonCombatants} refuses a run that never wired
   * it through.
   */
  nonCombatants: ReadonlySet<string> | undefined;
  /**
   * spec-0023: the compiler's combat plan — which encounters are mandatory, what
   * the content bills each as, and which checkpoint governs a death at it.
   * Absent (a delve with no mandatory combat, or an older build) → `kill` behaves
   * exactly as it did before spec-0023, assists and die-retry included out.
   */
  combatPlan: CombatPlan | undefined;
  /** Whether the die-retry ladder stage runs. */
  dieRetry = false;
  /** Every scripted death and what it proved about the retry loop. Entries are
   * appended when the death is TAKEN and mutated as the loop yields facts, so an
   * aborted run still carries the death it took. */
  readonly trials: DeathTrialRecord[] = [];
  /** Waves the die-retry stage entered, whether or not it finished with them.
   * Engagement without records is the silence the run report must not keep. */
  readonly dieRetryEngaged = new Set<string>();
  /** Every `rest` step the PATH declares, and which of them the bot performed —
   * the die-retry precondition reads both. Declared up front
   * rather than accumulated as they run, so "the route passed this fire without
   * resting" is a statement the check can actually make. */
  restSteps: readonly PerformedRest[] = [];
  readonly restedBonfires = new Set<number>();
  /** Bonfire → the step index this run last rested at it (run-back bookkeeping). */
  readonly restedAt = new Map<number, number>();
  /** Wave → the step index this run last cleared it at (run-back bookkeeping). */
  readonly waveClearedAt = new Map<string, number>();
  /** Every run-back this run fought, in order — named in the log and the assists. */
  readonly runBacksFought: RunBack[] = [];
  /** Encounters whose scripted deaths were SKIPPED because the checkpoint the
   * stage would measure against was never armed — the RUN's own gap, and red. */
  readonly preconditionFindings: string[] = [];
  /** Encounters whose scripted deaths were skipped because the campaign fires NO
   * checkpoint before them — a content fact, reported and not graded. */
  readonly preconditionAdvisories: string[] = [];
  readonly preconditionWaves = new Set<string>();
  /** The newest census summary the chat channel has delivered FOR EACH WAVE, and
   * the mob lines that closed each census, keyed by the server's own sequence
   * number. `censusSeq` is how a fresh answer is told from a stale one without the
   * harness ever writing a delve score to ask its question.
   *
   * Per wave, not one slot: the damage handlers now read a wave's census while a
   * step may be reading another's, and a single "latest" slot let one answer
   * overwrite the other before its asker polled — the asker then timed out on a
   * census the server had answered. */
  readonly censusSummaries = new Map<string, CensusSummary>();
  censusSeq = 0;
  readonly censusMobs = new Map<number, CensusMob[]>();
  /** The latest muster summary line per wave, and the body readings filed under
   * its seq. Per wave for the same reason as {@link censusSummaries}. */
  readonly musterSummaries = new Map<string, MusterSummary>();
  musterSeq = 0;
  readonly musterBodies = new Map<number, MusterBody[]>();
  /** What each wave's muster established — the encounter rows' evidence. */
  readonly musters = new Map<string, MusterVerdict>();
  /**
   * Every failure ANY muster reading produced, in order, `<wave>: <failure>`.
   * `musters` holds each wave's latest reading, and a later reading that found
   * the seating whole used to replace an earlier one that did not — the failure
   * then reached neither the report nor the exit code.
   */
  readonly musterFailureLog: string[] = [];
  /**
   * Every body the run took out of the delve by command, and why.
   *
   * Named loudly and separately from anything the delve did, because it is the one
   * thing in a run report that is the HARNESS acting rather than the delve
   * behaving. A reader must never have to work out whether a wave fell to the
   * campaign's own machinery or to this.
   */
  readonly stagedRemovals: StagedRemoval[] = [];
  /** Client ids already staged away, so one body is never struck twice. */
  readonly stagedIds = new Set<number>();
  /** True while a `sneak` leg is walking: nothing is staged away on one. */
  sneaking = false;
  /** The wave a `kill` step is clearing right now. Its bodies have been read and
   * are on their way out, so nothing protects them any longer. */
  clearing: string | undefined;
  /**
   * How many times the delve could have re-seated its `respawns_on_rest` waves
   * since the run began: one per rest and one per respawn (spec-0016 §1 — a death
   * respawns the party at the last fire and fires that rest's hooks).
   *
   * The unit a reading is owed in. A wave's muster is a reading of ONE seating, so
   * whether the run still owes it is "has this wave been read since the last time
   * it could have been put back" — a count, not a step index, because a rest and a
   * read inside one step are otherwise the same number.
   */
  seatEpoch = 0;
  /** Wave → the {@link seatEpoch} its last muster read. */
  readonly musteredEpoch = new Map<string, number>();
  /** Wave → the {@link seatEpoch} this run last cleared it in. */
  readonly clearedEpoch = new Map<string, number>();
  /** Forceloaded chunk → how many holders want it; `/forceload remove` only on the last. */
  readonly chunkHolds = new Map<string, number>();
  /** Client ids a damage handler is reading or staging right now, so a burst of
   * hits from one body opens one muster, not one per hit. */
  readonly stagingInFlight = new Set<number>();
  /** A muster already in flight per wave — a step's or a damage handler's — so
   * two readers of one seating share one reading. */
  readonly earlyMusters = new Map<string, Promise<void>>();
  /**
   * Every staging act a damage handler started and has not finished — a reading
   * of the body's wave, its removal, the refund. Awaited by {@link settleStaging}
   * before the run report is built: a muster the server was still answering when
   * the last stage ended was a reading the run took and then never recorded.
   */
  readonly stagingTasks = new Set<Promise<void>>();
  /** Set by {@link settleStaging}: the stages are over, and no new staging starts. */
  stagingClosed = false;
  /** Gates the walk into a lethal volume opened, with the state each stood in. */
  readonly gatesOpenedByTrial: { pos: Vec3Tuple; state: string }[] = [];
  /** When the latest respawn landed: wall clock, and the server's world age as the
   * last time packet before it reported it. The respawn-protection wait reads both. */
  lastSpawnAt: number | undefined;
  lastSpawnAge: number | undefined;
  /**
   * The encounter the die-retry stage is proving, and where its bodies last stood.
   *
   * That stage's whole subject is a LIVE encounter: death, respawn, the route
   * back, and a fight still there to re-engage. Staging its bodies away would
   * delete the thing being measured — measured on the gallery, where the damage
   * handlers cleared `wave/muster` mid-stage and the trial then reported the
   * campaign soft-locked. Anything that is NOT of this wave is still removed:
   * an ambusher from three rooms back is interference, not the subject.
   */
  protectedWave:
    | {
        readonly wave: string;
        census: ReadonlyArray<{ readonly pos: readonly [number, number, number] }>;
      }
    | undefined;
  /** How far `kill()` got with each encounter — the reading key for an empty
   * `assist_windows` array (spec-0023 takes no assist while deliberately dying,
   * nor on a billed encounter's honest first attempt). */
  readonly encounterPhases = new Map<string, EncounterPhase>();
  /**
   * Who felled each wave's bodies, as its last census answered. The floor gate's
   * verdict reads it, and so does the run report: an encounter cleared because a
   * lethal volume ate two thirds of its cohort is not an encounter the bot beat,
   * and before this nothing anywhere could tell the two apart.
   */
  readonly waveAttributions = new Map<string, FightAttribution>();
  /** Every NAMED entity death this run observed (`entityDead` on a body carrying a
   * custom name — an actor). Raw and unclassified; the entrypoint classifies each
   * against {@link classifyDeathDepth} (see teardown.ts) before it reaches the run
   * report — the 2026-08-06 island triage found that a scripted `despawn-actor`
   * vanish broadcasts the same "<name> died" line a real combat loss does. */
  readonly namedEntityDeathLog: NamedEntityDeath[] = [];
  /** Objectives the compiled path proves — decides which actor fights are reachable. */
  pathObjectives: ReadonlySet<string> = new Set();
  /** How many items the bot was carrying the last time it was known to be alive
   * and kitted. The baseline `keep_inventory` is judged against after a death. */
  itemsBeforeDeath = 0;
  /**
   * How many walked legs have been consumed. Legs are matched in lockstep
   * path order (not by destination coordinate), so an anchor visited more than once
   * — e.g. the cave entry the player returns to — never grabs the wrong leg's route.
   */
  legCursor = 0;
  /**
   * A run-back already walked part of the leg at `leg` (the index into the
   * waypoint legs) and fought beside waypoint `from`: the step's own walk of
   * that leg resumes there instead of walking back to the leg's start.
   */
  legResume: { leg: number; from: number } | undefined;
  /** Timestamp (ms) of the last damage attributed from a packet-named source. */
  lastAttributionAt = 0;
  /** The body the server last named as hitting the bot, and when — the health drop
   * that follows the damage packet is that body's blow. */
  lastNamedHit: { readonly id: number; readonly at: number } | undefined;
  /** Client id → health the bot lost to that body's named blows, not yet refunded. */
  readonly damageBy = new Map<number, number>();
  /** Last observed bot health, for the health-drop attribution fallback. */
  lastHealth: number | undefined;
  /** Walk legs in progress — while non-zero, the bot is held at full health. */
  walkLegs = 0;
  /** The label of the walk leg in progress, for the staging record. */
  walkLabel = "";
  /** The full-health restoration in flight, so a burst of drops issues one. */
  restoringHealth: Promise<void> | undefined;
  /** Timestamp (ms) of the last eat attempt, throttling both the action and its log. */
  lastEatAt = 0;

  constructor(config: BotConfig, env: Record<string, string | undefined> = process.env) {
    this.config = config;
    const raw = env["DELVEWRIGHT_CUTSCENE_GRACE_MS"];
    const parsed = raw === undefined ? NaN : Number.parseInt(raw, 10);
    this.cutsceneGraceMs = Number.isInteger(parsed) && parsed >= 0 ? parsed : 10_000;
    const settleRaw = env["DELVEWRIGHT_ENTITY_SETTLE_TIMEOUT_MS"];
    const settleParsed = settleRaw === undefined ? NaN : Number.parseInt(settleRaw, 10);
    this.entitySettleTimeoutMs =
      Number.isInteger(settleParsed) && settleParsed >= 0 ? settleParsed : ENTITY_SETTLE_TIMEOUT_MS;
    const crossRaw = env["DELVEWRIGHT_LOOP_CROSS_TIMEOUT_MS"];
    const crossParsed = crossRaw === undefined ? NaN : Number.parseInt(crossRaw, 10);
    this.loopCrossTimeoutMs =
      Number.isInteger(crossParsed) && crossParsed > 0 ? crossParsed : LOOP_CROSS_TIMEOUT_MS;
  }

  /** spec-0029 name-preference binding counters (see {@link NamePreference}). */
  namePreferenceDecisions = 0;
  namePreferenceWithName = 0;
  namePreferenceCandidates = 0;
  namePreferenceNamedCandidates = 0;
}

// The methods each object file holds, merged into the class's type and installed
// on its prototype. One line per file; a method is declared in exactly one.
type Methods = typeof connectionMethods &
  typeof chatMethods &
  typeof objectiveMethods &
  typeof triggerMethods &
  typeof interactMethods &
  typeof classMethods &
  typeof talkMethods &
  typeof walkMethods &
  typeof timedGateMethods &
  typeof climbMethods &
  typeof cutsceneMethods &
  typeof transportMethods &
  typeof loopMethods &
  typeof sustainMethods &
  typeof deathMethods &
  typeof repaintMethods &
  typeof pulseMethods &
  typeof scoreMethods &
  typeof lethalMethods &
  typeof stakeMethods &
  typeof waveMethods &
  typeof dieRetryMethods &
  typeof stagingMethods &
  typeof musterMethods &
  typeof restMethods &
  typeof witnessMethods &
  typeof watchMethods &
  typeof crosshairMethods &
  typeof collectMethods &
  typeof settleMethods;
export interface MineflayerExecutor extends Methods {}
for (const methods of [connectionMethods, chatMethods, objectiveMethods, triggerMethods, interactMethods, classMethods, talkMethods, walkMethods, timedGateMethods, climbMethods, cutsceneMethods, transportMethods, loopMethods, sustainMethods, deathMethods, repaintMethods, pulseMethods, scoreMethods, lethalMethods, stakeMethods, waveMethods, dieRetryMethods, stagingMethods, musterMethods, restMethods, witnessMethods, watchMethods, crosshairMethods, collectMethods, settleMethods]) Object.assign(MineflayerExecutor.prototype, methods);
