import { test } from "node:test";
import assert from "node:assert/strict";
import {
  BranchPlanParseError,
  BranchTierError,
  assertEntryChoicesOnPath,
  branchTierFromEnv,
  drivenBranchFromEnv,
  parseBranchPlan,
  resolveDrivenBranch,
  selectBranches,
} from "../src/branch-plan.ts";

/** The `branch-two-endings` fixture's plan, as `delvec build` emits it. */
function planJson(): Record<string, unknown> {
  return {
    version: "0.8.0",
    campaign_id: "hello-world",
    branches: [
      {
        id: "branch/hold",
        chronicle: "branch-chronicle-hold.md",
        path: "branch-path-hold.json",
        selection: { "branch-point/gate": "branch/hold" },
        flags: { set: ["flag/wait"], unset: ["flag/flee"] },
        opens_at: ["quest/decide"],
        leads_to: ["ending/held"],
        reachable: true,
        entry_choices: [
          { npc: "npc/keeper", option: 2, command: "/trigger dw.dlg_keeper set 2" },
        ],
        endings: ["ending/held"],
        critical_path: [],
      },
      {
        id: "branch/bolt",
        chronicle: "branch-chronicle-bolt.md",
        path: "branch-path-bolt.json",
        selection: { "branch-point/gate": "branch/bolt" },
        flags: { set: ["flag/flee"], unset: ["flag/wait"] },
        opens_at: ["quest/decide"],
        leads_to: ["ending/abandoned"],
        reachable: true,
        entry_choices: [
          { npc: "npc/keeper", option: 3, command: "/trigger dw.dlg_keeper set 3" },
        ],
        endings: ["ending/abandoned"],
        critical_path: [],
      },
    ],
  };
}

test("the plan parses every branch with its flags, entry choices and path file", () => {
  const plan = parseBranchPlan(planJson(), "/delve/validation");
  assert.equal(plan.campaignId, "hello-world");
  assert.equal(plan.branches.length, 2);
  const bolt = plan.branches[1]!;
  assert.equal(bolt.id, "branch/bolt");
  assert.equal(bolt.pathFile, "branch-path-bolt.json");
  assert.deepEqual([...bolt.flagsSet], ["flag/flee"]);
  assert.deepEqual([...bolt.flagsUnset], ["flag/wait"]);
  assert.deepEqual([...bolt.endings], ["ending/abandoned"]);
  assert.equal(bolt.entryChoices[0]!.command, "/trigger dw.dlg_keeper set 3");
});

test("a reachable branch with no exported path is a broken contract, not a quiet skip", () => {
  // The half-implemented case: the plan says a branch is playable and the build
  // shipped nothing to play it with. Accepting that would let a build report a
  // branch as skipped for a reason nobody wrote down.
  const json = planJson();
  (json["branches"] as Record<string, unknown>[])[0]!["path"] = null;
  assert.throws(() => parseBranchPlan(json, "/x"), BranchPlanParseError);
});

test("a malformed entry choice is rejected at its JSON pointer", () => {
  const json = planJson();
  const first = (json["branches"] as Record<string, unknown>[])[0]!;
  (first["entry_choices"] as Record<string, unknown>[])[0]!["command"] = 7;
  assert.throws(
    () => parseBranchPlan(json, "/x"),
    (err: unknown) =>
      err instanceof BranchPlanParseError &&
      err.pointer === "/branches/0/entry_choices/0/command",
  );
});

test("two branches with the same id cannot both be reported on", () => {
  const json = planJson();
  const branches = json["branches"] as Record<string, unknown>[];
  branches[1]!["id"] = branches[0]!["id"];
  assert.throws(() => parseBranchPlan(json, "/x"), BranchPlanParseError);
});

test("the default tier is 'all' — the release tier, every enumerated branch", () => {
  const plan = parseBranchPlan(planJson(), "/x");
  const selection = selectBranches(plan, branchTierFromEnv({}));
  assert.deepEqual([...selection.selected], ["branch/hold", "branch/bolt"]);
  assert.deepEqual([...selection.skipped], []);
  assert.equal(selection.tier, "all");
});

