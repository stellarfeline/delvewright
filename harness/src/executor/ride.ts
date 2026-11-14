// MineflayerExecutor: a ride step — a body carried by another (spec-0135 §6).

import type { Entity } from "prismarine-entity";
import type { RideStep, Vec3Tuple } from "../critical-path.ts";
import { TRANSPORT_NEAR, TRANSPORT_NEAR_Y, censusOf } from "../critical-path.ts";
import { CENSUS_SETTLE_MS } from "./census.ts";
import { isRejection } from "../rejection.ts";
import { delay } from "./connection.ts";
import { LEDGER_POLL_MS, SCORE_TRACK_TIMEOUT_MS } from "./score.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** Minecraft server ticks per second. */
const TICKS_PER_SECOND = 20;

/** Where {@link MineflayerExecutor.readRide} parks its answer (harness-owned). */
const RIDE_READ_STORAGE = "dw_harness:ride_read";

/** How long (ms) the server may take to seat the bot after the click. */
export const MOUNT_GRACE_MS = 3000;

/** How long (ms) the server may take to position the bot after a ride. */
export const RESYNC_GRACE_MS = 3000;

/** How far (blocks) a vehicle's body may stand from the seat and still be the one boarded. */
export const VEHICLE_NEAR = 3;

/**
 * How many blocks a driven ride reports before it asks the server where it
 * holds the rider. Under ten: the server refuses whole a reported move more
 * than ten blocks from where the vehicle stood when the tick began (the
 * vanilla record §5, T16), so a drive that ran on unread past a refusal could
 * never be taken again.
 */
export const DRIVE_STRETCH_BLOCKS = 4;

/** How far (blocks, horizontally) the rider may stand from the end of a stretch the server took. */
export const DRIVE_TAKEN_NEAR = 1;

/** How long (ms) a held drive waits before it reports on from where the server holds it. */
export const DRIVE_HELD_BACKOFF_MS = 500;

/**
 * How long (ms) a driven ride may stand held, summed over the ride, before it
 * fails by name: a body in the water ahead holds a boat (T16), and the body is
 * one that moves — a drowned of a wave — so the drive waits it out, then names
 * it.
 */
export const DRIVE_HELD_BUDGET_MS = 20000;

/** How far (blocks) from the held rider a body is named as what holds it. */
const HOLDER_NEAR = 3;

/** One ride, for the run report (spec-0135 §6). */
export interface RideRecord {
  readonly vehicle: string;
  readonly kind: string;
  readonly mediumCells: number;
  /** Blocks the harness reported the vehicle moved (0 for a cart). */
  readonly reported: number;
  /** Blocks the server saw the rider moved, seat to arrival. */
  readonly serverSeen: number | null;
  /** The rider's server position on arrival. */
  readonly seat: Vec3Tuple | null;
  /** The cell left at, and whether the body stood there. */
  readonly leave: Vec3Tuple;
  readonly left: boolean;
  readonly passed: boolean;
  readonly why?: string;
}

/** The vehicle the bot sits in after a ride that stayed aboard (spec-0135 §5). */
export interface Aboard {
  readonly vehicle: string;
  /** The height the ride reported the vehicle at — a boat's, fixed at its float. */
  readonly y: number;
  /** The rider's server position where the ride ended. */
  readonly at: Vec3Tuple;
}

/**
 * Make every look the bot takes a forced one, until the returned release is
 * called. mineflayer stops its physics at a mount, and an unforced `lookAt`
 * waits for the physics tick that sends the look — so `activateBlock` and
 * `openContainer`, which look before they use, never return for a rider. A
 * forced look turns the body at once; the use packet carries its own cursor,
 * and the server judges the use from the rider's eye (the vanilla record §15).
 */
export function holdLooksForced(bot: { lookAt: (point: never, force?: boolean) => Promise<void> }): () => void {
  const original = bot.lookAt;
  bot.lookAt = (point: never) => original.call(bot, point, true);
  return () => {
    bot.lookAt = original;
  };
}

