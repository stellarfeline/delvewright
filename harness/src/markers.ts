// The compiler's machine completion-marker channel — the harness's completion
// oracle (AUDIT-P0).
//
// Why this module exists. The harness used to observe completion by testing every
// incoming chat line for the *substring* `[Delvewright] complete <obj> <value>`,
// and only for the campaign objective. Two consequences, both observed live:
// nothing stopped authored (or LLM-translated) content from containing that
// substring, and a reach/interact/talk step could "pass" on arrival alone while its
// own objective never completed — a 22/22 green run whose campaign had in fact
// completed at step 12, the last ten steps hollow.
//
// The channel is now anchored and per-objective. One line, exactly:
//
//     [dw:complete <campaign-id> <token>]
//
// `<campaign-id>` is a bare kebab token, `<token>` is either `campaign` (the
// campaign-completion marker) or the completing objective's own `obj/<kebab>` id.
// Matching is whole-line and exact — never a substring of a longer line. Three
// independent properties make it unforgeable:
//   1. player chat reaches the client as `<name> …`, so no player utterance can
//      begin with the sigil;
//   2. the campaign id is part of the match, so a marker belonging to other content
//      cannot satisfy this campaign's step;
//   3. the compiler reserves the sigil in every player-visible string, authored or
//      translated (`DW0182`), so campaign content cannot contain one.
//
// Pure parsing — no game logic, no campaign knowledge (CLAUDE.md: the harness
// contains only assertions and navigation).

/** The token an anchored marker carries when the whole campaign completed. */
export const CAMPAIGN_TOKEN = "campaign";

/**
 * The exact wire grammar. Anchored at both ends: a marker is the ENTIRE chat line.
 * Ids are kebab (`[a-z0-9]+(-[a-z0-9]+)*`) on both sides, matching the DSL's id
 * syntax, so a lookalike with stray characters is rejected rather than accepted.
 */
const MARKER_RE =
  /^\[dw:complete ([a-z0-9]+(?:-[a-z0-9]+)*) (campaign|obj\/[a-z0-9]+(?:-[a-z0-9]+)*)\]$/;

/** A parsed completion marker: which campaign completed which thing. */
export interface CompletionMarker {
  /** The campaign that emitted it (must match the critical path's `campaign_id`). */
  readonly campaignId: string;
  /** `campaign`, or the completing objective's `obj/<id>`. */
  readonly token: string;
}

/**
 * Parse one chat line as a completion marker, or `undefined` if it is not exactly
 * one. Deliberately strict: no trimming, no substring search, no tolerance for
 * surrounding text. Anything a delve prints for a human — including a line that
 * merely mentions completion — parses as `undefined`.
 */
export function parseCompletionMarker(line: string): CompletionMarker | undefined {
  const match = MARKER_RE.exec(line);
  if (!match) return undefined;
  return { campaignId: match[1]!, token: match[2]! };
}

/**
 * Render a marker line. The harness never emits one to a server; this exists so
 * tests and diagnostics name the exact bytes the compiler is expected to produce
 * (a single source of truth for the format, mirroring `plan::marker_line`).
 */
export function markerLine(campaignId: string, token: string): string {
  return `[dw:complete ${campaignId} ${token}]`;
}

// ---------------------------------------------------------------------------
// The wave CENSUS channel (task #123)
// ---------------------------------------------------------------------------
//
// "What is standing at this encounter?" used to be answered by silhouette — every
// entity mineflayer tracked, no distance filter, anything taller than half a
// block. That counts a neighbouring wave's mobs and every ambush actor in
// tracking range as members of the wave being measured, and — since they are
// alive on both sides of a scripted death — reports them as survivors a re-seat
// failed to remove (#230, the drowned bell's false `carried_over` findings).
//
// The wave TAG is the only exact answer, and only the server can see it. So the
// compiler emits the census and the harness reads it: same sigil, same anchored
// whole-line matching, same three unforgeability properties as the completion
// channel. Two lines, both integers only:
//
//     [dw:census <campaign-id> <wave-id> <seq> <present> <branded> <damaged>]
//     [dw:censusmob <campaign-id> <wave-id> <seq> <x> <y> <z> <health> <max>]
//
// `seq` counts censuses server-side, so an answer can always be told from a stale
// one without the harness writing any delve state to ask its question. The
// `censusmob` fields are ×100 fixed-point, so positions and health cross chat as
// exact integers with no float formatting to parse.

/** A signed integer field as it appears on the wire. */
const INT = "(-?[0-9]+)";
const WAVE = "(wave\\/[a-z0-9]+(?:-[a-z0-9]+)*)";
const CAMPAIGN = "([a-z0-9]+(?:-[a-z0-9]+)*)";

const CENSUS_RE = new RegExp(
  `^\\[dw:census ${CAMPAIGN} ${WAVE} ${INT} ${INT} ${INT} ${INT}\\]$`,
);
const CENSUS_MOB_RE = new RegExp(
  `^\\[dw:censusmob ${CAMPAIGN} ${WAVE} ${INT} ${INT} ${INT} ${INT} ${INT} ${INT}\\]$`,
);

/** The summary line closing one census of one wave. */
export interface CensusSummary {
  readonly campaignId: string;
  readonly wave: string;
  /** Server-side census counter; strictly increasing per census. */
  readonly seq: number;
  /** How many mobs carrying this wave's tag are standing. */
  readonly present: number;
  /** How many of those still wear the brand applied before the last death. */
  readonly branded: number;
  /** How many of those are below their own `max_health`. */
  readonly damaged: number;
}

/** One mob's line inside a census. Positions and health are real units. */
export interface CensusMob {
  readonly campaignId: string;
  readonly wave: string;
  readonly seq: number;
  readonly pos: readonly [number, number, number];
  readonly health: number;
  readonly maxHealth: number;
}

/** Parse one chat line as a census summary, or `undefined`. Whole-line, strict. */
export function parseCensusSummary(line: string): CensusSummary | undefined {
  const m = CENSUS_RE.exec(line);
  if (!m) return undefined;
  return {
    campaignId: m[1]!,
    wave: m[2]!,
    seq: Number(m[3]),
    present: Number(m[4]),
    branded: Number(m[5]),
    damaged: Number(m[6]),
  };
}

/** Parse one chat line as a census mob line, or `undefined`. Whole-line, strict. */
export function parseCensusMob(line: string): CensusMob | undefined {
  const m = CENSUS_MOB_RE.exec(line);
  if (!m) return undefined;
  return {
    campaignId: m[1]!,
    wave: m[2]!,
    seq: Number(m[3]),
    pos: [Number(m[4]) / 100, Number(m[5]) / 100, Number(m[6]) / 100],
    health: Number(m[7]) / 100,
    maxHealth: Number(m[8]) / 100,
  };
}
