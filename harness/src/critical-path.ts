// Parser and strict runtime validation for `critical-path.json`, the compiler's
// bot-walkthrough contract output (spec-0002, amended 2026-07-30). The harness
// contains zero campaign-specific logic: it only interprets the closed set of
// step actions defined by the spec. Validation is hand-rolled (no external schema
// library) so error messages can point precisely at the offending JSON location.
//
// Amended contract (spec-0002, 2026-07-30): interactive steps carry the exact
// chat command the bot sends (`command`), replacing `option_path` — mineflayer
// cannot click 1.21.11 server-driven dialog buttons, so the bot drives dialog
// outcomes by chatting the same `/trigger` command each button runs.
// `assert-complete` carries a `scoreboard: { objective, value }` object.
//
// Contract format 2 (AUDIT-P0): the file carries `format_version`, and every
// objective-bearing step (`talk-to`/`reach`/`kill`/`collect`/`interact`) names the
// `obj/<id>` it proves. That id is the step's success criterion — the executor
// passes the step only on that objective's own anchored completion marker
// (`markers.ts`). The harness never infers an objective; it only compares the id the
// compiler put here (CLAUDE.md: assertions and navigation, no game logic).

/**
 * The critical-path format is versioned with the DSL (spec-0002). Each DSL
 * version is an additive superset (v0.3 kill/collect/interact steps; v0.4
 * sneak/transport/cutscene step fields), and the `version` field is
 * campaign-derived (a v0.2 delve still emits a v0.2 path). v0.5 (world
 * time/weather/lighting) and v0.6 (horizon/boundary, checkpoints/stealth,
 * sound/art, per-effect flag gating) add DSL/emission surface but leave the
 * critical-path step contract the bot consumes unchanged, so their paths parse
 * and run exactly as a v0.4 path does — the allowlist simply tracks the DSL.
 * v0.7 (the cast ledger) and v0.8 (spec-0025 branch points, per-node
 * `happening`, named endings) are the same kind of addition: they add
 * VALIDATION surface — `branch-plan.json`, the chronicles, the per-branch paths —
 * and change no step the bot walks, so a v0.8 path is walked exactly as a v0.6
 * one is. Without them here, the first campaign to declare a branch could not be
 * run at all, branch tier or not.
 */
export const SUPPORTED_DSL_VERSIONS = [
  "0.2.0",
  "0.3.0",
  "0.4.0",
  "0.5.0",
  "0.6.0",
  "0.7.0",
  "0.8.0",
] as const;

/**
 * The version of the critical-path **contract** itself, independent of the DSL
 * version above (the DSL describes the delve; this describes what the harness is
 * told about proving it).
 *
 * * `1` — the pre-oracle shape: steps carried no objective id, so a step could only
 *   be checked positionally and completion was a single unanchored chat substring.
 *   A path of this shape is unverifiable, and the harness has no way to tell a real
 *   pass from a hollow one — so it is REJECTED, not accepted with reduced checks
 *   (a run that cannot prove completion must not report success).
 * * `2` — every objective-bearing step names the `obj/<id>` it proves, and
 *   completion is proved by the anchored per-objective marker channel
 *   (`markers.ts`).
 *
 * Rebuild the delve with a current `delvec` to produce a supported path.
 */
export const CRITICAL_PATH_FORMAT_VERSION = 2;

/** The closed set of critical-path step actions (spec-0002 / spec-0001 enum). */
export const STEP_ACTIONS = [
  "select-class",
  "talk-to",
  "reach",
  "kill",
  "collect",
  "interact",
  "rest",
  "assert-complete",
] as const;

export type StepAction = (typeof STEP_ACTIONS)[number];

/** Absolute block position `[x, y, z]`, resolved by the compiler after placement. */
export type Vec3Tuple = readonly [number, number, number];

/** Select a class by chatting the compiler-assigned `/trigger dw.class set <n>`. */
export interface SelectClassStep {
  readonly action: "select-class";
  readonly class: string;
  /** The exact chat command the bot sends (`bot.chat(command)`). */
  readonly command: string;
}

/**
 * The absolute destination a step's completion teleports the player to, when the
 * next critical objective lives in a different area (gap 8). The compiler emits it
 * as the step's optional `transport` field; the harness waits for the position
 * discontinuity before starting the next step so a cross-area teleport does not
 * race the next step's pathfinding. Absent (undefined) on steps that stay in area.
 */
export type Transport = Vec3Tuple | undefined;

