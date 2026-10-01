// The death loop's pure half: the contract parser, the forfeit rule
// re-derived from spec-0032's text, the seat/row lookups, and the verdicts.
//
// Every assertion here is about a PROMISE the campaign made. Nothing consults the
// emitter, which is the point: an assertion written by reading the emitter cannot
// fail when the emitter is wrong, and the live run that motivated this module
// found `on_death` firing nothing at all on a player's first death.

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  DeathPlanParseError,
  SUPPORTED_DEATH_PLAN_FORMAT,
  bodyInVolume,
  PLAYER_HEIGHT,
  PLAYER_WIDTH,
  boxCells,
  deathLoopBinding,
  deathLoopBindingFailures,
  deathLoopStage,
  datumsPromised,
  dropOf,
  entryCellOf,
  gateVerdict,
  nearLip,
  termClause,
  markerAt,
  markersAt,
  expectedForfeit,
  inBox,
  lethalTrialFailures,
  openLethalTrial,
  openWager,
  promisedForfeit,
  overFootprint,
  parseDeathPlan,
  stagedBalance,
  wayInCandidates,
  SINK_BLOCKS_PER_TICK,
  sinkBudgetMs,
  volumeReachesCell,
  LETHAL_STEP_COST,
  lethalStepCost,
  seatAtRespawn,
  stakesDropped,
  tableAnchor,
  type DeathPlan,
  type GateTerm,
  type LethalTrial,
  type LethalVolume,
  type StakeRule,
} from "../src/death-loop.ts";
import type { Vec3Tuple } from "../src/critical-path.ts";
import { createRequire } from "node:module";

/** The economy fixture's plan, as `delvec` really emits it. */
function planDoc(): Record<string, unknown> {
  return {
    campaign_id: "economy",
    version: "0.2.0",
    format_version: SUPPORTED_DEATH_PLAN_FORMAT,
    lethal_volumes: [
      {
        id: "lethal/the-drop",
        region: { lo: [5, 65, 8], hi: [5, 65, 8] },
        keep_out: { lo: [4, 64, 7], hi: [6, 65, 9] },
        message: "The stone floor gives way beneath you.",
        message_key: "lethal.the-drop.message",
        damage_type: "minecraft:fall",
      },
    ],
    on_death: {
      effects: 1,
      drops_stake: [{ stake: "stake/embers", gates: [{ terms: [] }] }],
    },
    stakes: [
      {
        id: "stake/embers",
        currency: {
          state: "state/embers",
          objective: "dw.s_embers",
          initial: 5,
          scope: "player",
          name: "Embers",
          name_key: "state.embers.name",
        },
        forfeit: { kind: "all" },
        max_live: 1,
        on_full: "replace",
        collect_by: "owner",
        collected_message: "You take back what the drop took.",
        collected_message_key: "stake.embers.collected",
        marker_item: "minecraft:soul_lantern",
      },
    ],
    placement: {
      seats: [
        { cp: -1, label: "the campaign's entry spawn", cell: [5, 65, 2] },
        { cp: 0, label: "checkpoint anchor `anchor/keeper-stand`", cell: [5, 65, 4] },
      ],
      regions: [
        {
          label: "lethal volume `lethal/the-drop`",
          lethal: true,
          volume: "lethal/the-drop",
          region: { lo: [5, 65, 8], hi: [5, 65, 8] },
        },
        {
          label: "the runtime-mutable ground of gate anchor `anchor/door`",
          lethal: false,
          volume: null,
          region: { lo: [4, 65, 6], hi: [5, 67, 6] },
        },
      ],
      rows: [
        { seat: 0, region: 0, anchor: [4, 65, 8] },
        { seat: 0, region: 1, anchor: [4, 65, 5] },
        { seat: 1, region: 0, anchor: [4, 65, 8] },
        { seat: 1, region: 1, anchor: [4, 65, 5] },
      ],
    },
    binding: {
      lethal_volumes: 1,
      on_death_effects: 1,
      stakes: 1,
      respawn_seats: 2,
      placement_rows: 4,
      unbound: false,
      reason: null,
    },
  };
}

function plan(): DeathPlan {
  return parseDeathPlan(planDoc());
}

const VOLUME: LethalVolume = {
  id: "lethal/the-drop",
  region: { lo: [5, 65, 8], hi: [5, 65, 8] },
  keepOut: { lo: [4, 64, 7], hi: [6, 65, 9] },
  message: "The stone floor gives way beneath you.",
  messageKey: "lethal.the-drop.message",
  damageType: "minecraft:fall",
};

function stakeRule(over: Partial<StakeRule> = {}): StakeRule {
  return {
    id: "stake/embers",
    currency: {
      state: "state/embers",
      objective: "dw.s_embers",
      initial: 5,
      scope: "player",
      name: "Embers",
      nameKey: "state.embers.name",
    },
    forfeit: { kind: "all" },
    maxLive: 1,
    onFull: "replace",
    collectBy: "owner",
    collectedMessage: "You take back what the drop took.",
    markerItem: "minecraft:soul_lantern",
    ...over,
  };
}

/** A second stake the same death drops — a death that takes two things. */
function relicsRule(): StakeRule {
  return stakeRule({
    id: "stake/relics",
    currency: {
      state: "state/relics",
      objective: "dw.s_relics",
      initial: 3,
      scope: "player",
      name: "Relics",
      nameKey: "state.relics.name",
    },
    forfeit: { kind: "fixed", amount: 2 },
  });
}

/** A trial in which everything the campaign promised actually happened. */
function goodTrial(stakes: readonly StakeRule[] = [stakeRule()]): LethalTrial {
  const t = openLethalTrial(VOLUME, [5, 65, 8], stakes);
  t.enteredVolume = true;
  t.died = true;
  t.deathPos = [5, 65, 8];
  t.wordingSeen = true;
  for (const w of t.wagers) {
    const before = w.stake === "stake/relics" ? 3 : 5;
    w.balanceBefore = before;
    w.expectedForfeit = expectedForfeit(w.forfeit, before);
    w.balanceAfterDeath = before - w.expectedForfeit;
    w.balanceAfterCollect = before;
  }
  t.respawnPos = [5.5, 65, 4.5];
  t.respawnSeat = "checkpoint anchor `anchor/keeper-stand`";
  t.expectedAnchor = [4, 65, 8];
  t.markerPos = [4.5, 65, 8.5];
  t.markersFound = 1;
  t.walkedBack = true;
  t.collectClicks = 2;
  t.markerRetired = true;
  return t;
}

/** The wager for `stake`, in a trial that carries several. */
function wager(t: LethalTrial, stake: string) {
  const w = t.wagers.find((x) => x.stake === stake);
  assert.ok(w, `the trial carries a wager for ${stake}`);
  return w;
}

// --- the contract ----------------------------------------------------------

test("the emitted plan parses, and every declaration survives the round trip", () => {
  const p = plan();
  assert.equal(p.campaignId, "economy");
  assert.equal(p.volumes.length, 1);
  assert.equal(p.volumes[0]!.message, "The stone floor gives way beneath you.");
  assert.deepEqual(p.dropsStake, [{ stake: "stake/embers", gates: [{ terms: [] }] }]);
  assert.deepEqual(p.stakes[0]!.forfeit, { kind: "all" });
  assert.equal(p.stakes[0]!.currency.objective, "dw.s_embers");
  assert.equal(p.binding.unbound, false);
});

