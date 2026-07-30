// Parser and strict runtime validation for `critical-path.json`, the compiler's
// bot-walkthrough contract output (spec-0002). The harness contains zero
// campaign-specific logic: it only interprets the closed set of step actions
// defined by the spec. Validation is hand-rolled (no external schema library) so
// error messages can point precisely at the offending JSON location.

/** The critical-path format is versioned with the DSL (spec-0002). */
export const SUPPORTED_DSL_VERSION = "0.1.0";

/** The closed set of critical-path step actions (spec-0002 / spec-0001 enum). */
export const STEP_ACTIONS = [
  "select-class",
  "talk-to",
  "reach",
  "assert-complete",
] as const;

export type StepAction = (typeof STEP_ACTIONS)[number];

/** Absolute block position `[x, y, z]`, resolved by the compiler after placement. */
export type Vec3Tuple = readonly [number, number, number];

/** Select a class via the class-selection dialog; `optionPath` = dialog button indices. */
export interface SelectClassStep {
  readonly action: "select-class";
  readonly class: string;
  readonly optionPath: readonly number[];
}

/** Talk to an NPC at `pos`, following `optionPath` through its dialogue tree. */
export interface TalkToStep {
  readonly action: "talk-to";
  readonly npc: string;
  readonly pos: Vec3Tuple;
  readonly optionPath: readonly number[];
}

/** Reach an anchor: get within `radius` blocks of the absolute position `pos`. */
export interface ReachStep {
  readonly action: "reach";
  readonly anchor: string;
  readonly pos: Vec3Tuple;
  readonly radius: number;
}

/** Assert the campaign-completion scoreboard objective holds `value` (terminal step). */
export interface AssertCompleteStep {
  readonly action: "assert-complete";
  readonly objectiveScoreboard: string;
  readonly value: number;
}

export type Step =
  | SelectClassStep
  | TalkToStep
  | ReachStep
  | AssertCompleteStep;

export interface CriticalPath {
  readonly version: string;
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

/** A non-negative integer array (dialog button indices). */
function requireOptionPath(
  obj: Record<string, unknown>,
  pointer: string,
): readonly number[] {
  const value = obj["option_path"];
  if (!Array.isArray(value)) {
    fail(`${pointer}/option_path`, `must be an array, got ${describe(value)}`);
  }
  return value.map((entry, i) => {
    const at = `${pointer}/option_path/${i}`;
    if (typeof entry !== "number" || !Number.isInteger(entry)) {
      fail(at, `must be an integer index, got ${describe(entry)}`);
    }
    if (entry < 0) {
      fail(at, `must be a non-negative index, got ${entry}`);
    }
    return entry;
  });
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
      rejectUnknownKeys(obj, ["action", "class", "option_path"], pointer);
      return {
        action: "select-class",
        class: requireString(obj, "class", pointer),
        optionPath: requireOptionPath(obj, pointer),
      };
    }
    case "talk-to": {
      rejectUnknownKeys(obj, ["action", "npc", "pos", "option_path"], pointer);
      return {
        action: "talk-to",
        npc: requireString(obj, "npc", pointer),
        pos: requirePos(obj, pointer),
        optionPath: requireOptionPath(obj, pointer),
      };
    }
    case "reach": {
      rejectUnknownKeys(obj, ["action", "anchor", "pos", "radius"], pointer);
      const radius = obj["radius"];
      if (typeof radius !== "number" || !Number.isFinite(radius)) {
        fail(`${pointer}/radius`, `must be a finite number, got ${describe(radius)}`);
      }
      if (radius <= 0) {
        fail(`${pointer}/radius`, `must be positive, got ${radius}`);
      }
      return {
        action: "reach",
        anchor: requireString(obj, "anchor", pointer),
        pos: requirePos(obj, pointer),
        radius,
      };
    }
    case "assert-complete": {
      rejectUnknownKeys(obj, ["action", "objective_scoreboard", "value"], pointer);
      return {
        action: "assert-complete",
        objectiveScoreboard: requireString(obj, "objective_scoreboard", pointer),
        value: requireInteger(obj, "value", pointer),
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
  rejectUnknownKeys(root, ["version", "campaign_id", "steps"], "");

  const version = requireString(root, "version", "");
  if (version !== SUPPORTED_DSL_VERSION) {
    fail(
      "/version",
      `unsupported version ${JSON.stringify(version)}; harness supports ${SUPPORTED_DSL_VERSION}`,
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

  return { version, campaignId, steps };
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