/** What the server says of the bot and a vehicle. */
export interface RideReading {
  /** The bot's root vehicle is the vehicle. */
  readonly riding: boolean;
  /** The bot's server `Pos`. */
  readonly pos: Vec3Tuple;
}

/** The tag every body of `vehicle` carries — the compiler's `dw_vehicle_<safe>`. */
export function vehicleTag(vehicle: string): string {
  const local = vehicle.includes("/") ? vehicle.slice(vehicle.indexOf("/") + 1) : vehicle;
  return `dw_vehicle_${local.replace(/[-/.]/g, "_")}`;
}

/** The centre of a medium cell, as a ridden body's reported position. */
export function cellCentre(c: Vec3Tuple, y: number): Vec3Tuple {
  return [c[0] + 0.5, y, c[2] + 0.5];
}

/**
 * The reported positions along `medium` at `rate` blocks a second, one per
 * server tick: the medium's cell centres joined by straight runs, the y of
 * each point from `yOf`. Pure, for its unit test.
 */
export function drivePoints(
  medium: readonly Vec3Tuple[],
  rate: number,
  yOf: (cell: Vec3Tuple) => number,
): { readonly at: Vec3Tuple; readonly yaw: number }[] {
  const per = rate / TICKS_PER_SECOND;
  const out: { at: Vec3Tuple; yaw: number }[] = [];
  for (let i = 1; i < medium.length; i++) {
    const a = cellCentre(medium[i - 1]!, yOf(medium[i - 1]!));
    const b = cellCentre(medium[i]!, yOf(medium[i]!));
    const dx = b[0] - a[0];
    const dz = b[2] - a[2];
    const len = Math.hypot(dx, dz, b[1] - a[1]);
    const yaw = (Math.atan2(-dx, dz) * 180) / Math.PI;
    const n = Math.max(1, Math.ceil(len / per));
    for (let k = 1; k <= n; k++) {
      const t = k / n;
      out.push({ at: [a[0] + dx * t, a[1] + (b[1] - a[1]) * t, a[2] + dz * t], yaw });
    }
  }
  return out;
}

/**
 * Where a held drive goes on from: the index of the drive point, among
 * `points[from..=to]` — the stretch just reported — nearest (horizontally) to
 * where the server holds the rider. The next point reported is the one after
 * it. Pure, for its unit test.
 */
export function resumeIndex(
  points: readonly { readonly at: Vec3Tuple }[],
  held: Vec3Tuple,
  from: number,
  to: number,
): number {
  let best = from;
  let bestD = Infinity;
  for (let k = Math.max(0, from); k <= Math.min(to, points.length - 1); k++) {
    const p = points[k]!.at;
    const d = Math.hypot(p[0] - held[0], p[2] - held[2]);
    if (d < bestD) {
      bestD = d;
      best = k;
    }
  }
  return best;
}

/** Whether the server took a stretch: the rider stands within reach of its last point. */
export function stretchTaken(held: Vec3Tuple, end: Vec3Tuple): boolean {
  return Math.hypot(held[0] - end[0], held[2] - end[2]) <= DRIVE_TAKEN_NEAR;
}

/** Whether a server position is at `cell` within the landing tolerance. */
export function arrivedAt(pos: Vec3Tuple, cell: Vec3Tuple): boolean {
  return (
    Math.abs(pos[0] - (cell[0] + 0.5)) <= TRANSPORT_NEAR &&
    Math.abs(pos[2] - (cell[2] + 0.5)) <= TRANSPORT_NEAR &&
    Math.abs(pos[1] - cell[1]) <= TRANSPORT_NEAR_Y
  );
}

