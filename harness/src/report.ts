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
import type { AssistWindow, DeathTrial, EncounterPhase, EncounterTier } from "./combat.ts";

/**
 * One planned encounter, and how the run actually approached it.
 *
 * Added by task #102. `assist_windows: []` on a run where the bot demonstrably
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
}

/** The ladder's labelled stages. */
export const STAGES = ["critical-path", "die-retry"] as const;
export type StageName = (typeof STAGES)[number];

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
  private readonly encounters: EncounterReport[] = [];

  constructor(campaignId: string, difficulty: string) {
    this.campaignId = campaignId;
    this.difficulty = difficulty;
  }

  stage(result: StageResult): void {
    this.stages.set(result.stage, result);
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

  recordEncounters(entries: readonly EncounterReport[]): void {
    this.encounters.push(...entries);
  }

  /** Every advisory the run produced, for the one-line stderr summary. */
  findings(): string[] {
    return [...this.floor, ...[...this.stages.values()].flatMap((s) => [...s.findings])];
  }

  toJSON(): Record<string, unknown> {
    return {
      version: 1,
      campaign_id: this.campaignId,
      // The difficulty the run was verified AT: spec-0023 §3 proves orchestration
      // end-to-end at the SHIPPED difficulty, so the number it ran under belongs
      // in the artifact next to the assists that made it survivable.
      difficulty: this.difficulty,
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
      // Every encounter the compiler put in the plan, with the assist policy it
      // is approached under and the phase the run actually reached. Without this
      // an empty `assist_windows` says nothing: it is the expected reading for a
      // run that never got past the die-retry stage, and also the reading for an
      // assist mechanism that was never wired (task #102).
      encounters: this.encounters.map((e) => ({
        encounter: e.encounter,
        wave: e.wave,
        tier: e.tier,
        assist_policy: e.assistPolicy,
        phase_reached: e.phaseReached,
        assist_windows: e.assistWindows,
      })),
      // spec-0023 §3: "the run artifact names every assist window (encounter id,
      // ticks)". Loudly, and including any the harness failed to close.
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
      die_retry: this.trials.map((t) => ({
        encounter: t.encounter,
        wave: t.wave,
        attempt: t.attempt,
        phase: t.phase,
        // What was waiting at the end of the loop: `re-engaged`,
        // `cleared-before-retry` (both passes) or `stranded` (a soft lock).
        outcome: t.outcome,
        cause: t.cause ?? null,
        respawn_pos: t.respawnPos ?? null,
        at_checkpoint: t.atCheckpoint,
        returned: t.returned,
        re_engaged: t.reEngaged,
        objective_complete: t.objectiveComplete,
        objectives_intact: t.objectivesIntact,
        lost_objectives: [...t.lostObjectives],
        // A trial the run abandoned half-way is still IN this array — that is the
        // point of recording on death — so every entry says whether its loop
        // actually reached a verdict.
        completed: t.completed,
        aborted_with: t.abortedWith ?? null,
      })),
      floor_findings: [...this.floor],
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
