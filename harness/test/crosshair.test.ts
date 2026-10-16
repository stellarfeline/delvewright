import { test } from "node:test";
import assert from "node:assert/strict";

import {
  INTERACTION_DEFAULT_SIZE,
  INTERACTION_REACH,
  acquire,
  acquireFromStances,
  aimPoints,
  boxOf,
  direction,
  describeHitbox,
  hitboxDims,
  interactionDims,
  occlusionFailure,
  pickEntity,
  rayBlocks,
  rayBox,
  type BlockShapes,
  type Box,
  type Hitbox,
  type Vec3Like,
} from "../src/crosshair.ts";

/** A delve NPC's clickable body: the 1.0 x 2.0 `interaction` box the compiler
 * summons on the NPC's cell. Feet position, as mineflayer reports it. */
function npcHitbox(id: number, cell: [number, number, number], label?: string): Hitbox {
  return {
    id,
    name: "interaction",
    label,
    position: { x: cell[0] + 0.5, y: cell[1], z: cell[2] + 0.5 },
    width: 1.0,
    height: 2.0,
  };
}

/** An eye at a cell centre, standing height. */
function eyeAt(cell: [number, number, number]): Vec3Like {
  return { x: cell[0] + 0.5, y: cell[1] + 1.62, z: cell[2] + 0.5 };
}

test("boxOf centres the width on the feet and rises by the height", () => {
  const b = boxOf(npcHitbox(1, [0, 64, 0]));
  assert.deepEqual(b.min, { x: 0.0, y: 64, z: 0.0 });
  assert.deepEqual(b.max, { x: 1.0, y: 66, z: 1.0 });
});

test("rayBox reports the entry distance, and misses cleanly", () => {
  const box = boxOf(npcHitbox(1, [3, 64, 0]));
  const eye = { x: 0.5, y: 65, z: 0.5 };
  const hit = rayBox(eye, { x: 1, y: 0, z: 0 }, box);
  // The 1.0-wide box on cell x=3 spans x in [3.0, 4.0]; an eye at x=0.5 meets its
  // near face after 2.5 blocks.
  assert.equal(hit, 2.5);
  assert.equal(rayBox(eye, { x: 0, y: 0, z: 1 }, box), null, "a ray down z misses it");
});

test("a ray whose origin is inside the box enters at 0, as vanilla does", () => {
  const box = boxOf(npcHitbox(1, [0, 64, 0]));
  assert.equal(rayBox({ x: 0.5, y: 65, z: 0.5 }, { x: 1, y: 0, z: 0 }, box), 0);
});

test("pickEntity returns the nearest body the ray meets", () => {
  const near = npcHitbox(1, [2, 64, 0]);
  const far = npcHitbox(2, [4, 64, 0]);
  const eye = { x: 0.5, y: 65, z: 0.5 };
  const pick = pickEntity(eye, { x: 1, y: 0, z: 0 }, [far, near], 10);
  assert.ok(pick !== null && pick.hit !== null);
  assert.equal(pick.hit.id, near.id);
  assert.equal(pick.tied.length, 0);
});

test("pickEntity honours interaction reach — nothing beyond 3.0 blocks is picked", () => {
  const far = npcHitbox(1, [5, 64, 0]);
  const eye = { x: 0.5, y: 65, z: 0.5 };
  assert.equal(pickEntity(eye, { x: 1, y: 0, z: 0 }, [far], INTERACTION_REACH), null);
  assert.notEqual(pickEntity(eye, { x: 1, y: 0, z: 0 }, [far], 10), null);
});

test("coincident boxes are reported as a TIE, not as a winner", () => {
  const a = npcHitbox(1, [3, 64, 0]);
  const b = npcHitbox(2, [3, 64, 0]);
  const pick = pickEntity({ x: 0.5, y: 65, z: 0.5 }, { x: 1, y: 0, z: 0 }, [a, b], 10);
  assert.ok(pick !== null && pick.hit !== null);
  assert.equal(pick.tied.length, 1, "the second body ties the first");
});

