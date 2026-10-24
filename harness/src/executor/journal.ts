// MineflayerExecutor: the journal at every quest start — read off the book the bot holds.

import {
  JOURNAL_PAGES_COMMAND,
  JOURNAL_POLL_MS,
  JOURNAL_SETTLE_MS,
  pagesShow,
  scoreIsOne,
  scoreIsOneCommand,
  type JournalLedger,
} from "../journal.ts";
import { delay } from "./connection.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /** Adopt the build's journal ledger; from here every step boundary reads it. */
  useJournalLedger(this: MineflayerExecutor, ledger: JournalLedger): void {
    this.journalLedger = ledger;
  },

  /**
   * At the boundary after step `index`: every quest not yet seen started is
   * asked whether it is active; each that is has its owed pages read off the
   * journal, re-read until all are shown or {@link JOURNAL_SETTLE_MS} passes.
   * A read that is not answered is a missing page, never a pass.
   */
  async judgeJournalStarts(this: MineflayerExecutor, index: number): Promise<void> {
    const ledger = this.journalLedger;
    if (ledger === undefined) return;
    for (const q of ledger.quests) {
      if (this.journalStarted.has(q.quest)) continue;
      const reply = await this.bracketed([scoreIsOneCommand(ledger.party, q.active)]);
      if (!scoreIsOne(reply.lines, q.active)) continue;
      this.journalStarted.add(q.quest);
      if (q.told.length === 0) continue;
      const deadline = Date.now() + JOURNAL_SETTLE_MS;
      let pages: readonly string[] = [];
      let done = new Set<string>();
      let shown = new Set<string>();
      for (;;) {
        const replies = await this.bracketedReplies([
          ...q.told.map((t) => scoreIsOneCommand(ledger.party, t.done)),
          JOURNAL_PAGES_COMMAND,
        ]);
        done = new Set(
          q.told
            .filter((t, i) => scoreIsOne(replies[i]!.lines, t.done))
            .map((t) => t.objective),
        );
        const last = replies[replies.length - 1]!;
        pages = last.answered ? last.lines : [];
        shown = new Set(q.told.filter((t) => pagesShow(pages, t.hintKey)).map((t) => t.objective));
        if (q.told.every((t) => done.has(t.objective) || shown.has(t.objective))) break;
        if (Date.now() >= deadline) break;
        await delay(JOURNAL_POLL_MS);
      }
      for (const t of q.told) {
        const outcome = done.has(t.objective) && !shown.has(t.objective)
          ? "done-before-read"
          : shown.has(t.objective)
            ? "shown"
            : "missing";
        this.journalVerdicts.push({
          quest: q.quest,
          objective: t.objective,
          step: index,
          outcome,
          detail:
            outcome === "missing"
              ? `owed the hint ${t.hintKey}; the journal read ${pages.length === 0 ? "nothing" : `"${pages.join(" ").slice(0, 600)}"`}`
              : t.hintKey,
        });
      }
    }
  },
};
