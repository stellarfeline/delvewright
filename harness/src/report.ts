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
import type { AssistWindow, DeathTrial } from "./combat.ts";

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
        respawn_pos: t.respawnPos ?? null,
        at_checkpoint: t.atCheckpoint,
        returned: t.returned,
        re_engaged: t.reEngaged,
        objectives_intact: t.objectivesIntact,
        lost_objectives: [...t.lostObjectives],
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
