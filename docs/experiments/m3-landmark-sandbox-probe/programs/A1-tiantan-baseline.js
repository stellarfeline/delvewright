// ===================================================================
// CONDITION A (baseline) — Temple of Heaven, Hall of Prayer for Good
// Harvests (天坛 祈年殿). MineBench voxel.exec program. seed 121111.
// Primitives: block / box / line / rng. Grid 256, palette "advanced".
// ===================================================================
const CX = 128, CZ = 128;

// ---- circle rasterizers -------------------------------------------
function disc(y, r, t) {
  const rr = r * r + r * 0.5, R = Math.ceil(r);
  for (let dx = -R; dx <= R; dx++)
    for (let dz = -R; dz <= R; dz++)
      if (dx * dx + dz * dz <= rr) block(CX + dx, y, CZ + dz, t);
}
function annulus(y, rout, rin, t) {
  const ro = rout * rout + rout * 0.5, ri = rin * rin + rin * 0.5;
  const R = Math.ceil(rout);
  for (let dx = -R; dx <= R; dx++)
    for (let dz = -R; dz <= R; dz++) {
      const d = dx * dx + dz * dz;
      if (d <= ro && d > ri) block(CX + dx, y, CZ + dz, t);
    }
}
// annulus with angular gaps (doorways): skip if angle within `half` of a gap dir
function annulusGaps(y, rout, rin, t, gaps, half) {
  const ro = rout * rout + rout * 0.5, ri = rin * rin + rin * 0.5;
  const R = Math.ceil(rout);
  for (let dx = -R; dx <= R; dx++)
    for (let dz = -R; dz <= R; dz++) {
      const d = dx * dx + dz * dz;
      if (d > ro || d <= ri) continue;
      const a = Math.atan2(dz, dx);
      let skip = false;
      for (const g of gaps) {
        let da = Math.abs(a - g);
        if (da > Math.PI) da = 2 * Math.PI - da;
        if (da < half) skip = true;
      }
      if (!skip) block(CX + dx, y, CZ + dz, t);
    }
}

// ---- MASS 1: the three-tier round marble terrace ------------------
const MARBLE = "quartz_block", RAIL = "white_concrete";
const TIERS = [[64, 0, 3], [51, 4, 7], [38, 8, 11]];
for (const [r, y0, y1] of TIERS) for (let y = y0; y <= y1; y++) disc(y, r, MARBLE);

// paved apron so the temple is not floating in a void
annulus(0, 78, 64, "smooth_stone");
annulus(0, 80, 78, "andesite");

// balustrade on every tier + posts
for (const [r, , y1] of TIERS) {
  annulus(y1 + 1, r, r - 1, RAIL);
  for (let i = 0; i < 48; i++) {
    const a = (i / 48) * Math.PI * 2;
    block(CX + Math.round(Math.cos(a) * (r - 0.5)), y1 + 2,
          CZ + Math.round(Math.sin(a) * (r - 0.5)), MARBLE);
  }
}

// four cardinal stairways up each tier
const CARD = [0, Math.PI / 2, Math.PI, -Math.PI / 2];
function stairway(a, rTop, yTop, steps) {
  const ca = Math.cos(a), sa = Math.sin(a), px = -sa, pz = ca;
  for (let s = 0; s < steps; s++) {
    const rr = rTop + (steps - 1 - s);
    const y = yTop - (steps - 1 - s);
    for (let w = -5; w <= 5; w++)
      for (let k = 0; k <= 1; k++)
        block(Math.round(CX + ca * (rr + k) + px * w), y,
              Math.round(CZ + sa * (rr + k) + pz * w), MARBLE);
  }
}
for (const a of CARD) {
  stairway(a, 64, 3, 4);
  stairway(a, 51, 7, 4);
  stairway(a, 38, 11, 4);
}

// ---- MASS 2: the round hall body ----------------------------------
const RED = "red_concrete", DARKWOOD = "dark_oak_planks", GOLD = "gold_block";
disc(12, 34, MARBLE);                     // hall plinth
annulus(12, 34, 30, "smooth_stone");

