// Harness entrypoint: `node src/run.ts <critical-path.json>`.
// Loads and validates the compiler's critical-path contract (spec-0002), connects
// a mineflayer bot to the server (connection details from the environment — see
// executor.ts), executes the critical path, and exits 0 on success / 1 on any
// failure (parse error, ordering violation, or a failed step). No campaign
// knowledge lives here (spec-0003): everything comes from critical-path.json.

import { readFile } from "node:fs/promises";
import { parseCriticalPathJson } from "./critical-path.ts";
import { runSequence } from "./sequencer.ts";
import { botConfigFromEnv, MineflayerExecutor } from "./executor.ts";

async function main(): Promise<number> {
  const pathArg = process.argv[2];
  if (pathArg === undefined || pathArg.length === 0) {
    process.stderr.write("usage: node src/run.ts <path-to-critical-path.json>\n");
    return 1;
  }

  const text = await readFile(pathArg, "utf8");
  const criticalPath = parseCriticalPathJson(text);

  const config = botConfigFromEnv();
  process.stderr.write(
    `connecting to ${config.host}:${config.port} as ${config.username} ` +
      `(mc ${config.version}, auth ${config.auth})\n`,
  );

  const executor = new MineflayerExecutor(config);
  await executor.connect();
  try {
    await runSequence(criticalPath, executor);
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
