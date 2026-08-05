/**
 * Crosshair acquisition — target the way a PLAYER targets.
 *
 * ## The defect this exists for (owner playtest, island — terminal finding)
 *
 * Two crew NPCs stood on one cell at the cave mouth. A human could not put the
 * crosshair on the right one, so the dialogue carrying the take-or-wait decision
 * never opened — a hard soft-lock. **This ladder was green.** It was green
 * because nothing in the harness has ever cast the ray a player casts: the
 * dialogue path chats a `/trigger` after walking near a cell (there is no
 * client, so there is no dialog button to click), and the one real right-click
 * in the tree — the bonfire `rest` — picked its affordance by *proximity to a
 * coordinate*. Proximity does not care what is in front of what. Occlusion was
 * invisible to the machine and fatal to the player: a divergence in the
 * interaction model itself.
 *
 * ## What this module adds, and what it deliberately does not
 *
 * It does not replace the `/trigger`. On 1.21.11 a dialog button is drawn by the
 * CLIENT and mineflayer has no client, so chatting the option is the only way a
 * bot can take a dialogue step, and pretending otherwise would be the hack. What
 * changes is that the trigger is no longer sent on trust: before it fires, the
 * step must **prove the click was available** — cast the entity-pick ray a
 * player's crosshair casts and require the scripted target to be the first thing
 * it meets. Acquisition becomes real even where actuation cannot be.
 *
 * So the bot still acts by id; it may no longer *aim* by id. Interaction steps
 * (`talk-to`, `interact`, `rest`) all go through {@link acquireFromStances}.
 * Targeting an entity by id without a ray survives only where no crosshair is
 * modelled at all — the combat paths, which swing at whatever the wave spawned —
 * and that boundary is the module's whole point, not an oversight.
 *
 * ## The model (vanilla 1.21.11)
 *
 * `GameRenderer.pick` traces from the eye along the look vector out to
 * `player.entity_interaction_range` ({@link INTERACTION_REACH} = 3.0 blocks) and
 * `ProjectileUtil.getEntityHitResult` returns the entity whose bounding box the
 * ray meets **first**. The box is inflated by `Entity.getPickRadius()`, which is
 * `0.0` for every body a delve stages — so this is pure ray-vs-AABB with no
 * tolerance to hide in, which is exactly what {@link pickEntity} implements.
 *
 * A player is not pinned to one spot, so neither is this: a step permits any
 * standing cell its walk goal allows, and a target counts as acquirable if
 * **some** legal stance and **some** aim point on its box put it first. Only when
 * every stance fails does the step fail — and then it names both entities, which
 * is the sentence the owner had to write by hand.
 */

