// MineflayerExecutor: a ride step — a body carried by another (spec-0135 §6).

import type { Entity } from "prismarine-entity";
import type { RideStep, Vec3Tuple } from "../critical-path.ts";
import { TRANSPORT_NEAR, TRANSPORT_NEAR_Y } from "../critical-path.ts";
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

/** How far (blocks) a vehicle's body may stand from the seat and still be the one boarded. */
export const VEHICLE_NEAR = 3;

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
   * Ride: walk to `board`, look at the vehicle's body and mount it, assert the
   * server seats the bot; drive a boat or a mount by reporting its position
   * along `medium` at `rate` (the steering item in hand for a mount that needs
   * one), or wait for the rails to carry a cart; assert the server's rider is
   * at the last medium cell; leave by the sneak input, assert the server
   * unseats the bot, and walk to `leave`.
   */
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
    await this.walkTo(step.board, 0, `the boarding of ${step.vehicle}`, step.sneak);
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
    let reading = await this.readRide(tag);
    while (!reading.riding && Date.now() < mountBy) {
      await delay(250);
      reading = await this.readRide(tag);
    }
    if (!reading.riding) failRide("the server did not seat the bot on the vehicle after the click");
    const start = reading.pos;
    let reported = 0;
    if (step.kind === "minecart") {
      const travel = step.medium.length / step.rate;
      const by = Date.now() + travel * 1000 + MOUNT_GRACE_MS + 2000;
      while (!arrivedAt(reading.pos, last) && Date.now() < by) {
        await delay(500);
        reading = await this.readRide(tag);
      }
    } else {
      const y0 = (bot as unknown as { vehicle?: Entity | null }).vehicle?.position?.y ?? start[1];
      const yOf =
        step.kind === "boat" ? (): number => y0 : (c: Vec3Tuple): number => c[1];
      let prev = cellCentre(seat, yOf(seat));
      for (const p of drivePoints(step.medium, step.rate, yOf)) {
        bot._client.write("vehicle_move", { x: p.at[0], y: p.at[1], z: p.at[2], yaw: p.yaw, pitch: 0, onGround: true });
        reported += dist(prev, p.at);
        prev = p.at;
        await delay(1000 / TICKS_PER_SECOND);
      }
      await delay(700);
      reading = await this.readRide(tag);
    }
    const serverSeen = dist(start, reading.pos);
    if (!reading.riding) {
      failRide("the server unseated the bot before the ride's end", { reported, serverSeen, seat: reading.pos });
    }
    if (!arrivedAt(reading.pos, last)) {
      failRide(
        `the server holds the rider at ${JSON.stringify(reading.pos.map((v) => Number(v.toFixed(2))))}, ` +
          `not at the medium's last cell ${JSON.stringify(last)} — the server did not take the ride`,
        { reported, serverSeen, seat: reading.pos },
      );
    }
    const arrived = reading.pos;
    bot._client.write("player_input", { inputs: { shift: true } });
    await delay(400);
    bot._client.write("player_input", { inputs: { shift: false } });
    await delay(600);
    reading = await this.readRide(tag);
    if (reading.riding) {
      failRide("the sneak input did not unseat the bot", { reported, serverSeen, seat: arrived });
    }
    await this.walkTo(step.leave, 0, `the shore after ${step.vehicle}`, step.sneak);
    record({ reported, serverSeen, seat: arrived, left: true, passed: true });
    process.stderr.write(
      `[ride] ${step.vehicle} (${step.kind}): ${step.medium.length} cell(s), reported ` +
        `${reported.toFixed(2)} block(s), server saw ${serverSeen.toFixed(2)}, left at ` +
        `${JSON.stringify(step.leave)}\n`,
    );
  },
};
