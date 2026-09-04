import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import nodePath from "node:path";
import { fileURLToPath } from "node:url";
import { EXIT_HARNESS_CRASH, crashReason, reportCrash } from "../src/crash.ts";
import { RunReport } from "../src/report.ts";

function scratchDir(): string {
  return mkdtempSync(nodePath.join(tmpdir(), "dw-crash-"));
}

const SRC = fileURLToPath(new URL("../src/", import.meta.url));

test("an unhandled rejection lands in the report as a crash naming its stage", () => {
  // In a CHILD PROCESS, deliberately. An unhandled rejection is a property of a
  // process — `node:test` installs its own listener and fails the test on one — so
  // the only place the real thing can be observed is a process of its own. That is
  // also the only way to assert what actually went wrong live: the run died with an
  // exit code and no report at all, and the shell could say nothing better than "NO
  // run report — a validation-infrastructure fault".
  const dir = scratchDir();
  const path = nodePath.join(dir, "run-report.json");
  const driver = nodePath.join(dir, "driver.ts");
  try {
    writeFileSync(
      driver,
      [
        `import { installCrashReporter } from ${JSON.stringify(`${SRC}crash.ts`)};`,
        `import { RunReport } from ${JSON.stringify(`${SRC}report.ts`)};`,
        `const report = new RunReport("greyhithe-saltworks", "hard");`,
        `installCrashReporter({`,
        `  stage: () => "die-retry",`,
        `  report: () => report,`,
        `  reportPath: () => ${JSON.stringify(path)},`,
        `});`,
        // Exactly the shape mineflayer-pathfinder produces: a promise rejected from
        // a later turn of the loop, with every handler long gone.
        `const err = new Error("The goal was changed before it could be completed!");`,
        `err.name = "GoalChanged";`,
        `setTimeout(() => { void Promise.reject(err); }, 0);`,
        // Keep the process alive long enough for the rejection to be reported.
        `setTimeout(() => { process.exit(0); }, 5_000);`,
        ``,
      ].join("\n"),
      "utf8",
    );
    const run = spawnSync(process.execPath, [driver], { encoding: "utf8", timeout: 30_000 });

    assert.equal(
      run.status,
      EXIT_HARNESS_CRASH,
      `the run ends distinguishably (stderr: ${run.stderr})`,
    );
    const written = JSON.parse(readFileSync(path, "utf8")) as {
      campaign_id: string;
      harness_crash: { stage: string; reason: string } | null;
    };
    assert.equal(written.campaign_id, "greyhithe-saltworks");
    assert.deepEqual(written.harness_crash, {
      stage: "die-retry",
      reason: "GoalChanged: The goal was changed before it could be completed!",
    });
    assert.match(run.stderr, /HARNESS CRASH \(unhandledRejection\) during 'die-retry'/);
    assert.match(run.stderr, /NOT a verdict on the delve/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("a report that reached no crash carries harness_crash: null", () => {
  // A key that only appears on a crash is a key a reader has to know to look for.
  // The explicit null is what makes a non-null value legible.
  const report = new RunReport("greyhithe-saltworks", "hard");
  assert.equal((report.toJSON() as { harness_crash: unknown }).harness_crash, null);
  assert.equal(report.crash(), undefined);
});

test("the first crash wins — a cascade cannot overwrite the reason that killed the run", () => {
  const report = new RunReport("c", "hard");
  report.recordHarnessCrash({ stage: "die-retry", reason: "GoalChanged: the goal was changed" });
  report.recordHarnessCrash({ stage: "critical-path", reason: "and then everything else broke" });
  assert.deepEqual(report.crash(), {
    stage: "die-retry",
    reason: "GoalChanged: the goal was changed",
  });
});

test("a crash keeps everything the run had already established", () => {
  // The report is not a tombstone: what the run proved before it died is still the
  // most useful thing in the file, and a reader needs both halves at once.
  const dir = scratchDir();
  const path = nodePath.join(dir, "run-report.json");
  try {
    const report = new RunReport("greyhithe-saltworks", "hard");
    report.stage({
      stage: "critical-path",
      ran: true,
      passed: false,
      findings: [],
      failures: ["step 26 (kill) failed"],
    });
    const code = reportCrash(new Error("boom"), "uncaughtException", {
      stage: () => "death-loop",
      report: () => report,
      reportPath: () => path,
      write: () => {},
    });
    assert.equal(code, EXIT_HARNESS_CRASH);
    const written = JSON.parse(readFileSync(path, "utf8")) as {
      stages: Array<{ stage: string; failures: string[] }>;
      harness_crash: { stage: string };
    };
    assert.equal(written.harness_crash.stage, "death-loop");
    assert.deepEqual(written.stages[0]?.failures, ["step 26 (kill) failed"]);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("a crash reason keeps an informative error name and drops a useless one", () => {
  // `GoalChanged: ...` IS the diagnosis; `Error: ...` is noise in front of it.
  const named = new Error("The goal was changed before it could be completed!");
  named.name = "GoalChanged";
  assert.equal(crashReason(named), "GoalChanged: The goal was changed before it could be completed!");
  assert.equal(crashReason(new Error("plain")), "plain");
  assert.equal(crashReason("a string nobody wrapped"), "a string nobody wrapped");
});