/**
 * v0.4 (spec-0008 gap-7) presentation/stealth markers a walking step may carry.
 * Both are additive and default off, so a v0.2/v0.3 path is byte-identical:
 *   - `sneak` — walk this leg crouched, sprint disabled (stealth envelope).
 *   - `cutsceneSeconds` — after this step completes, the bot may be forced into
 *     spectator and flown ~n seconds; the harness waits for control to return.
 * `false`/absent `sneak` is normalized to the key being absent.
 */
export interface PresentationMarkers {
  readonly sneak?: boolean;
  readonly cutsceneSeconds?: number;
}

/** Talk to an NPC at `pos`, then chat the compiler-assigned dialog `/trigger`. */
export interface TalkToStep extends PresentationMarkers {
  readonly action: "talk-to";
  /** The `obj/<id>` this step must prove complete (format 2). */
  readonly objective: string;
  readonly npc: string;
  readonly pos: Vec3Tuple;
  /** The exact chat command the bot sends (`bot.chat(command)`). */
  readonly command: string;
  /** Cross-area teleport destination on completion, if any (gap 8). */
  readonly transport?: Transport;
}

/** Reach an anchor: get within `radius` blocks of the absolute position `pos`. */
export interface ReachStep extends PresentationMarkers {
  readonly action: "reach";
  /** The `obj/<id>` this step must prove complete (format 2). */
  readonly objective: string;
  readonly anchor: string;
  readonly pos: Vec3Tuple;
  readonly radius: number;
  /** Cross-area teleport destination on completion, if any (gap 8). */
  readonly transport?: Transport;
}

/** Slay a wave: go to `pos`, attack the wave's mobs until the wave is cleared (v0.3). */
export interface KillStep extends PresentationMarkers {
  readonly action: "kill";
  /** The `obj/<id>` this step must prove complete (format 2). */
  readonly objective: string;
  readonly wave: string;
  readonly pos: Vec3Tuple;
  /** Entity tag on the wave's mobs (informational; mineflayer cannot read tags). */
  readonly tag: string;
  /** Total mob count in the wave. */
  readonly count: number;
  /** Cross-area teleport destination on completion, if any (gap 8). */
  readonly transport?: Transport;
}

/** Collect `count` of `item` from a chest at `pos` (v0.3). */
export interface CollectStep extends PresentationMarkers {
  readonly action: "collect";
  /** The `obj/<id>` this step must prove complete (format 2). */
  readonly objective: string;
  readonly item: string;
  readonly count: number;
  readonly pos: Vec3Tuple;
  /** Cross-area teleport destination on completion, if any (gap 8). */
  readonly transport?: Transport;
}

/** Interact at `pos`, then chat `command`; `requires_item` may gate it (v0.3). */
export interface InteractStep extends PresentationMarkers {
  readonly action: "interact";
  /** The `obj/<id>` this step must prove complete (format 2). */
  readonly objective: string;
  readonly anchor: string;
  readonly pos: Vec3Tuple;
  /** The exact chat command the bot sends (`bot.chat(command)`). */
  readonly command: string;
  /** Item that must be held for the interaction to complete, or `null`. */
  readonly requiresItem: string | null;
  /** Cross-area teleport destination on completion, if any (gap 8). */
  readonly transport?: Transport;
}

/**
 * Rest at bonfire `bonfire` (spec-0016 §1, compiler #220). A path EXPORT step: it
 * proves no objective of its own, it performs the player loop the steps after it
 * are proven under.
 *
 * A bonfire only ARMS an affordance; the respawn point moves when the party
 * actually rests. A ladder that walked past every fire without touching one left
 * the checkpoint at world spawn, so a die-retry trial respawned on the beach and
 * blew the walk-back budget — judging the campaign for a proof that never
 * performed the loop (bell round 3).
 */
export interface RestStep extends PresentationMarkers {
  readonly action: "rest";
  /** Which bonfire, by the compiler's index — names the fire in every diagnostic. */
  readonly bonfire: number;
  readonly anchor: string;
  readonly pos: Vec3Tuple;
  /** The exact chat line the "rest and save" dialog button runs. */
  readonly command: string;
}

/** Assert the campaign-completion scoreboard objective holds `value` (terminal step). */
export interface AssertCompleteStep {
  readonly action: "assert-complete";
  /** The sidebar-displayed objective the bot reads. */
  readonly objective: string;
  readonly value: number;
}

export type Step =
  | SelectClassStep
  | TalkToStep
  | ReachStep
  | KillStep
  | CollectStep
  | InteractStep
  | RestStep
  | AssertCompleteStep;

