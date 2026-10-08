// MineflayerExecutor: the scoreboard: observing scores, tracking one, asking a gate term.

import type { Bot } from "mineflayer";
import { termClause, type GateTerm } from "../death-loop.ts";
import type { MineflayerExecutor } from "../executor.ts";

export const SCORE_POLL_MS = 250;

/**
 * How long to let the ledger settle after the respawn before reading it.
 *
 * The ORDER is guaranteed by the engine, not by this number: the death edge fires
 * `on_death` on the corpse (`if Health:0.0f`) and the re-seat on the living player
 * (`unless Health:0.0f`), so by the time a respawn is observed the forfeit has
 * already run. The poll below therefore stops the instant the ledger reaches the
 * value the campaign promised, and this ceiling only bounds the case where it
 * never does — which is the finding, not a flake.
 */
const LEDGER_SETTLE_MS = 5_000;

export const LEDGER_POLL_MS = 100;

/** How long to wait for a scoreboard objective to start reporting after tracking it. */
export const SCORE_TRACK_TIMEOUT_MS = 5_000;

/**
 * The scoreboard display slots the harness may take, to READ a ledger.
 *
 * A vanilla server only tracks — and so only broadcasts — an objective occupying
 * a display slot, and it tracks every occupied one, so the number of ledgers a
 * run can read across one death is the number of slots it holds. Vanilla
 * 1.21.11's `minecraft:scoreboard_slot` parser accepts `list`, `sidebar`,
 * `below_name` and one `sidebar.team.<colour>` per chat colour; the team-coloured
 * sidebars are the whole pool here. The plain `sidebar` is the delve's own: a
 * currency that declares `display: sidebar` (spec-0076) stands there for the
 * whole run, and this harness releases a slot by CLEARING it, so taking that one
 * would evict the campaign's readout and then blank it. `list`/`below_name`
 * change what a human watching the run sees. No player is on a colour team, so
 * a team-coloured sidebar is visible to nobody and the world reads as it did.
 *
 * Sixteen is a ceiling, not a promise: a campaign whose death forfeits more
 * datums than this gets a refusal by name from `trackScore`, never a ledger read
 * as unreported.
 */