test("aim is sampled across the whole box, not only its centre", () => {
  const pts = aimPoints(npcHitbox(1, [0, 64, 0]));
  assert.equal(pts.length, 27);
  assert.ok(
    pts.some((p) => p.y < 65) && pts.some((p) => p.y > 65),
    "aim reaches below and above the box centre, as a player's does",
  );
});

test("direction is unit length, and undefined when the two points coincide", () => {
  const d = direction({ x: 0, y: 0, z: 0 }, { x: 3, y: 0, z: 4 });
  assert.ok(d);
  assert.ok(Math.abs(Math.hypot(d.x, d.y, d.z) - 1) < 1e-9);
  assert.equal(direction({ x: 1, y: 1, z: 1 }, { x: 1, y: 1, z: 1 }), null);
});

test("a target with nothing in front of it is acquired", () => {
  const target = npcHitbox(1, [2, 64, 0]);
  const got = acquire(eyeAt([0, 64, 0]), target, []);
  assert.equal(got.ok, true);
});

// ---------------------------------------------------------------------------
// The owner's defect, in the geometry that produced it.
// ---------------------------------------------------------------------------

test("two NPCs on ONE cell cannot be told apart from any stance", () => {
  const eurylochus = npcHitbox(1, [9, 69, -44], "Eurylochus");
  const antiphos = npcHitbox(2, [9, 69, -44], "Antiphos");
  // Every stance a talk-to step allows: the whole standable disc of radius 3.
  const stances: Vec3Like[] = [];
  for (let dx = -3; dx <= 3; dx += 1) {
    for (let dz = -3; dz <= 3; dz += 1) {
      if (dx === 0 && dz === 0) continue;
      stances.push(eyeAt([9 + dx, 69, -44 + dz]));
    }
  }
  const verdict = acquireFromStances(stances, eurylochus, [antiphos]);
  assert.equal(verdict.ok, false, "a coincident body must never be acquirable");
  assert.ok(verdict.ok === false && verdict.blockers.some((b) => b.id === antiphos.id));

  const message = occlusionFailure(
    "talk-to npc/eurylochus",
    eurylochus,
    verdict.ok === false ? verdict.blockers : [],
    stances.length,
  );
  // The sentence must name BOTH bodies — that pair is the whole content of the bug.
  assert.match(message, /Eurylochus/);
  assert.match(message, /Antiphos/);
  assert.match(message, /COINCIDENT/);
  assert.match(message, /never the check/);
});

test("one block of separation still leaves a stance that works", () => {
  const target = npcHitbox(1, [0, 64, 0], "target");
  const neighbour = npcHitbox(2, [1, 64, 0], "neighbour");
  const stances: Vec3Like[] = [];
  for (let dx = -3; dx <= 3; dx += 1) {
    for (let dz = -3; dz <= 3; dz += 1) {
      if (dx === 0 && dz === 0) continue;
      stances.push(eyeAt([dx, 64, dz]));
    }
  }
  const verdict = acquireFromStances(stances, target, [neighbour]);
  assert.equal(verdict.ok, true, "the far side of the target is clear of the neighbour");
  assert.ok(
    verdict.ok === true && verdict.clearStances < verdict.triedStances,
    "…but not from every stance: some approaches put the neighbour in the ray",
  );
});

test("a body directly between eye and target steals the click from that stance", () => {
  const target = npcHitbox(1, [3, 64, 0], "behind");
  const blocker = npcHitbox(2, [1, 64, 0], "in front");
  const got = acquire(eyeAt([-1, 64, 0]), target, [blocker]);
  assert.equal(got.ok, false);
  assert.ok(got.ok === false && got.blockers[0]?.id === blocker.id);
});

test("a target out of interaction reach is not acquirable at all", () => {
  const target = npcHitbox(1, [12, 64, 0], "far away");
  const got = acquire(eyeAt([0, 64, 0]), target, []);
  assert.equal(got.ok, false);
  assert.equal(got.ok === false && got.blockers.length, 0, "nothing blocked it — it is just far");
});