/** A point in world space. */
export interface Vec3Like {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

/**
 * One ray-pickable body: mineflayer's `entity.position` (the FEET, not the
 * centre) plus the hitbox dimensions it reports.
 */
export interface Hitbox {
  /** The runtime entity id — for the failure message, never for aiming. */
  readonly id: number;
  /** Entity type as mineflayer names it: `mannequin`, `interaction`, `sheep`. */
  readonly name: string;
  /** The custom/display name when the server sent one (an NPC's own name). */
  readonly label?: string | undefined;
  /** Feet position. */
  readonly position: Vec3Like;
  /** Hitbox width in blocks (vanilla: 0.6 for a player model, 1.0 for an
   * `interaction` affordance). */
  readonly width: number;
  /** Hitbox height in blocks. */
  readonly height: number;
}

/** An axis-aligned box, world space. */
export interface Box {
  readonly min: Vec3Like;
  readonly max: Vec3Like;
}

/** `player.entity_interaction_range`, the vanilla 1.21.11 default, in blocks. */
export const INTERACTION_REACH = 3.0;

/** Vanilla `Entity.getPickRadius()` for every body a delve stages. Named rather
 * than inlined because it is the reason there is no tolerance in this geometry:
 * if vanilla ever grants slop, it belongs here and nowhere else. */
export const PICK_RADIUS = 0.0;

/**
 * The box every affordance the compiler summons occupies.
 *
 * A `minecraft:interaction` has no *type* size — its width and height are NBT
 * fields on the individual entity — so `minecraft-data` reports `0 x 0` for it
 * and a client-reported box is not available. That zero is not a small box, it
 * is an ABSENCE, and taking it literally is how this proof would have shipped
 * vacuous: every affordance in the world would have been a body no ray could
 * meet, every acquisition would have found "no target tracked", and the ladder
 * would have gone green by looking at nothing (which is exactly what the first
 * run of this code did).
 *
 * The real size is a compiler invariant, not a guess: `emit` summons every one
 * of them as `{width:1.0f,height:2.0f}` — NPC dialogue hitboxes, `interact`
 * objectives, `use`/`strike` triggers, bonfires, shortcut unlocks and trap
 * disarms alike — and `compiler::eclipse` measures against those same two
 * constants. **These must stay equal to `eclipse::AFFORDANCE_WIDTH` and
 * `eclipse::AFFORDANCE_HEIGHT`.**
 */
export const AFFORDANCE_WIDTH = 1.0;
export const AFFORDANCE_HEIGHT = 2.0;

/**
 * The hitbox to use for an entity, given what the client reported.
 *
 * `null` means "this body has no box the ray can meet" — a marker, a display —
 * and it is dropped from the geometry entirely rather than treated as a point.
 */
export function hitboxDims(
  name: string,
  width: number | undefined,
  height: number | undefined,
): { readonly width: number; readonly height: number } | null {
  if (typeof width === "number" && typeof height === "number" && width > 0 && height > 0) {
    return { width, height };
  }
  // The one type whose size the client cannot tell us — see above.
  if (name === "interaction") return { width: AFFORDANCE_WIDTH, height: AFFORDANCE_HEIGHT };
  return null;
}

/**
 * Fractions along each axis of the target box at which aim is sampled.
 *
 * A player may aim at ANY point on the box, so acquisition asks whether some
 * point works, not whether the centre does. The insets matter: a body taller or
 * narrower than its own co-located `interaction` affordance (a warden-bodied NPC
 * is 0.9 x 2.9 inside a 1.0 x 2.0 affordance) is reachable at the chest and not
 * at the head, which is exactly what a player finds.
 */
const AIM_FRACTIONS = [0.15, 0.5, 0.85] as const;

/** The box a hitbox occupies: width centred on the feet, height rising from them. */
export function boxOf(h: Hitbox): Box {
  const half = (h.width + PICK_RADIUS * 2) / 2;
  return {
    min: { x: h.position.x - half, y: h.position.y - PICK_RADIUS, z: h.position.z - half },
    max: {
      x: h.position.x + half,
      y: h.position.y + h.height + PICK_RADIUS,
      z: h.position.z + half,
    },
  };
}

/**
 * Ray-vs-AABB, the slab method — vanilla's `AABB.clip`.
 *
 * Returns the distance along `dir` (which must be unit length) at which the ray
 * enters `box`, or `null` if it misses. A ray whose origin is already inside the
 * box enters at 0, matching vanilla: standing inside a hitbox still picks it.
 */
export function rayBox(origin: Vec3Like, dir: Vec3Like, box: Box): number | null {
  let near = 0;
  let far = Number.POSITIVE_INFINITY;
  const axes = [
    [origin.x, dir.x, box.min.x, box.max.x],
    [origin.y, dir.y, box.min.y, box.max.y],
    [origin.z, dir.z, box.min.z, box.max.z],
  ] as const;
  for (const axis of axes) {
    const [o, d, lo, hi] = axis;
    if (Math.abs(d) < 1e-12) {
      // Parallel to this slab: a miss unless the origin is already between.
      if (o < lo || o > hi) return null;
      continue;
    }
    const t1 = (lo - o) / d;
    const t2 = (hi - o) / d;
    near = Math.max(near, Math.min(t1, t2));
    far = Math.min(far, Math.max(t1, t2));
    if (near > far) return null;
  }
  return far < 0 ? null : near;
}

/** Unit vector from `from` to `to`, or `null` when the two coincide. */
export function direction(from: Vec3Like, to: Vec3Like): Vec3Like | null {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const dz = to.z - from.z;
  const len = Math.sqrt(dx * dx + dy * dy + dz * dz);
  if (len < 1e-9) return null;
  return { x: dx / len, y: dy / len, z: dz / len };
}

/**
 * What the crosshair picks: the candidate whose box the ray meets first, within
 * `reach`. Ties (coincident boxes) are the client's iteration order, which is not
 * knowable here — {@link acquire} treats a tie as a failure to acquire, because a
 * click whose outcome depends on entity iteration order is not a click a campaign
 * may rely on.
 */
export function pickEntity(
  eye: Vec3Like,
  dir: Vec3Like,
  candidates: readonly Hitbox[],
  reach: number = INTERACTION_REACH,
): { readonly hit: Hitbox; readonly t: number; readonly tied: readonly Hitbox[] } | null {
  let best: { hit: Hitbox; t: number } | null = null;
  const tied: Hitbox[] = [];
  for (const c of candidates) {
    const t = rayBox(eye, dir, boxOf(c));
    if (t === null || t > reach) continue;
    if (best === null || t < best.t - 1e-9) {
      best = { hit: c, t };
      tied.length = 0;
    } else if (Math.abs(t - best.t) <= 1e-9) {
      tied.push(c);
    }
  }
  return best === null ? null : { hit: best.hit, t: best.t, tied };
}

/** The aim points sampled on a target's box, nearest-first is not required —
 * acquisition only asks whether ANY of them works. */
export function aimPoints(target: Hitbox): readonly Vec3Like[] {
  const b = boxOf(target);
  const out: Vec3Like[] = [];
  for (const fx of AIM_FRACTIONS) {
    for (const fy of AIM_FRACTIONS) {
      for (const fz of AIM_FRACTIONS) {
        out.push({
          x: b.min.x + (b.max.x - b.min.x) * fx,
          y: b.min.y + (b.max.y - b.min.y) * fy,
          z: b.min.z + (b.max.z - b.min.z) * fz,
        });
      }
    }
  }
  return out;
}

/** The verdict for one eye position. */
export type Acquisition =
  | { readonly ok: true; readonly eye: Vec3Like; readonly aim: Vec3Like }
  | { readonly ok: false; readonly blockers: readonly Hitbox[] };

/**
 * Can a player standing at `eye` put the crosshair on `target` and hit it first?
 *
 * `others` are every other ray-pickable body in the neighbourhood. The target's
 * own co-located body is legitimately among them: a delve NPC is a mannequin
 * inside a wider `interaction` affordance, so the ray meets the affordance first
 * from any side approach — modelling it honestly costs nothing and catches the
 * cases where it does not (a warden's head above its affordance).
 */
export function acquire(
  eye: Vec3Like,
  target: Hitbox,
  others: readonly Hitbox[],
  reach: number = INTERACTION_REACH,
): Acquisition {
  const candidates = [target, ...others];
  const blockers = new Map<number, Hitbox>();
  for (const aim of aimPoints(target)) {
    const dir = direction(eye, aim);
    if (dir === null) continue;
    const pick = pickEntity(eye, dir, candidates, reach);
    if (pick === null) continue;
    if (pick.hit.id === target.id && pick.tied.length === 0) {
      return { ok: true, eye, aim };
    }
    // Either something else is in front, or the pick is an exact tie the client
    // resolves by iteration order. Both are a lost click; record who did it.
    for (const t of pick.hit.id === target.id ? pick.tied : [pick.hit]) {
      if (t.id !== target.id) blockers.set(t.id, t);
    }
  }
  return { ok: false, blockers: [...blockers.values()] };
}

/** The verdict over every stance a step allows. */
export type StanceVerdict =
  | {
      readonly ok: true;
      readonly eye: Vec3Like;
      readonly aim: Vec3Like;
      /** How many of the offered stances worked — 1 of many is a finding. */
      readonly clearStances: number;
      readonly triedStances: number;
    }
  | { readonly ok: false; readonly blockers: readonly Hitbox[]; readonly triedStances: number };

/**
 * Acquire from any stance the step allows.
 *
 * `stances` are eye positions — the arrival stance first, so a target the bot is
 * already looking at costs one ray. A failure means the target was unpickable
 * from EVERY offered stance, which is the machine statement of "the player cannot
 * click it".
 */
export function acquireFromStances(
  stances: readonly Vec3Like[],
  target: Hitbox,
  others: readonly Hitbox[],
  reach: number = INTERACTION_REACH,
): StanceVerdict {
  const blockers = new Map<number, Hitbox>();
  let first: { eye: Vec3Like; aim: Vec3Like } | null = null;
  let clear = 0;
  for (const eye of stances) {
    const a = acquire(eye, target, others, reach);
    if (a.ok) {
      clear += 1;
      first ??= { eye: a.eye, aim: a.aim };
    } else {
      for (const b of a.blockers) blockers.set(b.id, b);
    }
  }
  if (first !== null) {
    return {
      ok: true,
      eye: first.eye,
      aim: first.aim,
      clearStances: clear,
      triedStances: stances.length,
    };
  }
  return { ok: false, blockers: [...blockers.values()], triedStances: stances.length };
}

/** How a body is named in a failure: type, server name, id and where it stands. */
export function describeHitbox(h: Hitbox): string {
  const named = h.label !== undefined && h.label !== "" ? ` "${h.label}"` : "";
  const p = h.position;
  return (
    `${h.name}${named} (entity #${h.id}, ${h.width} x ${h.height} blocks) at ` +
    `[${p.x.toFixed(2)}, ${p.y.toFixed(2)}, ${p.z.toFixed(2)}]`
  );
}

/**
 * The sentence a failed acquisition prints. It names BOTH sides — the thing the
 * script meant to click and the thing the crosshair reaches instead — because
 * that pair is the whole content of the bug, and until now only a human ever
 * wrote it down.
 */
export function occlusionFailure(
  what: string,
  target: Hitbox,
  blockers: readonly Hitbox[],
  triedStances: number,
): string {
  const who = blockers.map(describeHitbox).join("; ") || "an exact ray-pick tie";
  const coincident = blockers.some(
    (b) =>
      Math.abs(b.position.x - target.position.x) < 1e-6 &&
      Math.abs(b.position.z - target.position.z) < 1e-6,
  );
  const tie = coincident
    ? " Their hitboxes are COINCIDENT, so the client's entity ray-pick is an exact tie resolved " +
      "by iteration order — which body answers the click is not decidable at all."
    : "";
  return (
    `${what}: the crosshair cannot acquire ${describeHitbox(target)} from any of the ` +
    `${triedStances} standing position(s) this step allows within ${INTERACTION_REACH} blocks. ` +
    `The entity-pick ray reaches ${who} first.${tie} A player cannot click this target, so the ` +
    `step is not performable by hand however green the rest of the ladder is. Fix the staging ` +
    `(move one of the two bodies apart) — never the check.`
  );
}
