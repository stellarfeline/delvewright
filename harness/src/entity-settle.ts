// Entity-tracker settle race (2026-08-06 island triage). mineflayer's
// `bot.entities` starts EMPTY at spawn, and world-persisted entities' packets can
// arrive 2-4s late — after the join packet, not with it. The island bot logs in
// already within TALK_RANGE of its first talk-to target, so `walkTo` returns
// immediately with no walk to cover the gap, and the FIRST crosshair assertion
// (executor.ts's `requireCrosshair`) fired at ~t+0.5s against a tracker nobody
// had told about a single entity yet — printing "no `interaction` body tracked
// within 3 blocks", a diagnostic that is honest about ITS OWN data (see
// requireCrosshair's doc) but, to a reader, indistinguishable from a real
// acquisition failure. It cost a triage cycle: the entities were verified
// present at distance 0.00 once the tracker populated at t+4s.
//
// The fix is a bounded settle wait, once at spawn, before anything reads
// `bot.entities` for the first time: wait until the non-player entity count has
// been STABLE across several consecutive polls — not merely non-zero, since a
// stream of late-arriving spawn packets can look non-empty while still growing —
// with a hard timeout after which the run proceeds regardless and the existing
// "not tracked" warning still applies honestly to whatever the tracker actually
// holds.
//
// This module is pure (no mineflayer import) so the settle predicate is
// unit-testable without a server; the loop that actually polls `bot.entities`
// and sleeps between polls lives in executor.ts (`awaitEntitySettle`).

/** One poll's reading: how many non-player entities the tracker currently holds. */
export type EntityCount = number;

export interface SettlePolicy {
  /** Consecutive equal, non-zero counts required before calling it settled. */
  readonly stablePolls: number;
}

/** Three consecutive equal polls — enough to tell "the last packet just landed"
 * from "packets are still arriving one at a time". */
export const DEFAULT_SETTLE_POLICY: SettlePolicy = { stablePolls: 3 };

/**
 * Has the tracker settled? `history` is every non-player entity count observed
 * so far (oldest first), including the one just polled.
 *
 * Settled means: at least one non-player entity is tracked, and the count has
 * held for `stablePolls` consecutive polls in a row. An all-zero history never
 * settles on its own — a build that legitimately spawns nothing near the bot
 * must not hang forever behind this predicate, which is why the caller pairs it
 * with its own hard timeout rather than relying on this function to end the
 * wait.
 */
export function hasSettled(
  history: readonly EntityCount[],
  policy: SettlePolicy = DEFAULT_SETTLE_POLICY,
): boolean {
  if (history.length < policy.stablePolls) return false;
  const window = history.slice(-policy.stablePolls);
  const last = window[window.length - 1]!;
  return last > 0 && window.every((c) => c === last);
}
