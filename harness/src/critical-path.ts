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
 * run at all, branch tier or not. v0.9 (spec-0026) adds the stage-1
 * horizon-library surface (`horizon` object form, new base/shorthand names) —
 * world-generation input the compiler consumes to build the map, not a change
 * to the step contract, so a v0.9 path is walked exactly as a v0.8 one is.
 * v0.10 (spec-0031) adds three things, none of which touches the step
 * contract: **runtime state** — a declared integer datum, the
 * `set-state`/`add-state`/`clear-state` verbs and the `requires_state`
 * comparison every gate carries, all of it server-side scoreboard state the
 * datapack drives — the campaign-wide `on_death` effect root, effects that
 * run at the moment a player dies, and **lethal volumes**, which change what
 * the WORLD does to a body that enters a declared box. None exports a new step
 * or reorders one (a death beat is a reaction to something the bot may never
 * do, and the compiler proves no forced leg crosses a volume — `DW0510`), so a
 * v0.10 path is walked exactly as a v0.9 one is.
 *
 * **v0.11** adds two surfaces, and neither reaches the bot. The per-body
 * `traversal` declaration (spec-0034) — what a body can do when it moves — is a
 * compile-time claim about `move-npc`/`move-actor` puppets, proven by `delvec`
 * before anything ships (`DW0454`). The press-answer lift — a `narrate`
 * `actionbar` style and a trigger's `audience: presser` — is a reply to a
 * right-click on a sealed body, a gesture no critical path makes. Both export no
 * step, reorder none, and change nothing the bot walks: a v0.11 path is walked
 * exactly as a v0.10 one is.
 *
 * **v0.13** adds two campaign documents and no step: a geometry brief, which is
 * the whole map's written brief reduced to named numbers, and a layout graph,
 * which states the campaign's space as places and the connections between them
 * before any coordinate exists. Both are compile-time claims the compiler proves
 * on its own; neither exports a step, reorders one, or changes anything the bot
 * walks, so a v0.13 path is walked exactly as a v0.12 one is.
 *
 * **v0.14** adds one document and no step: a site plan, the geometric embedding
 * of that graph — the region, a box per place, a seam per connection on a face
 * the two boxes share, and the comparisons that hold all of it to the brief's
 * numbers. Like its two siblings it is a compile-time claim the compiler proves
 * on its own; it exports no step, reorders none, and changes nothing the bot
 * walks, so a v0.14 path is walked exactly as a v0.13 one is.
 *
 * **v0.15** adds one document and no step: a detail plan, which piece stands in
 * which of the plan's places. It carries no coordinate — the frame is computed
 * from the site plan's own box — and it exports no step, reorders none, and
 * changes nothing the bot walks, so a v0.15 path is walked exactly as a v0.14
 * one is. What it changes is what the bot walks THROUGH: a detailed place is a
 * building rather than a shell, and the critical path across it is the same
 * path, which is the property stage 6 exists to preserve.
 *
 * This allowlist must never trail the compiler's own `SUPPORTED_DSL_VERSION`
 * ceiling (`crates/dsl/src/envelope.rs`) — `tools/check-harness-dsl-version.py`
 * enforces that in CI: an allowlist that lags the ceiling refuses a v0.9.0
 * campaign at this gate after the server booted and the bot connected.
 */
