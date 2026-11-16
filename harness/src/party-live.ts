// The party run's live side (`party.ts` holds the pure half): three more
// mineflayer clients on the server the critical-path bot walks, each doing only
// what a friend does — join, take the class, stand, leave, come back — and the
// server asked about each of them through the walking bot's bracketed command
// channel at every step boundary.
//
// Assertions and navigation only (CLAUDE.md): the members never walk, and
// nothing here knows a campaign; the schedule comes from the critical path and
// the seats from the build's death plan.

import type { Bot } from "mineflayer";
import { createHarnessBot } from "./client-loaded.ts";
import type { BracketedReply } from "./command-reply.ts";
import type { Seat } from "./death-loop.ts";
import type { Vec3Tuple } from "./critical-path.ts";
import { parseCompletionMarker } from "./markers.ts";
import {
  AWAY,
  IDLER,
  LATE,
  MEMBERS,
  distance,
  inPlay,
  judgeArrival,
  judgeCarried,
  judgeIdler,
  judgeOwed,
  judgeTellings,
  parseReadings,
  readingCommands,
  owedPlace,
  type Crossing,
  type Heard,
  type MemberReading,
  type OwedVerdict,
  type PartySchedule,
  type Presence,
  type TellingVerdict,
} from "./party.ts";

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/** How long a member is given after its spawn before it acts. */
const SPAWN_SETTLE_MS = 3_000;
/** How long a class apply is given to teleport, kit and seat the member. */
const CLASS_SETTLE_MS = 3_000;
/** How long a reading waits for a member to come back into play, or level its grants. */
const SETTLE_POLL_MS = 15_000;
const POLL_STEP_MS = 500;

export interface PartyConnection {
  readonly host: string;
  readonly port: number;
  readonly version: string;
  readonly auth: "offline" | "microsoft";
}

export interface PartyRunOptions {
  readonly connection: PartyConnection;
  readonly campaignId: string;
  readonly schedule: PartySchedule;
  readonly seats: readonly Seat[];
  /** Each critical-path step's landing (`transport`), by step index: where a crossing that step makes carries the party. */
  readonly transports: readonly (Vec3Tuple | undefined)[];
  /** The walking bot's bracketed command channel (it is opped; the members are not). */
  readonly ask: (commands: readonly string[]) => Promise<BracketedReply>;
  /** The walking bot, whose hearing every member's is judged against. */
  readonly walker: Bot;
  readonly log: (line: string) => void;
}

/** What one party run established. */
export interface PartyResult {
  readonly schedule: PartySchedule;
  readonly idlerReadings: number;
  readonly arrival: { bound: boolean; ok: boolean; detail: string } | undefined;
  readonly owed: readonly OwedVerdict[];
  readonly tellings: Readonly<Record<string, TellingVerdict>>;
  readonly failures: readonly string[];
  readonly line: string;
}

interface Member {
  readonly name: string;
  bot: Bot | undefined;
  readonly heard: Heard[];
  readonly presence: { from: number; to?: number }[];
}

export class PartyRun {
  private readonly o: PartyRunOptions;
  private readonly members = new Map<string, Member>();
  private readonly walkerHeard: Heard[] = [];
  private readonly failures: string[] = [];
  private readonly owed: OwedVerdict[] = [];
  private arrival: PartyResult["arrival"];
  private serial = 0;
  private stand: Vec3Tuple | undefined;
  private standDeaths = 0;
  private idlerReadings = 0;
  private awayLeftAt: Vec3Tuple | undefined;
  /** The party's crossing count when the away member left. */
  private awayLeftCrossed = 0;
  /** The party's crossing count last read, and the last crossing it made. */
  private partyCrossed = 0;
  private crossing: Crossing | undefined;
  private crossingsRead = 0;
  private lateJoined = false;
  private awayCameBack = false;
  private rawLogged = false;

  constructor(options: PartyRunOptions) {
    this.o = options;
    for (const name of MEMBERS) this.members.set(name, { name, bot: undefined, heard: [], presence: [] });
    options.walker.on("messagestr", (line: string) => this.hear(this.walkerHeard, line));
  }