test("a volume's keep-out box is READ, and it is not the volume", () => {
  const p = plan();
  const v = p.volumes[0]!;
  // The compiler's answer, carried whole. The bot never derives it: the rule
  // that a body one cell out from a face is inside the volume lives in the
  // engine, and a second copy of it here is a copy no Rust test reaches.
  assert.deepEqual(v.keepOut, { lo: [4, 64, 7], hi: [6, 65, 9] });
  assert.notDeepEqual(v.keepOut, v.region);
  // …and the difference is exactly the class of death the old rule lost. A body
  // standing one cell east of this one-cell volume is killed by it and is not in
  // it, so the cell test says "outside" and the keep-out test says "this
  // volume".
  const besideTheFace: readonly [number, number, number] = [6, 65, 8];
  assert.equal(inBox(besideTheFace, v.region), false);
  assert.equal(inBox(besideTheFace, v.keepOut), true);
});

test("`bodyInVolume` and the compiler's exported `keep_out` are the same rule", () => {
  // **Two implementations of one fact, in two languages, and this is what holds
  // them together.** `bodyInVolume` is the server's rule re-derived here, over an
  // exact position; `keep_out` is the compiler's answer to the cell question —
  // which FEET CELLS a body can meet the volume from — computed by
  // `dsl::metrics::keep_out_box` and carried in the plan. Neither can replace the
  // other (one takes a position, one takes a cell), so the honest thing is to
  // make them provably agree rather than let them coexist.
  //
  // The bridge: a cell belongs in `keep_out` exactly when SOME position inside it
  // satisfies `bodyInVolume`. Sampled at the cell's own interior corners, which
  // is where the predicate's extremes are — it is monotone in each span.
  const v = plan().volumes[0]!;
  const d = 1e-6;
  const reachable = (cell: Vec3Tuple): boolean => {
    for (const dx of [d, 1 - d]) {
      for (const dz of [d, 1 - d]) {
        if (bodyInVolume([cell[0] + dx, cell[1], cell[2] + dz], v.region)) return true;
      }
    }
    return false;
  };
  let examined = 0;
  const disagreed: string[] = [];
  for (let x = v.region.lo[0] - 3; x <= v.region.hi[0] + 3; x++) {
    for (let y = v.region.lo[1] - 3; y <= v.region.hi[1] + 3; y++) {
      for (let z = v.region.lo[2] - 3; z <= v.region.hi[2] + 3; z++) {
        const cell: Vec3Tuple = [x, y, z];
        examined += 1;
        if (reachable(cell) !== inBox(cell, v.keepOut)) disagreed.push(`[${cell.join(", ")}]`);
      }
    }
  }
  // Binding, computed from the objects rather than written beside them: the
  // volume's box grown by three on every side.
  assert.equal(examined, 7 * 7 * 7);
  // **The one boundary they read differently, measured rather than predicted.**
  // Every disagreement sits on a single plane — feet exactly on the volume's
  // ceiling, `hi.y + 1` — and there are nine of them, the whole horizontal ring
  // at that height. One cause: `bodyInVolume` compares `min <= hi + 1`
  // NON-strictly, so a body whose feet touch the ceiling counts as intersecting,
  // while vanilla's own `AABB::intersects` is strict and `keep_out_box` takes
  // that reading.
  //
  // It is a difference of DIRECTION and each side is pointed the safe way for
  // what it decides. `keep_out` decides FOOTING, where a generous rule would
  // refuse ground that is fine, so it is strict. `bodyInVolume` decides CREDIT,
  // where generous can only fail to disown a real death and can never invent one
  // out of a body that is not there. The residue is named rather than smoothed
  // over: on this plane the credit rule would attribute to the volume a death
  // suffered by a body standing on top of it.
  //
  // Asserted as the PROPERTY and not as a list of cells, so it stays true for a
  // volume of another shape — and with a non-zero count, so it cannot go quietly
  // vacuous if one side stops answering.
  const ceiling = v.region.hi[1]! + 1;
  assert.equal(disagreed.length, 9, `disagreements: ${disagreed.join(" ")}`);
  assert.deepEqual(
    disagreed.filter((c) => !c.startsWith(`[`) || !c.includes(`, ${ceiling}, `)),
    [],
    "every cell the two readings differ on has its feet exactly on the volume's ceiling",
  );
});

test("a plan that omits keep_out is REFUSED — the bot may not guess the ring", () => {
  const doc = planDoc();
  const volumes = doc["lethal_volumes"] as Record<string, unknown>[];
  delete volumes[0]!["keep_out"];
  assert.throws(() => parseDeathPlan(doc), (e: unknown) => {
    assert.ok(e instanceof DeathPlanParseError);
    assert.equal(e.pointer, "/lethal_volumes/0/keep_out");
    return true;
  });
});

test("a plan from a newer contract is REFUSED, never half-read", () => {
  const doc = planDoc();
  doc["format_version"] = SUPPORTED_DEATH_PLAN_FORMAT + 1;
  assert.throws(() => parseDeathPlan(doc), (e: unknown) => {
    assert.ok(e instanceof DeathPlanParseError);
    assert.match(e.message, /never made/);
    return true;
  });
});

test("a forfeit rule this harness cannot compute is refused rather than skipped", () => {
  const doc = planDoc();
  (doc["stakes"] as Record<string, unknown>[])[0]!["forfeit"] = { kind: "half-on-tuesdays" };
  assert.throws(() => parseDeathPlan(doc), DeathPlanParseError);
});

test("a binding that disagrees with its own counts is refused", () => {
  const doc = planDoc();
  (doc["binding"] as Record<string, unknown>)["unbound"] = true;
  assert.throws(() => parseDeathPlan(doc), (e: unknown) => {
    assert.ok(e instanceof DeathPlanParseError);
    assert.match(e.message, /must be exactly/);
    return true;
  });
});

test("an unbound plan must say why — a zero binding is a finding, not a silence", () => {
  const doc = planDoc();
  doc["lethal_volumes"] = [];
  doc["binding"] = { ...(doc["binding"] as object), lethal_volumes: 0, unbound: true, reason: null };
  assert.throws(() => parseDeathPlan(doc), /must state why/);
});

// --- the forfeit rule, re-derived from spec-0032's text ---------------------

test("`all` takes the whole purse and `none` takes nothing", () => {
  assert.equal(expectedForfeit({ kind: "all" }, 7), 7);
  assert.equal(expectedForfeit({ kind: "none" }, 7), 0);
});

test("a proportion rounds TOWARD ZERO, as integer arithmetic (ADR-0006)", () => {
  assert.equal(expectedForfeit({ kind: "proportion", percent: 30 }, 7), 2, "2.1 → 2");
  assert.equal(expectedForfeit({ kind: "proportion", percent: 99 }, 1), 0, "0.99 → 0");
  assert.equal(expectedForfeit({ kind: "proportion", percent: 100 }, 7), 7);
});

test("a fixed forfeit is CAPPED at the balance, so a purse can never go negative", () => {
  assert.equal(expectedForfeit({ kind: "fixed", amount: 3 }, 7), 3);
  assert.equal(expectedForfeit({ kind: "fixed", amount: 30 }, 7), 7);
});

test("a negative balance forfeits nothing — a death must never HAND a player money", () => {
  for (const rule of [
    { kind: "all" },
    { kind: "proportion", percent: 50 },
    { kind: "fixed", amount: 4 },
  ] as const) {
    assert.equal(expectedForfeit(rule, -5), 0, JSON.stringify(rule));
  }
});

// --- geometry and the table ------------------------------------------------

test("box membership and enumeration agree", () => {
  const box = { lo: [0, 0, 0] as const, hi: [1, 0, 1] as const };
  assert.equal(boxCells(box).length, 4);
  assert.ok(inBox([1, 0, 1], box));
  assert.ok(!inBox([2, 0, 1], box));
});

