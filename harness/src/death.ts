// Bot-death handling for the harness (spec-0008 gap-7 subset). A delve's safe
// route is supposed to be completable without dying; when the bot dies anyway the
// harness must fail FAST with a clear diagnostic — step number, position at death,
// likely cause — instead of the pre-v0.4 behaviour, where the bot respawned at
// world spawn and the next step pathfound across the void until a misleading
// 2x60s pathfinder timeout. This module is pure (no mineflayer import) so the
// diagnostic construction is unit-testable without a server.

/** Absolute `[x, y, z]` block position, rounded to whole blocks for the message. */
export type DeathPos = readonly [number, number, number];

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
    const where = position ? ` at [${position.join(", ")}]` : " at an unknown position";
    const why = likelyCause ? ` — likely cause: ${likelyCause}` : " (cause not found in recent chat)";
    super(`bot died${where}${why}`);
    this.position = position;
    this.likelyCause = likelyCause;
  }
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