// ---------------------------------------------------------------------------
// The vacuity guard. `minecraft-data` reports `interaction` as 0 x 0 because the
// size lives in per-entity NBT, and the first cut of this module dropped every
// zero-sized body — so every affordance in the world became unmeetable, every
// acquisition reported "no target tracked", and a live ladder went green having
// proven nothing. A zero here is an ABSENCE, never a small box.
// ---------------------------------------------------------------------------

/** The registry's entity-data keys for `interaction`, 1.21.11. */
const INTERACTION_KEYS = [
  "shared_flags",
  "air_supply",
  "custom_name",
  "custom_name_visible",
  "silent",
  "no_gravity",
  "pose",
  "ticks_frozen",
  "width",
  "height",
  "response",
];

test("an interaction body the client sizes at 0 x 0 takes its size from its entity data", () => {
  const metadata: unknown[] = [];
  metadata[8] = 1.02;
  metadata[9] = 2.02;
  const data = interactionDims(metadata, INTERACTION_KEYS);
  assert.deepEqual(data, { width: 1.02, height: 2.02 });
  assert.deepEqual(hitboxDims("interaction", 0, 0, data), data);
});

test("an entity-data field the server did not send is vanilla's default, never a guess", () => {
  // The server sends only values that differ from the type's defaults.
  assert.equal(INTERACTION_DEFAULT_SIZE, 1.0);
  assert.deepEqual(interactionDims([], INTERACTION_KEYS), { width: 1.0, height: 1.0 });
  assert.deepEqual(hitboxDims("interaction", undefined, undefined), { width: 1.0, height: 1.0 });
});

test("a client-reported box is preferred, and a boxless body is dropped", () => {
  assert.deepEqual(hitboxDims("mannequin", 0.6, 1.8), { width: 0.6, height: 1.8 });
  assert.equal(hitboxDims("text_display", 0, 0), null, "a display meets no ray");
});

