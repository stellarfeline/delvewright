// Parser + tier selection for the compiler's branch plan
// (`validation/branch-plan.json`, emitted by `compiler::branch` — spec-0025 §3).
//
// The ladder used to prove ONE critical path. A campaign that forks — a choice
// that decides who lives, two endings — declared the fork in the DSL, had it
// reachability-checked as a graph, and then never played it. spec-0025 makes
// "provably completable by machine" quantify over BRANCHES: the compiler proves
// each branch structurally (`DW0480`–`DW0485`) and exports, per branch, its flag
// assignment, the dialogue choices that enter it, and an executable path
// (`validation/branch-path-<slug>.json`, the ordinary `critical-path.json`
// contract). This module reads that plan and decides which branches THIS run is
// answerable for.
//
// Assertions + navigation only (the harness contract): nothing here derives a
// command, a position or a flag — every one of those is read off the compiler's
// artifact. What the harness owns is the QUESTION of which branches ran, and the
// rule that a branch which did not run is NAMED, with a reason, never silent.

import { readFile } from "node:fs/promises";
import path from "node:path";

/** The sub-path of the branch plan relative to `critical-path.json`'s dir. */
const BRANCH_PLAN_SUBPATH = ["validation", "branch-plan.json"] as const;

/**
 * A dialogue choice that enters a branch, and the chat line that takes it.
 *
 * **How a dialogue choice is actuated.** A 1.21.11 dialog button is drawn by the
 * CLIENT; mineflayer has no client, so no bot can click one. Every option the
 * compiler emits is therefore backed by a `/trigger dw.dlg_<npc> set <n>` that the
 * button itself runs, and chatting that line is the player-legal primitive the
 * button stands for — the same substitution the exported critical path has made
 * for `talk-to` steps since spec-0002 was amended (2026-07-30), and the same shape
 * as the bonfire `rest` step's "rest and save" line. Scripting a
 * branch choice is therefore not a new mechanism at all: it is the compiler
 * choosing a different option for the same step.
 */
export interface BranchEntryChoice {
  readonly npc: string;
  /** The option's trigger value, 1-based across that NPC's tree. */
  readonly option: number;
  /** The exact chat line the bot sends (`bot.chat(command)`). */
  readonly command: string;
}

/** One enumerated branch, as the compiler proved and exported it. */
export interface PlannedBranch {
  readonly id: string;
  /** The per-branch chronicle file (generation-time narrative review). */
  readonly chronicle: string;
  /**
   * The executable path file for this branch, relative to the plan's own
   * directory — or `undefined` when the branch is unreachable, in which case there
   * is no world that plays it and nothing for the bot to walk.
   */
  readonly pathFile: string | undefined;
  /** Flags pinned SET on this branch. */
  readonly flagsSet: readonly string[];
  /** Flags pinned UNSET on this branch. */
  readonly flagsUnset: readonly string[];
  /** Whether a playthrough realizes this branch's flag assignment. */
  readonly reachable: boolean;
  readonly entryChoices: readonly BranchEntryChoice[];
  /** The endings that fire on this branch. */
  readonly endings: readonly string[];
}

/** The parsed branch plan. */
export interface BranchPlan {
  readonly version: string;
  readonly campaignId: string;
  readonly branches: readonly PlannedBranch[];
  /** Directory the plan was read from — where its `pathFile`s live. */
  readonly dir: string;
}