  private hear(into: Heard[], line: string): void {
    const m = parseCompletionMarker(line);
    if (m && m.campaignId === this.o.campaignId) into.push({ token: m.token, at: Date.now() });
  }

  private member(name: string): Member {
    const m = this.members.get(name);
    if (!m) throw new Error(`no party member ${name}`);
    return m;
  }

  /** Join `name`, wait for its spawn, and take the walker's class. */
  private async join(name: string): Promise<void> {
    const m = this.member(name);
    const { bot } = createHarnessBot({
      host: this.o.connection.host,
      port: this.o.connection.port,
      username: name,
      version: this.o.connection.version,
      auth: this.o.connection.auth,
      // A member's death is the server's to seat; a friend clicks Respawn.
      respawn: true,
    });
    bot.on("messagestr", (line: string) => this.hear(m.heard, line));
    // A member's socket error after its join is logged, never left to an
    // emitter with no listener (which ends the process).
    bot.on("error", (e: Error) => this.o.log(`[party] ${name}: ${e.message}`));
    bot.on("kicked", (r: unknown) => this.o.log(`[party] ${name} was kicked: ${JSON.stringify(r)}`));
    await new Promise<void>((resolve, reject) => {
      bot.once("spawn", () => resolve());
      bot.once("error", reject);
      bot.once("kicked", (r: unknown) => reject(new Error(`${name} was kicked: ${JSON.stringify(r)}`)));
    });
    m.bot = bot;
    m.presence.push({ from: Date.now() });
    bot.on("end", () => {
      const open = m.presence[m.presence.length - 1];
      if (open && open.to === undefined) open.to = Date.now();
    });
    await sleep(SPAWN_SETTLE_MS);
    if (this.o.schedule.classCommand !== undefined) {
      bot.chat(this.o.schedule.classCommand);
      await sleep(CLASS_SETTLE_MS);
    }
    this.o.log(`[party] ${name} joined${this.o.schedule.classCommand ? " and took the class" : ""}`);
  }

  private async leave(name: string): Promise<void> {
    const m = this.member(name);
    const bot = m.bot;
    if (!bot) return;
    const ended = new Promise<void>((r) => bot.once("end", () => r()));
    bot.quit();
    await Promise.race([ended, sleep(5_000)]);
    const open = m.presence[m.presence.length - 1];
    if (open && open.to === undefined) open.to = Date.now();
    m.bot = undefined;
    this.o.log(`[party] ${name} left`);
  }

  /** Ask the server about every member. */
  async read(): Promise<Map<string, MemberReading>> {
    this.serial += 1;
    const serial = this.serial;
    const reply = await this.o.ask(readingCommands(serial, MEMBERS));
    if (!reply.answered) throw new Error(`party reading ${serial} was never answered`);
    const readings = parseReadings(serial, reply.lines, MEMBERS);
    // A member with a connection the server states no body for is the reading
    // failing, not the member being away: its raw lines are logged once.
    const unread = MEMBERS.filter((n) => this.member(n).bot !== undefined && !readings.get(n)!.online);
    if (unread.length > 0 && !this.rawLogged) {
      this.rawLogged = true;
      this.o.log(`[party] reading ${serial} states no body for ${unread.join(", ")}: ${JSON.stringify(reply.lines)}`);
    }
    return readings;
  }

  /** Read until `done` holds of `name`'s reading, or the poll runs out; the last reading. */
  private async readUntil(name: string, done: (r: MemberReading) => boolean): Promise<MemberReading> {
    const deadline = Date.now() + SETTLE_POLL_MS;
    for (;;) {
      const r = (await this.read()).get(name)!;
      if (done(r) || Date.now() >= deadline) return r;
      await sleep(POLL_STEP_MS);
    }
  }

  /** Before the first step: the idler and the member who will leave are there from the start. */
  async start(): Promise<void> {
    const s = this.o.schedule;
    this.o.log(
      `[party] schedule over ${s.boundaries} step boundary(ies): ${IDLER} and ${AWAY} from the start; ` +
        `${AWAY} leaves after step ${s.dropAfter} and returns after step ${s.rejoinAfter}; ` +
        `${LATE} joins after step ${s.lateJoinAfter}; class ${s.classCommand ?? "none (the campaign has no class)"}`,
    );
    await this.join(IDLER);
    await this.join(AWAY);
  }

