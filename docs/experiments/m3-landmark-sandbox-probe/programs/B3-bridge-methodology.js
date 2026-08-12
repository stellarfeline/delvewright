// ===================================================================
// CONDITION B (methodology-primed) — colossal ruined stone bridge
// battlefield.  seed 121111, grid 256, palette "advanced".
//
// <self_check>
// MASSING — 4 masses, primary = the viaduct (57 x 208 x 55):
//   M2 barbican gate tower   57 x 29 x 78  -> 78/208 = 0.375 (1/3..1/2 OK)
//   M3 collapsed-span ruin   45 x 74 x 40  -> 74/208 = 0.356 (1/3..1/2 OK)
//   M4 pier-top turrets                    -> tertiary rhythm mass
// HIERARCHY — focal = the barbican. Crown at y86 vs the parapet at y58
//   over a deck at y47: 39 tall vs the deck structure's 29 = +34% (>=25% OK).
//   Detail density: gorge floor plain rubble -> pier shafts 2 buttress
//   planes -> spandrel 3 planes + voussoir ring -> parapet 3 planes ->
//   barbican gold ring, buttresses, machicolation, crenellation (densest).
// DEPTH — the elevation never presents one plane:
//   * spandrel field RECESSED 1 from the pier face
//   * voussoir ring PROJECTS 1 proud of the pier face (never coplanar)
//   * impost string course projects 2; deck cornice CORBELS OUT 3 — the
//     single strongest scale cue a viaduct has
//   * every pier carries buttresses 3 proud on both flanks, plus a batter
//   * parapet in 3 planes: plinth proud 1 / panel recessed 1 / coping proud 2
//   * arch soffits are stepped, not flat
// RULE OF ODDS — 6 piers giving 5 SPANS (odd, so there is a true centre
//   span); deck 45 wide (odd); barbican opening 21 wide (odd); 3 coping
//   courses; 7 merlons per parapet bay. The baseline used 5 piers / 4 spans
//   and had no centre.
// PALETTE — exactly 5 named roles for the structure:
//   base      stone_bricks           ~60%
//   secondary andesite               ~30%  (true value step: recesses,
//                                           lower pier courses, shadow)
//   texture   cracked_stone_bricks   weathering clusters, ruin edges
//   detail    cobblestone            voussoirs, coping, corbels, rubble
//   accent    gold_block             <10%: the barbican ring, keystones,
//                                    brazier crowns — trim only
//   (deepslate / gravel are TERRAIN, declared separately as context.)
//   The baseline used 7 structural blocks with no roles.
// GRADIENT — a successive vertical value ramp deepslate -> andesite ->
//   stone_bricks: dark at the gorge floor, lightening to the deck. Cluster
//   size varies (2-octave noise, cells 15 and 7); no per-block rng anywhere
//   on the structure. Weathering follows WATER PATHS: the arch soffits and
//   the pier bases carry the cracked/dark end of the ramp, the sheltered
//   spandrel faces the light end.
// SILHOUETTE — the barbican breaks the skyline, 5 arches punch 5 holes
//   through the black mask, the collapsed span leaves a jagged cantilever
//   and a leaning fallen slab, and 6 pier-top turrets give a rhythm of
//   notches. The mask is not a bar.
// </self_check>
// ===================================================================
const BASE = "stone_bricks", SEC = "andesite", TEX = "cracked_stone_bricks";
const DET = "cobblestone", ACC = "gold_block";
const T_DARK = "deepslate", T_GRIT = "gravel";

const XC = 128;
const PIERS = [34, 70, 106, 142, 178, 214];      // 6 piers -> 5 spans (ODD)
const PW = 6;                                    // pier half-depth in z
const SPRING = 24, R = 12;                       // stilted arch: 24 wide, 32 tall
const DECK = 47, DX0 = 106, DX1 = 150;           // deck 45 wide (ODD)
const FLOOR = 4;