export const SUPPORTED_DSL_VERSIONS = [
  "0.2.0",
  "0.3.0",
  "0.4.0",
  "0.5.0",
  "0.6.0",
  "0.7.0",
  "0.8.0",
  "0.9.0",
  "0.10.0",
  "0.11.0",
  "0.12.0",
  "0.13.0",
  "0.14.0",
  "0.15.0",
  "0.16.0",
  "0.17.0",
  "0.18.0",
  "0.19.0",
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
 * * `3` — a `reach` step carries `completion`, the volume the datapack actually
 *   adjudicates in. The walk goal is derived from that and no longer from the
 *   authored `radius`, which the datapack had stopped reading at DSL v0.3 without
 *   telling anyone. Required in both directions, which is what makes it a format
 *   change rather than an addition: a format-2 artifact cannot tell a current bot
 *   where the objective completes, and this bot refuses the field's absence rather
 *   than falling back to a completion model of its own — that fallback is the
 *   defect.
 * * `4` — the path carries `non_combatants`, the delve's own statement of which
 *   entity kinds are never a combat target. The bot classifies a body by the name
 *   its client reports, because mineflayer on 1.21.11 cannot read entity tags —
 *   and "a mannequin is an NPC" is not a fact about Minecraft, it is a fact about
 *   what the compiler summons an NPC as. Required in both directions for the same
 *   reason as `3`: the fallback — a literal set of entity names living in the
 *   harness — IS the defect, because it is right only for the campaigns whose
 *   author happened to pick those bodies.
 *
 * Rebuild the delve with a current `delvec` to produce a supported path.
 */
export const CRITICAL_PATH_FORMAT_VERSION = 4;

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

/**
 * The volume the DATAPACK completes a reach objective in, exported by the
 * compiler rather than re-derived here.
 *
 * The harness had a model of this, and it was wrong. Its comment said the server
 * ran a `distance=..radius` check on the anchor point, so it aimed one block
 * tighter than `radius`; from DSL v0.3 the server had in fact been testing a
 * fixed ±1 cube and ignoring `radius` altogether. On a `radius: 3` reach the bot
 * was therefore entitled to stop three blocks out — outside the cube — and then
 * wait forever on a completion that could not arrive. Nothing was red, because a
 * `GoalNear` usually overshoots inward; the failure was intermittent, which is
 * the worst way for it to be wrong.
 *
 * The compiler now emits the region and its adjudicating line from one value, so
 * "the harness never contains game logic, only assertions and navigation" is
 * true here rather than aspirational: the completion rule is the server's, this
 * file navigates into it, and {@link Executor.reach} asserts arrival in it.
 */
export type ReachCompletion =
  /** Pre-v0.3 emission: `distance=..radius` about the anchor's block corner. */
  | { readonly kind: "sphere"; readonly pos: Vec3Tuple; readonly radius: number }
  /** v0.3+: an axis-aligned block region, inclusive corners. */
  | { readonly kind: "cube"; readonly lo: Vec3Tuple; readonly hi: Vec3Tuple };

/** Reach an anchor: walk into the objective's exported completion volume. */
export interface ReachStep extends PresentationMarkers {
  readonly action: "reach";
  /** The `obj/<id>` this step must prove complete (format 2). */
  readonly objective: string;
  readonly anchor: string;
  readonly pos: Vec3Tuple;
  /** The AUTHORED radius — reported, never used to decide where to stop. */
  readonly radius: number;
  /** What the server actually adjudicates. See {@link ReachCompletion}. */
  readonly completion: ReachCompletion;
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

/**
 * Collect `count` of `item` from a chest at `pos` (v0.3) — or, when
 * `droppedBy` is set, off the ground the named wave died on (v0.9).
 */
export interface CollectStep extends PresentationMarkers {
  readonly action: "collect";
  /** The `obj/<id>` this step must prove complete (format 2). */
  readonly objective: string;
  readonly item: string;
  readonly count: number;
  readonly pos: Vec3Tuple;
  /**
   * The wave whose declared drop provides the item (v0.9). Present means there
   * is NO container at `pos`: the compiler places none, because the item only
   * exists once the fight is over. The bot walks the ground instead of opening
   * a block that is not there.
   */
  readonly droppedBy?: string;
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
 * Rest at bonfire `bonfire` (spec-0016 §1). A path EXPORT step: it
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
  /**
   * Scheduled-ending tail: the compiler-computed maximum tick offset
   * between the terminal objective completing and `campaign-complete` firing —
   * a `sequence`-scheduled finale (the-wake: 250t) or a `move-npc`/`move-actor`
   * arrival bundle. The completion window must cover it: the window becomes
   * `max(default settle, tail + margin)`. Absent (undefined) for a synchronous
   * ending, which keeps paths that predate the field byte-identical.
   */
  readonly endingTailTicks?: number;
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

/**
 * One entity kind the campaign stages as an NPC *and* as something the party
 * fights, with the compiler's account of why it could not be excluded.
 *
 * The bot may swing at these. That is the compiler's ruling, not the harness's:
 * excluding the kind would make the fight it belongs to unwinnable, and a delve
 * that cannot be finished is a worse outcome than a quest-giver taking a hit.
 * The run prints them, so nobody discovers it from a corpse.
 */
export interface AmbiguousBody {
  readonly kind: string;
  readonly why: string;
}

/**
 * The delve's cast statement: which entity kinds are never a combat target.
 *
 * Stated in the client's vocabulary (`mannequin`, `villager`) because that is the
 * only identity a mineflayer bot can read off a body. Compiler-derived from the
 * emitter's own NPC rule; see `combat::non_combatants`.
 */
export interface NonCombatants {
  /** Kinds no body of which is ever a fight. */
  readonly kinds: ReadonlySet<string>;
  /** Kinds that are an NPC body AND a fightable body — excluded, and named. */
  readonly ambiguous: readonly AmbiguousBody[];
  /** How many NPC bodies the compiler examined — the binding count. */
  readonly examined: number;
  /** `examined === 0`. */
  readonly unbound: boolean;
  /** Present exactly when `unbound` — the compiler's own words for the zero. */
  readonly reason?: string;
}

export interface CriticalPath {
  readonly version: string;
  /** Contract version; see {@link CRITICAL_PATH_FORMAT_VERSION}. */
  readonly formatVersion: number;
  readonly campaignId: string;
  /** Who the bot may never swing at. Required — see the format-4 note. */
  readonly nonCombatants: NonCombatants;
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

/** A named `[x, y, z]` field, same coordinate shape as `pos`. */
function requireVec3(
  obj: Record<string, unknown>,
  key: string,
  pointer: string,
): Vec3Tuple {
  const value = obj[key];
  if (!Array.isArray(value)) {
    fail(`${pointer}/${key}`, `must be an array, got ${describe(value)}`);
  }
  if (value.length !== 3) {
    fail(`${pointer}/${key}`, `must have exactly 3 elements, got ${value.length}`);
  }
  const coords = value.map((entry, i) => {
    const at = `${pointer}/${key}/${i}`;
    if (typeof entry !== "number" || !Number.isFinite(entry)) {
      fail(at, `must be a finite number, got ${describe(entry)}`);
    }
    return entry;
  });
  return [coords[0]!, coords[1]!, coords[2]!];
}

/**
 * The required `completion` volume on a reach step.
 *
 * Required, deliberately: an optional field with a fallback would be the harness
 * keeping its own completion model alive under a nicer name, and that model is
 * exactly what was wrong. If the artifact does not say what the server
 * adjudicates, this bot has no business guessing.
 */
function requireCompletion(
  obj: Record<string, unknown>,
  pointer: string,
): ReachCompletion {
  const at = `${pointer}/completion`;
  const value = obj["completion"];
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    fail(at, `must be an object, got ${describe(value)}`);
  }
  const c = value as Record<string, unknown>;
  const kind = c["kind"];
  if (kind === "cube") {
    rejectUnknownKeys(c, ["kind", "lo", "hi"], at);
    return { kind: "cube", lo: requireVec3(c, "lo", at), hi: requireVec3(c, "hi", at) };
  }
  if (kind === "sphere") {
    rejectUnknownKeys(c, ["kind", "pos", "radius"], at);
    const radius = c["radius"];
    if (typeof radius !== "number" || !Number.isFinite(radius) || radius <= 0) {
      fail(`${at}/radius`, `must be a positive finite number, got ${describe(radius)}`);
    }
    return { kind: "sphere", pos: requireVec3(c, "pos", at), radius };
  }
  fail(`${at}/kind`, `must be "cube" or "sphere", got ${describe(kind)}`);
}

/**
 * Whether `p` (a precise entity position) lies inside `c`.
 *
 * Block-region semantics, matching the `@s[x=..,dx=..]` the server runs: the
 * region a `cube` names spans `lo` to `hi + 1` in continuous coordinates, because
 * `x=lo,dx=hi-lo` covers whole blocks `lo..=hi`. The sphere form measures from the
 * anchor's block corner, which is what `distance=..r` does.
 */
export function insideCompletion(p: Vec3Tuple, c: ReachCompletion): boolean {
  if (c.kind === "cube") {
    return [0, 1, 2].every((i) => p[i]! >= c.lo[i]! && p[i]! <= c.hi[i]! + 1);
  }
  const d = Math.hypot(p[0]! - c.pos[0]!, p[1]! - c.pos[1]!, p[2]! - c.pos[2]!);
  return d <= c.radius;
}

/**
 * The walk goal for a reach step: aim at the middle of the completion volume,
 * with a range that cannot put the bot outside it.
 *
 * `GoalNear` is block-granular and the server's check is not, so the goal is one
 * block tighter than the volume's own half-extent — landing well inside rather
 * than on the boundary. Derived from the SERVER's volume, never from the authored
 * radius: those were the same number until v0.3 and have not been since.
 */
export function reachGoal(c: ReachCompletion): { pos: Vec3Tuple; range: number } {
  if (c.kind === "cube") {
    const mid: Vec3Tuple = [
      Math.floor((c.lo[0]! + c.hi[0]!) / 2),
      Math.floor((c.lo[1]! + c.hi[1]!) / 2),
      Math.floor((c.lo[2]! + c.hi[2]!) / 2),
    ];
    const half = Math.min(
      c.hi[0]! - mid[0]!,
      c.hi[1]! - mid[1]!,
      c.hi[2]! - mid[2]!,
    );
    return { pos: mid, range: Math.max(1, half - 1) };
  }
  return { pos: c.pos, range: Math.max(1, c.radius - 1) };
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
        [
          "action",
          "objective",
          "anchor",
          "pos",
          "radius",
          "completion",
          "transport",
          "sneak",
          "cutscene_seconds",
        ],
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
        completion: requireCompletion(obj, pointer),
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
        [
          "action",
          "objective",
          "item",
          "count",
          "pos",
          "dropped_by",
          "transport",
          "sneak",
          "cutscene_seconds",
        ],
        pointer,
      );
      const droppedBy = obj["dropped_by"];
      if (droppedBy !== undefined && typeof droppedBy !== "string") {
        fail(`${pointer}/dropped_by`, `must be a string, got ${describe(droppedBy)}`);
      }
      return {
        action: "collect",
        objective: requireObjectiveId(obj, pointer),
        item: requireString(obj, "item", pointer),
        count: requireInteger(obj, "count", pointer),
        pos: requirePos(obj, pointer),
        ...(droppedBy === undefined ? {} : { droppedBy }),
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
      rejectUnknownKeys(obj, ["action", "scoreboard", "ending_tail_ticks"], pointer);
      const board = requireScoreboard(obj, pointer);
      const tail = obj["ending_tail_ticks"];
      if (tail !== undefined) {
        if (typeof tail !== "number" || !Number.isInteger(tail) || tail <= 0) {
          fail(
            `${pointer}/ending_tail_ticks`,
            `must be a positive integer, got ${describe(tail)}`,
          );
        }
      }
      return {
        action: "assert-complete",
        objective: board.objective,
        value: board.value,
        ...(tail !== undefined ? { endingTailTicks: tail as number } : {}),
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
  rejectUnknownKeys(
    root,
    ["version", "format_version", "campaign_id", "non_combatants", "steps"],
    "",
  );

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

  const nonCombatants = parseNonCombatants(root["non_combatants"], "/non_combatants");

  return {
    version,
    formatVersion: CRITICAL_PATH_FORMAT_VERSION,
    campaignId,
    nonCombatants,
    steps,
  };
}

/**
 * The cast statement, or a refusal.
 *
 * There is deliberately no default. A path that does not say who is off-limits
 * cannot be walked safely, and the only fallback available — a set of entity
 * names written into this file — is the thing this field exists to delete.
 */
function parseNonCombatants(value: unknown, pointer: string): NonCombatants {
  const obj = requireObject(value, pointer);
  rejectUnknownKeys(obj, ["kinds", "ambiguous", "examined", "unbound", "reason"], pointer);
  const rawKinds = obj["kinds"];
  if (!Array.isArray(rawKinds)) {
    fail(`${pointer}/kinds`, `must be an array, got ${describe(rawKinds)}`);
  }
  const kinds = new Set<string>();
  rawKinds.forEach((k, i) => {
    if (typeof k !== "string" || k.length === 0) {
      fail(`${pointer}/kinds/${i}`, `must be a non-empty string, got ${describe(k)}`);
    }
    if (k.includes(":")) {
      // The compiler states the client's vocabulary, namespace stripped. A
      // namespaced id here would never match a body and the exclusion would
      // silently bind to nothing.
      fail(`${pointer}/kinds/${i}`, `must be a client entity name, not a namespaced id: ${k}`);
    }
    kinds.add(k);
  });
  const rawAmbiguous = obj["ambiguous"];
  if (!Array.isArray(rawAmbiguous)) {
    fail(`${pointer}/ambiguous`, `must be an array, got ${describe(rawAmbiguous)}`);
  }
  const ambiguous = rawAmbiguous.map((a, i) => {
    const entry = requireObject(a, `${pointer}/ambiguous/${i}`);
    return {
      kind: requireString(entry, "kind", `${pointer}/ambiguous/${i}`),
      why: requireString(entry, "why", `${pointer}/ambiguous/${i}`),
    };
  });
  const examined = requireInteger(obj, "examined", pointer);
  if (examined < 0) fail(`${pointer}/examined`, "must be a non-negative integer");
  const unbound = obj["unbound"];
  if (typeof unbound !== "boolean") {
    fail(`${pointer}/unbound`, `must be a boolean, got ${describe(unbound)}`);
  }
  if (unbound !== (examined === 0)) {
    fail(pointer, "`unbound` must be exactly `examined === 0`");
  }
  const reason = obj["reason"];
  if (unbound && (typeof reason !== "string" || reason.length === 0)) {
    fail(`${pointer}/reason`, "a census that examined nothing must state why");
  }
  if (!unbound && reason !== undefined) {
    fail(`${pointer}/reason`, "a bound census carries no reason to explain");
  }
  return {
    kinds,
    ambiguous,
    examined,
    unbound,
    ...(typeof reason === "string" ? { reason } : {}),
  };
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