export interface CriticalPath {
  readonly version: string;
  /** Contract version; see {@link CRITICAL_PATH_FORMAT_VERSION}. */
  readonly formatVersion: number;
  readonly campaignId: string;
  readonly steps: readonly Step[];
}

/**
 * Raised when `critical-path.json` is structurally invalid. The message names a
 * JSON-pointer-style path to the offending location.
 */
export class CriticalPathParseError extends Error {
  override readonly name = "CriticalPathParseError";
  /** JSON-pointer-style location of the fault (e.g. `/steps/1/pos/2`). */
  readonly pointer: string;

  constructor(pointer: string, detail: string) {
    super(`critical-path${pointer} ${detail}`);
    this.pointer = pointer;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function fail(pointer: string, detail: string): never {
  throw new CriticalPathParseError(pointer, detail);
}

function requireObject(
  value: unknown,
  pointer: string,
): Record<string, unknown> {
  if (!isRecord(value)) {
    fail(pointer, `must be an object, got ${describe(value)}`);
  }
  return value;
}

function requireString(
  obj: Record<string, unknown>,
  key: string,
  pointer: string,
): string {
  const value = obj[key];
  if (typeof value !== "string") {
    fail(`${pointer}/${key}`, `must be a string, got ${describe(value)}`);
  }
  if (value.length === 0) {
    fail(`${pointer}/${key}`, "must be a non-empty string");
  }
  return value;
}

function requireInteger(
  obj: Record<string, unknown>,
  key: string,
  pointer: string,
): number {
  const value = obj[key];
  if (typeof value !== "number" || !Number.isInteger(value)) {
    fail(`${pointer}/${key}`, `must be an integer, got ${describe(value)}`);
  }
  return value;
}

/** An absolute `[x, y, z]` block position. */
function requirePos(
  obj: Record<string, unknown>,
  pointer: string,
): Vec3Tuple {
  const value = obj["pos"];
  if (!Array.isArray(value)) {
    fail(`${pointer}/pos`, `must be an array, got ${describe(value)}`);
  }
  if (value.length !== 3) {
    fail(`${pointer}/pos`, `must have exactly 3 elements, got ${value.length}`);
  }
  const coords = value.map((entry, i) => {
    const at = `${pointer}/pos/${i}`;
    if (typeof entry !== "number" || !Number.isFinite(entry)) {
      fail(at, `must be a finite number, got ${describe(entry)}`);
    }
    return entry;
  });
  return [coords[0]!, coords[1]!, coords[2]!];
}

/**
 * The optional `transport: [x, y, z]` marker (gap 8): the absolute destination a
 * step's completion teleports the player to. Returns a spreadable object — `{}`
 * when the field is absent (so a step with no transport is byte-identical to the
 * pre-gap-8 shape, with no `transport` key at all), or `{ transport: [x,y,z] }`
 * when present. Same coordinate shape as `pos`.
 */
function transportFields(
  obj: Record<string, unknown>,
  pointer: string,
): { transport?: Vec3Tuple } {
  const value = obj["transport"];
  if (value === undefined) {
    return {};
  }
  if (!Array.isArray(value)) {
    fail(`${pointer}/transport`, `must be an array, got ${describe(value)}`);
  }
  if (value.length !== 3) {
    fail(`${pointer}/transport`, `must have exactly 3 elements, got ${value.length}`);
  }
  const coords = value.map((entry, i) => {
    const at = `${pointer}/transport/${i}`;
    if (typeof entry !== "number" || !Number.isFinite(entry)) {
      fail(at, `must be a finite number, got ${describe(entry)}`);
    }
    return entry;
  });
  return { transport: [coords[0]!, coords[1]!, coords[2]!] };
}

/**
 * The v0.4 presentation/stealth markers (spec-0008 gap-7): optional `sneak: true`
 * and `cutscene_seconds: <positive int>`. Returns a spreadable object so an absent
 * marker leaves no key at all (`false` sneak is normalized to absent — treated as
 * the default). Same additive philosophy as `transportFields`.
 */
function presentationFields(
  obj: Record<string, unknown>,
  pointer: string,
): { sneak?: boolean; cutsceneSeconds?: number } {
  const out: { sneak?: boolean; cutsceneSeconds?: number } = {};
  const sneak = obj["sneak"];
  if (sneak !== undefined) {
    if (typeof sneak !== "boolean") {
      fail(`${pointer}/sneak`, `must be a boolean, got ${describe(sneak)}`);
    }
    if (sneak) out.sneak = true; // false ≡ absent (default)
  }
  const cutscene = obj["cutscene_seconds"];
  if (cutscene !== undefined) {
    if (typeof cutscene !== "number" || !Number.isInteger(cutscene) || cutscene <= 0) {
      fail(
        `${pointer}/cutscene_seconds`,
        `must be a positive integer, got ${describe(cutscene)}`,
      );
    }
    out.cutsceneSeconds = cutscene;
  }
  return out;
}

/**
 * The `objective` field an objective-bearing step carries (format 2): the
 * `obj/<kebab>` id whose anchored completion marker proves this step. Validated
 * against the DSL id syntax so a malformed id fails at parse time rather than as an
 * unexplained marker timeout mid-run — the harness only ever compares it, never
 * derives anything from it.
 */
function requireObjectiveId(
  obj: Record<string, unknown>,
  pointer: string,
): string {
  const value = requireString(obj, "objective", pointer);
  if (!/^obj\/[a-z0-9]+(?:-[a-z0-9]+)*$/.test(value)) {
    fail(
      `${pointer}/objective`,
      `must be an \`obj/<kebab>\` objective id, got ${JSON.stringify(value)}`,
    );
  }
  return value;
}

/** The `scoreboard: { objective, value }` object on assert-complete. */
function requireScoreboard(
  obj: Record<string, unknown>,
  pointer: string,
): { objective: string; value: number } {
  const board = requireObject(obj["scoreboard"], `${pointer}/scoreboard`);
  rejectUnknownKeys(board, ["objective", "value"], `${pointer}/scoreboard`);
  return {
    objective: requireString(board, "objective", `${pointer}/scoreboard`),
    value: requireInteger(board, "value", `${pointer}/scoreboard`),
  };
}

function rejectUnknownKeys(
  obj: Record<string, unknown>,
  allowed: readonly string[],
  pointer: string,
): void {
  for (const key of Object.keys(obj)) {
    if (!allowed.includes(key)) {
      fail(`${pointer}/${key}`, "is not a recognized field for this action");
    }
  }
}

function parseStep(value: unknown, pointer: string): Step {
  const obj = requireObject(value, pointer);
  const action = obj["action"];
  if (typeof action !== "string") {
    fail(`${pointer}/action`, `must be a string, got ${describe(action)}`);
  }
  switch (action) {
    case "select-class": {
      rejectUnknownKeys(obj, ["action", "class", "command"], pointer);
      return {
        action: "select-class",
        class: requireString(obj, "class", pointer),
        command: requireString(obj, "command", pointer),
      };
    }
    case "talk-to": {
      rejectUnknownKeys(
        obj,
        ["action", "objective", "npc", "pos", "command", "transport", "sneak", "cutscene_seconds"],
        pointer,
      );
      return {
        action: "talk-to",
        objective: requireObjectiveId(obj, pointer),
        npc: requireString(obj, "npc", pointer),
        pos: requirePos(obj, pointer),
        command: requireString(obj, "command", pointer),
        ...transportFields(obj, pointer),
        ...presentationFields(obj, pointer),
      };
    }
    case "reach": {
      rejectUnknownKeys(
        obj,
        ["action", "objective", "anchor", "pos", "radius", "transport", "sneak", "cutscene_seconds"],
        pointer,
      );
      const radius = obj["radius"];
      if (typeof radius !== "number" || !Number.isFinite(radius)) {
        fail(`${pointer}/radius`, `must be a finite number, got ${describe(radius)}`);
      }
      if (radius <= 0) {
        fail(`${pointer}/radius`, `must be positive, got ${radius}`);
      }
      return {
        action: "reach",
        objective: requireObjectiveId(obj, pointer),
        anchor: requireString(obj, "anchor", pointer),
        pos: requirePos(obj, pointer),
        radius,
        ...transportFields(obj, pointer),
        ...presentationFields(obj, pointer),
      };
    }
    case "kill": {
      rejectUnknownKeys(
        obj,
        ["action", "objective", "wave", "pos", "tag", "count", "transport", "sneak", "cutscene_seconds"],
        pointer,
      );
      return {
        action: "kill",
        objective: requireObjectiveId(obj, pointer),
        wave: requireString(obj, "wave", pointer),
        pos: requirePos(obj, pointer),
        tag: requireString(obj, "tag", pointer),
        count: requireInteger(obj, "count", pointer),
        ...transportFields(obj, pointer),
        ...presentationFields(obj, pointer),
      };
    }
    case "collect": {
      rejectUnknownKeys(
        obj,
        ["action", "objective", "item", "count", "pos", "transport", "sneak", "cutscene_seconds"],
        pointer,
      );
      return {
        action: "collect",
        objective: requireObjectiveId(obj, pointer),
        item: requireString(obj, "item", pointer),
        count: requireInteger(obj, "count", pointer),
        pos: requirePos(obj, pointer),
        ...transportFields(obj, pointer),
        ...presentationFields(obj, pointer),
      };
    }
    case "interact": {
      rejectUnknownKeys(
        obj,
        [
          "action",
          "objective",
          "anchor",
          "pos",
          "command",
          "requires_item",
          "transport",
          "sneak",
          "cutscene_seconds",
        ],
        pointer,
      );
      const ri = obj["requires_item"];
      if (ri !== null && typeof ri !== "string") {
        fail(`${pointer}/requires_item`, `must be a string or null, got ${describe(ri)}`);
      }
      return {
        action: "interact",
        objective: requireObjectiveId(obj, pointer),
        anchor: requireString(obj, "anchor", pointer),
        pos: requirePos(obj, pointer),
        command: requireString(obj, "command", pointer),
        requiresItem: ri,
        ...transportFields(obj, pointer),
        ...presentationFields(obj, pointer),
      };
    }
    case "rest": {
      rejectUnknownKeys(
        obj,
        ["action", "bonfire", "anchor", "pos", "command", "sneak", "cutscene_seconds"],
        pointer,
      );
      const bonfire = obj["bonfire"];
      if (!Number.isInteger(bonfire) || (bonfire as number) < 0) {
        fail(`${pointer}/bonfire`, `must be a non-negative integer, got ${describe(bonfire)}`);
      }
      return {
        action: "rest",
        bonfire: bonfire as number,
        anchor: requireString(obj, "anchor", pointer),
        pos: requirePos(obj, pointer),
        command: requireString(obj, "command", pointer),
        ...presentationFields(obj, pointer),
      };
    }
    case "assert-complete": {
      rejectUnknownKeys(obj, ["action", "scoreboard"], pointer);
      const board = requireScoreboard(obj, pointer);
      return {
        action: "assert-complete",
        objective: board.objective,
        value: board.value,
      };
    }
    default:
      fail(
        `${pointer}/action`,
        `must be one of ${STEP_ACTIONS.join(", ")}, got ${JSON.stringify(action)}`,
      );
  }
}

/**
 * Validate and normalize a parsed JSON value into a {@link CriticalPath}. Throws
 * {@link CriticalPathParseError} on any structural fault. Does NOT check step
 * ordering invariants — see `validateStepOrder` in `sequencer.ts`.
 */
export function parseCriticalPath(raw: unknown): CriticalPath {
  const root = requireObject(raw, "");
  rejectUnknownKeys(root, ["version", "format_version", "campaign_id", "steps"], "");

  // The contract version gates everything below: a path the harness cannot verify
  // is refused outright rather than run with weaker checks.
  const formatVersion = root["format_version"];
  if (formatVersion !== CRITICAL_PATH_FORMAT_VERSION) {
    fail(
      "/format_version",
      `must be ${CRITICAL_PATH_FORMAT_VERSION}, got ${describe(formatVersion)}` +
        `${formatVersion === undefined ? " (absent)" : ` (${JSON.stringify(formatVersion)})`}` +
        " — this critical path predates the per-objective completion oracle and " +
        "cannot be verified; rebuild the delve with a current delvec",
    );
  }

  const version = requireString(root, "version", "");
  if (!(SUPPORTED_DSL_VERSIONS as readonly string[]).includes(version)) {
    fail(
      "/version",
      `unsupported version ${JSON.stringify(version)}; harness supports ${SUPPORTED_DSL_VERSIONS.join(", ")}`,
    );
  }

  const campaignId = requireString(root, "campaign_id", "");

  const stepsValue = root["steps"];
  if (!Array.isArray(stepsValue)) {
    fail("/steps", `must be an array, got ${describe(stepsValue)}`);
  }
  if (stepsValue.length === 0) {
    fail("/steps", "must contain at least one step");
  }
  const steps = stepsValue.map((entry, i) => parseStep(entry, `/steps/${i}`));

  return { version, formatVersion: CRITICAL_PATH_FORMAT_VERSION, campaignId, steps };
}

/** Parse `critical-path.json` text: JSON.parse then structural validation. */
export function parseCriticalPathJson(text: string): CriticalPath {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch (cause) {
    const detail = cause instanceof Error ? cause.message : String(cause);
    throw new CriticalPathParseError("", `is not valid JSON: ${detail}`);
  }
  return parseCriticalPath(raw);
}

function describe(value: unknown): string {
  if (value === null) return "null";
  if (Array.isArray(value)) return "array";
  return typeof value;
}
