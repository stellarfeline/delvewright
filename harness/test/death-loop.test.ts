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
  boxCells,
  deathLoopBinding,
  deathLoopBindingFailures,
  entryCellOf,
  markerAt,
  expectedForfeit,
  inBox,
  lethalTrialFailures,
  openLethalTrial,
  parseDeathPlan,
  seatAtRespawn,
  tableAnchor,
  type DeathPlan,
  type LethalTrial,
  type LethalVolume,
  type StakeRule,
} from "../src/death-loop.ts";
import type { Vec3Tuple } from "../src/critical-path.ts";

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
    on_death: { effects: 1, drops_stake: ["stake/embers"] },
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

/** A trial in which everything the campaign promised actually happened. */
function goodTrial(): LethalTrial {
  const t = openLethalTrial(VOLUME, [5, 65, 8], stakeRule());
  t.enteredVolume = true;
  t.died = true;
  t.deathPos = [5, 65, 8];
  t.wordingSeen = true;
  t.balanceBefore = 5;
  t.expectedForfeit = 5;
  t.balanceAfterDeath = 0;
  t.respawnPos = [5.5, 65, 4.5];
  t.respawnSeat = "checkpoint anchor `anchor/keeper-stand`";
  t.expectedAnchor = [4, 65, 8];
  t.markerPos = [4.5, 65, 8.5];
  t.walkedBack = true;
  t.collectClicks = 2;
  t.balanceAfterCollect = 5;
  t.markerRetired = true;
  return t;
}

// --- the contract ----------------------------------------------------------

test("the emitted plan parses, and every declaration survives the round trip", () => {
  const p = plan();
  assert.equal(p.campaignId, "economy");
  assert.equal(p.volumes.length, 1);
  assert.equal(p.volumes[0]!.message, "The stone floor gives way beneath you.");
  assert.deepEqual(p.dropsStake, ["stake/embers"]);
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
  t.balanceAfterDeath = 5; // the engine took nothing — the live first-death defect
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
  t.balanceAfterCollect = 10; // both clicks paid out
  const out = lethalTrialFailures(t).join("\n");
  assert.match(out, /not idempotent/);
});

test("a stake that short-changes the player is caught by the same clause", () => {
  const t = goodTrial();
  t.balanceAfterCollect = 3;
  assert.match(lethalTrialFailures(t).join("\n"), /short-changed/);
});

test("a collected stake whose hardware still stands is a failure", () => {
  const t = goodTrial();
  t.markerRetired = false;
  assert.match(lethalTrialFailures(t).join("\n"), /still standing/);
});

test("a trial that could not be exercised is a failure, never a quiet pass", () => {
  const t = openLethalTrial(VOLUME, [5, 65, 8], stakeRule());
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
  const t = openLethalTrial(VOLUME, [5, 65, 8], stakeRule());
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
    stakesExamined: 1,
    seatsMatched: 1,
    walksBack: 1,
  });
  assert.deepEqual(deathLoopBindingFailures(b), []);
});