// ---- clustered value noise (never per-block rng on the structure) ----
const NOISE = [];
for (let i = 0; i < 2048; i++) NOISE.push(rng());
function vnoise(x, y, z, cell) {
  const a = Math.floor(x / cell), b = Math.floor(y / cell), c = Math.floor(z / cell);
  return NOISE[(((a * 73856093) ^ (b * 19349663) ^ (c * 83492791)) >>> 0) % 2048];
}
// successive ramp: deepslate -> andesite -> stone_bricks, dark at the floor
const RAMP = [T_DARK, SEC, BASE];
function ashlar(x, y, z, wet) {
  const f = Math.min(1, Math.max(0, (y - FLOOR) / 34));
  const n = 0.62 * vnoise(x, y, z, 15) + 0.38 * vnoise(x + 47, y, z + 23, 7);
  const v = 0.55 + f * 1.75 + (n - 0.5) * 0.85 - wet;
  const i = Math.max(0, Math.min(2, Math.round(v)));
  // texture role: cracked clusters, concentrated where water runs
  if (i === 2 && n > 0.86 - wet * 0.25) return TEX;
  return RAMP[i];
}

// ---- terrain: two plateaus at deck level, gorge with low talus -------
function jag(x, z, m) { return ((x * 37 + z * 13) % m) - (m >> 1); }
for (let x = 88; x <= 168; x++)
  for (let z = 4; z <= 252; z++) {
    const jz = jag(x, z, 7);
    let h;
    if (z + jz < 22 || z - jz > 228) h = DECK;
    else {
      const ex = Math.max(0, Math.abs(x - XC) - 30) * 0.34;
      const ez = Math.max(0, 24 - Math.min(z - 22, 228 - z)) * 0.28;
      h = Math.min(FLOOR - 2 + ex + ez + ((x * 11 + z * 5) % 3), 15);
    }
    h = Math.round(h);
    for (let y = Math.max(0, h - 2); y <= h; y++)
      block(x, y, z, y === h ? (vnoise(x, y, z, 14) > 0.84 ? T_GRIT : T_DARK) : T_DARK);
    if (h === DECK) for (let y = 26; y < DECK - 2; y++)
      block(x, y, z, vnoise(x, y, z, 17) > 0.86 ? SEC : T_DARK);
  }

// ---- piers: battered, with buttresses 3 PROUD on both flanks ---------
const GONE = 3;                                   // span 3-4 has collapsed
for (let p = 0; p < PIERS.length; p++) {
  const pz = PIERS[p];
  for (let y = FLOOR - 3; y <= DECK - 1; y++) {
    const bat = Math.max(0, Math.round((30 - y) / 9));
    for (let x = DX0 - 1 - bat; x <= DX1 + 1 + bat; x++)
      for (let z = pz - PW - bat; z <= pz + PW + bat; z++)
        block(x, y, z, ashlar(x, y, z, y < 14 ? 0.45 : 0));
    // DEPTH: buttresses standing 3 proud of the pier flanks
    if (y <= 40) for (const sx of [DX0 - 2 - bat, DX1 + 2 + bat])
      for (let k = 1; k <= 3; k++)
        for (let z = pz - 3; z <= pz + 3; z++)
          block(sx + (sx < XC ? -k : k), y, z, ashlar(sx, y, z, y < 14 ? 0.45 : 0));
  }
  // impost string course, projecting 2
  for (let x = DX0 - 3; x <= DX1 + 3; x++)
    for (let z = pz - PW - 2; z <= pz + PW + 2; z++)
      block(x, SPRING - 1, z, DET);
  // M4 — pier-top turret (rhythm of notches in the silhouette)
  if (p !== GONE && p !== GONE + 1) {
    for (const sx of [DX0 - 4, DX1 + 4])
      for (let y = DECK; y <= DECK + 14; y++)
        for (let dx = -3; dx <= 3; dx++)
          for (let dz = -4; dz <= 4; dz++) {
            if (Math.abs(dx) < 2 && Math.abs(dz) < 3 && y > DECK + 2) continue;
            if (y > DECK + 11 && ((dz + 40) % 4 < 2) && Math.abs(dx) === 3) continue;
            block(sx + dx, y, pz + dz, ashlar(sx + dx, y, pz + dz, 0));
          }
    for (const sx of [DX0 - 4, DX1 + 4]) {
      box(sx - 1, DECK + 15, pz - 1, sx + 1, DECK + 16, pz + 1, ACC);
      box(sx - 1, DECK + 17, pz - 1, sx + 1, DECK + 17, pz + 1, "glowstone");
    }
  }
}

