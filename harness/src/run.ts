// Harness entrypoint: `node src/run.ts <critical-path.json>`.
// Loads and validates the compiler's critical-path contract (spec-0002), connects
// a mineflayer bot to the server (connection details from the environment — see
// executor.ts), executes the critical path under a hard wall-clock budget, and
// exits 0 on success / 1 on any failure (parse error, ordering violation, a failed
// step, or timeout). No campaign knowledge lives here (spec-0003): everything
// comes from critical-path.json.

import { readFile } from "node:fs/promises";
import { parseCriticalPathJson } from "./critical-path.ts";
import { runSequence } from "./sequencer.ts";
import { botConfigFromEnv, MineflayerExecutor } from "./executor.ts";

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

  const config = botConfigFromEnv();
  const budgetMs = runTimeoutMs();
  process.stderr.write(
    `connecting to ${config.host}:${config.port} as ${config.username} ` +
      `(mc ${config.version}, auth ${config.auth}, budget ${budgetMs}ms)\n`,
  );

  const executor = new MineflayerExecutor(config);
  try {
    await withTimeout(
      (async () => {
        await executor.connect();
        await runSequence(criticalPath, executor);
      })(),
      budgetMs,
    );
    process.stderr.write(
      `critical path '${criticalPath.campaignId}' PASSED (${criticalPath.steps.length} steps)\n`,
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
    process.exit(1);
  });
