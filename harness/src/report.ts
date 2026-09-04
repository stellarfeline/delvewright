// The bot ladder's run report (spec-0023).
//
// Before spec-0023 the critical-path bot's entire output was an exit code and a
// stream of unstructured stderr lines. That was enough while the only question
// was "did the whole thing pass"; it is not enough now that the run makes
// CLAIMS about how it passed — which encounters it took a combat assist at and
// for how long, how many scripted deaths each encounter survived, and which
// billed fights the unassisted bot beat cold. spec-0023 requires the run
// ARTIFACT to name every assist window, so the report is part of the contract
// rather than a convenience.
//
// The report is written whenever DELVEWRIGHT_RUN_REPORT names a path; absent, the
// run behaves exactly as before (stderr only). Deterministic key order, so two
// runs of the same delve diff cleanly.

import { writeFile } from "node:fs/promises";
import type {
  ActorTrial,
  AssistWindow,
  BindingCount,
  DeathTrial,
  DieRetryBinding,
  EncounterPhase,
  EncounterTier,
  FightAttribution,
  FloorLedger,
  PerformedRest,
} from "./combat.ts";
import type { DeathLoopBinding, LethalTrial } from "./death-loop.ts";
import type { ClassifiedDeath } from "./teardown.ts";
import type { NamePreference } from "./executor.ts";

/**
 * One tiered actor, and what this run did about it.
 *
 * Every actor in the plan gets a row, fought or not — an actor missing from the
 * report is the silence the floor-gate ledger exists to end. A row that did not
 * run always carries the reason it did not.
 */
export interface ActorReport {
  readonly actor: string;
  readonly tier: EncounterTier;
  readonly entity: string;
  readonly anchor: string;
  /** The compiler's own coverage verdict, carried through verbatim. */
  readonly covered: boolean;
  readonly exercised: boolean;
  /** Why this run did not fight it. `undefined` only when it did. */
  readonly reason?: string;
  /** The engagement, when there was one. */
  readonly trial?: ActorTrial;
}

/**
 * One planned encounter, and how the run actually approached it.
 *
 * `assist_windows: []` on a run where the bot demonstrably
 * died was unreadable: spec-0023 takes NO assist while the die-retry stage is
 * deliberately dying, and none on a billed `elite`/`boss`'s honest first
 * attempt, so an empty ledger is often exactly per policy — but it looks
 * identical to an assist mechanism that was never wired. Stating the policy and
 * the phase the run reached per encounter makes the two distinguishable in the
 * artifact, which is the only evidence a reader has.
 */
export interface EncounterReport {
  readonly encounter: string;
  readonly wave: string;
  readonly tier: EncounterTier;
  readonly assistPolicy: "assisted" | "unassisted-first";
  readonly phaseReached: EncounterPhase;
  readonly assistWindows: number;
  /**
   * Who felled this encounter's bodies, as the compiler's census answered.
   *
   * `phase_reached: cleared` says the step ended; it has never said who ended it.
   * A delve is full of things that kill a mob with no bot in them, and the
   * engine's own gallery seats `wave/muster` within a stride of a lethal volume
   * and a drop — so an encounter can read `cleared` over a cohort the bot barely
   * touched. This is the evidence that separates the two, and `unattributed` (with
   * its reason) is a legitimate value: no census answered is a fact about the
   * probe, and it must not be readable as a clean win.
   */
  readonly attribution: FightAttribution;
}

/**
 * One enumerated branch, and what this run did about it (spec-0025 §3).
 *
 * `ran: false` is always accompanied by a `reason`: the spec's rule is that a
 * skipped branch is NAMED, never silent — a branch list that quietly omitted the
 * branches nobody walked would read exactly like full coverage.
 */
export interface BranchOutcome {
  readonly branch: string;
  readonly ran: boolean;
  /** Meaningful only when `ran`; a branch that did not run passed nothing. */
  readonly passed: boolean;
  /** Why it did not run. `undefined` only when it did. */
  readonly reason?: string;
  /** The executable path file walked, when one was. */
  readonly pathFile?: string;
  /** The per-branch chronicle the generation-time narrative review reads. */
  readonly chronicle: string;
  /** The chat lines that took the branching choices, when the branch ran. */
  readonly entryCommands: readonly string[];
  /** The endings the compiler proved fire on this branch. */
  readonly endings: readonly string[];
}