  /** Every step boundary: the idler judged, and whatever the schedule puts here. */
  async boundary(index: number): Promise<void> {
    const at = `after step ${index}`;
    const idler = await this.readUntil(IDLER, inPlay);
    if (this.stand === undefined && !inPlay(idler)) {
      this.fail(`${at}: the idler could not be read in play (game mode ${idler.mode ?? "unread"}, ${idler.online ? "a body" : "no body"})`);
    } else if (this.stand === undefined) {
      // The idler's place is where it stands once every member has taken the
      // class at the entry: taking one is a move the table names.
      this.stand = idler.pos;
      this.standDeaths = idler.deaths ?? 0;
      this.o.log(`[party] ${IDLER} stands at ${idler.pos?.map((v) => v.toFixed(2)).join(" ") ?? "nowhere"}`);
    } else if ((idler.partyCrossed ?? 0) > this.partyCrossed) {
      // The party crossed on this step: the idler is carried with it, onto a
      // seat around the step's landing, and stands there from now on.
      this.partyCrossed = idler.partyCrossed ?? 0;
      const landing = this.o.transports[index];
      if (landing === undefined) {
        this.fail(`${at}: the party crossed and the step names no landing`);
      } else {
        this.crossing = { after: index, landing, cp: idler.cp };
        this.crossingsRead += 1;
        this.idlerReadings += 1;
        const f = judgeCarried(idler, this.crossing, `${at}: the idler`);
        if (f !== undefined) this.fail(f);
        else this.o.log(`[party] ok ${IDLER} carried by the crossing after step ${index}`);
      }
      this.stand = idler.pos;
      this.standDeaths = idler.deaths ?? 0;
    } else {
      this.idlerReadings += 1;
      const f = judgeIdler(this.stand, this.standDeaths, idler, this.o.seats, at, this.crossing);
      if (f !== undefined) {
        this.failures.push(f);
        this.o.log(`[party] FAIL ${f}`);
      }
    }
    const s = this.o.schedule;
    if (index === s.dropAfter) {
      const leaving = (await this.read()).get(AWAY);
      this.awayLeftAt = leaving?.pos;
      this.awayLeftCrossed = leaving?.partyCrossed ?? 0;
      await this.leave(AWAY);
    }
    if (index === s.rejoinAfter) {
      await this.join(AWAY);
      this.awayCameBack = true;
      const back = await this.readUntil(AWAY, (r) => inPlay(r) && r.granted === r.partyGranted);
      this.judgeOwed(back, `${AWAY} back ${at}`);
      if ((back.partyCrossed ?? 0) > this.awayLeftCrossed) {
        // The party crossed while it was away: it is owed the crossing, and
        // is put where the party is.
        const place = owedPlace(this.o.seats, back.cp, this.crossing);
        if (place === undefined) {
          this.fail(`${AWAY} came back after a crossing and the build names no place for it`);
        } else if (!back.pos || distance(back.pos, place.point) > 1.5) {
          this.fail(
            `${AWAY} came back after the party crossed and stands ${back.pos ? distance(back.pos, place.point).toFixed(2) : "nowhere"} ` +
              `from where the party is (${place.label}) — a member who was away is owed the crossing`,
          );
        } else {
          this.o.log(`[party] ok ${AWAY} came back after a crossing onto ${place.label}`);
        }
      } else if (this.awayLeftAt && back.pos) {
        const d = distance(back.pos, this.awayLeftAt);
        if (d > 1.5) {
          this.fail(`${AWAY} came back ${d.toFixed(2)} blocks from where it left — a returning member is moved`);
        }
      }
      if (!inPlay(back)) this.fail(`${AWAY} came back out of play (game mode ${back.mode ?? "unread"})`);
    }
    if (index === s.lateJoinAfter) {
      await this.join(LATE);
      this.lateJoined = true;
      const late = await this.readUntil(LATE, (r) => inPlay(r) && r.granted === r.partyGranted);
      this.arrival = judgeArrival(late, this.o.seats, this.crossing);
      this.o.log(`[party] ${this.arrival.ok ? "ok" : "FAIL"} ${this.arrival.detail}`);
      if (!this.arrival.ok) this.failures.push(this.arrival.detail);
      this.judgeOwed(late, `${LATE} on joining ${at}`);
    }
  }

