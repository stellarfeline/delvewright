// `node src/branch-select.ts <critical-path.json>` — print the branches the
// current tier is answerable for, one id per line (spec-0025 §3).
//
// The branch-run driver (`validation/branch-runs.sh`) needs to know which
// branches to give a fresh world to, and `run.ts` needs to know which branch it
// may drive. Both answers must come from ONE implementation, or a tier could
// select a branch the run then refuses — so this entry point is a thin shell over
// the same `selectBranches` the run uses, and never re-decides anything.
//
// stdout: the selected branch ids (the driver's loop).
// stderr: every skipped branch with its reason (a skipped branch is named).
// exit 2: a tier this toolchain cannot honestly serve (e.g. `from-diff`).

import {
  BranchTierError,
  branchTierFromEnv,
  loadBranchPlanForCriticalPath,
  selectBranches,
} from "./branch-plan.ts";

async function main(): Promise<number> {
  const pathArg = process.argv[2];
  if (pathArg === undefined || pathArg.length === 0) {
    process.stderr.write("usage: node src/branch-select.ts <path-to-critical-path.json>\n");
    return 2;
  }
  const plan = await loadBranchPlanForCriticalPath(pathArg);
  if (plan === undefined) {
    process.stderr.write(
      "this build declares no narrative branches (no validation/branch-plan.json)\n",
    );
    return 0;
  }
  const selection = selectBranches(plan, branchTierFromEnv());
  for (const s of selection.skipped) {
    process.stderr.write(`[skipped] ${s.branch}: ${s.reason}\n`);
  }
  for (const id of selection.selected) {
    process.stdout.write(`${id}\n`);
  }
  return 0;
}

main()
  .then((code) => process.exit(code))
  .catch((err: unknown) => {
    const message = err instanceof Error ? err.message : String(err);
    process.stderr.write(`FAILED: ${message}\n`);
    process.exit(err instanceof BranchTierError ? 2 : 1);
  });
