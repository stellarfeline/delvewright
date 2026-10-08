// MineflayerExecutor: the crosshair: hitboxes, stances, the interaction target.

import type { Vec3Tuple } from "../critical-path.ts";
import {
  INTERACTION_REACH,
  acquireFromStances,
  hitboxDims,
  occlusionFailure,
  type Hitbox,
  type Vec3Like,
} from "../crosshair.ts";
import { displayNameOf } from "./wave.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** How far from a rest step's anchor cell its `interaction` affordance may sit. */
export const AFFORDANCE_RADIUS = 3;

/** Vanilla standing-player hitbox, 1.21.11 — the body the stance sweep has to fit
 * into a cell, and the reason a player can never share a column with an NPC. */
const PLAYER_HITBOX_WIDTH = 0.6;

const PLAYER_HITBOX_HEIGHT = 1.8;

/** Vanilla standing eye height, 1.21.11: where the entity-pick ray starts. */
const PLAYER_EYE_HEIGHT = 1.62;

/** Slack beyond interaction reach when collecting bodies that might occlude a
 * target: a wide body whose CENTRE is past reach can still put a shoulder in the
 * ray, so the search is generous and the ray does the deciding. */
const CROSSHAIR_SEARCH_MARGIN = 5;

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Every ray-pickable body the client currently tracks near `pos`, as crosshair
   * geometry. Anything with a hitbox counts — a body occludes whether or not it
   * is itself clickable, which is the whole reason the owner's two crew NPCs
   * blocked each other.
   */
  hitboxesNear(this: MineflayerExecutor, pos: Vec3Tuple, radius: number): Hitbox[] {
    const bot = this.requireBot();
    const out: Hitbox[] = [];
    for (const e of Object.values(bot.entities)) {
      if (!e?.position || e.id === bot.entity?.id) continue;
      const dims = hitboxDims(e.name ?? "", e.width, e.height);
      if (!dims) continue;
      const d = Math.sqrt(
        (e.position.x - (pos[0] + 0.5)) ** 2 +
          (e.position.y - pos[1]) ** 2 +
          (e.position.z - (pos[2] + 0.5)) ** 2,
      );
      if (d > radius) continue;
      out.push({
        id: e.id,
        name: e.name ?? "unknown",
        label: displayNameOf(e),
        position: { x: e.position.x, y: e.position.y, z: e.position.z },
        width: dims.width,
        height: dims.height,
      });
    }
    return out;
  },

  /**
   * The body a step means to click at `pos`: the `minecraft:interaction` box the
   * compiler summoned there.
   *
   * Every clickable thing in a delve is one of those — an NPC's dialogue hitbox
   * as much as an objective's affordance — so this is the single acquisition rule
   * for all three interaction steps. Selection is still by proximity to the
   * scripted cell (a client cannot read the entity tag the compiler uses), but
   * proximity now only proposes the target; the RAY decides whether it is
   * reachable.
   */
  interactionTargetAt(this: MineflayerExecutor, pos: Vec3Tuple, candidates: readonly Hitbox[]): Hitbox | undefined {
    let best: Hitbox | undefined;
    let bestDist = AFFORDANCE_RADIUS;
    for (const c of candidates) {
      if (c.name !== "interaction") continue;
      const d = Math.sqrt(
        (c.position.x - (pos[0] + 0.5)) ** 2 +
          (c.position.y - pos[1]) ** 2 +
          (c.position.z - (pos[2] + 0.5)) ** 2,
      );
      if (d <= bestDist) {
        best = c;
        bestDist = d;
      }
    }
    return best;
  },

  /**
   * Whether a player could stand with their feet in `cell`: solid support below,
   * two cells of clear air, and no entity body already occupying the column.
   *
   * Block-shape based (`boundingBox`), like {@link gateOpen}, so it stays correct
   * for whatever the campaign built with. A cell whose chunk is not loaded reads
   * as NOT standable — the conservative direction here, since inventing a stance
   * would let a real occlusion pass.
   */
  stanceStandable(this: MineflayerExecutor, cell: Vec3Tuple, bodies: readonly Hitbox[]): boolean {
    const bot = this.requireBot();
    const p = bot.entity.position;
    const at = (dy: number) =>
      bot.blockAt(p.offset(cell[0] - p.x, cell[1] + dy - p.y, cell[2] - p.z));
    const support = at(-1);
    const feet = at(0);
    const head = at(1);
    if (!support || !feet || !head) return false;
    if (support.boundingBox === "empty") return false;
    if (feet.boundingBox !== "empty" || head.boundingBox !== "empty") return false;
    // A player is 0.6 wide and cannot share a column with another body.
    const cx = cell[0] + 0.5;
    const cz = cell[2] + 0.5;
    return !bodies.some((b) => {
      const half = (b.width + PLAYER_HITBOX_WIDTH) / 2;
      return (
        Math.abs(b.position.x - cx) < half &&
        Math.abs(b.position.z - cz) < half &&
        b.position.y < cell[1] + PLAYER_HITBOX_HEIGHT &&
        cell[1] < b.position.y + b.height
      );
    });
  },

  /**
   * Every eye position this step allows, arrival stance first.
   *
   * The step's walk goal is `GoalNear(pos, range)`, so any standable cell inside
   * that disc is a place the player may legally be standing when they click —
   * which is why a failure here means "unclickable from ANYWHERE the step
   * permits", not "unclickable from where the bot happened to stop".
   */
  stancesAround(this: MineflayerExecutor, pos: Vec3Tuple, range: number, bodies: readonly Hitbox[]): Vec3Like[] {
    const bot = this.requireBot();
    const eye = bot.entity.position;
    const out: Vec3Like[] = [{ x: eye.x, y: eye.y + PLAYER_EYE_HEIGHT, z: eye.z }];
    for (let dy = -1; dy <= 1; dy += 1) {
      for (let dx = -range; dx <= range; dx += 1) {
        for (let dz = -range; dz <= range; dz += 1) {
          const cell: Vec3Tuple = [pos[0] + dx, pos[1] + dy, pos[2] + dz];
          if (!this.stanceStandable(cell, bodies)) continue;
          out.push({
            x: cell[0] + 0.5,
            y: cell[1] + PLAYER_EYE_HEIGHT,
            z: cell[2] + 0.5,
          });
        }
      }
    }
    return out;
  },

  /**
   * Prove a player could put the crosshair on this step's target before the step
   * acts on it — the assertion the island's terminal finding needed.
   *
   * Throws, naming both bodies, when the target is unpickable from every stance
   * the step allows. Returns the acquired target (and the aim point) when it is
   * reachable, so a caller with a real click to make can look at it first.
   *
   * When no `interaction` body is tracked at `pos` at all, this reports a finding
   * and returns `undefined` rather than failing: absence is "the client has not
   * been told", not "the player cannot click", and inventing a verdict from
   * missing data is the failure mode this whole change exists to end.
   */
  requireCrosshair(
    this: MineflayerExecutor,
    pos: Vec3Tuple,
    what: string,
    range: number,
  ): { target: Hitbox; aim: Vec3Like } | undefined {
    const bodies = this.hitboxesNear(pos, INTERACTION_REACH + CROSSHAIR_SEARCH_MARGIN);
    const target = this.interactionTargetAt(pos, bodies);
    if (!target) {
      process.stderr.write(
        `[crosshair] ${what}: no \`interaction\` body tracked within ${AFFORDANCE_RADIUS} ` +
          `blocks of [${pos.join(", ")}] — acquisition unproven for this step\n`,
      );
      return undefined;
    }
    const others = bodies.filter((b) => b.id !== target.id);
    const stances = this.stancesAround(pos, range, bodies);
    const verdict = acquireFromStances(stances, target, others);
    if (!verdict.ok) {
      throw new Error(occlusionFailure(what, target, verdict.blockers, verdict.triedStances));
    }
    if (verdict.clearStances < verdict.triedStances) {
      process.stderr.write(
        `[crosshair] ${what}: acquired from ${verdict.clearStances} of ` +
          `${verdict.triedStances} allowed stances — the target is clickable, but not from ` +
          `every place the party may be standing\n`,
      );
    }
    return { target, aim: verdict.aim };
  },
};