/** Raised when the branch plan is present but structurally invalid. */
export class BranchPlanParseError extends Error {
  override readonly name = "BranchPlanParseError";
  readonly pointer: string;
  constructor(pointer: string, detail: string) {
    super(`branch-plan${pointer} ${detail}`);
    this.pointer = pointer;
  }
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function fail(pointer: string, detail: string): never {
  throw new BranchPlanParseError(pointer, detail);
}

function str(obj: Record<string, unknown>, key: string, pointer: string): string {
  const v = obj[key];
  if (typeof v !== "string" || v.length === 0) {
    fail(`${pointer}/${key}`, `must be a non-empty string, got ${JSON.stringify(v)}`);
  }
  return v;
}

function strArray(obj: Record<string, unknown>, key: string, pointer: string): string[] {
  const v = obj[key];
  if (!Array.isArray(v)) {
    fail(`${pointer}/${key}`, `must be an array, got ${JSON.stringify(v)}`);
  }
  return v.map((entry, i) => {
    if (typeof entry !== "string") {
      fail(`${pointer}/${key}/${i}`, `must be a string, got ${JSON.stringify(entry)}`);
    }
    return entry;
  });
}

function parseEntryChoice(value: unknown, pointer: string): BranchEntryChoice {
  if (!isRecord(value)) fail(pointer, `must be an object, got ${JSON.stringify(value)}`);
  const option = value["option"];
  if (typeof option !== "number" || !Number.isInteger(option) || option < 1) {
    fail(`${pointer}/option`, `must be a positive integer, got ${JSON.stringify(option)}`);
  }
  return {
    npc: str(value, "npc", pointer),
    option,
    // The whole point of the artifact carrying this: without it the harness would
    // have to rebuild the compiler's id mangling to guess a command, which is
    // game logic and does not belong here.
    command: str(value, "command", pointer),
  };
}

function parseBranch(value: unknown, pointer: string): PlannedBranch {
  if (!isRecord(value)) fail(pointer, `must be an object, got ${JSON.stringify(value)}`);
  const reachable = value["reachable"];
  if (typeof reachable !== "boolean") {
    fail(`${pointer}/reachable`, `must be a boolean, got ${JSON.stringify(reachable)}`);
  }
  const flags = value["flags"];
  if (!isRecord(flags)) {
    fail(`${pointer}/flags`, `must be an object, got ${JSON.stringify(flags)}`);
  }
  const rawPath = value["path"];
  if (rawPath !== null && typeof rawPath !== "string") {
    fail(`${pointer}/path`, `must be a string or null, got ${JSON.stringify(rawPath)}`);
  }
  // A reachable branch the compiler exported no path for is not a run the harness
  // may quietly skip — it is a broken contract between the two halves of
  // spec-0025, and a silent skip is exactly the failure mode this spec exists to
  // end.
  if (reachable && (rawPath === null || rawPath.length === 0)) {
    fail(`${pointer}/path`, "a reachable branch must name an executable path file");
  }
  const choices = value["entry_choices"];
  if (!Array.isArray(choices)) {
    fail(`${pointer}/entry_choices`, `must be an array, got ${JSON.stringify(choices)}`);
  }
  return {
    id: str(value, "id", pointer),
    chronicle: str(value, "chronicle", pointer),
    pathFile: typeof rawPath === "string" && rawPath.length > 0 ? rawPath : undefined,
    flagsSet: strArray(flags, "set", `${pointer}/flags`),
    flagsUnset: strArray(flags, "unset", `${pointer}/flags`),
    reachable,
    entryChoices: choices.map((c, i) => parseEntryChoice(c, `${pointer}/entry_choices/${i}`)),
    endings: strArray(value, "endings", pointer),
  };
}

/** Parse a decoded `branch-plan.json`. `dir` is where its path files live. */
export function parseBranchPlan(value: unknown, dir: string): BranchPlan {
  if (!isRecord(value)) fail("", `must be an object, got ${JSON.stringify(value)}`);
  const branches = value["branches"];
  if (!Array.isArray(branches)) {
    fail("/branches", `must be an array, got ${JSON.stringify(branches)}`);
  }
  if (branches.length === 0) {
    // The compiler emits the artifact only for a campaign that declares branch
    // points, and such a campaign always enumerates at least one branch.
    fail("/branches", "must not be empty — a branch plan with no branch proves nothing");
  }
  const parsed = branches.map((b, i) => parseBranch(b, `/branches/${i}`));
  const seen = new Set<string>();
  for (const [i, b] of parsed.entries()) {
    if (seen.has(b.id)) {
      fail(`/branches/${i}/id`, `duplicate branch id ${JSON.stringify(b.id)}`);
    }
    seen.add(b.id);
  }
  return {
    version: str(value, "version", ""),
    campaignId: str(value, "campaign_id", ""),
    branches: parsed,
    dir,
  };
}

/**
 * Load the branch plan that accompanies a critical path, if the build has one.
 *
 * Absence is NOT an error: a campaign that declares no branch points emits no
 * plan, and the run is the ordinary single-path run it has always been. Malformed
 * data is a hard failure — same stance as the waypoints artifact.
 */
export async function loadBranchPlanForCriticalPath(
  criticalPathPath: string,
): Promise<BranchPlan | undefined> {
  const p = path.join(path.dirname(criticalPathPath), ...BRANCH_PLAN_SUBPATH);
  let text: string;
  try {
    text = await readFile(p, "utf8");
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return undefined;
    throw err;
  }
  return parseBranchPlan(JSON.parse(text) as unknown, path.dirname(p));
}

// ---------------------------------------------------------------------------
// Tiering (spec-0025 §3)
// ---------------------------------------------------------------------------

/**
 * Which branches a run is answerable for.
 *
 * * `all` — the RELEASE tier: every enumerated branch, full run.
 * * `list` — an explicit set of branch ids.
 * * `from-diff` — the PR tier spec-0025 describes: the branches whose content the
 *   diff touches. **Not available**: that mapping is compiler-side (changed
 *   quests/casts/effects → the branches they participate in) and the emission does
 *   not carry it. Selecting it fails loudly rather than silently degrading to
 *   `all` (which would be a lie about cost) or to nothing (which would be a lie
 *   about coverage).
 */
export type BranchTier =
  | { readonly mode: "all" }
  | { readonly mode: "list"; readonly ids: readonly string[] }
  | { readonly mode: "from-diff" };

/** Raised when a tier is named that the toolchain cannot honestly serve. */
export class BranchTierError extends Error {
  override readonly name = "BranchTierError";
}

/**
 * Read the tier from `DELVEWRIGHT_BRANCHES`: `all` (default), `from-diff`, or a
 * comma-separated list of branch ids.
 */
export function branchTierFromEnv(env = process.env): BranchTier {
  const raw = (env["DELVEWRIGHT_BRANCHES"] ?? "all").trim();
  if (raw.length === 0 || raw === "all") return { mode: "all" };
  if (raw === "from-diff") return { mode: "from-diff" };
  const ids = raw
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
  if (ids.length === 0) {
    throw new BranchTierError(
      `DELVEWRIGHT_BRANCHES=${JSON.stringify(raw)} names no branch — use 'all', 'from-diff', ` +
        `or a comma-separated list of branch ids`,
    );
  }
  return { mode: "list", ids };
}

/** A branch that will not run, and why. The reason is never optional. */
export interface SkippedBranch {
  readonly branch: string;
  readonly reason: string;
}

/** The outcome of applying a tier to a plan. */
export interface BranchSelection {
  readonly tier: string;
  /** Branch ids this tier is answerable for, in plan order. */
  readonly selected: readonly string[];
  /** Every branch that will not run, each with a named reason. */
  readonly skipped: readonly SkippedBranch[];
}

/**
 * Apply a tier to a plan: which branches run, and why each of the rest does not.
 *
 * An unreachable branch is skipped under every tier — the compiler exported no
 * path because no playthrough realizes its assignment, which `DW0482` has already
 * failed the build for; naming it here keeps the run report's branch list complete
 * rather than quietly short.
 */
export function selectBranches(plan: BranchPlan, tier: BranchTier): BranchSelection {
  if (tier.mode === "from-diff") {
    throw new BranchTierError(
      "DELVEWRIGHT_BRANCHES=from-diff: the diff→branches mapping is compiler-side and is not " +
        "emitted yet (spec-0025 §3 PR tier). Name the branches explicitly " +
        "(DELVEWRIGHT_BRANCHES=branch/a,branch/b) or run the release tier " +
        "(DELVEWRIGHT_BRANCHES=all) until the compiler exports it.",
    );
  }
  const ids = plan.branches.map((b) => b.id);
  if (tier.mode === "list") {
    const unknown = tier.ids.filter((id) => !ids.includes(id));
    if (unknown.length > 0) {
      // A typo must not read as "that branch was skipped": it would name a
      // reason for a branch that does not exist while the real one goes unrun.
      throw new BranchTierError(
        `DELVEWRIGHT_BRANCHES names branch(es) this build does not declare: ` +
          `${unknown.join(", ")} (declared: ${ids.join(", ")})`,
      );
    }
  }
  const wanted = (b: PlannedBranch): boolean =>
    tier.mode === "all" ? true : tier.ids.includes(b.id);
  const selected: string[] = [];
  const skipped: SkippedBranch[] = [];
  for (const b of plan.branches) {
    if (!b.reachable) {
      skipped.push({
        branch: b.id,
        reason:
          "unreachable: no playthrough realizes this branch's flag assignment, so the " +
          "compiler exported no path to walk (see DW0482)",
      });
      continue;
    }
    if (!wanted(b)) {
      skipped.push({
        branch: b.id,
        reason: `not selected by this tier (DELVEWRIGHT_BRANCHES=${describeTier(tier)})`,
      });
      continue;
    }
    selected.push(b.id);
  }
  return { tier: describeTier(tier), selected, skipped };
}

/** The tier as it is written in the environment — quoted back in every reason. */
export function describeTier(tier: BranchTier): string {
  return tier.mode === "list" ? tier.ids.join(",") : tier.mode;
}

/**
 * Which branch THIS process drives (`DELVEWRIGHT_BRANCH`), or `undefined` for the
 * ordinary single-path run.
 *
 * One branch per invocation, by construction: a delve's quest/flag state is party
 * state that only ever moves forward, so a second branch needs a second WORLD, not
 * a second pass. `validation/branch-runs.sh` is the loop that gives it one.
 */
export function drivenBranchFromEnv(env = process.env): string | undefined {
  const raw = env["DELVEWRIGHT_BRANCH"];
  return raw !== undefined && raw.trim().length > 0 ? raw.trim() : undefined;
}

/**
 * Resolve the branch this invocation drives against the plan and the tier.
 *
 * Fails loudly when the named branch is not in the plan, or is not one the tier
 * selected — a run that quietly walked a branch its tier excluded would report
 * coverage it was never asked for.
 */
export function resolveDrivenBranch(
  plan: BranchPlan,
  selection: BranchSelection,
  id: string,
): PlannedBranch {
  const branch = plan.branches.find((b) => b.id === id);
  if (branch === undefined) {
    throw new BranchTierError(
      `DELVEWRIGHT_BRANCH=${JSON.stringify(id)} is not a branch of this build ` +
        `(declared: ${plan.branches.map((b) => b.id).join(", ")})`,
    );
  }
  if (!selection.selected.includes(id)) {
    const why = selection.skipped.find((s) => s.branch === id)?.reason ?? "not selected";
    throw new BranchTierError(
      `DELVEWRIGHT_BRANCH=${JSON.stringify(id)} cannot run: ${why}`,
    );
  }
  return branch;
}

/**
 * Assert the branch's exported path really takes the choices that ENTER it.
 *
 * The one thing a branch run must not be able to do is pass while having walked
 * somebody else's storyline. Every entry choice's chat line has to appear as a
 * `talk-to` step's command on the path being walked; if one does not, the path and
 * the plan disagree about what this branch is, and the run stops before it can
 * report coverage it does not have.
 *
 * Returns the commands it matched, for the run report.
 */
export function assertEntryChoicesOnPath(
  branch: PlannedBranch,
  steps: readonly { readonly action: string; readonly command?: string }[],
): string[] {
  const commands = new Set(
    steps.flatMap((s) => (s.action === "talk-to" && s.command !== undefined ? [s.command] : [])),
  );
  const missing = branch.entryChoices.filter((c) => !commands.has(c.command));
  if (missing.length > 0) {
    throw new BranchTierError(
      `branch ${branch.id}: its exported path never takes the choice that enters it — ` +
        `${missing.map((c) => `${c.command} (option #${c.option} of ${c.npc})`).join(", ")} ` +
        `is on no talk-to step. The plan and the path disagree about this branch; ` +
        `rebuild with a current delvec.`,
    );
  }
  return branch.entryChoices.map((c) => c.command);
}
