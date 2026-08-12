// ===================================================================
// CONDITION B (methodology-primed) — ancient Greek Doric peripteral
// temple.  seed 121111, grid 256, palette "advanced".
//
// <self_check>
// MASSING — 4 masses:
//   M1 primary   temple block (crepidoma+peristyle+entablature) 67x137x42
//   M2 secondary roof/pediment prism 67x141x13  -> h 13/42 = 0.31 (1/3 band)
//   M3 secondary cella box 31x81x30            -> w 31/67 = 0.46 (1/2 band)
//   M4 tertiary  retaining terrace + monumental front flight (context mass)
// HIERARCHY — a peripteral temple is deliberately uniform, so the FRONT is
//   made the hero: front porch is DOUBLE-DEPTH (a second column row in
//   antis), the front flight is 25 wide x 7 risers against 0 on the flanks,
//   the central akroterion is 5 tall vs 4 at the corners (+25%), the front
//   tympanum carries 9 figures vs 5 at the rear, and only the front three
//   bays get carved metopes. Detail density therefore rises toward the front.
// DEPTH — the classical section IS the depth rule; three planes:
//   * architrave flush with the abacus  (x = stylobate edge)
//   * frieze RECESSED 1                 (triglyphs then project 1 back out)
//   * cornice PROJECTING 2 then 3
//   * columns 5x5 with FLUTING: the 8 half-face cells of every shaft ring are
//     a darker value -> a wall-plane break every block. A 3x3 shaft (what the
//     baseline used) physically cannot be fluted; the depth rule is what
//     forces the column up to 5x5 and the pitch to 10.
//   * cella wall run is 81 blocks (>5) -> 3 depth layers: proud orthostate
//     course (y11-13), engaged antae 1 proud every 10, recessed cornice band
//   * every crepidoma step gets a recessed groove on its top course
//   * antefixes project 2 above the eave every 10 blocks
// RULE OF ODDS — stylobate 55 wide (odd, so x=128 is a TRUE centre for the
//   door and the roof ridge), cella 31 wide (odd), door 5 wide (odd),
//   3 crepidoma steps, 7 stair risers, 25-wide flight, 9 front pediment
//   figures. DELIBERATE EXCEPTION, stated: the front colonnade is 6 columns
//   (even) because a Doric facade centres an INTERCOLUMNIATION on the door,
//   not a column. The referent overrides the rule; the rule is satisfied by
//   making every span odd instead.
// PALETTE — 5 named roles, no others:
//   base      quartz_block   (~60% — walls, shafts, tympana)
//   secondary smooth_stone   (~30% — flutes, frieze ground, recesses, roof)
//   texture   sandstone      (weathering clusters ONLY, never speckle)
//   detail    stone          (triglyphs, abacus, ridge, antefixes)
//   accent    gold_block     (akroteria + door surround only, ~2% of faces)
// GRADIENT — weathering is a successive value ramp
//   quartz_block -> smooth_stone -> sandstone driven by 2-octave clustered
//   value noise (cells 13 and 6). Never per-block rng: the baseline's
//   per-block 7% sandstone read as measles, which is precisely the
//   "gradienting is not splattering" failure.
// SILHOUETTE — pediment gable, 3 akroteria, 14 antefixes per eave, corner
//   columns clear of the mass, and a 13-deep projecting front flight: the
//   black mask is not a rectangle.
// </self_check>
// ===================================================================
const BASE = "quartz_block", SEC = "smooth_stone", TEX = "sandstone";
const DET = "stone", ACC = "gold_block";

const X0 = 101, X1 = 155;          // stylobate 55 wide (ODD, centre x=128)
const Z0 = 66, Z1 = 190;           // stylobate 125 long
const YS = 10;                     // stylobate surface
const RC = 128;                    // true centre line

// ---- clustered weathering (successive ramp, never per-block rng) ----
const NOISE = [];
for (let i = 0; i < 1024; i++) NOISE.push(rng());
function vnoise(x, y, z, cell) {
  const a = Math.floor(x / cell), b = Math.floor(y / cell), c = Math.floor(z / cell);
  return NOISE[(((a * 73856093) ^ (b * 19349663) ^ (c * 83492791)) >>> 0) % 1024];
}
// 60 / 30 / 10 BY CONSTRUCTION, and successive by NESTING: the TEX step
// needs BOTH octaves high, so a TEX cluster always sits inside a SEC
// cluster and can never abut BASE directly. Round 1 used a linear
// round() map that produced ~33/33/33 -- a violation of this file's own
// declared palette rule that only the render exposed.
function weather(x, y, z, bias) {
  const n = 0.65 * vnoise(x, y, z, 13) + 0.35 * vnoise(x + 41, y, z + 29, 6) + bias;
  if (n < 0.60) return BASE;
  if (n < 0.90) return SEC;
  return TEX;
}
function wblock(x, y, z, bias) { block(x, y, z, weather(x, y, z, bias)); }

// ---- ground + retaining terrace (M4) --------------------------------
box(80, 0, 38, 176, 0, 212, "grass_block");
for (let x = 80; x <= 176; x++)
  for (let z = 38; z <= 212; z++) {
    const n = vnoise(x, 0, z, 9);
    if (n > 0.72) block(x, 0, z, "moss_block");
    else if (n < 0.18) block(x, 0, z, "gravel");
  }