// 12 outer eave columns (the twelve-hours pillars), 3x3, red
for (let i = 0; i < 12; i++) {
  const a = (i / 12) * Math.PI * 2 + Math.PI / 12;
  const bx = CX + Math.round(Math.cos(a) * 29), bz = CZ + Math.round(Math.sin(a) * 29);
  box(bx - 1, 13, bz - 1, bx + 1, 30, bz + 1, RED);
  box(bx - 1, 31, bz - 1, bx + 1, 31, bz + 1, GOLD);
}

// drum wall with four cardinal door openings
for (let y = 13; y <= 30; y++)
  annulusGaps(y, 22, 20, RED, CARD, y <= 24 ? 0.20 : 0.0);
for (let y = 13; y <= 24; y++)          // latticed doors in the openings
  annulusGaps(y, 21, 20, DARKWOOD, [0, Math.PI / 2, Math.PI, -Math.PI / 2, 9], 0.0);
for (const a of CARD)
  for (let y = 13; y <= 24; y++)
    for (let w = -4; w <= 4; w++) {
      const t = (y % 3 === 0 || w % 2 === 0) ? DARKWOOD : RED;
      block(CX + Math.round(Math.cos(a) * 21 - Math.sin(a) * w), y,
            CZ + Math.round(Math.sin(a) * 21 + Math.cos(a) * w), t);
    }
// gilded eave band
annulus(31, 23, 20, GOLD);
annulus(31, 20, 18, "green_concrete");

// ---- MASS 3: the triple conical blue roof -------------------------
const BLUE = "blue_concrete";
// concave (upturned-eave) cone profile
function roof(y0, y1, rBot, rTop, t) {
  const n = y1 - y0;
  for (let i = 0; i <= n; i++) {
    const f = Math.pow(i / n, 1.35);
    const r = rBot + (rTop - rBot) * f;
    const rNext = rBot + (rTop - rBot) * Math.pow((i + 1) / n, 1.35);
    annulus(y0 + i, r, Math.max(rNext - 1.2, 0), t);
  }
  // radial ridge lines (垂脊), gilded
  for (let k = 0; k < 12; k++) {
    const a = (k / 12) * Math.PI * 2;
    for (let i = 0; i <= n; i++) {
      const r = rBot + (rTop - rBot) * Math.pow(i / n, 1.35);
      block(CX + Math.round(Math.cos(a) * r), y0 + i,
            CZ + Math.round(Math.sin(a) * r), GOLD);
    }
  }
}
roof(32, 43, 32, 19, BLUE);
// second storey drum
for (let y = 44; y <= 49; y++) annulus(y, 18, 16, RED);
annulus(50, 19, 16, GOLD);
roof(51, 60, 25, 13, BLUE);
// third storey drum
for (let y = 61; y <= 65; y++) annulus(y, 12, 10, RED);
annulus(66, 13, 10, GOLD);
roof(67, 78, 18, 2, BLUE);

// ---- MASS 4: gilded finial ----------------------------------------
box(CX - 1, 79, CZ - 1, CX + 1, 82, CZ + 1, GOLD);
disc(83, 3, GOLD);
disc(84, 4, GOLD);
disc(85, 3, GOLD);
box(CX, 86, CZ, CX, 88, CZ, GOLD);

// ---- scene: cypress ring on the apron ------------------------------
for (let i = 0; i < 16; i++) {
  const a = (i / 16) * Math.PI * 2 + 0.19;
  const tx = CX + Math.round(Math.cos(a) * 74), tz = CZ + Math.round(Math.sin(a) * 74);
  box(tx, 1, tz, tx, 9, tz, "spruce_log");
  for (let y = 4; y <= 12; y++) {
    const rr = Math.max(0, 3 - Math.floor((y - 4) / 3));
    for (let dx = -rr; dx <= rr; dx++)
      for (let dz = -rr; dz <= rr; dz++)
        if (dx * dx + dz * dz <= rr * rr + rr)
          block(tx + dx, y, tz + dz, "oak_leaves");
  }
}