const SCORE_DISPLAY_SLOTS: readonly string[] = [
  "sidebar.team.aqua",
  "sidebar.team.black",
  "sidebar.team.blue",
  "sidebar.team.dark_aqua",
  "sidebar.team.dark_blue",
  "sidebar.team.dark_gray",
  "sidebar.team.dark_green",
  "sidebar.team.dark_purple",
  "sidebar.team.dark_red",
  "sidebar.team.gold",
  "sidebar.team.gray",
  "sidebar.team.green",
  "sidebar.team.light_purple",
  "sidebar.team.red",
  "sidebar.team.white",
  "sidebar.team.yellow",
];

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Watch the ledgers off the wire. See {@link scores} for why the raw packet and
   * not mineflayer's own scoreboard model.
   */
  installScoreObserver(this: MineflayerExecutor, bot: Bot): void {
    // The unit tests attach a stub bot that models only the high-level API, so a
    // missing raw client is "there is no wire here", not a fault. Absent → no
    // ledger is ever observed, and every currency assertion reports that it could
    // not read rather than passing.
    const client = bot._client as Bot["_client"] | undefined;
    if (typeof client?.on !== "function") return;
    // spec-0080 §5.2: every chunk packet, for the repaint ledger. Off the raw
    // stream for the reason the score observer is: mineflayer's world model
    // applies a biome update without saying so.
    client.on("packet", (data: unknown, meta: { name?: unknown }) => {
      if (this.repaintWatch && typeof meta?.name === "string") {
        this.repaintWatch.packet(meta.name, data, Date.now());
      }
    });
    client.on("scoreboard_score", (packet: unknown) => {
      if (typeof packet !== "object" || packet === null) return;
      const p = packet as { itemName?: unknown; scoreName?: unknown; value?: unknown };
      if (typeof p.itemName !== "string" || typeof p.scoreName !== "string") return;
      if (typeof p.value !== "number") return;
      let board = this.scores.get(p.scoreName);
      if (!board) {
        board = new Map<string, number>();
        this.scores.set(p.scoreName, board);
      }
      board.set(p.itemName, p.value);
    });
    client.on("reset_score", (packet: unknown) => {
      if (typeof packet !== "object" || packet === null) return;
      const p = packet as { entity_name?: unknown; objective_name?: unknown };
      if (typeof p.entity_name !== "string") return;
      if (typeof p.objective_name === "string") {
        this.scores.get(p.objective_name)?.delete(p.entity_name);
        return;
      }
      for (const board of this.scores.values()) board.delete(p.entity_name);
    });
  },

  /**
   * Put `objective` where the server will report it, and wait until it does.
   *
   * A vanilla server only tracks — and therefore only broadcasts — an objective
   * that occupies a display slot, so reading one means asking for it. This is a
   * HARNESS action on the world, of exactly the class spec-0023 already sanctions
   * for `/damage @s` and `/effect give`: it is applied and removed by the harness,
   * it is named in the run report, and no delve content can reach it. The shipped
   * image is untouched — the bot is opped by a compose environment variable.
   *
   * **One slot per objective, out of {@link SCORE_DISPLAY_SLOTS}.** Swapping ONE
   * slot between objectives stops the server tracking the one it left, so that
   * ledger's cached values freeze at whatever they last were — and a frozen ledger
   * read as a live one is exactly the shape of a currency assertion that passes
   * over a forfeit that never happened. A death that forfeits four datums has to
   * read four ledgers ACROSS the same death, so the slots cannot be taken in turn:
   * with one, three of the four read as never-reported (measured on the gallery,
   * whose death drops four stakes). Vanilla's scoreboard has more than one
   * display slot and this uses them.
   *
   * Returns false when the objective never starts reporting: the bot is not opped,
   * the objective does not exist, or the pool is exhausted. Each is a finding,
   * never a silent pass.
   */
  async trackScore(this: MineflayerExecutor, objective: string): Promise<boolean> {
    const bot = this.requireBot();
    if (this.trackedSlots.has(objective) && this.scores.has(objective)) return true;
    const taken = new Set(this.trackedSlots.values());
    const slot = SCORE_DISPLAY_SLOTS.find((s) => !taken.has(s));
    if (slot === undefined) {
      process.stderr.write(
        `[death-loop] every one of the ${SCORE_DISPLAY_SLOTS.length} scoreboard display slot(s) ` +
          `is already holding a ledger, so '${objective}' cannot be read at all. A campaign whose ` +
          `death forfeits more datums than vanilla has display slots is a finding about the ` +
          `campaign; reading them in turn is not the repair, because an objective that leaves a ` +
          `slot stops being reported and its last value freezes\n`,
      );
      return false;
    }
    bot.chat(`/scoreboard objectives setdisplay ${slot} ${objective}`);
    this.trackedSlots.set(objective, slot);
    const ok = await this.waitFor(
      () => this.scores.get(objective) !== undefined,
      SCORE_TRACK_TIMEOUT_MS,
      LEDGER_POLL_MS,
    );
    if (!ok) {
      process.stderr.write(
        `[death-loop] the server never reported objective '${objective}' after it was put in ` +
          `display slot '${slot}'. Either the bot is not opped (compose sets DELVE_OPS_OFFLINE to ` +
          `the bot's name — check it matches DELVEWRIGHT_BOT_USERNAME), the delve declares no ` +
          `such ledger, or the server refused that slot name. The currency assertions cannot be ` +
          `made\n`,
      );
    }
    return ok;
  },

  /** Release every display slot the harness took. Idempotent; best effort. */
  clearScoreDisplay(this: MineflayerExecutor): void {
    if (this.trackedSlots.size === 0) return;
    for (const [objective, slot] of this.trackedSlots) {
      try {
        this.requireBot().chat(`/scoreboard objectives setdisplay ${slot}`);
      } catch {
        // teardown must never mask the run's own verdict
      }
      this.scores.delete(objective);
    }
    this.trackedSlots.clear();
  },

  /**
   * **Ask the server one gate term, and read which way it answered.**
   *
   * Not a value read. Two things make that the wrong instrument here, and both
   * were measured on this delve rather than reasoned about.
   *
   * *The display-slot channel cannot see an empty ledger.* {@link trackScore}
   * exists to watch a value move ACROSS a death, and a vanilla server only emits
   * a `scoreboard_score` packet for a holder that HAS a score — so an unset flag,
   * which is exactly how a `forbids_flags` gate stands open, is indistinguishable
   * from a ledger nothing could read.
   *
   * *And the delve silences command feedback.* Every build emits `gamerule
   * send_command_feedback false` so the engine's bookkeeping never reaches a
   * player, which means `/scoreboard players get` answers the bot with nothing at
   * all on the success path. Measured on the gallery: every gate term came back
   * unread and both gated stakes were reported as unassertable — the honest
   * failure, and still a failure.
   *
   * So the question is put to the server in the form it already adjudicates —
   * `execute <clause> run tellraw @s …`, once for the term and once for its
   * negation — and `tellraw` reaches its target as a system message whatever that
   * gamerule says. Exactly one of the two must land; the answer is the server's
   * own reading of the clause the compiler wrote, which is the strongest form the
   * question has. Both, or neither, establishes nothing and is reported as such.
   *
   * The token is serial-numbered so a line left over from an earlier term can
   * never be read as this one's answer, and it wears the `[dw:` sigil `DW0182`
   * reserves in every player-visible string — authored text cannot forge one.
   */
  async askTerm(this: MineflayerExecutor, term: GateTerm): Promise<boolean | undefined> {
    const bot = this.requireBot();
    const serial = ++this.gateAsks;
    const yes = `[dw:gate ${serial} in]`;
    const no = `[dw:gate ${serial} out]`;
    const from = this.chatMark();
    bot.chat(`/execute ${termClause(term)} run tellraw @s ${JSON.stringify({ text: yes })}`);
    bot.chat(
      `/execute ${termClause({ ...term, negate: !term.negate })} run tellraw @s ` +
        JSON.stringify({ text: no }),
    );
    let answer: boolean | undefined;
    await this.waitFor(
      () => {
        const said = this.chatSince(from).lines;
        const saidYes = said.some((l) => l.includes(yes));
        const saidNo = said.some((l) => l.includes(no));
        if (saidYes === saidNo) return false;
        answer = saidYes;
        return true;
      },
      SCORE_TRACK_TIMEOUT_MS,
      LEDGER_POLL_MS,
    );
    if (answer === undefined) {
      process.stderr.write(
        `[death-loop] the server answered neither way for \`${termClause(term)}\` — the gate it ` +
          `belongs to cannot be read, and nothing resting on it may be asserted. What it DID say ` +
          `in that window: ${JSON.stringify(this.chatSince(from).lines)} ` +
          `(${this.chatSince(from).lost} line(s) dropped by the chat ring)\n`,
      );
    }
    return answer;
  },

  /** The bot's own value in a tracked ledger, or `undefined` if it has none. */
  myScore(this: MineflayerExecutor, objective: string): number | undefined {
    return this.scores.get(objective)?.get(this.config.username);
  },

  /**
   * Wait until a tracked ledger reads `want`, then return whatever it reads.
   *
   * It stops early on the promised value and otherwise runs the ceiling out, so a
   * correct engine is fast and a wrong one still yields its real number for the
   * failure message. This is not "waiting for green": the value returned is the
   * observation, and the caller asserts against it either way.
   */
  async settledScore(this: MineflayerExecutor, objective: string, want: number): Promise<number | undefined> {
    await this.waitFor(() => this.myScore(objective) === want, LEDGER_SETTLE_MS, LEDGER_POLL_MS);
    return this.myScore(objective);
  },
};