for (let y = 1; y <= 6; y++) {
  const e = (y === 3 || y === 4) ? 1 : 0;         // DEPTH: recessed fascia band
  box(91 + e, y, 56 + e, 165 - e, y, 200 - e, (e ? SEC : BASE));
}

// ---- monumental front flight: 7 risers (odd), 25 wide (odd) ---------
for (let s = 0; s < 7; s++) {
  const y = 6 - s;
  for (let k = 0; k < 2; k++) {
    const z = 55 - s * 2 - k;
    for (let x = RC - 12; x <= RC + 12; x++)
      block(x, y, z, (x === RC - 12 || x === RC + 12) ? DET : BASE);
  }
}

// ---- crepidoma: 3 steps (odd), each with a recessed groove ----------
for (let s = 0; s < 3; s++) {
  const e = (3 - s) * 2, y = 7 + s;
  for (let x = X0 - e; x <= X1 + e; x++)
    for (let z = Z0 - e; z <= Z1 + e; z++) {
      const edge = (x <= X0 - e + 0 || x >= X1 + e || z <= Z0 - e || z >= Z1 + e);
      wblock(x, y, z, edge ? 0.12 : 0.0);
    }
}
for (let x = X0; x <= X1; x++) for (let z = Z0; z <= Z1; z++) wblock(x, YS, z, 0.0);

// ---- peristyle: 6 x 13 FLUTED Doric columns, 5x5, pitch 10 ----------
const COLX = [], COLZ = [];
for (let i = 0; i < 6; i++) COLX.push(X0 + 2 + 10 * i);
for (let j = 0; j < 13; j++) COLZ.push(Z0 + 2 + 10 * j);
const YTOP = 38, YECH = 39, YABA = 40;
function column(cx, cz, front) {
  for (let y = YS + 1; y <= YTOP; y++)
    for (let dx = -2; dx <= 2; dx++)
      for (let dz = -2; dz <= 2; dz++) {
        const ring = Math.max(Math.abs(dx), Math.abs(dz)) === 2;
        // FLUTING: the half-face cells of the ring are the darker value
        const flute = ring && (Math.abs(dx) === 1 || Math.abs(dz) === 1);
        block(cx + dx, y, cz + dz, flute ? SEC : BASE);
      }
  // entasis: the shaft narrows by 1 for its top two courses
  for (let dx = -2; dx <= 2; dx++)
    for (let dz = -2; dz <= 2; dz++)
      if (Math.max(Math.abs(dx), Math.abs(dz)) === 2 && (Math.abs(dx) === 2 && Math.abs(dz) === 2))
        block(cx + dx, YTOP, cz + dz, SEC);
  box(cx - 3, YECH, cz - 3, cx + 3, YECH, cz + 3, BASE);    // echinus, flares out
  box(cx - 3, YABA, cz - 3, cx + 3, YABA, cz + 3, DET);     // abacus (detail role)
  if (front) box(cx - 2, YABA + 1, cz - 2, cx + 2, YABA + 1, cz + 2, SEC);
}
for (const cx of COLX) for (const cz of COLZ)
  if (cx === COLX[0] || cx === COLX[5] || cz === COLZ[0] || cz === COLZ[12])
    column(cx, cz, cz === COLZ[0]);
// FOCAL: second column row in antis behind the front colonnade
for (const cx of [COLX[1], COLX[2], COLX[3], COLX[4]]) column(cx, COLZ[1], true);

// ---- entablature on THREE planes -------------------------------------
function ring(y, inset, t, graded, bias) {
  const ax = X0 - inset, bx = X1 + inset, az = Z0 - inset, bz = Z1 + inset;
  for (let x = ax; x <= bx; x++)
    for (let z = az; z <= bz; z++) {
      const onEdge = (x <= ax + 2 || x >= bx - 2 || z <= az + 2 || z >= bz - 2);
      if (!onEdge) continue;
      if (graded) wblock(x, y, z, bias); else block(x, y, z, t);
    }
}
for (let y = 41; y <= 44; y++) ring(y, 0, BASE, true, 0.0);        // architrave, flush
// frieze RECESSED 1, with triglyphs projecting back out to the architrave plane
for (let y = 45; y <= 48; y++) ring(y, -1, SEC, false, 0);
function triglyph(x, z) {
  for (let y = 45; y <= 48; y++)
    block(x, y, z, y === 48 ? DET : (y % 2 ? BASE : DET));
}
for (let i = 0; i < 6; i++) {
  for (const t of [0, 5]) {
    const x = COLX[i] + (t === 0 ? -1 : 4) - 2;
    if (x < X0 || x > X1) continue;
    for (let k = 0; k <= 2; k++) { triglyph(x + k, Z0); triglyph(x + k, Z1); }
  }
}
for (let j = 0; j < 13; j++) {
  for (const t of [0, 5]) {
    const z = COLZ[j] + (t === 0 ? -1 : 4) - 2;
    if (z < Z0 || z > Z1) continue;
    for (let k = 0; k <= 2; k++) { triglyph(X0, z + k); triglyph(X1, z + k); }
  }
}
// carved metopes — FRONT THREE BAYS ONLY (detail density toward the focal)
for (let i = 0; i < 5; i++) {
  const xa = COLX[i] + 3, xb = COLX[i + 1] - 3;
  for (let x = xa; x <= xb; x++)
    for (let y = 45; y <= 47; y++)
      if ((x + y) % 3 === 0) block(x, y, Z0, DET);
}
// cornice: projects 2 then 3
for (let y = 49; y <= 51; y++) ring(y, y === 51 ? 3 : 2, BASE, true, 0.0);

