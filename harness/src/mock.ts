// spec-0108 §6.5: an act the bot cannot drive, performed by the engine's own
// function for its bundle — the pure half: the command, and the verdict.

import type { MockStep } from "./critical-path.ts";
import { termText, type GateTerm } from "./death-loop.ts";
import { isRejection } from "./rejection.ts";

/** What one mock step did, for the run log and the run report. */
export interface MockResult {
  readonly act: string;
  readonly kind: string;
  readonly function: string;
  readonly passed: boolean;
  /** Why it failed, or what it asserted. */
  readonly detail: string;
}

/**
 * The command a mock sends: the act's own function, run with the bot as `@s`
 * and at its position — the context `execute as @a[…] run function …` gives the
 * bundle when a player performs the act.
 */
export function mockCommand(step: MockStep): string {
  return `/execute as @s at @s run function ${step.function}`;
}

/**
 * The verdict on one mock: every `before` term held, the command was answered
 * and not refused (the shared rejection rule, `rejection.ts`), and every
 * `after` term holds. `undefined` for a term means the server answered neither
 * way, which establishes nothing and fails the step.
 */
export function mockVerdict(
  step: MockStep,
  before: readonly (boolean | undefined)[],
  reply: { readonly answered: boolean; readonly lines: readonly string[] },
  after: readonly (boolean | undefined)[],
): MockResult {
  const base = { act: step.act, kind: step.kind, function: step.function };
  const failed = (terms: readonly GateTerm[], read: readonly (boolean | undefined)[]): string[] =>
    terms.flatMap((t, i) =>
      read[i] === true ? [] : [`${termText(t)} ${read[i] === undefined ? "unread" : "does not hold"}`],
    );
  const pre = failed(step.before, before);
  if (pre.length > 0) {
    return { ...base, passed: false, detail: `before the act: ${pre.join("; ")}` };
  }
  if (!reply.answered) {
    return {
      ...base,
      passed: false,
      detail: `the server never closed the reply to \`${mockCommand(step)}\` — it was not observed`,
    };
  }
  const refused = reply.lines.find((l) => isRejection(l));
  if (refused !== undefined) {
    return { ...base, passed: false, detail: `the server refused \`${mockCommand(step)}\`: ${refused}` };
  }
  const post = failed(step.after, after);
  if (post.length > 0) {
    return { ...base, passed: false, detail: `after the act: ${post.join("; ")}` };
  }
  return {
    ...base,
    passed: true,
    detail: `${step.after.length} term(s) changed: ${step.after.map(termText).join("; ")}`,
  };
}