test("describeHitbox names type, label, id and place", () => {
  const s = describeHitbox(npcHitbox(7, [1, 2, 3], "Perimedes"));
  assert.match(s, /interaction "Perimedes" \(entity #7, 1 x 2 blocks\)/);
  assert.match(s, /\[1\.50, 2\.00, 3\.50\]/);
});

// ---------------------------------------------------------------------------
// Block occlusion. The client clips the blocks first and keeps an entity only
// when its box is STRICTLY nearer than the block hit: a box whose face lies on a
// block's face loses every ray that reaches that face. That is the defect a full
// prop under a flush `1.0`-wide box shipped with — many presses before one
// registered — and a bot that cast no block ray could never see it.
// ---------------------------------------------------------------------------

/** A world of full cubes at `cells`, air elsewhere. */
function cubes(...cells: Array<[number, number, number]>): BlockShapes {
  const solid = new Set(cells.map((c) => c.join(",")));
  return (x, y, z): readonly Box[] =>
    solid.has(`${x},${y},${z}`) ? [{ min: { x, y, z }, max: { x: x + 1, y: y + 1, z: z + 1 } }] : [];
}

/** An `interaction` box at a cell, the given size, feet on `floor`. */
function box(id: number, cell: [number, number, number], width: number, height: number, floor: number): Hitbox {
  return {
    id,
    name: "interaction",
    position: { x: cell[0] + 0.5, y: floor, z: cell[2] + 0.5 },
    width,
    height,
  };
}

/** Every eye a player could stand at around the prop at [0, 64, 0]: the ring of
 * cells one and two out, on the prop's floor. */
const RING: readonly Vec3Like[] = [
  [-1, 0], [1, 0], [0, -1], [0, 1], [-2, 0], [2, 0], [0, -2], [0, 2], [1, 1], [-1, -1],
].map(([dx, dz]) => eyeAt([dx!, 64, dz!]));

test("rayBlocks meets the first block face along the ray", () => {
  const hit = rayBlocks({ x: -1.5, y: 64.5, z: 0.5 }, { x: 1, y: 0, z: 0 }, 3, cubes([0, 64, 0]));
  assert.ok(hit !== null);
  assert.equal(hit.t, 1.5);
  assert.deepEqual(hit.cell, [0, 64, 0]);
});

test("an unknown cell is a full block: inventing air would let an occlusion pass", () => {
  const hit = rayBlocks({ x: -1.5, y: 64.5, z: 0.5 }, { x: 1, y: 0, z: 0 }, 3, () => null);
  assert.ok(hit !== null, "the unknown cell the eye stands in stops the ray");
});

test("the perturbation: a flush 1.0 x 2.0 box over a full prop is never acquired at the prop", () => {
  // The prop at [0, 64, 0] is a full cube; the box shares its four side faces and
  // its floor. Aiming at the prop's faces the block wins every tie; the box is
  // still reachable ABOVE the prop (its head half stands in air), so the target
  // is acquired — but never by a ray through the block's own faces.
  const world = cubes([0, 64, 0]);
  const flush = box(1, [0, 64, 0], 1.0, 2.0, 64);
  const eye = eyeAt([-1, 64, 0]);
  const pick = pickEntity(eye, direction(eye, { x: 0.0, y: 64.5, z: 0.5 })!, [flush], 3, world);
  assert.ok(pick !== null && pick.hit === null, "the ray at the prop's west face picks the block");
  assert.deepEqual(pick.block.cell, [0, 64, 0]);
});

test("the rule: a box one margin past the block grid wins the ray at the prop's own face", () => {
  const world = cubes([0, 64, 0]);
  const fitted = box(1, [0, 64, 0], 1.02, 2.02, 63.99);
  let rays = 0;
  for (const eye of RING) {
    for (const aim of [
      { x: 0.0, y: 64.5, z: 0.5 },
      { x: 1.0, y: 64.5, z: 0.5 },
      { x: 0.5, y: 64.5, z: 0.0 },
      { x: 0.5, y: 64.5, z: 1.0 },
      { x: 0.5, y: 65.0, z: 0.5 },
    ]) {
      const dir = direction(eye, aim)!;
      const pick = pickEntity(eye, dir, [fitted], 3, world);
      // A face pointing away from this eye is behind the prop; only a ray that
      // reaches the prop at all is a click on it.
      const block = rayBlocks(eye, dir, 3, world);
      if (block === null) continue;
      rays += 1;
      assert.ok(
        pick !== null && pick.hit !== null && pick.hit.id === fitted.id,
        `from [${eye.x}, ${eye.y}, ${eye.z}] toward [${aim.x}, ${aim.y}, ${aim.z}] the box wins`,
      );
    }
  }
  // Every one of the 50 rays reaches the prop: none was skipped.
  assert.equal(rays, RING.length * 5);
});

test("a prop whose head cell is a block too: only the margin wins, and acquisition says which block took it", () => {
  // Prop at [0, 64, 0] and an authored block in the head cell [0, 65, 0]: the
  // flush 1.0 x 2.0 box is now entirely inside blocks' faces, unacquirable from
  // every stance, and the failure names the block.
  const world = cubes([0, 64, 0], [0, 65, 0]);
  const flush = box(1, [0, 64, 0], 1.0, 2.0, 64);
  const lost = acquireFromStances(RING, flush, [], 3, world);
  assert.equal(lost.ok, false);
  if (!lost.ok) {
    assert.ok(lost.blocks.length > 0, "the blocks that took the click are named");
    const msg = occlusionFailure("interact obj/open", flush, lost.blockers, lost.triedStances, lost.blocks);
    assert.match(msg, /block block at \[0, 6[45], 0\]/);
    assert.match(msg, /strictly nearer/);
  }
  const fitted = box(1, [0, 64, 0], 1.02, 2.02, 63.99);
  const won = acquireFromStances(RING, fitted, [], 3, world);
  assert.equal(won.ok, true, "the rule's box is acquired from the ring");
  if (won.ok) assert.equal(won.clearStances, RING.length, "from every stance");
});

test("a block between the eye and the target occludes it", () => {
  const target = npcHitbox(1, [3, 64, 0]);
  const wall = cubes([1, 64, 0], [1, 65, 0], [1, 66, 0]);
  const a = acquire(eyeAt([0, 64, 0]), target, [], 3, wall);
  assert.equal(a.ok, false);
});
