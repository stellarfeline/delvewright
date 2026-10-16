// Drive the harness's game-tick wait on a live pinned server at the slowest
// clock the server allows, and under a frozen one, and print what each did.
//
//   node probe/clock-stopped.ts
//
// Connects as the harness bot does (`botConfigFromEnv`: DELVEWRIGHT_MC_HOST,
// DELVEWRIGHT_MC_PORT, DELVEWRIGHT_BOT_USERNAME), and needs the bot opped at
// level 3 or more, as the validation ladder ops it. The `/tick` commands are the
// bot's own, each read through the shared rejection rule. In order:
//
//   1. `/tick rate 0.5` must be refused: the floor `GameClockWatch` rests on.
//   2. `/tick rate 1`: a 5-tick wait must pass, and the longest run of prompt
//      answers seen at one tick is printed (the slowest live clock).
//   3. `/tick freeze`: a 2-tick wait must red with `GameTimeStoppedError`.
//   4. `/tick unfreeze` and `/tick rate 20`, whatever happened.
//
// Exit 0 when all three hold; 1 otherwise, naming the one that did not.
import { MineflayerExecutor } from "../src/executor.ts";
import { botConfigFromEnv } from "../src/executor/connection.ts";
import { GameClockWatch, GameTimeStoppedError, STOPPED_CLOCK_MS } from "../src/game-time.ts";

const executor = new MineflayerExecutor(botConfigFromEnv());
await executor.connect();
const failures: string[] = [];

async function must(command: string): Promise<void> {
  const refusal = await executor.refusalOf(command);
  if (refusal !== undefined) throw new Error(`${command} was refused: ${refusal}`);
}

/** The longest prompt-answer run at one tick over `ms` of readings, as the judge counts it. */
async function longestRun(ms: number): Promise<{ maxMs: number; readings: number }> {
  const first = await executor.readGameTime("the slow clock's sample");
  const watch = new GameClockWatch("the slow clock's sample", first.tick, Number.MAX_SAFE_INTEGER);
  watch.observe(first.tick, performance.now());
  let maxMs = 0;
  let readings = 1;
  const end = performance.now() + ms;
  while (performance.now() < end) {
    await new Promise((r) => setTimeout(r, 50));
    const r = await executor.readGameTime("the slow clock's sample");
    watch.observe(r.tick, performance.now());
    readings += 1;
    maxMs = Math.max(maxMs, watch.seen.ms);
  }
  return { maxMs, readings };
}

try {
  const floor = await executor.refusalOf("/tick rate 0.5");
  process.stdout.write(`[clock] /tick rate 0.5: ${floor === undefined ? "ACCEPTED" : `refused (${floor})`}\n`);
  if (floor === undefined) failures.push("the server accepted a tick rate below 1");

  await must("/tick rate 1");
  const slow = await longestRun(20_000);
  process.stdout.write(
    `[clock] at /tick rate 1: ${slow.readings} reading(s) over 20000ms; the longest prompt-answer run at one tick ` +
      `was ${slow.maxMs}ms (stopped is ${STOPPED_CLOCK_MS}ms)\n`,
  );
  const t0 = performance.now();
  const passed = await executor.waitForGameTicks(5, "the slow clock's wait");
  process.stdout.write(`[clock] at /tick rate 1: a 5-tick wait passed at tick ${passed.tick} in ${performance.now() - t0}ms\n`);
  await must("/tick rate 20");

  await must("/tick freeze");
  const t1 = performance.now();
  try {
    await executor.waitForGameTicks(2, "the frozen clock's wait");
    failures.push("a 2-tick wait passed on a frozen clock");
  } catch (err) {
    process.stdout.write(`[clock] frozen: after ${performance.now() - t1}ms, ${(err as Error).name}: ${(err as Error).message}\n`);
    if (!(err instanceof GameTimeStoppedError)) failures.push(`the frozen wait failed otherwise: ${String(err)}`);
  }
} catch (err) {
  failures.push(String(err));
} finally {
  await executor.refusalOf("/tick unfreeze");
  await executor.refusalOf("/tick rate 20");
}
for (const f of failures) process.stdout.write(`[clock] FAIL: ${f}\n`);
process.stdout.write(`[clock] ${failures.length === 0 ? "PASS" : "FAIL"}\n`);
process.exit(failures.length === 0 ? 0 : 1);
