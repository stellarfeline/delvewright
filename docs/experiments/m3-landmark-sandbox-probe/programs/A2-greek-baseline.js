// ===================================================================
// CONDITION A (baseline) — ancient Greek Doric peripteral temple.
// MineBench voxel.exec program. seed 121111, grid 256, "advanced".
// Hexastyle 6 x 13, stepped crepidoma, full colonnade, entablature with
// triglyph frieze, gable roof with pediments at both ends, cella inside.
// ===================================================================
const MARBLE = "quartz_block", STONE = "smooth_stone", WEATH = "sandstone";
const SHADE = "stone", GOLD = "gold_block";

const X0 = 107, X1 = 149;        // stylobate in X (43 wide  = 5*8 + 3)
const Z0 = 79, Z1 = 177;         // stylobate in Z (99 long  = 12*8 + 3)
const YS = 4;                    // stylobate surface

// ---- ground: a low rock outcrop so the temple is not floating -------
box(X0 - 22, 0, Z0 - 22, X1 + 22, 0, Z1 + 22, "grass_block");
for (let x = X0 - 20; x <= X1 + 20; x++)
  for (let z = Z0 - 20; z <= Z1 + 20; z++)
    if (rng() < 0.22) block(x, 0, z, rng() < 0.5 ? "andesite" : "gravel");

// ---- crepidoma: three steps ----------------------------------------
for (let s = 0; s < 3; s++) {
  const e = (3 - s) * 2;
  box(X0 - e, 1 + s, Z0 - e, X1 + e, 1 + s, Z1 + e, s === 2 ? MARBLE : STONE);
}
box(X0, YS, Z0, X1, YS, Z1, MARBLE);       // stylobate

// ---- peristyle: 6 x 13 Doric columns --------------------------------
const COLX = [], COLZ = [];
for (let i = 0; i < 6; i++) COLX.push(X0 + 1 + 8 * i);
for (let j = 0; j < 13; j++) COLZ.push(Z0 + 1 + 8 * j);
const YCAP = 25;
function column(cx, cz) {
  for (let y = YS + 1; y <= 23; y++)
    for (let dx = -1; dx <= 1; dx++)
      for (let dz = -1; dz <= 1; dz++)
        block(cx + dx, y, cz + dz, rng() < 0.07 ? WEATH : MARBLE);
  box(cx - 2, 24, cz - 2, cx + 2, 24, cz + 2, MARBLE);   // echinus
  box(cx - 2, YCAP, cz - 2, cx + 2, YCAP, cz + 2, STONE); // abacus
}
for (const cx of COLX) for (const cz of COLZ)
  if (cx === COLX[0] || cx === COLX[5] || cz === COLZ[0] || cz === COLZ[12])
    column(cx, cz);

// ---- entablature -----------------------------------------------------
// architrave (plain, 3 courses)
for (let y = 26; y <= 28; y++) {
  box(X0 - 1, y, Z0 - 1, X1 + 1, y, Z0 + 1, MARBLE);
  box(X0 - 1, y, Z1 - 1, X1 + 1, y, Z1 + 1, MARBLE);
  box(X0 - 1, y, Z0 - 1, X0 + 1, y, Z1 + 1, MARBLE);
  box(X1 - 1, y, Z0 - 1, X1 + 1, y, Z1 + 1, MARBLE);
}
// frieze: triglyphs every 4 blocks (over each column and each mid-bay)
function friezeRun(ax, az, bx, bz, along) {
  for (let y = 29; y <= 31; y++)
    for (let x = ax; x <= bx; x++)
      for (let z = az; z <= bz; z++) {
        const u = along === "x" ? x : z;
        const tri = ((u - (along === "x" ? X0 + 1 : Z0 + 1)) % 4 + 4) % 4 < 2;
        block(x, y, z, tri ? STONE : MARBLE);
      }
}
friezeRun(X0 - 1, Z0 - 1, X1 + 1, Z0 + 1, "x");
friezeRun(X0 - 1, Z1 - 1, X1 + 1, Z1 + 1, "x");
friezeRun(X0 - 1, Z0 - 1, X0 + 1, Z1 + 1, "z");
friezeRun(X1 - 1, Z0 - 1, X1 + 1, Z1 + 1, "z");
// cornice, overhanging 2
for (let y = 32; y <= 33; y++) {
  const e = y === 32 ? 2 : 3;
  box(X0 - e, y, Z0 - e, X1 + e, y, Z0 + 1, MARBLE);
  box(X0 - e, y, Z1 - 1, X1 + e, y, Z1 + e, MARBLE);
  box(X0 - e, y, Z0 - e, X0 + 1, y, Z1 + e, MARBLE);
  box(X1 - 1, y, Z0 - e, X1 + e, y, Z1 + e, MARBLE);
}

