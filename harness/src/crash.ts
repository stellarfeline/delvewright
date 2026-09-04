// The last line of defence: a harness that dies still says so in its artifact.
//
// A step that fails is a verdict, and the run report carries it. A harness that
// CRASHES is not a verdict, and before this existed it left nothing at all: the
// process exited, no report was written, and the shell could say only "NO run
// report — a validation-infrastructure fault". Everything the run had already
// established — the legs it walked, the assists it opened, the trials it
// completed — went with it, and a reader of the stage was left with a red that
// looked exactly like content.
//
// So the two ways a Node process dies without anyone's `catch` seeing it are
// caught here, at the process, and turned into `harness_crash: {stage, reason}` in
// the report:
//
//   * `unhandledRejection` — a promise rejected with no handler attached. The
//     class that motivated this: `mineflayer-pathfinder` rejects an in-flight
//     `goto` out of band whenever a goal is set, so any walk the harness stopped
//     awaiting can reject into nothing. navigation.ts removes that source; this
//     catches whatever remains, including sources nobody has met yet.
//   * `uncaughtException` — a throw from a callback or a timer, outside any stack
//     the run controls.
//
// This is a REPORTER, never a recovery: the process still ends, and it ends
// failed. Continuing after an unhandled rejection would be running a bot whose
// state nobody can describe, which is worse than a red.

import { writeFileSync } from "node:fs";
import type { CrashStage, RunReport } from "./report.ts";

/**
 * Exit code for a harness crash — distinct from a step failure (1) and from a bot
 * death (3), so a reader of an exit code alone can still tell the harness dying
 * from the delve failing.
 */
export const EXIT_HARNESS_CRASH = 4;

/** How the crash reporter reaches the world; injectable so a test can watch it. */
export interface CrashHooks {
  /** What the run is doing right now, read at the moment of the crash. */
  readonly stage: () => CrashStage;
  /** The report to stamp, and where to write it (absent → stderr only). */
  readonly report: () => RunReport;
  readonly reportPath: () => string | undefined;
  /** Defaults to `process.exit`. */
  readonly exit?: (code: number) => void;
  /** Defaults to `process.stderr.write`. */
  readonly write?: (line: string) => void;
}

/**
 * Render an unknown thrown value as one line a reader can act on. An `Error` with
 * a meaningful `name` keeps it: `GoalChanged: ...` is the whole diagnosis of the
 * defect this was built for, and `Error: ...` adds nothing, so it is dropped.
 */
export function crashReason(err: unknown): string {
  if (err instanceof Error) {
    const name = err.name;
    return name && name !== "Error" ? `${name}: ${err.message}` : err.message;
  }
  return String(err);
}

/**
 * Stamp `report` with the crash, write it out, and return the exit code. Split
 * from the process handlers so the whole behaviour is reachable from a test
 * without an actual unhandled rejection.
 */
export function reportCrash(err: unknown, kind: string, hooks: CrashHooks): number {
  const write = hooks.write ?? ((line: string): void => void process.stderr.write(line));
  const reason = crashReason(err);
  const report = hooks.report();
  const stage = hooks.stage();
  report.recordHarnessCrash({ stage, reason });
  write(
    `HARNESS CRASH (${kind}) during '${stage}': ${reason}\n` +
      `  This is a fault in the harness, NOT a verdict on the delve. No stage below ` +
      `it decides anything about the content.\n`,
  );
  if (err instanceof Error && err.stack) write(`${err.stack}\n`);
  const path = hooks.reportPath();
  if (path !== undefined) {
    try {
      // Synchronous on purpose: the process is about to end, and an awaited write
      // in a crash handler is a write that may never happen.
      writeFileSync(path, `${JSON.stringify(report.toJSON(), null, 2)}\n`, "utf8");
      write(`run report (harness crash) written to ${path}\n`);
    } catch (writeErr) {
      write(`could not write the crash report to ${path}: ${crashReason(writeErr)}\n`);
    }
  }
  return EXIT_HARNESS_CRASH;
}

/**
 * Arm the process-level handlers. Idempotent per process by construction — call it
 * once, at startup, before anything can reject.
 */
export function installCrashReporter(hooks: CrashHooks): void {
  const exit = hooks.exit ?? ((code: number): void => process.exit(code));
  let crashed = false;
  const handle = (err: unknown, kind: string): void => {
    // One crash per process: a handler that re-enters (the write itself throwing,
    // a cascade of rejections from the same broken state) must not loop.
    if (crashed) return;
    crashed = true;
    exit(reportCrash(err, kind, hooks));
  };
  process.on("unhandledRejection", (err: unknown) => handle(err, "unhandledRejection"));
  process.on("uncaughtException", (err: unknown) => handle(err, "uncaughtException"));
}