// The gallery's west pit, exactly as `delvec` emits it — the volume that measured
// this rule live.
const WEST_PIT = { lo: [1, 63, 2] as const, hi: [3, 67, 4] as const };

test("a lethal volume reaches a body its declared CELL box does not contain", () => {
  // `@a[x=1,dx=2,y=63,dy=4,z=2,dz=2]`: `dx` is a span, so the region is
  // [1,4] x [63,68] x [2,5] in continuous coordinates, and vanilla intersects a
  // 0.6-wide hitbox against it. A body at z = 5.1 stands in cell 5 — outside the
  // declared box — with its hitbox reaching back to 4.8, so the selector matches
  // it and the volume kills it.
  assert.ok(!inBox([3, 65, 5], WEST_PIT), "cell 5 is outside the declared box");
  assert.ok(bodyInVolume([3.5, 65, 5.1], WEST_PIT), "and the volume kills a body standing there");
  // The same body a third of a block further out is beyond the reach, and saying
  // so is what stops this crediting a death the volume had nothing to do with.
  assert.ok(!bodyInVolume([3.5, 65, 5.4], WEST_PIT));
});

test("the reach is the hitbox, on every axis and in both directions", () => {
  // -x/-z: the hitbox leads by half a width.
  assert.ok(bodyInVolume([0.75, 65, 3.5], WEST_PIT));
  assert.ok(!bodyInVolume([0.65, 65, 3.5], WEST_PIT));
  // -y: a body standing two courses under the floor of the box still has 1.8
  // blocks of head in it.
  assert.ok(bodyInVolume([2.5, 61.5, 3.5], WEST_PIT), "a head inside the box is a body inside it");
  assert.ok(!bodyInVolume([2.5, 61.0, 3.5], WEST_PIT));
  // +y: the region's ceiling is `hi + 1`, so feet on it are still in it.
  assert.ok(bodyInVolume([2.5, 68, 3.5], WEST_PIT));
  assert.ok(!bodyInVolume([2.5, 68.01, 3.5], WEST_PIT));
});

test("a body at the centre of any cell of the box is one the volume kills", () => {
  for (const c of boxCells(WEST_PIT)) {
    assert.ok(
      bodyInVolume([c[0] + 0.5, c[1], c[2] + 0.5], WEST_PIT),
      `the volume reaches a body standing at the centre of [${c.join(", ")}]`,
    );
  }
  assert.equal(boxCells(WEST_PIT).length, 45, "45 cells examined, not a subset of them");
});

/**
 * **One rule, two languages, and only one of them had a proof.**
 *
 * `volumeReachesCell` here and
 * `delvewright_dsl::metrics::selector_reaches_body_in_cell` in the compiler are
 * the SAME rule — which cells a lethal volume's selector can kill a standing body
 * in. They have to be: the compiler uses it to pick the cell it puts a recovery
 * stake's anchor on, and the harness uses it to decide which cells the bot may
 * walk through to reach that anchor. If they part company by one cell of shell,
 * the compiler anchors a stake in a cell the harness routes the bot straight
 * into, and the delve kills the player at the very place it invited them back to.
 *
 * They are also written differently, which is why agreeing by inspection is not
 * enough. The Rust side sweeps the cell's whole extent — `[cell - half,
 * cell + 1 + half]` — and asks whether that intersects the region. This side
 * clamps to the single position inside the cell nearest the volume on each axis
 * and asks {@link bodyInVolume} once, which is sound only because the volume is
 * an interval per axis and `bodyInVolume` is monotone in the position. Two
 * arguments, one answer, and each side's doc comment asserted the other's
 * agreement with nothing checking it.
 *
 * `crates/dsl/tests/metrics.rs::the_cell_rule_agrees_with_a_swept_body_box`
 * sweeps this exact box over this exact grid and states 441 examined / 175
 * reached. This is the mirror of it, over the same box and the same grid, with
 * the same two numbers — so the two implementations are pinned to one measurement
 * rather than to one another's prose, and either drifting reds on its own side.
 */
test("volumeReachesCell agrees with a swept body box, and with the compiler's count", () => {
  // Every body position the cell can hold, at 1/20 of a block — the thing the
  // clamp is a shortcut FOR, written out rather than reused.
  const swept = (c: readonly [number, number, number]): boolean => {
    const half = PLAYER_WIDTH / 2;
    for (let i = 0; i <= 20; i++) {
      for (let k = 0; k <= 20; k++) {
        const px = c[0] + i / 20;
        const pz = c[2] + k / 20;
        const lo = [px - half, c[1], pz - half];
        const hi = [px + half, c[1] + PLAYER_HEIGHT, pz + half];
        if ([0, 1, 2].every((a) => lo[a]! <= WEST_PIT.hi[a]! + 1 && hi[a]! >= WEST_PIT.lo[a]!)) {
          return true;
        }
      }
    }
    return false;
  };
  let examined = 0;
  let reached = 0;
  for (let x = -1; x <= 5; x++) {
    for (let y = 61; y <= 69; y++) {
      for (let z = 0; z <= 6; z++) {
        const c = [x, y, z] as const;
        assert.equal(volumeReachesCell(c, WEST_PIT), swept(c), `cell [${c.join(", ")}]`);
        examined += 1;
        if (swept(c)) reached += 1;
      }
    }
  }
  assert.equal(examined, 7 * 9 * 7, "441 cells examined, not a subset of them");
  // The box is 3x5x3 and the reach is one cell of shell on every axis: 5x7x5.
  // The compiler's own test states this same 175 over this same box; a rule that
  // widened or narrowed on either side parts from the other here.
  assert.equal(reached, 5 * 7 * 5);
  // …and the reading this replaces — cell containment, which is what
  // `applyLethalExclusion` and `choose_anchor` both used — covers 3x5x3, so a
  // collapse back to it loses 130 of the 175.
  assert.equal(boxCells(WEST_PIT).length, 3 * 5 * 3);
});

/**
 * **The navigator and the server must agree on which cells kill.**
 *
 * The pathfinder's lethal exclusion asked `inBox`, so it kept the bot out of the
 * cells inside a volume and left the shell of cells the volume can still reach a
 * body in wide open. The gallery measured it three runs out of three: the
 * east-pit trial opened with the bot parked at the west pit's own stake anchor
 * `[1, 65, 5]` and it was killed at `[3.85, 65.00, 5.14]`, `[3.70, 65.00, 5.29]`
 * and `[3.63, 65.00, 5.30]` — all cell `[3, 65, 5]`, all outside the declared
 * box, all inside the reach — and the east pit was reported unexercised every
 * time.
 */
test("the pathfinder's exclusion covers every cell the volume can kill in", () => {
  // The cell the runs died in. Outside the box; inside the reach.
  assert.ok(!inBox([3, 65, 5], WEST_PIT), "the box does not contain it");
  assert.ok(volumeReachesCell([3, 65, 5], WEST_PIT), "and the selector reaches it anyway");
  // The stake anchor `delvec` used to choose, for the same reason.
  assert.ok(volumeReachesCell([1, 65, 5], WEST_PIT));
  // Every cell of the box, and one shell around it on every axis, is excluded —
  // and nothing beyond that, so the detour this costs is exactly one cell.
  for (const c of boxCells(WEST_PIT)) {
    assert.ok(volumeReachesCell(c, WEST_PIT), `[${c.join(", ")}] is in the box`);
  }
  assert.ok(volumeReachesCell([0, 65, 1], WEST_PIT), "the -x/-z corner of the shell");
  assert.ok(volumeReachesCell([4, 65, 5], WEST_PIT), "the +x/+z corner of the shell");
  assert.ok(volumeReachesCell([2, 62, 3], WEST_PIT), "a course below: the head is inside");
  assert.ok(volumeReachesCell([2, 68, 3], WEST_PIT), "a course above: the feet are on the ceiling");
  assert.ok(!volumeReachesCell([5, 65, 5], WEST_PIT), "two out is clear");
  assert.ok(!volumeReachesCell([2, 65, 6], WEST_PIT), "two out is clear");
  assert.ok(!volumeReachesCell([2, 61, 3], WEST_PIT), "two below is clear");
  assert.ok(!volumeReachesCell([2, 69, 3], WEST_PIT), "two above is clear");
});

