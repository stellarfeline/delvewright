// Harness entrypoint: `node src/run.ts <critical-path.json>`.
// Loads and validates the compiler's critical-path contract (spec-0002), connects
// a mineflayer bot to the server (connection details from the environment — see
// executor.ts), executes the critical path under a hard wall-clock budget, and
// exits 0 on success / 1 on any failure (parse error, ordering violation, a failed
// step, or timeout). No campaign knowledge lives here (spec-0003): everything
// comes from critical-path.json.

import { readFile } from "node:fs/promises";
import { parseCriticalPathJson } from "./critical-path.ts";
import { runSequence, StepExecutionError } from "./sequencer.ts";
import { botConfigFromEnv, MineflayerExecutor } from "./executor.ts";
import { BotDeathError } from "./death.ts";
import { loadWaypointsForCriticalPath } from "./waypoints.ts";
import {
  assistPolicy,
  dieRetryCoverageFailures,
  dieRetryFindings,
  loadCombatPlanForCriticalPath,
} from "./combat.ts";
import { RunReport, reportPathFromEnv, writeRunReport, type EncounterReport } from "./report.ts";

/**
 * Exit code for a run that failed specifically because the bot died (spec-0008),
 * distinct from the generic failure code (1) used for parse/ordering/navigation
 * faults, so the ladder can tell a lethal-content death from a navigation bug.
 */
const EXIT_BOT_DEATH = 3;

/** True if `err` (or the step failure wrapping it) is a bot death. */
function isBotDeath(err: unknown): boolean {
  if (err instanceof BotDeathError) return true;
  return err instanceof StepExecutionError && err.cause instanceof BotDeathError;
}

/**
 * Whether to retry once after a bot death (spec-0008). Opt-in via
 * DELVEWRIGHT_RETRY_ON_DEATH (`1`/`true`); default fail-fast. For future
 * lethal-delve validation — the safe-route ladder leaves it off.
 */
function retryOnDeathFromEnv(env = process.env): boolean {
  const raw = env["DELVEWRIGHT_RETRY_ON_DEATH"];
  return raw === "1" || raw === "true";
}

/**
 * Hard wall-clock budget for the whole run (spec-0003: 20 min for M1).
 * Override with DELVEWRIGHT_RUN_TIMEOUT_MS. A run exceeding this fails red — a
 * hung bot must not hang CI.
 */
function runTimeoutMs(env = process.env): number {
  const raw = env["DELVEWRIGHT_RUN_TIMEOUT_MS"];
  if (raw === undefined || raw.length === 0) {
    return 20 * 60 * 1000;
  }
  const ms = Number.parseInt(raw, 10);
  if (!Number.isInteger(ms) || ms <= 0) {
    throw new Error(
      `DELVEWRIGHT_RUN_TIMEOUT_MS must be a positive integer, got ${JSON.stringify(raw)}`,
    );
  }
  return ms;
}

/**
 * Whether the die-retry ladder stage runs (spec-0023 §1). ON whenever the build
 * carries a combat plan: it is a REQUIRED stage, not an option — "dying is always
 * safe" is the load-bearing property of a souls delve, and the machine has to
 * prove it. `DELVEWRIGHT_DIE_RETRY=0` skips it for local iteration only, and the
 * run report says so, so a skipped stage can never be mistaken for a passed one.
 */
function dieRetryFromEnv(env = process.env): boolean {
  return env["DELVEWRIGHT_DIE_RETRY"] !== "0";
}

function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  const guard = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      reject(new Error(`run exceeded wall-clock budget of ${ms}ms`));
    }, ms);
  });
  return Promise.race([promise, guard]).finally(() => clearTimeout(timer));
}

