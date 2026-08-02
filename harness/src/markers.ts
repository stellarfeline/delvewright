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