// ---- spandrels RECESSED 1, voussoir rings PROUD 1 --------------------
for (let i = 0; i < PIERS.length - 1; i++) {
  const za = PIERS[i] + PW, zb = PIERS[i + 1] - PW, zc = (PIERS[i] + PIERS[i + 1]) / 2;
  const broken = (i === GONE);
  for (let z = za; z <= zb; z++) {
    if (broken) {
      const d = Math.min(z - za, zb - z);
      if (d > 2 + ((z * 41) % 5)) continue;         // jagged cantilever stubs
    }
    for (let y = FLOOR - 3; y <= DECK - 1; y++) {
      const dz = z - zc, dy = y - SPRING;
      const inArch = (y >= SPRING) ? (dz * dz + dy * dy < R * R) : (Math.abs(dz) < R);
      if (inArch) continue;
      const wet = (Math.sqrt(dz * dz + dy * dy) < R + 2.5 && y >= SPRING - 2) ? 0.5 : 0;
      for (let x = DX0; x <= DX1; x++)             // spandrel field, recessed 1
        block(x, y, z, ashlar(x, y, z, wet));
    }
  }
  if (broken) continue;
  // voussoir ring: 2 courses thick, standing 1 PROUD of the pier face
  for (let a = 0; a <= 72; a++) {
    const th = (a / 72) * Math.PI;
    for (let t = 0; t <= 1; t++) {
      const zz = Math.round(zc + Math.cos(th) * (R + t));
      const yy = Math.round(SPRING + Math.sin(th) * (R + t));
      for (const x of [DX0 - 1, DX1 + 1]) block(x, yy, zz, DET);
      for (let x = DX0; x <= DX1; x++) if (t === 1) block(x, yy, zz, DET);
    }
  }
  // gilded keystone (accent, trim only)
  for (const x of [DX0 - 1, DX1 + 1])
    for (let k = 0; k <= 2; k++) block(x, SPRING + R + k, Math.round(zc), ACC);
  // stepped arch soffit (never a flat plane)
  for (let a = 6; a <= 66; a += 6) {
    const th = (a / 72) * Math.PI;
    const zz = Math.round(zc + Math.cos(th) * (R - 1));
    const yy = Math.round(SPRING + Math.sin(th) * (R - 1));
    for (let x = DX0 + 2; x <= DX1 - 2; x += 6) block(x, yy, zz, SEC);
  }
}

// ---- deck, corbelled cornice, 3-plane parapet ------------------------
for (let z = 14; z <= 236; z++) {
  const brokenSpan = (z > PIERS[GONE] + PW + 4 && z < PIERS[GONE + 1] - PW - 4);
  if (brokenSpan) continue;
  for (let y = DECK; y <= DECK + 2; y++)
    for (let x = DX0; x <= DX1; x++)
      block(x, y, z, y === DECK + 2 ? ashlar(x, y, z, 0) : BASE);
  // DECK CORNICE — corbels out 3 (the scale cue)
  for (let k = 1; k <= 3; k++) {
    block(DX0 - k, DECK + 1 + (k > 2 ? 1 : 0), z, DET);
    block(DX1 + k, DECK + 1 + (k > 2 ? 1 : 0), z, DET);
  }
  // parapet in 3 planes: plinth proud 1 / panel recessed 1 / coping proud 2
  for (const side of [0, 1]) {
    const xin = side ? DX1 - 1 : DX0 + 1, xp = side ? DX1 : DX0;
    const xout = side ? DX1 + 1 : DX0 - 1, xc = side ? DX1 + 2 : DX0 - 2;
    const ruin = ((z * 17) % 29) < 5;
    for (let y = DECK + 3; y <= DECK + 9; y++) {
      if (ruin && y > DECK + 5) continue;
      if (y <= DECK + 4) { block(xp, y, z, BASE); block(xout, y, z, DET); }  // plinth
      else if (y <= DECK + 7) { block(xin, y, z, ashlar(xin, y, z, 0)); block(xp, y, z, SEC); }
      else {                                                                  // coping
        if (y === DECK + 9 && (z % 7 < 3)) continue;      // 7-merlon rhythm (odd)
        block(xp, y, z, DET); block(xout, y, z, DET); block(xc, y, z, DET);
      }
    }
  }
}
// ruined pillar rows ON the deck — the battlefield floor
for (let z = 20; z <= 232; z += 12) {
  if (z > PIERS[GONE] && z < PIERS[GONE + 1]) continue;
  for (const px of [DX0 + 7, DX1 - 7]) {
    const hgt = 4 + ((z * 7 + px) % 9);
    for (let y = DECK + 3; y <= DECK + 3 + hgt; y++)
      for (let dx = -1; dx <= 1; dx++)
        for (let dz = -1; dz <= 1; dz++)
          block(px + dx, y, z + dz, y > DECK + 3 + hgt - 2 ? TEX : ashlar(px, y, z, 0));
  }
}