async function main(): Promise<number> {
  const pathArg = process.argv[2];
  if (pathArg === undefined || pathArg.length === 0) {
    process.stderr.write("usage: node src/run.ts <path-to-critical-path.json>\n");
    return 1;
  }

  const text = await readFile(pathArg, "utf8");
  const criticalPath = parseCriticalPathJson(text);

  // task #38: if the compiler's proven waypoint artifact accompanies the critical
  // path, the executor navigates each walked leg through it (successive nearby
  // goals) so no single distant A* solve strands the bot on a large open cave.
  // Absent → single-goal navigation (fallback); malformed → hard failure.
  const waypoints = await loadWaypointsForCriticalPath(pathArg);

  const config = botConfigFromEnv();
  const budgetMs = runTimeoutMs();
  process.stderr.write(
    `connecting to ${config.host}:${config.port} as ${config.username} ` +
      `(mc ${config.version}, auth ${config.auth}, budget ${budgetMs}ms)\n`,
  );

  const executor = new MineflayerExecutor(config);
  // Scope the completion oracle to this campaign: only markers naming it count
  // (AUDIT-P0). Comes from the contract, never inferred.
  executor.useCampaign(criticalPath.campaignId);
  if (waypoints) {
    executor.useWaypoints(waypoints);
    process.stderr.write(
      `using compiler-proven waypoints: ${waypoints.legs.length} walked leg(s)\n`,
    );
  }
  // spec-0023: with the compiler's combat plan, `kill` steps become verified
  // ENCOUNTERS — the die-retry stage proves dying is safe, and the fights run under
  // bounded, labelled combat assist so bot fencing skill never caps how hard a
  // delve is allowed to be. Absent → the pre-spec-0023 run, unchanged.
  const combatPlan = await loadCombatPlanForCriticalPath(pathArg);
  const dieRetry = combatPlan !== undefined && dieRetryFromEnv();
  const report = new RunReport(criticalPath.campaignId, combatPlan?.difficulty ?? "unknown");
  if (combatPlan) {
    executor.useCombatPlan(combatPlan, dieRetry);
    process.stderr.write(
      `combat plan: ${combatPlan.encounters.length} mandatory encounter(s) at ` +
        `difficulty '${combatPlan.difficulty}'; die-retry ${dieRetry ? "ON" : "SKIPPED"}\n`,
    );
  }

  try {
    let failure: unknown;
    try {
      await withTimeout(
        (async () => {
          await executor.connect();
          await runSequence(criticalPath, executor, {
            retryOnDeath: retryOnDeathFromEnv(),
          });
        })(),
        budgetMs,
      );
    } catch (err) {
      failure = err;
    }

    // The report is written whether the run passed or failed: a red run's assist
    // windows and death trials are exactly what a reader needs to see.
    const trials = executor.deathTrials();
    const assists = executor.assistWindows();
    // Two independent ways the stage can be red: a trial that reached a verdict
    // and failed it, and a trial (or a whole encounter) the run never proved at
    // all. The second is the one an empty `die_retry` array used to hide.
    const dieRetryFailures = [
      ...dieRetryFindings(trials),
      ...(dieRetry
        ? dieRetryCoverageFailures(
            combatPlan?.encounters ?? [],
            executor.dieRetryEngagements(),
            trials,
          )
        : []),
    ];
    const leaked = executor.leakedAssists();
    report.recordAssists(assists);
    report.recordTrials(trials);
    // Per-encounter assist policy + how far the run got, so a reader can tell a
    // policy-empty assist ledger from an unwired one (task #102).
    const encounterReports: EncounterReport[] = (combatPlan?.encounters ?? []).map(
      (enc): EncounterReport => ({
        encounter: enc.objective,
        wave: enc.wave,
        tier: enc.tier,
        assistPolicy: assistPolicy(enc),
        phaseReached: executor.encounterPhase(enc.wave),
        assistWindows: assists.filter((w) => w.wave === enc.wave).length,
      }),
    );
    report.recordEncounters(encounterReports);
    for (const f of executor.floorGateFindings()) report.recordFloorFinding(f);
    report.stage({
      stage: "critical-path",
      ran: true,
      passed: failure === undefined,
      findings: leaked.map(
        (w) => `assist window on ${w.wave} was never closed — harness bug, not content`,
      ),
      failures:
        failure === undefined
          ? []
          : [failure instanceof Error ? failure.message : String(failure)],
    });
    report.stage({
      stage: "die-retry",
      ran: dieRetry,
      passed: dieRetry && dieRetryFailures.length === 0,
      findings: dieRetry
        ? []
        : [
            combatPlan === undefined
              ? "no combat plan in this build — the campaign declares no mandatory combat"
              : "skipped via DELVEWRIGHT_DIE_RETRY=0",
          ],
      failures: dieRetryFailures,
    });

    const reportPath = reportPathFromEnv();
    if (reportPath) {
      await writeRunReport(reportPath, report);
      process.stderr.write(`run report written to ${reportPath}\n`);
    }
    for (const finding of report.findings()) {
      process.stderr.write(`[finding] ${finding}\n`);
    }

    if (failure !== undefined) throw failure;
    // A die-retry failure is a red run in its own right: the delve may be
    // completable and still ship a broken retry loop, which is the one thing a
    // souls delve cannot do.
    if (dieRetryFailures.length > 0) {
      throw new Error(
        `die-retry stage FAILED (${dieRetryFailures.length} finding(s)):\n` +
          dieRetryFailures.map((f) => `  ${f}`).join("\n"),
      );
    }
    process.stderr.write(
      `critical path '${criticalPath.campaignId}' PASSED (${criticalPath.steps.length} steps` +
        `${trials.length > 0 ? `, ${trials.length} scripted death(s) survived` : ""}` +
        `${report.findings().length > 0 ? `, ${report.findings().length} advisory finding(s)` : ""})\n`,
    );
    return 0;
  } finally {
    executor.close();
  }
}

main()
  .then((code) => {
    process.exit(code);
  })
  .catch((err: unknown) => {
    const message = err instanceof Error ? err.message : String(err);
    process.stderr.write(`FAILED: ${message}\n`);
    process.exit(isBotDeath(err) ? EXIT_BOT_DEATH : 1);
  });