test("the cell rule is the body rule, asked of the nearest body the cell can hold", () => {
  // Not a second reading of the server: every answer above is `bodyInVolume` at
  // the position inside the cell that comes closest to the volume. Checked here
  // by sweeping the cell by hand and comparing, so the two cannot drift.
  const cells: Vec3Tuple[] = [];
  for (let x = -1; x <= 5; x++) for (let z = 0; z <= 6; z++) cells.push([x, 65, z]);
  let reached = 0;
  for (const c of cells) {
    let any = false;
    for (let dx = 0; dx <= 1; dx += 0.05) {
      for (let dz = 0; dz <= 1; dz += 0.05) {
        if (bodyInVolume([c[0] + dx, c[1], c[2] + dz], WEST_PIT)) any = true;
      }
    }
    assert.equal(volumeReachesCell(c, WEST_PIT), any, `[${c.join(", ")}]`);
    if (any) reached += 1;
  }
  assert.equal(cells.length, 49, "49 cells swept, not a subset of them");
  assert.equal(reached, 25, "5x5 of them — the 3x3 box plus one cell of shell");
});

test("the entry cell is the nearest cell of the box, ties broken lexicographically", () => {
  const box = { lo: [0, 0, 0] as const, hi: [2, 0, 0] as const };
  assert.deepEqual(entryCellOf(box, [5, 0, 0]), [2, 0, 0]);
  assert.deepEqual(entryCellOf(box, [1, 0, 5]), [1, 0, 0]);
  // Equidistant from [0,0,0] and [2,0,0] → the lexicographically first wins.
  assert.deepEqual(entryCellOf(box, [1, 0, 0]), [1, 0, 0]);
});

test("the entry cell is one a BODY can be in — a box corner filled by a block is not one", () => {
  // The gallery's east pit declares [21,63,20]..[23,67,24]; a 4x4x2 structure
  // stands in [21,65,20] and [21,66,20], so that corner — the cell nearest every
  // approach from the west — is the one cell of the seventy-five no player can
  // occupy. Chosen, the walk in drives at a wall until its deadline.
  const box = { lo: [21, 65, 20] as const, hi: [21, 65, 22] as const };
  const solid = (c: readonly number[]): boolean => !(c[0] === 21 && c[1] === 65 && c[2] === 20);
  assert.deepEqual(entryCellOf(box, [16, 65, 19]), [21, 65, 20], "nearest, with no world to read");
  assert.deepEqual(
    entryCellOf(box, [16, 65, 19], (c) => solid(c)),
    [21, 65, 21],
    "the nearest cell a body can be in, once the world is readable",
  );
  assert.equal(
    entryCellOf(box, [16, 65, 19], () => false),
    undefined,
    "a volume no body can be inside has no entry cell, and that is a finding rather than a guess",
  );
});

// --- which body is the stake -----------------------------------------------

const body = (name: string, x: number, y: number, z: number) => ({
  name,
  position: { x, y, z },
});

test("the stake is the interaction NEAREST the anchor, not the first one the map yields", () => {
  // A stray interaction inside the search radius, offered first. Taking it makes
  // the reported drift a fact about entity-map iteration order: the gallery's
  // west-pit stake was reported 3.6 blocks off an anchor it was standing exactly
  // on, measured at [7.5, 65.0, 18.5] over rcon on four consecutive deaths.
  const anchor = [7, 65, 18] as const;
  const stray = body("interaction", 10.6, 65, 20.4);
  const stake = body("interaction", 7.5, 65, 18.5);
  const chosen = markerAt(
    [stray, stake],
    [body("item_display", 10.6, 65, 20.4), body("item_display", 7.5, 65, 18.5)],
    anchor,
    4,
    0.5,
  );
  assert.deepEqual(chosen, stake);
});

test("a display somewhere in the radius does not vouch for an interaction elsewhere in it", () => {
  const anchor = [7, 65, 18] as const;
  const lone = body("interaction", 7.5, 65, 18.5);
  assert.equal(
    markerAt([lone], [body("item_display", 10.6, 65, 20.4)], anchor, 4, 0.5),
    undefined,
    "the two halves are summoned at one position by one function; anything else is a " +
      "different object vouching for this one",
  );
  assert.deepEqual(
    markerAt([lone], [body("item_display", 7.5, 65, 18.5)], anchor, 4, 0.5),
    lone,
  );
});

test("nothing outside the search radius is the stake, however well paired", () => {
  const far = body("interaction", 20.5, 65, 18.5);
  assert.equal(markerAt([far], [body("item_display", 20.5, 65, 18.5)], [7, 65, 18], 4, 0.5), undefined);
});

test("the respawn seat is identified from the OBSERVED position, not from engine state", () => {
  const p = plan();
  // Vanilla lands a respawning player at cell + (0.5, 0.1, 0.5).
  assert.equal(seatAtRespawn(p.seats, [5.5, 65.1, 4.5]), 1);
  assert.equal(seatAtRespawn(p.seats, [5.5, 65.1, 2.5]), 0);
  assert.equal(
    seatAtRespawn(p.seats, [40, 65, 40]),
    undefined,
    "a player who came back somewhere the campaign never declared matches NO seat — " +
      "which is itself the finding",
  );
});

test("the placement table answers per (seat, volume), and says nothing it was not asked", () => {
  const p = plan();
  assert.deepEqual(tableAnchor(p, 1, "lethal/the-drop"), [4, 65, 8]);
  assert.equal(tableAnchor(p, 1, "lethal/nowhere"), undefined);
});

// --- the verdicts ----------------------------------------------------------

test("a loop in which every promise was kept produces no failures", () => {
  assert.deepEqual(lethalTrialFailures(goodTrial()), []);
});

test("standing in a lethal volume and surviving is the first and loudest failure", () => {
  const t = goodTrial();
  t.died = false;
  const out = lethalTrialFailures(t);
  assert.equal(out.length, 1, "nothing downstream of the death edge is even reported");
  assert.match(out[0]!, /did NOT die/);
});

test("a trial that never got the bot inside says THAT, and never that it stood there", () => {
  // The gallery's east pit reported `the bot stood inside the declared lethal
  // volume at [21, 65, 20] and did NOT die` over a cell filled by a block. The
  // volume kills a real player at every one of the seventy-five cells of that
  // box, measured live; what the run had established was that a walk did not
  // arrive, and the verdict said something else entirely.
  const t = goodTrial();
  t.died = false;
  t.enteredVolume = false;
  const out = lethalTrialFailures(t);
  assert.equal(out.length, 1);
  assert.doesNotMatch(
    out[0]!,
    /stood inside/,
    "a verdict may not assert a position the trial never observed the bot at",
  );
  assert.match(out[0]!, /never OBSERVED inside/);
  assert.match(out[0]!, /the fault is the walk in, not the volume/);
});

test("a death with the volume's own wording withheld is a failure", () => {
  const t = goodTrial();
  t.wordingSeen = false;
  assert.match(lethalTrialFailures(t).join("\n"), /never reached them/);
});