/** The ladder's labelled stages. */
export const STAGES = ["branch-run", "critical-path", "die-retry", "death-loop"] as const;
export type StageName = (typeof STAGES)[number];

/**
 * Where a run was when the HARNESS itself died — a labelled stage, or one of the
 * two phases that precede every stage.
 */
export type CrashStage = StageName | "startup" | "connect";

/**
 * The harness crashed: an exception or a promise rejection nobody was listening
 * for took the process down, rather than a step failing.
 *
 * This is a distinct OUTCOME, not a red stage, and the difference is the whole
 * point of recording it. A red stage is a verdict on the delve — a step the bot
 * could not complete, a fight that killed it, a retry loop that did not hold. A
 * crash is a verdict on the harness: the run never reached a verdict at all, and
 * nothing in it may be read as one. Before this existed the process simply exited
 * with no report, and `bot-1 exited with code 1` was indistinguishable from a
 * content failure to everyone downstream.
 */
export interface HarnessCrash {
  /** What the run was doing when it died. */
  readonly stage: CrashStage;
  /** The error's message (and name, when it has an informative one). */
  readonly reason: string;
}

/** One stage's outcome. `findings` are advisory; `failures` are why it went red. */
export interface StageResult {
  readonly stage: StageName;
  readonly ran: boolean;
  readonly passed: boolean;
  readonly findings: readonly string[];
  readonly failures: readonly string[];
}

/** The accumulating run report. */
export class RunReport {
  readonly campaignId: string;
  readonly difficulty: string;
  private readonly stages = new Map<StageName, StageResult>();
  private readonly assists: AssistWindow[] = [];
  private readonly trials: DeathTrial[] = [];
  private readonly floor: string[] = [];
  /** Bodies that outlived the melee budget their encounter's arithmetic gave
   * them. A separate channel from {@link floor}: the floor gate is about a
   * fight being too EASY for its billing, this is about a body not dying at
   * all, and folding them together would make each read as the other. */
  private readonly unkillable: string[] = [];
  private readonly encounters: EncounterReport[] = [];
  private readonly rests: PerformedRest[] = [];
  private readonly namedEntityDeaths: ClassifiedDeath[] = [];
  private branches: BranchOutcome[] | undefined;
  private branchTier: string | undefined;
  private drivenBranch: string | undefined;
  private readonly actors: ActorReport[] = [];
  private floorLedger: FloorLedger | undefined;
  private actorsGate: BindingCount | undefined;
  /** Every walk into a lethal volume, and what the stage examined. */
  private readonly lethalTrials: LethalTrial[] = [];
  private deathLoopBinding: DeathLoopBinding | undefined;
  /** What the die-retry stage examined — recorded on EVERY run, zero included. */
  private dieRetryBinding: DieRetryBinding | undefined;
  /** Set only when the harness itself died; `null` in the artifact otherwise. */
  private harnessCrash: HarnessCrash | undefined;
  /** spec-0029: the name-preference binding, zero until the run records one. */
  private namePreference: NamePreference = {
    decisions: 0,
    withUsableName: 0,
    candidates: 0,
    namedCandidates: 0,
  };

  constructor(campaignId: string, difficulty: string) {
    this.campaignId = campaignId;
    this.difficulty = difficulty;
  }

  stage(result: StageResult): void {
    this.stages.set(result.stage, result);
  }

  /**
   * Record that the harness died. First writer wins: the reason that took the
   * process down is the one worth reading, and a cascade of follow-on rejections
   * must not overwrite it.
   */
  recordHarnessCrash(crash: HarnessCrash): void {
    this.harnessCrash ??= crash;
  }

  /** The crash this run recorded, if any. */
  crash(): HarnessCrash | undefined {
    return this.harnessCrash;
  }

  recordAssists(windows: readonly AssistWindow[]): void {
    this.assists.push(...windows);
  }

  recordTrials(trials: readonly DeathTrial[]): void {
    this.trials.push(...trials);
  }

  recordFloorFinding(finding: string): void {
    this.floor.push(finding);
  }

  /** A body that did not fall inside its encounter's own melee budget. */
  recordUnkillableFinding(finding: string): void {
    this.unkillable.push(finding);
  }

  recordEncounters(entries: readonly EncounterReport[]): void {
    this.encounters.push(...entries);
  }