// ---- cella (naos) ----------------------------------------------------
const CX0 = X0 + 8, CX1 = X1 - 8, CZ0 = Z0 + 17, CZ1 = Z1 - 17;
for (let y = YS + 1; y <= 28; y++) {
  // front wall, with a 7-wide doorway left open (no air block exists)
  for (let x = CX0; x <= CX1; x++)
    for (let z = CZ0; z <= CZ0 + 1; z++)
      if (!(y <= 16 && x >= 125 && x <= 131)) block(x, y, z, MARBLE);
  box(CX0, y, CZ1 - 1, CX1, y, CZ1, MARBLE);
  box(CX0, y, CZ0, CX0 + 1, y, CZ1, MARBLE);
  box(CX1 - 1, y, CZ0, CX1, y, CZ1, MARBLE);
}
box(125, 17, CZ0, 131, 17, CZ0 + 1, STONE);   // lintel
// cult statue
box(127, YS + 1, 140, 129, YS + 8, 142, GOLD);
box(126, YS + 9, 139, 130, YS + 12, 143, GOLD);

// ---- roof: gable with pediments at both ends -------------------------
const RC = 128, HALF = 23, RISE = 8, YE = 34;
function roofY(x) { return YE + RISE - Math.round(Math.abs(x - RC) * RISE / HALF); }
for (let x = RC - HALF; x <= RC + HALF; x++) {
  const y = roofY(x);
  for (let z = Z0 - 5; z <= Z1 + 5; z++) block(x, y, z, MARBLE);
  for (let z = Z0 - 5; z <= Z1 + 5; z++) block(x, y - 1, z, STONE);
}
// ridge beam
for (let z = Z0 - 5; z <= Z1 + 5; z++) box(RC, YE + RISE + 1, z, RC, YE + RISE + 1, z, STONE);
// tympana (the two pediment triangles)
for (const zf of [Z0 - 3, Z1 + 3]) {
  for (let x = RC - HALF; x <= RC + HALF; x++)
    for (let y = YE; y < roofY(x); y++)
      for (let d = 0; d <= 1; d++)
        block(x, y, zf + (zf === Z0 - 3 ? d : -d),
              (y === YE) ? STONE : (rng() < 0.12 ? WEATH : MARBLE));
  // sculpture group suggestion in each tympanum
  for (let k = -4; k <= 4; k++) {
    const h = 3 + Math.max(0, 4 - Math.abs(k)) * 1.4;
    for (let y = YE + 1; y <= YE + h; y++)
      block(RC + k * 2, y, zf + (zf === Z0 - 3 ? -1 : 1), SHADE);
  }
  // akroterion at the apex
  box(RC, YE + RISE + 1, zf, RC, YE + RISE + 3, zf, GOLD);
}
// weathering pass over the marble
for (let x = X0 - 8; x <= X1 + 8; x++)
  for (let z = Z0 - 8; z <= Z1 + 8; z++)
    if (rng() < 0.02) block(x, YS, z, WEATH);