test("the forfeit is judged against the DECLARED rule, and names all three numbers", () => {
  const t = goodTrial();
  wager(t, "stake/embers").balanceAfterDeath = 5; // the engine took nothing
  const out = lethalTrialFailures(t).join("\n");
  assert.match(out, /was 5 before and 5 after/);
  assert.match(out, /should be 0/);
});

test("a respawn at no declared seat is a failure, and says why it matters", () => {
  const t = goodTrial();
  t.respawnSeat = undefined;
  t.respawnPos = [0.5, 65, 0.5];
  assert.match(lethalTrialFailures(t).join("\n"), /not at any respawn seat/);
});

test("a missing stake at the table's anchor is a failure", () => {
  const t = goodTrial();
  t.markerPos = undefined;
  assert.match(lethalTrialFailures(t).join("\n"), /no recovery stake stands at \[4, 65, 8\]/);
});

test("a stake standing somewhere other than the proven anchor is a failure", () => {
  const t = goodTrial();
  t.markerPos = [9.5, 65, 9.5];
  assert.match(lethalTrialFailures(t).join("\n"), /blocks from \[4, 65, 8\]/);
});

test("a double click that credits the purse twice is caught by the amount, not by a race", () => {
  const t = goodTrial();
  wager(t, "stake/embers").balanceAfterCollect = 10; // both clicks paid out
  const out = lethalTrialFailures(t).join("\n");
  assert.match(out, /not idempotent/);
});

test("a stake that short-changes the player is caught by the same clause", () => {
  const t = goodTrial();
  wager(t, "stake/embers").balanceAfterCollect = 3;
  assert.match(lethalTrialFailures(t).join("\n"), /short-changed/);
});

test("a collected stake whose hardware still stands is a failure", () => {
  const t = goodTrial();
  t.markerRetired = false;
  assert.match(lethalTrialFailures(t).join("\n"), /still standing/);
});

// --- one death, every datum, one place --------------------------------------

test("a death that forfeits two datums is asserted on BOTH, never on the first declared", () => {
  // The defect this replaced: `plan.stakes.find(s => dropsStake.includes(s.id))`
  // asserted the first stake the plan happened to list and reported on the whole
  // death. On a campaign whose death forfeits four datums that is a quarter of the
  // promise, and WHICH quarter is not decidable from the campaign at all.
  const t = goodTrial([stakeRule(), relicsRule()]);
  assert.deepEqual(
    t.wagers.map((w) => w.stake),
    ["stake/embers", "stake/relics"],
    "one wager per stake the death drops, in the plan's own order",
  );
  assert.deepEqual(lethalTrialFailures(t), [], "both promises kept is no finding");

  // The second datum's forfeit is judged, not merely carried.
  const wrong = goodTrial([stakeRule(), relicsRule()]);
  wager(wrong, "stake/relics").balanceAfterDeath = 3; // fixed: 2 → should be 1
  const out = lethalTrialFailures(wrong).join("\n");
  assert.match(out, /wrong amount for `stake\/relics`/);
  assert.match(out, /dw\.s_relics/);
  assert.doesNotMatch(out, /stake\/embers/, "the first datum was correct and is not accused");

  // …and so is the second datum's restoration: one press at one place gives back
  // every datum that death forfeited there.
  const short = goodTrial([stakeRule(), relicsRule()]);
  wager(short, "stake/relics").balanceAfterCollect = 1;
  const back = lethalTrialFailures(short).join("\n");
  assert.match(back, /short-changed/);
  assert.match(back, /`stake\/relics`/);
});

test("two markers at one anchor is a finding, and the count is what states it", () => {
  // A death leaves ONE place. Two `minecraft:interaction` boxes there are 1.0 x
  // 2.0 at one cell centre and therefore coincident: every pick ray enters them
  // at the same distance and the client resolves the tie by entity iteration
  // order, so which one answers a right-click is not decidable from the campaign.
  // A reading that took "the nearest interaction" could never state it — a tie
  // has no nearest — which is why the trial records how many stood there.
  const t = goodTrial([stakeRule(), relicsRule()]);
  t.markersFound = 2;
  const out = lethalTrialFailures(t).join("\n");
  assert.match(out, /2 recovery-stake markers stand at \[4, 65, 8\]/);
  assert.match(out, /one death\s+leaves ONE place/);
  assert.match(out, /iteration order/);

  // One is the whole population, and it is silent.
  const one = goodTrial([stakeRule(), relicsRule()]);
  assert.equal(one.markersFound, 1);
  assert.deepEqual(lethalTrialFailures(one), []);
});

test("markersAt returns every paired interaction at the anchor, nearest first", () => {
  const anchor = [7, 65, 18] as const;
  const coincidentA = body("interaction", 7.5, 65, 18.5);
  const coincidentB = body("interaction", 7.5, 65, 18.5);
  const stray = body("interaction", 10.6, 65, 20.4);
  const found = markersAt(
    [stray, coincidentA, coincidentB],
    [
      body("item_display", 7.5, 65, 18.5),
      body("item_display", 10.6, 65, 20.4),
    ],
    anchor,
    4,
    0.5,
  );
  assert.equal(found.length, 3, "every paired box in the radius, not the nearest one");
  assert.deepEqual(found[0], coincidentA, "…and the nearest first, so `markerAt` is unchanged");
  assert.deepEqual(found[2], stray);
  // An unpaired box is still not a marker — the display is the assertion.
  assert.equal(
    markersAt([body("interaction", 7.5, 65, 18.5)], [], anchor, 4, 0.5).length,
    0,
  );
});

test("the datums a run promises to examine are counted from the plan, not from the run", () => {
  // playtest-methodology rule 1: a binding count is only a measurement beside its
  // population. A run that examined one datum of four looks exactly as green as
  // one that examined all four unless the population is stated.
  const p = plan();
  assert.deepEqual(
    stakesDropped(p).map((s) => s.id),
    ["stake/embers"],
    "`on_death`'s own declaration decides which stakes a death drops",
  );
  assert.equal(datumsPromised(p), 1, "one volume x one dropped stake");

  const b = deathLoopBinding(p, [goodTrial([stakeRule(), relicsRule()])]);
  assert.equal(b.stakesExamined, 1, "one PLACE");
  assert.equal(b.datumsExamined, 2, "…holding two DATUMS, which is a different number");
});

test("a trial that could not be exercised is a failure, never a quiet pass", () => {
  const t = openLethalTrial(VOLUME, [5, 65, 8], [stakeRule()]);
  t.abandoned = "the near lip could not be reached";
  const out = lethalTrialFailures(t);
  assert.equal(out.length, 1);
  assert.match(out[0]!, /could not be exercised/);
});

// --- binding ---------------------------------------------------------------

test("a stage that entered no volume reports VACUOUS, not pass", () => {
  const p = plan();
  const b = deathLoopBinding(p, []);
  assert.equal(b.volumesEntered, 0);
  const out = deathLoopBindingFailures(b);
  assert.equal(out.length, 1);
  assert.match(out[0]!, /examined nothing/);
});

test("a stage that entered a volume and saw no death is unbound downstream", () => {
  const p = plan();
  const t = openLethalTrial(VOLUME, [5, 65, 8], [stakeRule()]);
  const b = deathLoopBinding(p, [t]);
  assert.equal(b.deathsObserved, 0);
  assert.match(deathLoopBindingFailures(b).join("\n"), /ZERO\s+player deaths/);
});