  private fail(f: string): void {
    this.failures.push(f);
    this.o.log(`[party] FAIL ${f}`);
  }

  private judgeOwed(r: MemberReading, who: string): void {
    const v = judgeOwed(r, who);
    this.owed.push(v);
    this.o.log(`[party] ${v.ok ? "ok" : "FAIL"} ${v.detail}`);
    if (!v.ok) this.failures.push(v.detail);
  }

  /** After the path: every member levelled, the idler where it stood, every telling judged. */
  async finish(pathProven: boolean): Promise<PartyResult> {
    const s = this.o.schedule;
    if (pathProven) {
      if (!this.lateJoined) this.fail(`${LATE} never joined: the boundary after step ${s.lateJoinAfter} was not reached`);
      if (!this.awayCameBack) this.fail(`${AWAY} never came back: the boundary after step ${s.rejoinAfter} was not reached`);
      if (this.stand !== undefined) {
        const idler = await this.readUntil(IDLER, inPlay);
        this.idlerReadings += 1;
        const f = judgeIdler(this.stand, this.standDeaths, idler, this.o.seats, "after the path", this.crossing, true);
        if (f !== undefined) this.fail(f);
      }
      // Settled before the final grant reading: the last beat's grant reaches a
      // member on the tick after it is issued.
      await sleep(1_000);
      const readings = await this.read();
      for (const name of MEMBERS) {
        const r = readings.get(name)!;
        if (r.online) this.judgeOwed(r, `${name} at the end`);
      }
    }
    // Let the last broadcast land on every client before judging who heard it.
    await sleep(1_000);
    const tellings: Record<string, TellingVerdict> = {};
    for (const name of MEMBERS) {
      const m = this.member(name);
      const v = judgeTellings(this.walkerHeard, m.heard, m.presence as readonly Presence[]);
      tellings[name] = v;
      for (const t of v.missing) this.fail(`${name} was present and never told \`${t}\``);
      for (const t of v.replayed) this.fail(`${name} was told \`${t}\`, which the walker was not told then — a replayed beat`);
    }
    const boundOwed = this.owed.filter((v) => v.bound).length;
    const told = Object.values(tellings).reduce((n, v) => n + v.owed, 0);
    // A party run that judged nothing is not a pass (playtest-methodology
    // rule 1): the idler read at no boundary, or a walker that heard beats no
    // member was owed, is the run failing to bind.
    if (pathProven && this.idlerReadings === 0) this.fail("the idler was read at no step boundary");
    if (pathProven && this.walkerHeard.length > 0 && told === 0) {
      this.fail(`the walker heard ${this.walkerHeard.length} beat(s) and no member was owed one`);
    }
    const line =
      `party binding (spec-0141): ${MEMBERS.length} member(s) beside the walker; ` +
      `${this.idlerReadings} idler reading(s) over ${s.boundaries} boundary(ies); ` +
      `${this.crossingsRead} crossing(s) the idler was carried by; ` +
      `late arrival ${this.arrival === undefined ? "never read" : this.arrival.bound ? "judged" : "unbound"}; ` +
      `${boundOwed} of ${this.owed.length} grant reading(s) bound; ` +
      `${told} telling(s) owed to members, ${this.walkerHeard.length} heard by the walker; ` +
      `${this.failures.length} failure(s)`;
    return {
      schedule: s,
      idlerReadings: this.idlerReadings,
      arrival: this.arrival,
      owed: [...this.owed],
      tellings,
      failures: [...this.failures],
      line,
    };
  }

  /** Disconnect every member still on the server. */
  close(): void {
    for (const m of this.members.values()) {
      try {
        m.bot?.quit();
      } catch {
        // already gone
      }
      m.bot = undefined;
    }
  }
}