  recordRests(entries: readonly PerformedRest[]): void {
    this.rests.push(...entries);
  }

  /**
   * Record every named-entity death this run observed, already classified
   * scripted-teardown vs combat (teardown.ts). The island run's report surfaced
   * five such deaths with no way to tell which two were the compiler's
   * `despawn-actor` vanishes and which three were real losses — this array is
   * the fix: reclassified, never suppressed, so both kinds stay visible.
   */
  recordNamedEntityDeaths(entries: readonly ClassifiedDeath[]): void {
    this.namedEntityDeaths.push(...entries);
  }

  /**
   * Record the branch tier and every enumerated branch's outcome (spec-0025 §3).
   *
   * Called only for a build that HAS a branch plan, so a campaign with no declared
   * fork produces exactly the report it produced before — no empty section that
   * would have to be read as "no branches" rather than "no branch machinery".
   */
  /**
   * Record the compiler's floor-gate ledger and every tiered actor's outcome.
   *
   * The ledger is printed VERBATIM, both sides: what the inverted floor gate
   * covers, and what it cannot with the reason. Before this the ladder's only
   * surfacing of an unmeasurable elite was a build-time `DW0477` warning, so a
   * reader holding a green run report had no way to learn that its empty findings
   * list covered a fight nobody ever had.
   */
  recordCombatCoverage(ledger: FloorLedger, actors: readonly ActorReport[]): void {
    this.floorLedger = ledger;
    this.actors.push(...actors);
  }

  /**
   * Record `actors[]`'s own binding count (playtest-methodology.md rule 1):
   * how many actors this build's tier machinery tracked at all, distinct from
   * `floorGate`'s count — an all-`ordinary` actor binds this one and not that
   * one. `undefined` for a plan from a delvec that predates the field.
   */
  recordActorsGate(gate: BindingCount | undefined): void {
    this.actorsGate = gate;
  }

  /**
   * Record the death loop: every walk into a lethal volume, and the binding count
   * of what was examined.
   *
   * The binding is recorded even when it is all zeros — especially then. This is
   * the one mechanic a souls-shaped delve is entirely made of, and a stage that
   * examined nothing must be legible as such from the artifact alone rather than
   * inferred from an empty trial list.
   */
  recordDeathLoop(binding: DeathLoopBinding, trials: readonly LethalTrial[]): void {
    this.deathLoopBinding = binding;
    this.lethalTrials.push(...trials);
  }

  /**
   * Record what the die-retry stage examined (playtest-methodology rule 1).
   *
   * Recorded on every run, including — especially — a run where it is all zeros.
   * The stage's per-encounter arithmetic runs over an already-emptied list when
   * every encounter is excluded for want of a governing checkpoint, so it reports
   * `passed: true` having scripted no death at all; measured 2026-08-11, that is
   * the state of EVERY campaign and fixture in both repos. Without this a reader
   * has to notice an empty `die_retry` array to learn it.
   */
  recordDieRetryBinding(binding: DieRetryBinding): void {
    this.dieRetryBinding = binding;
  }

  /**
   * Record the same-type name-preference binding (spec-0029): how many
   * candidate-preference decisions the run made and how many had a usable name.
   *
   * i18n v2 emits an authored custom name as a translate component, so the
   * heuristic that prefers a body by its name reads a component rather than a
   * string. That weakens a preference, never an identity (`executor.ts` says so),
   * but the spec requires the weakening be MEASURED: a run that made decisions
   * and found zero usable names is a finding, and a run that made none is an
   * unbound gate, which is also a finding.
   */
  recordNamePreference(binding: NamePreference): void {
    this.namePreference = binding;
  }

  recordBranches(tier: string, driven: string | undefined, outcomes: readonly BranchOutcome[]): void {
    this.branchTier = tier;
    this.drivenBranch = driven;
    this.branches = [...outcomes];
  }

  /** Every advisory the run produced, for the one-line stderr summary. */
  findings(): string[] {
    return [
      ...this.floor,
      ...this.unkillable,
      ...[...this.stages.values()].flatMap((s) => [...s.findings]),
    ];
  }