test("the binding counts what was really examined", () => {
  const p = plan();
  const b = deathLoopBinding(p, [goodTrial()]);
  assert.deepEqual(b, {
    declaredVolumes: 1,
    volumesEntered: 1,
    deathsObserved: 1,
    datumsPromised: 1,
    stakesExamined: 1,
    datumsExamined: 1,
    datumsKept: 0,
    forfeitsExamined: 1,
    seatsMatched: 1,
    walksBack: 1,
  });
  assert.deepEqual(deathLoopBindingFailures(b), []);
});

// --- the gate on a `drop-stake`: a conditional promise, read as one ----------
//
// A `drop-stake` carries a `when` like every other effect, so "this death
// forfeits this stake" is CONDITIONAL. The gallery declares two of its four drops
// behind `flag/hall-sealed`, which is set long before the death-loop stage runs;
// every run of the stage reported the engine's correct refusal to take those two
// purses as "the death took the wrong amount for `stake/tokens`". The plan now
// carries the gate and these are the rules that read it.

/** A term, spelled the way the compiler emits one. */
function term(over: Partial<GateTerm> = {}): GateTerm {
  return {
    objective: "dw.f_hall_sealed",
    holder: "#party",
    min: 1,
    max: 1,
    negate: false,
    ...over,
  };
}

test("a term is asked as the `execute` clause the compiler wrote, in every op shape", () => {
  // The harness never reads a value and re-applies the rule: it puts the clause to
  // the server, whose answer IS the rule. `death_plan_gate.rs` holds the plan's
  // terms against the emitted guard from the other side, so these two renderings
  // of one declaration cannot drift apart unnoticed.
  assert.equal(termClause(term()), "if score #party dw.f_hall_sealed matches 1");
  assert.equal(
    termClause(term({ negate: true })),
    "unless score #party dw.f_hall_sealed matches 1",
  );
  assert.equal(
    termClause(term({ objective: "dw.s_labels_read", min: undefined, max: 9 })),
    "if score #party dw.s_labels_read matches ..9",
  );
  assert.equal(
    termClause(term({ objective: "dw.s_purse", holder: "@s", min: 500, max: undefined })),
    "if score @s dw.s_purse matches 500..",
  );
  assert.equal(
    termClause(term({ objective: "dw.s_rung", min: 2, max: 5 })),
    "if score #party dw.s_rung matches 2..5",
  );
});

test("a shut gate withholds the wager, and the verdict names the term that shut it", () => {
  const drop = { stake: "stake/relics", gates: [{ terms: [term({ negate: true })] }] };
  const v = gateVerdict(drop, () => false);
  assert.equal(v.kind, "shut");
  assert.match(v.kind === "shut" ? v.why : "", /dw\.f_hall_sealed/);
});

test("an open gate wagers, and an unconditional drop is one alternative with no terms", () => {
  assert.equal(gateVerdict({ stake: "s", gates: [{ terms: [] }] }, () => undefined).kind, "open");
  const gated = { stake: "s", gates: [{ terms: [term({ negate: true })] }] };
  assert.equal(gateVerdict(gated, () => true).kind, "open");
});

test("one stake dropped by two effects is forfeited when EITHER gate is open", () => {
  // The disjunction is the campaign's, not a convenience: two `drop-stake`
  // effects naming one stake promise the forfeit under either condition, and a
  // reading that took only the first would withhold a wager the death really makes.
  const drop = {
    stake: "stake/toll",
    gates: [{ terms: [term()] }, { terms: [term({ objective: "dw.f_bell_rung" })] }],
  };
  const v = gateVerdict(drop, (t) => t.objective === "dw.f_bell_rung");
  assert.equal(v.kind, "open");
});

test("a term the server never answered leaves the promise UNESTABLISHED, never quietly shut", () => {
  // The direction that matters: treating it as shut would skip the forfeit
  // assertion and report the run green over an assertion it never made.
  const drop = { stake: "stake/relics", gates: [{ terms: [term()] }] };
  const v = gateVerdict(drop, () => undefined);
  assert.equal(v.kind, "unread");
  assert.match(v.kind === "unread" ? v.why : "", /did not answer/);
});

test("…but an alternative that is plainly open settles it, whatever else went unanswered", () => {
  const drop = {
    stake: "stake/toll",
    gates: [{ terms: [term()] }, { terms: [] }],
  };
  assert.equal(gateVerdict(drop, () => undefined).kind, "open");
});

test("a gate nobody could read is a FAILING trial, not a quieter one", () => {
  const t = openLethalTrial(VOLUME, [5, 65, 8], []);
  t.gateUnread.push({ stake: "stake/embers", why: "the ledger could not be read" });
  const out = lethalTrialFailures(t);
  assert.match(out.join("\n"), /could not be established/);
});

test("a death-loop that examined ZERO forfeits is a finding, however many keeps it proved", () => {
  // The unrun vacuity mode, on the stake half's own denominator: a campaign whose
  // every `drop-stake` is gated shut by the time this stage runs declares a
  // recovery loop the bot tier never exercises. Its keeps are asserted, and they
  // prove nothing about placement, the walk back or the collection.
  const t = goodTrial();
  t.wagers.length = 0;
  t.wagers.push(keptWager(stakeRule()));
  const b = deathLoopBinding(plan(), [t]);
  assert.equal(b.datumsKept, 1);
  assert.equal(b.datumsExamined, 1, "the keep was read across the death, so it was examined");
  assert.equal(b.forfeitsExamined, 0);
  assert.match(deathLoopBindingFailures(b).join("\n"), /ZERO of the 1 datum/);
});

// --- a shut gate is a promise too: the death KEEPS the datum -----------------
//
// The death plan carries each `drop-stake`'s gate exactly as the emitter guards
// it (`Plan::gate_terms`, the one reduction). The gallery seals its hall long
// before the death loop runs, so `stake/relics` (`forbids_flags: [hall-sealed]`)
// is KEPT there: the plan stating it unconditional made the stage expect 1 → 0
// and see 1 → 1. Reading the gate, the stage must assert 1 → 1 — not skip it,
// because a skip is green over an engine that took the relics anyway.

/** A kept wager — its gate read shut — over a purse of 10, unchanged by the death. */
function keptWager(rule: StakeRule) {
  const w = openWager(rule, "the campaign gates it on `dw.f_hall_sealed` for #party NOT in 1, which does not hold");
  w.balanceBefore = 10;
  w.expectedForfeit = promisedForfeit(w, 10);
  w.balanceAfterDeath = 10;
  return w;
}

test("the promised forfeit is the rule under an open gate and nothing under a shut one", () => {
  const open = openWager(relicsRule());
  const kept = openWager(relicsRule(), "shut");
  assert.equal(open.forfeits, true);
  assert.equal(kept.forfeits, false);
  assert.equal(promisedForfeit(open, 10), 2, "fixed 2 of 10");
  assert.equal(promisedForfeit(kept, 10), 0, "a shut gate keeps the purse whole");
});

test("a kept datum left whole is a pass, and a death that keeps everything needs no stake", () => {
  const t = goodTrial();
  t.wagers.length = 0;
  t.wagers.push(keptWager(relicsRule()));
  // Nothing was forfeited, so nothing stands anywhere and nothing is walked to.
  t.markerPos = undefined;
  t.markersFound = 0;
  t.walkedBack = false;
  t.markerRetired = false;
  t.expectedAnchor = undefined;
  assert.deepEqual(lethalTrialFailures(t), []);
});

