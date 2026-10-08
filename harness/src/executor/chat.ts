// MineflayerExecutor: the chat window: marks, reads since a mark, a command's refusal.

import { linesSince, type ChatWindow } from "../death.ts";
import { isRejection } from "../rejection.ts";
import { echoCommand } from "../command-reply.ts";
import { delay } from "./connection.ts";
import type { MineflayerExecutor } from "../executor.ts";

/**
 * How long to wait for the server's answer to a staged blow before reading the
 * chat buffer for a refusal. One round trip on a local server; the check is over
 * a buffered window, not a single line, so a slow reply is still seen.
 */
export const STAGED_REPLY_MS = 400;

/**
 * How long a bracketed command's closing marker may take before its reply is
 * reported as never observed. See `command-reply.ts`.
 */
const BRACKET_TIMEOUT_MS = 5_000;

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /** A point in the chat stream, to read forward from. See {@link chatSeen}. */
  chatMark(this: MineflayerExecutor): number {
    return this.chatSeen;
  },

  /**
   * Every chat line seen since `mark`, and how many the ring dropped before this
   * reader got to them. A non-zero `lost` is part of the answer: it says the
   * window was not fully observed, which is different from observing nothing in
   * it.
   */
  chatSince(this: MineflayerExecutor, mark: number): ChatWindow {
    return linesSince(this.recentChat, this.chatSeen, mark);
  },

  /**
   * **Send `command` and return the server's refusal of it, or `undefined`.**
   *
   * The command is bracketed (`command-reply.ts`), so the refusal read is this
   * command's and no other's, whatever else is in flight. It waits at least
   * `settleMs` — the pace the callers were written to — and at most until the
   * closing marker. A reply that never closed was not observed, and is returned as
   * a refusal saying so: silence is never read as consent.
   */
  async refusalOf(this: MineflayerExecutor, command: string, settleMs = STAGED_REPLY_MS): Promise<string | undefined> {
    const bot = this.requireBot();
    const bracket = this.brackets.begin();
    // One synchronous turn: nothing else can reach the socket between these.
    bot.chat(echoCommand(bracket.open));
    bot.chat(command);
    bot.chat(echoCommand(bracket.close));
    const [reply] = await Promise.all([
      this.brackets.reply(bracket, BRACKET_TIMEOUT_MS),
      delay(settleMs),
    ]);
    if (!reply.answered) {
      return (
        `no reply observed: the closing marker of \`${command}\` did not arrive within ` +
        `${BRACKET_TIMEOUT_MS}ms`
      );
    }
    return reply.lines.find((line) => isRejection(line));
  },
};
