import { test } from "node:test";
import assert from "node:assert/strict";

import {
  ARROW_DRAG,
  ARROW_GRAVITY,
  ARROW_LAUNCH_DROP,
  ARROW_LAUNCH_SPEED,
  arrowHeightAt,
  shotAim,
  shotOutOfReach,
  shotPitch,
  shotSpent,
} from "../src/shot.ts";

const EYE_HEIGHT = 1.62;

/** How far over the target's centre the arrow loosed toward `aim` from `eye` crosses the centre's column. */
function missAt(
  eye: readonly [number, number, number],
  target: readonly [number, number, number],
  aim: readonly [number, number, number],
): number {
  const c = [target[0] + 0.5, target[1] + 0.5, target[2] + 0.5];
  const d = Math.hypot(c[0] - eye[0], c[2] - eye[2]);
  const pitch = Math.atan2(aim[1] - eye[1], Math.hypot(aim[0] - eye[0], aim[2] - eye[2]));
  const h = arrowHeightAt(pitch, d);
  assert.ok(h !== undefined, "the flight reaches the target's column");
  return eye[1] - ARROW_LAUNCH_DROP + h - c[1];
}

test("the model reproduces the record's level shot at 27 blocks (vehicles-animals-and-targets.md §1)", () => {
  // The probe's geometry: feet at y 101, the target block at y 102, 27 east of
  // the bot's centre, its near face 26.5 out. Hits at 1.5, 2 and 2.5 over the
  // centre; misses at 0, 1, 3 and 4; the centre (signal >= 8) only at 2.
  const eyeY = 101 + EYE_HEIGHT;
  const arrive = (elev: number) => {
    const pitch = Math.atan2(102.5 + elev - eyeY, 27);
    return eyeY - ARROW_LAUNCH_DROP + arrowHeightAt(pitch, 26.5)! - 102.5;
  };
  for (const e of [1.5, 2, 2.5]) assert.ok(Math.abs(arrive(e)) < 0.5, `elev ${e} hits: ${arrive(e)}`);
  for (const e of [0, 1, 3, 4]) assert.ok(Math.abs(arrive(e)) > 0.5, `elev ${e} misses: ${arrive(e)}`);
  assert.ok(Math.abs(arrive(2)) < 0.1, `elev 2 strikes the centre: ${arrive(2)}`);
});

test("the aim's arc passes through the target's centre across rises and falls", () => {
  const eye: [number, number, number] = [132.5, 91 + EYE_HEIGHT, 223.5];
  let cases = 0;
  for (const [dx, dz] of [[4, 0], [17, 15], [27, 0], [0, 30], [28, 28]]) {
    for (const rise of [-30, -17, -5, 0, 5, 17, 25]) {
      const target: [number, number, number] = [132 + dx, 91 + rise, 223 + dz];
      const aim = shotAim(eye, target);
      assert.ok(aim !== undefined, `(${dx}, ${rise}, ${dz}) is in reach`);
      const miss = missAt(eye, target, aim);
      assert.ok(Math.abs(miss) < 0.01, `(${dx}, ${rise}, ${dz}): the arc passes ${miss} off the centre`);
      cases += 1;
    }
  }
  assert.equal(cases, 35);
});

/**
 * The first box a full-drawn arrow loosed from `eye` toward `aim` passes
 * through, walked in twentieths of a tick along the model's flight.
 */
function firstStruck(
  eye: readonly [number, number, number],
  aim: readonly [number, number, number],
  boxes: Record<string, readonly [readonly [number, number, number], readonly [number, number, number]]>,
): string | undefined {
  const h = Math.hypot(aim[0] - eye[0], aim[2] - eye[2]);
  const pitch = Math.atan2(aim[1] - eye[1], h);
  const [ux, uz] = [(aim[0] - eye[0]) / h, (aim[2] - eye[2]) / h];
  let v = [ARROW_LAUNCH_SPEED * Math.cos(pitch) * ux, ARROW_LAUNCH_SPEED * Math.sin(pitch), ARROW_LAUNCH_SPEED * Math.cos(pitch) * uz];
  let p = [eye[0], eye[1] - ARROW_LAUNCH_DROP, eye[2]];
  for (let n = 0; n < 60; n++) {
    for (let s = 0; s < 20; s++) {
      const q = [0, 1, 2].map((i) => p[i] + (v[i] * s) / 20);
      for (const [name, [lo, hi]] of Object.entries(boxes)) {
        if ([0, 1, 2].every((i) => q[i] >= lo[i] && q[i] < hi[i])) return name;
      }
    }
    p = [0, 1, 2].map((i) => p[i] + v[i]);
    v = [v[0] * ARROW_DRAG, v[1] * ARROW_DRAG - ARROW_GRAVITY, v[2] * ARROW_DRAG];
  }
  return undefined;
}

test("the town's belfry shot: the arc strikes the target where a table scaled from level shots strikes the slab under it", () => {
  // Stance [132, 91, 223], target [149, 108, 238] over a top slab at
  // [149, 107, 238] (the town's built world): 17 up over about 23 across.
  const eye: [number, number, number] = [132.5, 91 + EYE_HEIGHT, 223.5];
  const target: [number, number, number] = [149, 108, 238];
  const boxes = {
    target: [[149, 108, 238], [150, 109, 239]],
    slab: [[149, 107.5, 238], [150, 108, 239]],
  } as const;
  const aim = shotAim(eye, target)!;
  assert.ok(Math.abs(missAt(eye, target, aim)) < 0.01);
  assert.equal(firstStruck(eye, aim, boxes), "target");
  const c = [149.5, 108.5, 238.5];
  const d = Math.hypot(c[0] - eye[0], c[2] - eye[2]);
  for (const e of [2.0, 1.5, 2.5]) {
    const table: [number, number, number] = [c[0], c[1] + (e * d) / 27, c[2]];
    assert.equal(firstStruck(eye, table, boxes), "slab", `the table's ${e}`);
  }
});

test("every point the compiler may place a stance from (40 blocks, any elevation) is in reach", () => {
  for (let deg = -89; deg <= 89; deg += 1) {
    const a = (deg * Math.PI) / 180;
    assert.ok(shotPitch(40 * Math.cos(a), 40 * Math.sin(a)) !== undefined, `${deg} degrees`);
  }
});

test("a point no full draw reaches has no aim, and the failures name the step", () => {
  assert.equal(shotPitch(10, 200), undefined);
  assert.equal(shotAim([0.5, 65.62, 0.5], [3, 400, 0]), undefined);
  assert.equal(shotAim([0.5, 65.62, 0.5], [40, 300, 0]), undefined);
  assert.match(shotOutOfReach("trigger t (shot)"), /^trigger t \(shot\): no full-drawn arrow's arc reaches/);
  assert.match(shotSpent("trigger t (shot)", 8), /no arrow of 8 landed on the target/);
});
