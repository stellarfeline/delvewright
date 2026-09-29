// Bot-death handling for the harness (spec-0008 gap-7 subset). A delve's safe
// route is supposed to be completable without dying; when the bot dies anyway the
// harness must fail FAST with a clear diagnostic — step number, position at death,
// likely cause — instead of the pre-v0.4 behaviour, where the bot respawned at
// world spawn and the next step pathfound across the void until a misleading
// 2x60s pathfinder timeout. This module is pure (no mineflayer import) so the
// diagnostic construction is unit-testable without a server.

/**
 * Absolute `[x, y, z]` world position at the moment of death — the body's own
 * coordinates, **not** a block cell.
 *
 * It used to be `Math.round`ed "for the message", and that rounding was read
 * downstream as a cell: the death-loop stage asked whether the rounded triple was
 * inside a lethal volume's declared block box. A body killed at `z = 4.6` — cell
 * 4, inside the box — rounds to `5`, and the stage reported that a real kill by
 * that very volume had happened OUTSIDE it and refused to credit it. Rounding is
 * not the cell a body is in (flooring is), and a value shaped for a sentence is
 * the wrong thing for a predicate to read.
 *
 * So the position is exact and every consumer says how it reads it: a cell
 * question floors, and the question "would this volume's selector have matched
 * this body" is {@link bodyInVolume}, which is the server's own rule.
 */
export type DeathPos = readonly [number, number, number];

/** A death position as a sentence reads it — two decimals, never a block cell. */
export function formatDeathPos(pos: DeathPos | undefined): string {
  return pos === undefined
    ? "an unknown position"
    : `[${pos.map((n) => n.toFixed(2)).join(", ")}]`;
}

/**
 * Raised (and surfaced through the step sequencer) when the bot dies mid-run. Carries
 * the death position and a best-effort cause line so the wrapping StepExecutionError
 * can report exactly where and why. Distinct type so the runner can exit with a
 * death-specific code rather than the generic navigation-failure code.
 */
export class BotDeathError extends Error {
  override readonly name = "BotDeathError";
  /** Bot position at the moment of death, if it could be read. */
  readonly position: DeathPos | undefined;
  /** The likely cause line lifted from recent chat (a death message), if any. */
  readonly likelyCause: string | undefined;

  constructor(position: DeathPos | undefined, likelyCause: string | undefined) {
    const where = ` at ${formatDeathPos(position)}`;
    const why = likelyCause ? ` — likely cause: ${likelyCause}` : " (cause not found in recent chat)";
    super(`bot died${where}${why}`);
    this.position = position;
    this.likelyCause = likelyCause;
  }
}

/** What a reader saw in one window of the chat stream, and what it missed. */
export interface ChatWindow {
  readonly lines: readonly string[];
  /** Lines that fell out of the ring before this reader got to them. */
  readonly lost: number;
}

/**
 * **Everything said since `mark`, read out of a BOUNDED ring.**
 *
 * The obvious spelling — mark `ring.length`, then `ring.slice(mark)` — is a
 * silent pass, and it is the reason this function exists. A bounded ring's
 * `length` stops growing the moment it is full: the mark is then the cap, the
 * slice is `[]` for the rest of the run, and a caller reading a command's reply
 * that way finds nothing however loudly the server answered. A `/say` this
 * harness sent, which the server logged and broadcast, was read as silence.
 *
 * So the mark is a position in the stream's own index space (`seen`, every line
 * ever observed) rather than in the ring, and eviction is REPORTED: a window that
 * lost lines did not observe nothing, it failed to observe, and only the caller
 * can say which of the two its verdict may rest on.
 */
export function linesSince(ring: readonly string[], seen: number, mark: number): ChatWindow {
  const since = Math.max(0, seen - mark);
  return {
    lines: ring.slice(Math.max(0, ring.length - since)),
    lost: Math.max(0, since - ring.length),
  };
}

/**
 * Pick the most likely death-cause line from a buffer of recent chat messages.
 * Minecraft broadcasts death messages that begin with the victim's name
 * (e.g. `delve-bot was slain by Zombie`, `delve-bot fell from a high place`),
 * so we return the most recent line that starts with the bot's username. Returns
 * `undefined` when nothing matches (unknown username, or no death message seen).
 */
export function likelyDeathCause(
  recentChat: readonly string[],
  username: string,
): string | undefined {
  if (username.length === 0) return undefined;
  const prefix = `${username} `;
  for (let i = recentChat.length - 1; i >= 0; i--) {
    const line = recentChat[i]!;
    if (line.startsWith(prefix)) return line;
  }
  return undefined;
}