test("a death that takes a datum its gate keeps is a failure, naming the gate", () => {
  // The direction a skip could never see: the engine forfeits `stake/relics`
  // although `hall-sealed` shuts its `drop-stake`.
  const t = goodTrial();
  t.wagers.length = 0;
  const w = keptWager(relicsRule());
  w.balanceAfterDeath = 0;
  t.wagers.push(w);
  const out = lethalTrialFailures(t).join("\n");
  assert.match(out, /the death took from `stake\/relics`, which its `on_death` gate keeps/);
  assert.match(out, /dw\.f_hall_sealed/);
  assert.match(out, /was 10 before and 0 after/);
  assert.match(out, /leave the purse at 10/);
});

test("a kept datum beside a forfeited one is asserted on both sides of the collection", () => {
  const t = goodTrial([stakeRule()]);
  t.wagers.push(keptWager(relicsRule()));
  wager(t, "stake/relics").balanceAfterCollect = 10;
  assert.deepEqual(lethalTrialFailures(t), [], "one forfeit and one keep, both kept");

  // The collection may not hand back what was never taken.
  const paid = goodTrial([stakeRule()]);
  paid.wagers.push(keptWager(relicsRule()));
  wager(paid, "stake/relics").balanceAfterCollect = 12;
  assert.match(lethalTrialFailures(paid).join("\n"), /`stake\/relics`/);

  const b = deathLoopBinding(plan(), [t]);
  assert.equal(b.datumsKept, 1);
  assert.equal(b.forfeitsExamined, 1);
  assert.equal(b.datumsExamined, 2);
});

test("a keep asserted over an empty purse is UNBOUND, as a forfeit is", () => {
  const t = goodTrial();
  t.wagers.length = 0;
  const w = keptWager(relicsRule());
  w.balanceBefore = 0;
  w.expectedForfeit = 0;
  w.balanceAfterDeath = 0;
  t.wagers.push(w);
  assert.match(lethalTrialFailures(t).join("\n"), /`stake\/relics` was UNBOUND/);
});

test("the plan's drops are addressed by stake, and an undropped stake has no gate", () => {
  const p = plan();
  assert.equal(dropOf(p, "stake/embers")?.gates.length, 1);
  assert.equal(dropOf(p, "stake/nothing"), undefined);
});

test("a gate term with both ends open is refused — it gates nothing", () => {
  const doc = planDoc();
  (doc["on_death"] as Record<string, unknown>)["drops_stake"] = [
    { stake: "stake/embers", gates: [{ terms: [{ ...term(), min: null, max: null }] }] },
  ];
  assert.throws(
    () => parseDeathPlan(doc),
    (err: unknown) => err instanceof DeathPlanParseError,
  );
});

test("a drop with no alternatives at all is refused, never read as unconditional", () => {
  const doc = planDoc();
  (doc["on_death"] as Record<string, unknown>)["drops_stake"] = [
    { stake: "stake/embers", gates: [] },
  ];
  assert.throws(
    () => parseDeathPlan(doc),
    (err: unknown) => err instanceof DeathPlanParseError && err.pointer.endsWith("/gates"),
  );
});

// --- the near lip: the reachable cell NEAREST the volume ---------------------

test("the near lip is the anchor nearest the volume, not the first row in the table", () => {
  // vesperhold's numbers: `lethal/undertide` at [35,58,79]..[37,60,81] is anchored
  // at [31,68,78] from three seats, [35,80,79] from another and [9,66,79] from the
  // entry spawn — which is the row the table lists FIRST. Taking it sent the
  // approach twenty-six blocks across the map, through live encounters, to a place
  // that is not this volume's lip.
  const p = plan();
  const far: DeathPlan = {
    ...p,
    volumes: [{ ...p.volumes[0]!, region: { lo: [35, 58, 79], hi: [37, 60, 81] } }],
    regions: [{ ...p.regions[0]!, region: { lo: [35, 58, 79], hi: [37, 60, 81] } }],
    rows: [
      { seat: 0, region: 0, anchor: [9, 66, 79] },
      { seat: 1, region: 0, anchor: [31, 68, 78] },
    ],
  };
  assert.deepEqual(nearLip(far, far.volumes[0]!.id), [31, 68, 78]);
});

test("the near lip ties break lexicographically, so a run is reproducible", () => {
  const p = plan();
  const box = p.volumes[0]!.region;
  const tied: DeathPlan = {
    ...p,
    rows: [
      { seat: 0, region: 0, anchor: [box.hi[0] + 2, box.lo[1], box.lo[2]] },
      { seat: 1, region: 0, anchor: [box.lo[0] - 2, box.lo[1], box.lo[2]] },
    ],
  };
  assert.deepEqual(nearLip(tied, p.volumes[0]!.id), [box.lo[0] - 2, box.lo[1], box.lo[2]]);
});

// --- the lethal exclusion, fed what the REAL pathfinder feeds it -------------
//
// `Movements.getBlock` is the library's own, not a model of it: for a cell the
// client has not loaded (`bot.blockAt` answers null) it returns a stub with no
// `position`, and a diagonal move hands that stub to every `exclusionAreasStep`
// callback through `exclusionStep`. The harness's callback read `position.x` off
// it and threw a TypeError out of the path search, ending a ladder run.
const requireCjs = createRequire(import.meta.url);
const { Movements } = requireCjs("mineflayer-pathfinder") as {
  Movements: {
    prototype: {
      getBlock(this: unknown, pos: unknown, dx: number, dy: number, dz: number): Record<string, unknown>;
      exclusionStep(this: unknown, block: unknown): number;
    };
  };
};

/** The block the real `getBlock` returns for a cell `blockAt` does not know. */
function unloadedCell(): Record<string, unknown> {
  const self = { bot: { blockAt: (): null => null } };
  return Movements.prototype.getBlock.call(self, { x: 5, y: 65, z: 8 }, 1, 0, 1);
}

test("the library's own unloaded-cell stub carries no position and is unsafe", () => {
  const stub = unloadedCell();
  assert.equal(stub["position"], undefined, "the contract this guard answers has moved");
  assert.equal(stub["safe"], false, "the library no longer walls an unknown cell itself");
});

test("an unloaded cell costs no exclusion and does not throw out of the path search", () => {
  const box = VOLUME.region;
  const through = { exclusionAreasStep: [(b: unknown) => lethalStepCost(b as never, [box])] };
  assert.equal(Movements.prototype.exclusionStep.call(through, unloadedCell()), 0);
  assert.equal(lethalStepCost(undefined, [box]), 0);
  assert.equal(lethalStepCost(null, [box]), 0);
});

test("a loaded cell the volume reaches is walled, and one it does not is free", () => {
  const box = VOLUME.region;
  // Inside the declared box, and one cell outside it (the shell the server still
  // kills in, `volumeReachesCell`), are both walled; three cells away is free.
  assert.equal(lethalStepCost({ position: { x: 5, y: 65, z: 8 } }, [box]), LETHAL_STEP_COST);
  assert.equal(volumeReachesCell([6, 65, 8], box), true);
  assert.equal(lethalStepCost({ position: { x: 6, y: 65, z: 8 } }, [box]), LETHAL_STEP_COST);
  assert.equal(lethalStepCost({ position: { x: 9, y: 65, z: 8 } }, [box]), 0);
  assert.ok(LETHAL_STEP_COST > 100, "the library treats only a cost above 100 as no move");
});

// --- the stage row: a report that says a stage was not reached must be true ---

/** The death-loop stage's input for a run whose path was proven. */
function stageInput(over: Partial<Parameters<typeof deathLoopStage>[0]> = {}) {
  const trials = [goodTrial()];
  return {
    enabled: true,
    disabledReason: "skipped via DELVEWRIGHT_DEATH_LOOP=0",
    pathProven: true,
    interruption: undefined,
    skipReason: undefined,
    binding: deathLoopBinding(plan(), trials),
    trials,
    trialsFinished: trials.length,
    ...over,
  };
}