  toJSON(): Record<string, unknown> {
    // spec-0025 §3: the branch set, what this run was answerable for, and what it
    // did about each branch. Present only when the build declares branches, so a
    // single-path delve's report is byte-identical to the pre-spec-0025 one.
    const branches =
      this.branches === undefined
        ? {}
        : {
            branches: {
              // The tier as the environment named it, so a reader can tell a
              // deliberate one-branch PR run from a release run that lost coverage.
              tier: this.branchTier ?? "all",
              // Which branch THIS session walked (one per world, by construction).
              driven: this.drivenBranch ?? null,
              outcomes: this.branches.map((b) => ({
                branch: b.branch,
                ran: b.ran,
                passed: b.ran && b.passed,
                reason: b.reason ?? null,
                path: b.pathFile ?? null,
                chronicle: b.chronicle,
                entry_commands: [...b.entryCommands],
                endings: [...b.endings],
              })),
            },
          };
    return {
      version: 1,
      campaign_id: this.campaignId,
      ...branches,
      // The difficulty the run was verified AT: spec-0023 §3 proves orchestration
      // end-to-end at the SHIPPED difficulty, so the number it ran under belongs
      // in the artifact next to the assists that made it survivable.
      difficulty: this.difficulty,
      // The harness's own failure, named as such. `null` on every run that
      // reached a verdict — which is what makes a non-null value legible: the
      // stages below it are whatever the run had established when the process
      // died, and NONE of them is a verdict on the delve.
      harness_crash:
        this.harnessCrash === undefined
          ? null
          : { stage: this.harnessCrash.stage, reason: this.harnessCrash.reason },
      stages: STAGES.filter((s) => this.stages.has(s)).map((s) => {
        const r = this.stages.get(s)!;
        return {
          stage: r.stage,
          ran: r.ran,
          passed: r.passed,
          findings: [...r.findings],
          failures: [...r.failures],
        };
      }),
      // The bonfires the bot actually RESTED at. A bonfire only
      // arms an affordance; the respawn point moves when the party rests, so this
      // list is what makes every `at_checkpoint` below mean anything.
      rests: this.rests.map((r) => ({
        bonfire: r.bonfire,
        anchor: r.anchor,
        pos: [...r.pos],
        step: r.step,
      })),
      // Every NAMED entity death this run observed, classified `scripted_teardown`
      // (a `despawn-actor style: vanish` reads as an ordinary death on purpose — see
      // teardown.ts) or `combat` (everything else). Reclassified, never dropped: a
      // reader must be able to tell the two apart at a glance without re-deriving it
      // from raw Y coordinates themselves.
      named_entity_deaths: this.namedEntityDeaths.map((d) => ({
        name: d.name,
        entity_id: d.entityId,
        position: [...d.position],
        kind: d.kind,
      })),
      // Every encounter the compiler put in the plan, with the assist policy it
      // is approached under and the phase the run actually reached. Without this
      // an empty `assist_windows` says nothing: it is the expected reading for a
      // run that never got past the die-retry stage, and also the reading for an
      // assist mechanism that was never wired.
      encounters: this.encounters.map((e) => ({
        encounter: e.encounter,
        wave: e.wave,
        tier: e.tier,
        assist_policy: e.assistPolicy,
        phase_reached: e.phaseReached,
        assist_windows: e.assistWindows,
        attribution:
          e.attribution.kind === "measured"
            ? {
                bodies: e.attribution.bodies,
                standing: e.attribution.standing,
                credited: e.attribution.credited,
                uncredited: e.attribution.uncredited,
              }
            : { unattributed: e.attribution.reason },
      })),
      // spec-0023 §3: "the run artifact names every assist window (encounter id,
      // ticks)". Loudly, and including any the harness failed to close.
      // The compiler's floor-gate ledger, verbatim. `present: false`
      // means the build shipped NO ledger — a plan from a delvec older than the
      // ledger — which is a different fact from a campaign that bills nothing
      // hard, and the two must never be read as one. `not_covered` carries the
      // compiler's own reason per entry: this is the line that stops an empty
      // findings list being mistaken for a pass over fights nobody had.
      floor_gate: {
        present: this.floorLedger?.present ?? false,
        covered: (this.floorLedger?.covered ?? []).map((e) => ({
          kind: e.kind,
          id: e.id,
          tier: e.tier ?? null,
        })),
        // `tier: null` is an UNTIERED hostile — an actor the
        // campaign unleashes on the party while declaring nothing about the
        // fight. It is written as an explicit null, never dropped: a key that
        // vanishes is the same silence this ledger exists to end.
        not_covered: (this.floorLedger?.notCovered ?? []).map((e) => ({
          kind: e.kind,
          id: e.id,
          tier: e.tier ?? null,
          reason: e.reason ?? null,
        })),
        // playtest-methodology.md rule 1: the ledger's own binding count,
        // carried through verbatim. `null` when the plan predates the field
        // (same reason `present` can be `false`) — never a substitute for
        // reading `covered`/`not_covered`, only a REPORTED statement of what
        // they add up to, so an unbound gate cannot be mistaken for a pass.
        examined: this.floorLedger?.binding?.examined ?? null,
        unbound: this.floorLedger?.binding?.unbound ?? null,
        reason: this.floorLedger?.binding?.reason ?? null,
      },
      // `actors[]`'s own binding count (rule 1): distinct question from
      // `floor_gate`'s — an all-`ordinary` actor binds this one and not that
      // one. `null` when the plan predates the field.
      // spec-0029 name-preference binding. `unbound` is stated explicitly so a
      // run that never exercised the preference cannot read as one that
      // exercised it successfully — a green gate that binds to nothing is
      // vacuous, not a pass (CLAUDE.md).
      name_preference: {
        decisions: this.namePreference.decisions,
        with_usable_name: this.namePreference.withUsableName,
        candidates: this.namePreference.candidates,
        named_candidates: this.namePreference.namedCandidates,
        unbound: this.namePreference.decisions === 0,
      },
      actors_gate:
        this.actorsGate === undefined
          ? null
          : {
              examined: this.actorsGate.examined,
              unbound: this.actorsGate.unbound,
              reason: this.actorsGate.reason ?? null,
            },
      // Every tiered actor the plan declares, fought or not — and when not, why.
      actors: this.actors.map((a) => ({
        actor: a.actor,
        tier: a.tier,
        entity: a.entity,
        anchor: a.anchor,
        covered: a.covered,
        exercised: a.exercised,
        reason: a.reason ?? null,
        outcome: a.trial?.outcome ?? null,
        after_objective: a.trial?.afterObjective ?? null,
        swings: a.trial?.swings ?? null,
        elapsed_ms: a.trial?.elapsedMs ?? null,
        detail: a.trial?.detail ?? null,
      })),
      // The death loop, the one mechanic a PackTest can never witness
      // (a fake player is permanently undamageable, measured twice). Every field
      // is an OBSERVATION: the ledger before and after, the position the player
      // came back at, the position the marker really stood at. `null` means the
      // run never got far enough to look, which is deliberately distinct from a
      // value that was looked at and found wrong.
      //
      // The binding is stated first and always, including all zeros: a stage that
      // entered no volume examined nothing, and rule 1 makes that a finding rather
      // than a pass.
      death_loop: {
        binding:
          this.deathLoopBinding === undefined
            ? null
            : {
                declared_volumes: this.deathLoopBinding.declaredVolumes,
                volumes_entered: this.deathLoopBinding.volumesEntered,
                deaths_observed: this.deathLoopBinding.deathsObserved,
                stakes_examined: this.deathLoopBinding.stakesExamined,
                seats_matched: this.deathLoopBinding.seatsMatched,
                walks_back: this.deathLoopBinding.walksBack,
                unbound: this.deathLoopBinding.deathsObserved === 0,
              },
        trials: this.lethalTrials.map((t) => ({
          volume: t.volume,
          entry_cell: [...t.entryCell],
          stake: t.stake ?? null,
          objective: t.objective ?? null,
          died: t.died,
          death_pos: t.deathPos ?? null,
          // The volume's OWN promised line, seen by the player it was about.
          wording_seen: t.wordingSeen,
          balance_before: t.balanceBefore ?? null,
          balance_after_death: t.balanceAfterDeath ?? null,
          // Computed from the DECLARED forfeit rule, never from the emission.
          expected_forfeit: t.expectedForfeit ?? null,
          respawn_pos: t.respawnPos ?? null,
          respawn_seat: t.respawnSeat ?? null,
          // Where the compile-time placement table said the stake would be.
          expected_anchor: t.expectedAnchor ?? null,
          marker_pos: t.markerPos ?? null,
          walked_back: t.walkedBack,
          // Packets SENT in one event-loop turn, not collections adjudicated: a
          // client cannot observe how many the server resolved in one tick, and
          // vanilla's once-per-tick advancement grant usually absorbs the second.
          // The claim is the outcome below, never this count.
          collect_clicks_sent: t.collectClicks,
          balance_after_collect: t.balanceAfterCollect ?? null,
          marker_retired: t.markerRetired,
          abandoned: t.abandoned ?? null,
        })),
      },
      assist_windows: this.assists.map((w) => ({
        encounter: w.encounter,
        wave: w.wave,
        tier: w.tier,
        amplifier: w.amplifier,
        ticks: w.ticks,
        reason: w.reason,
        opened_at_ms: w.openedAtMs,
        closed_at_ms: w.closedAtMs ?? null,
      })),
      // What the die-retry stage EXAMINED, beside what it found. `unbound: true`
      // means zero scripted deaths were taken, whatever the stage's `passed` says
      // — the two are different questions and only this one answers "was anything
      // about dying looked at".
      die_retry_binding:
        this.dieRetryBinding === undefined
          ? null
          : {
              declared_encounters: this.dieRetryBinding.declared,
              engaged: this.dieRetryBinding.engaged,
              deaths_scripted: this.dieRetryBinding.deathsScripted,
              trials_completed: this.dieRetryBinding.trialsCompleted,
              skipped_no_checkpoint: this.dieRetryBinding.skippedNoCheckpoint,
              skipped_unarmed_checkpoint: this.dieRetryBinding.skippedUnarmed,
              unbound: this.dieRetryBinding.unbound,
              reason: this.dieRetryBinding.reason ?? null,
            },
      die_retry: this.trials.map((t) => ({
        encounter: t.encounter,
        wave: t.wave,
        attempt: t.attempt,
        phase: t.phase,
        // What was waiting at the end of the loop: `re-engaged`,
        // `cleared-before-retry` (both passes), `stranded` (a soft lock) or
        // `unproven` (the loop never got into a position to look).
        outcome: t.outcome,
        cause: t.cause ?? null,
        // MEASURED, never planned: the bot's own position read the moment the
        // respawn settled, before anything else could move it.
        // `at_checkpoint` is derived from it and from nothing else.
        respawn_pos: t.respawnPos ?? null,
        at_checkpoint: t.atCheckpoint,
        kit_kept: t.kitKept,
        returned: t.returned,
        // Observed ONLY when `returned`. A trial that never walked back reports
        // `re_engaged: false`, `reengage: null` and `outcome: "unproven"` — it did
        // not look, which is not the same as looking and finding nothing.
        re_engaged: t.reEngaged,
        objective_complete: t.objectiveComplete,
        reseats_on_rest: t.reseats,
        // What the settled probe actually saw. `settle_ms` is the reading key for
        // a `present: 0`: a probe that answered instantly saw an empty room, one
        // that spent its whole budget waited for a room that never filled.
        reengage:
          t.reengage === undefined
            ? null
            : {
                present: t.reengage.present,
                declared: t.reengage.declared,
                carried_over: t.reengage.carriedOver,
                health_readable: t.reengage.healthReadable,
                damaged: t.reengage.damaged,
                // The count half's correction, stated beside the count it corrects.
                credited: t.reengage.credited,
                nearest_blocks: t.reengage.nearest ?? null,
                farthest_blocks: t.reengage.farthest ?? null,
                settle_ms: t.reengage.settleMs,
              },
        objectives_intact: t.objectivesIntact,
        lost_objectives: [...t.lostObjectives],
        // A trial the run abandoned half-way is still IN this array — that is the
        // point of recording on death — so every entry says whether its loop
        // actually reached a verdict.
        completed: t.completed,
        aborted_with: t.abortedWith ?? null,
      })),
      floor_findings: [...this.floor],
      unkillable_findings: [...this.unkillable],
    };
  }
}

/** Where to write the report, or `undefined` to write none. */
export function reportPathFromEnv(env = process.env): string | undefined {
  const raw = env["DELVEWRIGHT_RUN_REPORT"];
  return raw !== undefined && raw.length > 0 ? raw : undefined;
}

export async function writeRunReport(path: string, report: RunReport): Promise<void> {
  await writeFile(path, `${JSON.stringify(report.toJSON(), null, 2)}\n`, "utf8");
}
