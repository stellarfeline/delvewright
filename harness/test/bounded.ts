/**
 * Bounded waits for tests: a stuck wait fails its test by name, saying what it
 * waited for.
 *
 * A test that awaits something nothing guarantees will settle sits at 0 CPU and
 * reports nothing. Two bounds close that:
 *
 * - `npm test` runs `node --test --test-timeout=<ms>`, so no test outlives that
 *   bound, labelled or not; node:test fails it by name.
 * - {@link within} races one wait against {@link WAIT_BOUND_MS} and rejects naming
 *   the wait. It must be shorter than the per-test bound, so a labelled wait that
 *   never settles fails with its own label before the test's timeout fires; a
 *   per-test bound at or below it is refused when this module loads.
 *
 * The wait bound is a generous multiple of the slowest legitimate test (about
 * 31 s, a die-retry trial sitting out its settle windows); it is a hang detector,
 * never a performance budget.
 */

/** The longest one labelled wait may take before it fails, naming itself. */
export const WAIT_BOUND_MS = 90_000;

// node:test hands `--test-timeout` to each test file's process; read it there.
const testTimeoutArg = process.execArgv.findLast((a) => a.startsWith("--test-timeout="));
const testTimeoutMs = testTimeoutArg === undefined ? 0 : Number(testTimeoutArg.split("=")[1]);
if (testTimeoutMs > 0 && testTimeoutMs <= WAIT_BOUND_MS) {
  throw new Error(
    `--test-timeout=${testTimeoutMs} is not above WAIT_BOUND_MS (${WAIT_BOUND_MS}ms): a stuck ` +
      `wait would fail as an anonymous test timeout instead of naming what it waited for`,
  );
}

/**
 * `wait`, or a rejection saying `what` was still outstanding after `ms`.
 *
 * The timer is cleared the moment `wait` settles, so a bounded wait that resolves
 * leaves nothing behind to hold the process open.
 */
export function within<T>(what: string, wait: Promise<T>, ms: number = WAIT_BOUND_MS): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const deadline = new Promise<never>((_, reject) => {
    timer = setTimeout(
      () => reject(new Error(`still waiting for ${what} after ${ms}ms — it never settled`)),
      ms,
    );
  });
  return Promise.race([wait, deadline]).finally(() => clearTimeout(timer));
}