test("a death loop the run's budget cut off is reported as entered and unfinished, never as unreached", () => {
  // The local ladder that motivated this: the critical path passed, the stage
  // walked the bot to the well, and the wall-clock budget expired mid-trial. The
  // report then said the critical path had failed and the stage was never
  // reached, beside its own `death_loop` block recording `volumes_entered: 1`.
  const t = openLethalTrial(VOLUME, [5, 65, 8], [stakeRule()]);
  const row = deathLoopStage(
    stageInput({
      interruption: "run exceeded wall-clock budget of 2700000ms",
      trials: [t],
      binding: deathLoopBinding(plan(), [t]),
      trialsFinished: 0,
    }),
  );
  assert.equal(row.ran, true, "the stage was entered, so it ran");
  assert.equal(row.passed, false);
  assert.ok(
    row.findings.every((f) => !/never reached/.test(f)) &&
      row.failures.every((f) => !/never reached/.test(f)),
    `nothing may say the stage was never reached: ${JSON.stringify(row)}`,
  );
  assert.equal(row.failures.length, 1, JSON.stringify(row.failures));
  assert.match(row.failures[0]!, /did not finish/);
  assert.match(row.failures[0]!, /run exceeded wall-clock budget of 2700000ms/);
  assert.match(row.failures[0]!, /1 of 1 declared lethal volume/);
});

test("an interrupted stage still judges the trials it finished, and only those", () => {
  const finished = goodTrial();
  finished.markerRetired = false; // a real verdict the finished trial owes
  const cut = openLethalTrial(VOLUME, [5, 65, 8], [stakeRule()]);
  const row = deathLoopStage(
    stageInput({
      interruption: "run exceeded wall-clock budget of 2700000ms",
      trials: [finished, cut],
      binding: deathLoopBinding(plan(), [finished, cut]),
      trialsFinished: 1,
    }),
  );
  assert.equal(row.failures.length, 2, JSON.stringify(row.failures));
  assert.match(row.failures[1]!, /still standing/);
  assert.ok(!row.failures.some((f) => /never OBSERVED/.test(f)), "the cut trial is not judged");
});

test("a critical path that failed leaves the stage unreached, and says exactly that", () => {
  const row = deathLoopStage(
    stageInput({ pathProven: false, trials: [], binding: deathLoopBinding(plan(), []), trialsFinished: 0 }),
  );
  assert.equal(row.ran, false);
  assert.equal(row.passed, false);
  assert.deepEqual(row.failures, []);
  assert.match(row.findings[0]!, /critical path failed, so the death loop was never reached/);
});

test("a finished stage is judged whole, and a disabled one carries its reason", () => {
  assert.deepEqual(deathLoopStage(stageInput()), {
    stage: "death-loop",
    ran: true,
    passed: true,
    findings: [],
    failures: [],
  });
  const off = deathLoopStage(stageInput({ enabled: false }));
  assert.equal(off.ran, false);
  assert.deepEqual(off.findings, ["skipped via DELVEWRIGHT_DEATH_LOOP=0"]);
});

// --- letting go over a submerged volume ------------------------------------

/** vesperhold's well: the bottom three layers of a still column y=58..67. */
const UNDERTIDE = { lo: [35, 58, 79] as const, hi: [37, 60, 81] as const };

test("a body is over a volume's footprint exactly where the selector's region lies below it", () => {
  // The continuous region is [lo, hi + 1] on x and z.
  assert.equal(overFootprint([35.0, 66.9, 79.0], UNDERTIDE), true);
  assert.equal(overFootprint([38.0, 66.9, 82.0], UNDERTIDE), true);
  assert.equal(overFootprint([36.5, 120, 80.5], UNDERTIDE), true, "height is not the question");
  assert.equal(overFootprint([34.99, 66.9, 80.5], UNDERTIDE), false);
  assert.equal(overFootprint([36.5, 66.9, 82.01], UNDERTIDE), false);
  // The lip the placement table proved nearest the well is not over it.
  assert.equal(overFootprint([31.5, 68, 80.5], UNDERTIDE), false);
});

test("the sink budget is the measured descent, and it covers the descents that were measured", () => {
  assert.equal(SINK_BLOCKS_PER_TICK, 0.025);
  // Released at the surface (feet 66.97), the body is matched once its feet reach
  // the region's top face, y = hi + 1 = 61: 5.97 blocks. The six measured
  // descents took 12.13-12.27 s from release to death.
  const depth = 66.97 - (UNDERTIDE.hi[1] + 1);
  const budget = sinkBudgetMs(depth);
  assert.ok(budget > 12_270, `budget ${budget}ms must cover the slowest measured descent`);
  assert.ok(budget < 20_000, `budget ${budget}ms is a bound, not a guess`);
  assert.equal(sinkBudgetMs(-2), 1_000, "a body already level with the volume waits only for the kill");
  assert.ok(sinkBudgetMs(12) > sinkBudgetMs(6), "deeper is longer");
});

// --- a forfeit is asserted over a purse that holds something ------------------

test("a trial whose forfeit could only be observed at zero is refused as unbound", () => {
  // vesperhold, every branch: tallow 0 before, 0 after the death, 0 after the
  // collection — and the stage passed, over an assertion a missing forfeit
  // satisfies exactly as well as a working one.
  const t = goodTrial();
  const w = wager(t, "stake/embers");
  w.balanceBefore = 0;
  w.expectedForfeit = 0;
  w.balanceAfterDeath = 0;
  w.balanceAfterCollect = 0;
  const failures = lethalTrialFailures(t);
  assert.ok(
    failures.some((f) => /forfeit of `stake\/embers` was UNBOUND/.test(f)),
    JSON.stringify(failures),
  );
  // The same trial over a purse of 5 is bound and passes.
  assert.deepEqual(lethalTrialFailures(goodTrial()), []);
});

test("the staged balance makes every rule's forfeit observable, and none's too", () => {
  const rules = [
    { kind: "all" },
    { kind: "fixed", amount: 2 },
    { kind: "fixed", amount: 7 },
    { kind: "proportion", percent: 50 },
    { kind: "proportion", percent: 1 },
  ] as const;
  for (const rule of rules) {
    const v = stagedBalance(rule);
    const taken = expectedForfeit(rule, v);
    assert.ok(taken > 0, `${JSON.stringify(rule)} takes ${taken} of ${v}`);
    if (rule.kind !== "all") assert.ok(taken < v, `${JSON.stringify(rule)} is not "all" at ${v}`);
  }
  assert.ok(stagedBalance({ kind: "none" }) > 0, "none is asserted over a purse holding something");
});

test("a blocked walk in asks for the sill a player jumps to, nearest the volume first", () => {
  // vesperhold's well after the choir fix: the lip is the floor of a dry cut at
  // [30, 67, 79]; the sill of the curb's opening stands at [31, 68, 80]; the
  // floor west of the cut at [29, 68, 80]. The volume is the shaft's bottom.
  const standable = new Set(["31,68,80", "29,68,80", "29,68,79", "30,67,80", "31,70,81"]);
  const got = wayInCandidates([30, 67, 79], UNDERTIDE, (c) => standable.has(c.join(",")));
  assert.deepEqual(got[0], [31, 68, 80], "the sill first: nearest the volume, the smallest climb");
  assert.ok(!got.some((c) => c[0] <= 30), "nothing no nearer the volume than the body already is");
  assert.deepEqual(wayInCandidates([30, 67, 79], UNDERTIDE, () => false), []);
});