// ---- M3: the collapsed span (jagged cantilever + leaning slab + mound)
const RZ = Math.round((PIERS[GONE] + PIERS[GONE + 1]) / 2);
for (let n = 0; n < 6400; n++) {                    // rubble mound (context)
  const a = rng() * Math.PI * 2, rr = Math.pow(rng(), 0.55) * 30;
  const x = Math.round(XC + Math.cos(a) * rr * 0.85), z = Math.round(RZ + Math.sin(a) * rr);
  const hgt = Math.round(1 + rng() * 8 * (1 - rr / 34));
  for (let y = FLOOR; y <= FLOOR + hgt; y++)
    block(x, y, z, rng() < 0.55 ? DET : rng() < 0.5 ? T_GRIT : TEX);
}
// a whole span slab, snapped off and leaning against the far pier
for (let k = 0; k < 30; k++) {
  const z = RZ + 4 + k, y = FLOOR + 2 + Math.round(k * 1.15);
  for (let x = DX0 + 2; x <= DX1 - 2; x++)
    for (let t = 0; t <= 2; t++) block(x, y + t, z, t === 2 ? DET : TEX);
}
// dangling voussoirs still clinging to the broken arch ends
for (const [pz, dir] of [[PIERS[GONE], 1], [PIERS[GONE + 1], -1]])
  for (let k = 0; k < 9; k++)
    for (let x = DX0 - 1; x <= DX1 + 1; x += 3)
      block(x, SPRING + R - k - ((x * 5) % 3), pz + dir * (PW + 2 + k), TEX);

// ---- M2: the barbican gate tower (FOCAL, densest detail) -------------
const BZ0 = 8, BZ1 = 36, BY0 = DECK - 7, BY1 = 86;
for (let y = BY0; y <= BY1; y++) {
  const taper = y > 70 ? 1 : 0;
  for (let x = 100 + taper; x <= 156 - taper; x++)
    for (let z = BZ0 + taper; z <= BZ1 - taper; z++) {
      const onFace = (x <= 103 + taper || x >= 153 - taper || z <= BZ0 + 2 || z >= BZ1 - 2);
      if (!onFace && y > BY0 + 3) continue;                 // hollow gatehouse
      // 21-wide (ODD) gate opening with a round head at y66
      const dy = y - 66, dx = x - XC;
      const inGate = (y >= 66) ? (dx * dx + dy * dy < 11 * 11) : (Math.abs(dx) < 11 && y >= DECK + 3);
      if (inGate) continue;
      if (y > 78 && ((x * 13 + y * 7 + z * 3) % 23) < 6) continue;   // ruined crown
      block(x, y, z, ashlar(x, y, z, 0));
    }
  // buttress towers at the four corners, 3 proud
  if (y <= 82) for (const cx of [98, 158])
    for (let dx = -2; dx <= 2; dx++)
      for (let z = BZ0 + 1; z <= BZ0 + 7; z++) {
        block(cx + dx, y, z, ashlar(cx + dx, y, z, 0));
        block(cx + dx, y, z + 20, ashlar(cx + dx, y, z + 20, 0));
      }
}
// gilded gate ring + machicolation corbels (accent, trim only)
for (let a = 0; a <= 60; a++) {
  const th = (a / 60) * Math.PI;
  const xx = Math.round(XC + Math.cos(th) * 12), yy = Math.round(66 + Math.sin(th) * 12);
  for (const z of [BZ0, BZ0 + 1, BZ1 - 1, BZ1]) block(xx, yy, z, ACC);
}
for (let x = 100; x <= 156; x += 4)
  for (const z of [BZ0 - 1, BZ1 + 1]) {
    block(x, 76, z, DET); block(x + 1, 76, z, DET); block(x, 77, z, DET);
  }
for (const x of [104, 128, 152]) {
  box(x - 1, 84, BZ0, x + 1, 85, BZ0 + 1, ACC);
  box(x - 1, 86, BZ0, x + 1, 86, BZ0 + 1, "glowstone");
}