test("a listed tier selects what it names and NAMES what it skips", () => {
  const plan = parseBranchPlan(planJson(), "/x");
  const tier = branchTierFromEnv({ DELVEWRIGHT_BRANCHES: "branch/bolt" });
  const selection = selectBranches(plan, tier);
  assert.deepEqual([...selection.selected], ["branch/bolt"]);
  assert.equal(selection.skipped.length, 1);
  assert.equal(selection.skipped[0]!.branch, "branch/hold");
  // The rule spec-0025 states: a skipped branch is named, never silent — and the
  // reason quotes the tier, so a reader can tell WHY coverage stopped there.
  assert.match(selection.skipped[0]!.reason, /not selected by this tier/);
  assert.match(selection.skipped[0]!.reason, /branch\/bolt/);
});

test("an unreachable branch is skipped under every tier, with the reason", () => {
  const json = planJson();
  const bolt = (json["branches"] as Record<string, unknown>[])[1]!;
  bolt["reachable"] = false;
  bolt["path"] = null;
  const plan = parseBranchPlan(json, "/x");
  const selection = selectBranches(plan, { mode: "all" });
  assert.deepEqual([...selection.selected], ["branch/hold"]);
  assert.match(selection.skipped[0]!.reason, /unreachable/);
  assert.match(selection.skipped[0]!.reason, /DW0482/);
});

test("a tier naming a branch the build does not declare fails loudly", () => {
  // A typo must not read as "that branch was skipped": the real branch would go
  // unrun while the report explained a branch that does not exist.
  const plan = parseBranchPlan(planJson(), "/x");
  assert.throws(
    () => selectBranches(plan, { mode: "list", ids: ["branch/blot"] }),
    (err: unknown) => err instanceof BranchTierError && /does not declare/.test(err.message),
  );
});

test("from-diff refuses to run rather than silently covering the wrong set", () => {
  // The PR tier spec-0025 describes needs a compiler-side diff→branches mapping
  // that is not emitted. Degrading to `all` would lie about cost; degrading to
  // nothing would lie about coverage.
  const plan = parseBranchPlan(planJson(), "/x");
  assert.throws(
    () => selectBranches(plan, branchTierFromEnv({ DELVEWRIGHT_BRANCHES: "from-diff" })),
    (err: unknown) => err instanceof BranchTierError && /compiler-side/.test(err.message),
  );
});

test("DELVEWRIGHT_BRANCH names the branch this session drives; unset = the exported run", () => {
  assert.equal(drivenBranchFromEnv({}), undefined);
  assert.equal(drivenBranchFromEnv({ DELVEWRIGHT_BRANCH: "  " }), undefined);
  assert.equal(drivenBranchFromEnv({ DELVEWRIGHT_BRANCH: " branch/bolt " }), "branch/bolt");
});

test("driving a branch the tier excluded is refused, quoting the skip reason", () => {
  const plan = parseBranchPlan(planJson(), "/x");
  const selection = selectBranches(plan, { mode: "list", ids: ["branch/hold"] });
  assert.throws(
    () => resolveDrivenBranch(plan, selection, "branch/bolt"),
    (err: unknown) => err instanceof BranchTierError && /not selected by this tier/.test(err.message),
  );
  assert.equal(resolveDrivenBranch(plan, selection, "branch/hold").id, "branch/hold");
});

test("a branch path that never takes the branching choice cannot pass as a branch run", () => {
  const plan = parseBranchPlan(planJson(), "/x");
  const bolt = plan.branches[1]!;
  const wrongPath = [
    { action: "select-class", command: "/trigger dw.class set 1" },
    // the HOLD option — this is the other branch's storyline
    { action: "talk-to", command: "/trigger dw.dlg_keeper set 2" },
  ];
  assert.throws(
    () => assertEntryChoicesOnPath(bolt, wrongPath),
    (err: unknown) => err instanceof BranchTierError && /never takes the choice/.test(err.message),
  );
  const rightPath = [
    { action: "select-class", command: "/trigger dw.class set 1" },
    { action: "talk-to", command: "/trigger dw.dlg_keeper set 3" },
    { action: "reach" },
  ];
  assert.deepEqual(assertEntryChoicesOnPath(bolt, rightPath), [
    "/trigger dw.dlg_keeper set 3",
  ]);
});