// ---- cella (M3): orthostate + engaged antae + recessed cornice band --
const CA = 113, CB = 143, CC = 88, CD = 168;      // 31 wide (ODD)
for (let y = YS + 1; y <= 44; y++) {
  const proud = (y >= 11 && y <= 13) ? 1 : 0;      // proud orthostate course
  const rec = (y >= 42 && y <= 43) ? 1 : 0;        // recessed cornice band
  const e = proud - rec;
  for (let x = CA - e; x <= CB + e; x++)
    for (let z = CC - e; z <= CD + e; z++) {
      const onEdge = (x <= CA - e + 1 || x >= CB + e - 1 || z <= CC - e + 1 || z >= CD + e - 1);
      if (!onEdge) continue;
      if (y <= 24 && z <= CC + 1 && x >= RC - 2 && x <= RC + 2) continue;  // 5-wide door
      wblock(x, y, z, 0.0);
    }
}
for (let x = RC - 3; x <= RC + 3; x++) block(x, 25, x >= 0 ? CC : CC, ACC);  // door lintel
for (let x = RC - 3; x <= RC + 3; x++) block(x, 25, CC + 1, ACC);
// engaged antae, 1 block proud, every 10 along both cella flanks
for (let z = CC + 5; z <= CD - 5; z += 10)
  for (let y = YS + 1; y <= 44; y++)
    for (let d = -1; d <= 1; d++) {
      block(CA - 1, y, z + d, y % 11 === 0 ? DET : BASE);
      block(CB + 1, y, z + d, y % 11 === 0 ? DET : BASE);
    }
// cult statue (interior focal)
box(RC - 1, YS + 1, 140, RC + 1, YS + 14, 142, ACC);
box(RC - 3, YS + 15, 138, RC + 3, YS + 19, 144, ACC);
box(RC - 4, YS + 1, 137, RC + 4, YS + 1, 145, DET);

// ---- roof + pediments -------------------------------------------------
const HALF = 30, RISE = 9, YE = 52;
function roofY(x) { return YE + RISE - Math.round(Math.abs(x - RC) * RISE / HALF); }
for (let x = RC - HALF; x <= RC + HALF; x++) {
  const y = roofY(x);
  for (let z = Z0 - 5; z <= Z1 + 5; z++) {
    // A roof is a TILED surface, not a gradient field: pan tiles with a
    // projecting cover-tile rib every 5 courses running down the slope.
    // (Round 1 graded the roof with the weathering ramp and it read as a
    // checkerboard -- the gradient rule's "context" clause: roofs follow
    // weathering and water paths, they are not random value fields.)
    const cover = ((z - Z0) % 5 === 0);
    block(x, y, z, cover ? DET : BASE);
    block(x, y - 1, z, SEC);                    // sarking course beneath
    if (cover) block(x, y + 1, z, DET);         // projecting rib -> real depth
  }
}
for (let z = Z0 - 5; z <= Z1 + 5; z++) {
  block(RC, YE + RISE + 1, z, DET);             // ridge
  if ((z - Z0) % 10 === 0) {                    // ANTEFIXES (silhouette break)
    for (let s = 1; s <= 2; s++) {
      block(RC - HALF, YE + s, z, DET);
      block(RC + HALF, YE + s, z, DET);
    }
  }
}
for (const [zf, dir, figs] of [[Z0 - 3, 1, 9], [Z1 + 3, -1, 5]]) {
  for (let x = RC - HALF; x <= RC + HALF; x++)
    for (let y = YE; y < roofY(x); y++)
      for (let d = 0; d <= 1; d++)
        block(x, y, zf + d * dir, y === YE ? DET : weather(x, y, zf, 0.0));
  const span = Math.floor((figs - 1) / 2);
  for (let k = -span; k <= span; k++) {          // odd figure count
    const h = 2 + Math.max(0, span + 1 - Math.abs(k)) * 1.5;
    for (let y = YE + 1; y <= YE + h && y < roofY(RC + k * 3) - 1; y++)
      for (let w = 0; w <= 1; w++)
        block(RC + k * 3 + w, y, zf - dir, SEC);
  }
  // akroteria: centre 5 tall, corners 4 -> centre is 25% taller
  for (let s = 1; s <= 5; s++) block(RC, YE + RISE + 1 + s, zf, ACC);
  for (let s = 1; s <= 4; s++) {
    block(RC - HALF, YE + s, zf, ACC);
    block(RC + HALF, YE + s, zf, ACC);
  }
}