/** Horizontal-and-vertical distance between two positions. */
function dist(a: Vec3Tuple, b: Vec3Tuple): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
}

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Whether the server seats the bot on `tag`'s body, and the bot's server
   * `Pos`, read through `tellraw` because the delve seals command feedback
   * off. A refused or missing answer is an error, never a guess.
   */
  async readRide(this: MineflayerExecutor, tag: string): Promise<RideReading> {
    const bot = this.requireBot();
    const serial = ++this.posReads;
    const marker = `[dw:ride ${serial} `;
    const from = this.chatMark();
    bot.chat(`/data modify storage ${RIDE_READ_STORAGE} on set value 0`);
    bot.chat(
      `/execute store success storage ${RIDE_READ_STORAGE} on int 1 on vehicle if entity @s[tag=${tag}]`,
    );
    bot.chat(
      `/tellraw @s ${JSON.stringify([
        { text: marker },
        { nbt: "on", storage: RIDE_READ_STORAGE },
        { text: " " },
        { nbt: "Pos", entity: "@s" },
        { text: "]" },
      ])}`,
    );
    const deadline = Date.now() + SCORE_TRACK_TIMEOUT_MS;
    for (;;) {
      const lines = this.chatSince(from).lines;
      const refused = lines.find((l) => isRejection(l));
      if (refused !== undefined) throw new Error(`ride read refused: ${refused}`);
      for (const l of lines) {
        const m = /\[dw:ride \d+ (\d)b? \[(-?[\d.E-]+)d,\s*(-?[\d.E-]+)d,\s*(-?[\d.E-]+)d\]\]/.exec(l);
        if (m && l.includes(marker)) {
          return { riding: m[1] === "1", pos: [Number(m[2]), Number(m[3]), Number(m[4])] };
        }
      }
      if (Date.now() > deadline) {
        throw new Error(`ride read: no answer in ${SCORE_TRACK_TIMEOUT_MS}ms`);
      }
      await delay(LEDGER_POLL_MS);
    }
  },

  /**
   * Have the server position the bot where it stands. mineflayer stops its
   * physics at a mount and starts it again only at the server's next position
   * packet, and a dismount by the sneak input sends none (the client's vehicle
   * stays stale, the vanilla record §4): until one comes the bot cannot walk.
   * A teleport to its own position is that packet and moves nothing. Returns
   * whether it came.
   */
  async resyncBody(this: MineflayerExecutor): Promise<boolean> {
    const bot = this.requireBot();
    const moved = new Promise<boolean>((resolve) => {
      const timer = setTimeout(() => {
        bot.removeListener("forcedMove", on);
        resolve(false);
      }, RESYNC_GRACE_MS);
      const on = (): void => {
        clearTimeout(timer);
        resolve(true);
      };
      bot.once("forcedMove", on);
    });
    bot.chat("/tp @s ~ ~ ~");
    return moved;
  },

  /**
   * Ride: walk to `board`, look at the vehicle's body and mount it, assert the
   * server seats the bot; drive a boat or a mount by reporting its position
   * along `medium` at `rate` (the steering item in hand for a mount that needs
   * one), or wait for the rails to carry a cart; assert the server's rider is
   * at the last medium cell; leave by the sneak input, assert the server
   * unseats the bot, and walk to `leave`.
   *
   * A ride that does not board goes on from aboard (spec-0135 §5): the server
   * must already seat the bot on the vehicle. A ride that stays ends aboard:
   * the bot is not unseated, and the next step's act is done from the seat.
   */
  /** Clear the seat a ride that stayed left the bot in, and its forced looks. */
  leaveSeat(this: MineflayerExecutor): void {
    this.aboard = undefined;
    this.releaseSeatLooks?.();
    this.releaseSeatLooks = undefined;
  },

  async ride(this: MineflayerExecutor, step: RideStep): Promise<void> {
    const bot = this.requireBot();
    const tag = vehicleTag(step.vehicle);
    const seat = step.medium[0]!;
    const last = step.medium[step.medium.length - 1]!;
    const record = (r: Omit<RideRecord, "vehicle" | "kind" | "mediumCells" | "leave">): void => {
      this.rides.push({
        vehicle: step.vehicle,
        kind: step.kind,
        mediumCells: step.medium.length,
        leave: step.leave,
        ...r,
      });
    };
    const failRide = (why: string, partial: Partial<RideRecord> = {}): never => {
      record({ reported: 0, serverSeen: null, seat: null, left: false, passed: false, why, ...partial });
      throw new Error(`ride ${step.vehicle} (${step.kind}): ${why}`);
    };
    const wasAboard = this.aboard;
    this.leaveSeat();
    let reading: RideReading;
    if (!step.boards) {
      if (wasAboard?.vehicle !== step.vehicle) {
        failRide(
          `the ride goes on from aboard, and the bot sits in ${wasAboard?.vehicle ?? "nothing"} — ` +
            "the ride before it did not stay in this vehicle",
        );
      }
      reading = await this.readRide(tag);
      if (!reading.riding) failRide("the ride goes on from aboard, and the server does not seat the bot on the vehicle");
    } else {
      this.rideBody = step.body;
      try {
        await this.walkTo(step.board, 0, `the boarding of ${step.vehicle}`, step.sneak);
      } finally {
        this.rideBody = [];
      }
      if (step.steeringItem !== undefined) {
        const want = step.steeringItem.replace(/^minecraft:/, "");
        const item = bot.inventory.items().find((i) => i.name === want);
        if (item === undefined) failRide(`the bot holds no ${step.steeringItem} to steer by`);
        await bot.equip(item!, "hand");
      }
      const centre = cellCentre(seat, seat[1]);
      const body: Entity | null = bot.nearestEntity(
        (e) =>
          e.type !== "player" &&
          e.name !== "item" &&
          Math.abs(e.position.x - centre[0]) <= VEHICLE_NEAR &&
          Math.abs(e.position.z - centre[2]) <= VEHICLE_NEAR &&
          Math.abs(e.position.y - centre[1]) <= VEHICLE_NEAR,
      );
      if (body === null) failRide(`no body stands within ${VEHICLE_NEAR} blocks of the seat ${JSON.stringify(seat)}`);
      await bot.lookAt(body!.position.offset(0, (body!.height ?? 0.5) / 2, 0), true);
      bot.mount(body!);
      const mountBy = Date.now() + MOUNT_GRACE_MS;
      reading = await this.readRide(tag);
      while (!reading.riding && Date.now() < mountBy) {
        await delay(250);
        reading = await this.readRide(tag);
      }
      if (!reading.riding) failRide("the server did not seat the bot on the vehicle after the click");
    }
    const start = reading.pos;
    let reported = 0;
    let ridY = start[1];
    // Where the server held a waited-for rider, reading by reading: said when
    // the ride does not arrive.
    const trace: string[] = [];
    if (step.kind === "minecart") {
      const travel = step.medium.length / step.rate;
      const by = Date.now() + travel * 1000 + MOUNT_GRACE_MS + 2000;
      const t0 = Date.now();
      while (!arrivedAt(reading.pos, last) && Date.now() < by) {
        await delay(500);
        reading = await this.readRide(tag);
        trace.push(
          `${((Date.now() - t0) / 1000).toFixed(1)}s ${JSON.stringify(reading.pos.map((v) => Number(v.toFixed(2))))}`,
        );
      }
    } else {
      const vehicleY = (bot as unknown as { vehicle?: Entity | null }).vehicle?.position?.y;
      const y0 = wasAboard?.y ?? vehicleY ?? start[1];
      ridY = y0;
      process.stderr.write(
        `[ride] ${step.vehicle}: seated, the server holds the rider at ` +
          `${JSON.stringify(start.map((v) => Number(v.toFixed(2))))}; the ${step.kind} is reported at y ` +
          `${y0.toFixed(2)}, from ${wasAboard !== undefined ? "the ride before it" : vehicleY === undefined ? "the rider's seat" : "the client's vehicle"}\n`,
      );
      const yOf =
        step.kind === "boat" ? (): number => y0 : (c: Vec3Tuple): number => c[1];
      // The drive is closed on the server: a stretch reported, then the
      // server asked where it holds the rider. A stretch it did not take —
      // a body in the water ahead holds a boat (the vanilla record §5, T16)
      // — is reported again from where it holds it, after a pause, until it
      // is taken or the held time runs past the budget.
      const points = drivePoints(step.medium, step.rate, yOf);
      const perStretch = Math.max(1, Math.ceil((DRIVE_STRETCH_BLOCKS * TICKS_PER_SECOND) / step.rate));
      let prev = cellCentre(seat, yOf(seat));
      let heldMs = 0;
      let holds = 0;
      let i = 0;
      while (i < points.length) {
        const end = Math.min(points.length, i + perStretch);
        for (let k = i; k < end; k++) {
          const p = points[k]!;
          bot._client.write("vehicle_move", { x: p.at[0], y: p.at[1], z: p.at[2], yaw: p.yaw, pitch: 0, onGround: true });
          // The client does not move a rider it steers itself: keep its own
          // position with the seat, so what is read of where the bot stands
          // during the ride (a cue that plays at the rider) is where it is.
          bot.entity.position.set(p.at[0], p.at[1], p.at[2]);
          reported += dist(prev, p.at);
          prev = p.at;
          await delay(1000 / TICKS_PER_SECOND);
        }
        if (end === points.length) await delay(700);
        reading = await this.readRide(tag);
        if (!reading.riding) break;
        const last = points[end - 1]!.at;
        if (stretchTaken(reading.pos, last)) {
          i = end;
          continue;
        }
        holds += 1;
        if (heldMs >= (this.driveHeldBudgetMs ?? DRIVE_HELD_BUDGET_MS)) {
          const bodies = Object.values((bot as unknown as { entities?: Record<string, Entity> }).entities ?? {})
            .filter((e) => e !== bot.entity && e.type !== "player" && e.name !== step.kind && !String(e.name ?? "").endsWith("boat"))
            .filter((e) => Math.hypot(e.position.x - reading.pos[0], e.position.z - reading.pos[2]) <= HOLDER_NEAR)
            .map((e) => `${e.name ?? e.type} at [${[e.position.x, e.position.y, e.position.z].map((v) => v.toFixed(1)).join(", ")}]`);
          failRide(
            `the server held the rider at ${JSON.stringify(reading.pos.map((v) => Number(v.toFixed(2))))} for ` +
              `${heldMs}ms over ${holds} stretch(es), short of ${JSON.stringify(last.map((v) => Number(v.toFixed(2))))} — ` +
              `the server did not take the ride; ` +
              (bodies.length > 0 ? `within ${HOLDER_NEAR} blocks: ${bodies.join(", ")}` : `no body within ${HOLDER_NEAR} blocks of it`),
            { reported, serverSeen: dist(start, reading.pos), seat: reading.pos },
          );
        }
        process.stderr.write(
          `[ride] ${step.vehicle}: held at ${JSON.stringify(reading.pos.map((v) => Number(v.toFixed(2))))}, short of ` +
            `${JSON.stringify(last.map((v) => Number(v.toFixed(2))))}; reporting on from there in ${DRIVE_HELD_BACKOFF_MS}ms\n`,
        );
        await delay(DRIVE_HELD_BACKOFF_MS);
        heldMs += DRIVE_HELD_BACKOFF_MS;
        i = resumeIndex(points, reading.pos, i === 0 ? 0 : i - 1, end - 1) + 1;
        prev = reading.pos;
        bot.entity.position.set(reading.pos[0], reading.pos[1], reading.pos[2]);
      }
      if (holds > 0) {
        process.stderr.write(`[ride] ${step.vehicle}: held ${holds} time(s), ${heldMs}ms in all, and taken\n`);
      }
    }
    const serverSeen = dist(start, reading.pos);
    if (!reading.riding) {
      failRide("the server unseated the bot before the ride's end", { reported, serverSeen, seat: reading.pos });
    }
    if (!arrivedAt(reading.pos, last)) {
      failRide(
        `the server holds the rider at ${JSON.stringify(reading.pos.map((v) => Number(v.toFixed(2))))}, ` +
          `not at the medium's last cell ${JSON.stringify(last)} — the server did not take the ride` +
          (trace.length > 0 ? `; the rider was held at ${trace.join(", ")}` : ""),
        { reported, serverSeen, seat: reading.pos },
      );
    }
    const arrived = reading.pos;
    process.stderr.write(
      `[ride] ${step.vehicle}: arrived; the server holds the rider at ` +
        `${JSON.stringify(arrived.map((v) => Number(v.toFixed(2))))}\n`,
    );
    // spec-0135 §5: a ride whose objective waits for its own rider carries
    // that census. The bot, aboard, is the one body it counts: one census
    // period, then the datum is read on the server, and a count that
    // disagrees fails the ride by name.
    for (const row of censusOf(step)) {
      if (row.riders === undefined) continue;
      if (row.riders !== tag) {
        failRide(`its census \`${row.state}\` counts the riders of ${row.riders}, not of the vehicle ridden`, { reported, serverSeen, seat: arrived });
      }
      await delay(CENSUS_SETTLE_MS);
      const read = await this.askTerm({ objective: row.objective, holder: "#party", min: 1, max: 1, negate: false });
      const passed = read === true && row.count === 1;
      const detail =
        read === undefined
          ? `the server never said what \`${row.objective}\` holds`
          : read === false
            ? `the game's census of \`${row.state}\` does not read 1 with the bot aboard`
            : row.count === 1
              ? `\`${row.state}\` reads 1, the bot aboard`
              : `\`${row.state}\` reads 1 with the bot aboard, and the step needs ${row.count}: one bot is one rider`;
      this.censusResults.push({ state: row.state, placed: 1, passed, detail });
      process.stderr.write(`[census] ${row.state}: ${passed ? "counted" : "FAILED"} — ${detail}\n`);
      if (!passed) failRide(`census ${row.state}: ${detail}`, { reported, serverSeen, seat: arrived });
    }
    if (step.stays) {
      // spec-0135 §5: the act after this ride is done from the seat. The
      // client's own body stands where the server holds the rider, so the act
      // aims from the eye the server judges it by.
      this.aboard = { vehicle: step.vehicle, y: ridY, at: arrived };
      this.releaseSeatLooks = holdLooksForced(bot as never);
      bot.entity.position.set(arrived[0], arrived[1], arrived[2]);
      record({ reported, serverSeen, seat: arrived, left: false, passed: true });
      process.stderr.write(
        `[ride] ${step.vehicle} (${step.kind}): ${step.medium.length} cell(s), reported ` +
          `${reported.toFixed(2)} block(s), server saw ${serverSeen.toFixed(2)}, aboard at ` +
          `${JSON.stringify(arrived.map((v) => Number(v.toFixed(2))))} for the act from the seat\n`,
      );
      return;
    }
    bot._client.write("player_input", { inputs: { shift: true } });
    await delay(400);
    bot._client.write("player_input", { inputs: { shift: false } });
    await delay(600);
    reading = await this.readRide(tag);
    if (reading.riding) {
      failRide("the sneak input did not unseat the bot", { reported, serverSeen, seat: arrived });
    }
    if (!(await this.resyncBody())) {
      failRide("the server did not position the bot after it left the vehicle", { reported, serverSeen, seat: arrived });
    }
    this.landingSwims = step.kind === "boat";
    try {
      const p = bot.entity.position;
      process.stderr.write(
        `[ride] ${step.vehicle}: left; the bot stands at [${p.x.toFixed(2)}, ${p.y.toFixed(2)}, ` +
          `${p.z.toFixed(2)}], the server holds it at ${JSON.stringify(reading.pos.map((v) => Number(v.toFixed(2))))}\n`,
      );
      await this.walkTo(step.leave, 0, `the shore after ${step.vehicle}`, step.sneak);
    } finally {
      this.landingSwims = false;
    }
    record({ reported, serverSeen, seat: arrived, left: true, passed: true });
    process.stderr.write(
      `[ride] ${step.vehicle} (${step.kind}): ${step.medium.length} cell(s), reported ` +
        `${reported.toFixed(2)} block(s), server saw ${serverSeen.toFixed(2)}, left at ` +
        `${JSON.stringify(step.leave)}\n`,
    );
  },
};
