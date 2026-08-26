/* Interactive prefab viewer.
 *
 * The blocks are drawn by deepslate, from the pinned version's own blockstate
 * definitions, models and textures — the same chain the game walks. This file is
 * everything that is not that: the atlas the library does not pack for
 * non-16×16 textures, the resource provider, the camera, the overlays (anchors,
 * bounds, ground), the panel, and the checks that say whether what the reviewer
 * is looking at is the building the file describes.
 *
 * Geometry arrives run-length encoded over the grid and is rebuilt through the
 * renderer's own `addBlock`, so a zone reassembled from several structure
 * templates is one building here and nothing downstream knows a tile existed.
 */
"use strict";

(function () {
  const D = deepslate;
  const DATA = JSON.parse(document.getElementById("delve-model").textContent);
  const EYE = DATA.eye_height;
  const WAY_IN = DATA.way_in_stems || [];

  const canvas = document.getElementById("view");
  const labelLayer = document.getElementById("labels");
  const readout = document.getElementById("readout");
  const fallback = document.getElementById("fallback");

  const gl = canvas.getContext("webgl", { antialias: true, alpha: false })
    || canvas.getContext("experimental-webgl", { antialias: true, alpha: false });

  if (!gl) {
    canvas.hidden = true;
    fallback.hidden = false;
    fallback.textContent =
      "This browser reports no WebGL context, so the model cannot be drawn. "
      + "The page is otherwise intact: the block list, the anchor list and the "
      + "findings below are all readable in the panel.";
    readout.hidden = true;
  }

  /* ---------------------------------------------------------------- math -- */

  function mat4() { return new Float32Array(16); }

  function lookAt(out, eye, center, up) {
    let zx = eye[0] - center[0], zy = eye[1] - center[1], zz = eye[2] - center[2];
    let zl = Math.hypot(zx, zy, zz) || 1;
    zx /= zl; zy /= zl; zz /= zl;
    let xx = up[1] * zz - up[2] * zy, xy = up[2] * zx - up[0] * zz, xz = up[0] * zy - up[1] * zx;
    let xl = Math.hypot(xx, xy, xz);
    if (xl < 1e-6) { xx = 1; xy = 0; xz = 0; } else { xx /= xl; xy /= xl; xz /= xl; }
    const yx = zy * xz - zz * xy, yy = zz * xx - zx * xz, yz = zx * xy - zy * xx;
    out[0] = xx; out[1] = yx; out[2] = zx; out[3] = 0;
    out[4] = xy; out[5] = yy; out[6] = zy; out[7] = 0;
    out[8] = xz; out[9] = yz; out[10] = zz; out[11] = 0;
    out[12] = -(xx * eye[0] + xy * eye[1] + xz * eye[2]);
    out[13] = -(yx * eye[0] + yy * eye[1] + yz * eye[2]);
    out[14] = -(zx * eye[0] + zy * eye[1] + zz * eye[2]);
    out[15] = 1;
    return out;
  }

  function perspective(out, fovy, aspect, near, far) {
    const f = 1 / Math.tan(fovy / 2), nf = 1 / (near - far);
    out.fill(0);
    out[0] = f / aspect; out[5] = f; out[11] = -1;
    out[10] = (far + near) * nf; out[14] = 2 * far * near * nf;
    return out;
  }

  function multiply(out, a, b) {
    for (let c = 0; c < 4; c++) {
      const b0 = b[c * 4], b1 = b[c * 4 + 1], b2 = b[c * 4 + 2], b3 = b[c * 4 + 3];
      out[c * 4] = b0 * a[0] + b1 * a[4] + b2 * a[8] + b3 * a[12];
      out[c * 4 + 1] = b0 * a[1] + b1 * a[5] + b2 * a[9] + b3 * a[13];
      out[c * 4 + 2] = b0 * a[2] + b1 * a[6] + b2 * a[10] + b3 * a[14];
      out[c * 4 + 3] = b0 * a[3] + b1 * a[7] + b2 * a[11] + b3 * a[15];
    }
    return out;
  }

  /* ------------------------------------------------------------ decoding -- */

  function unbase64(s) {
    const bin = atob(s);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  }

  /** Run-length pairs (value u16, length u16) back into a dense grid. */
  function decodeGrid(b64, cells) {
    const bytes = unbase64(b64);
    const grid = new Uint16Array(cells);
    let at = 0;
    for (let i = 0; i + 3 < bytes.length; i += 4) {
      const v = bytes[i] | (bytes[i + 1] << 8);
      const n = bytes[i + 2] | (bytes[i + 3] << 8);
      if (v === 0) { at += n; continue; }
      const end = Math.min(at + n, cells);
      grid.fill(v, at, end);
      at += n;
    }
    return grid;
  }

  /* ----------------------------------------------------------- the atlas -- */
  /*
   * Packed here rather than by the library. `TextureAtlas.fromBlobs` crops every
   * texture to 16×16, which destroys a chest (64×64) and every sign and bed —
   * and it sizes its canvas from `upperPowerOfTwo(sqrt(n + 1))` while writing
   * the first texture at index 1, so at a count whose square root is already a
   * power of two the LAST textures land one row past the bottom edge and vanish
   * with nothing said. A jar-scale atlas is squarely in that range.
   *
   * So: shelf-pack at native size, then check the packing. Both failures are
   * invisible in a finished picture — a dropped texture is magenta, which is
   * exactly what a prefab naming a block the version dropped also looks like —
   * so the check reports a count and the page states it.
   */
  const ATLAS_MAX = 8192;

  function shelfPack(cells, size) {
    // Tallest first, ties by id, so the packing is a pure function of the input.
    const sorted = cells.slice().sort((a, b) => b.h - a.h || (a.id < b.id ? -1 : 1));
    const out = [];
    // The first 16×16 cell is reserved for the invalid-texture checker, so an
    // id nobody supplied reads as magenta rather than as some other block.
    let x = 16, y = 0, shelf = 16;
    for (const c of sorted) {
      if (c.w > size || c.h > size) return null;
      if (x + c.w > size) { x = 0; y += shelf; shelf = 0; }
      if (y + c.h > size) return null;
      out.push({ id: c.id, x, y, w: c.w, h: c.h });
      x += c.w;
      if (c.h > shelf) shelf = c.h;
    }
    return out;
  }

  /**
   * Every way the packing can be wrong, as a list of complaints.
   *
   * Counted, in bounds, non-overlapping, and clear of the checker cell. That is
   * the whole of what "correctly placed" means, and a page that reported nothing
   * here without checking would be reporting that it had not looked.
   */
  function auditPacking(placed, ids, size) {
    const bad = [];
    if (placed.length !== ids.length) {
      bad.push("packed " + placed.length + " of " + ids.length + " textures");
    }
    for (const p of placed) {
      if (p.x < 0 || p.y < 0 || p.x + p.w > size || p.y + p.h > size) {
        bad.push(p.id + " is placed outside the atlas");
      }
      if (p.x < 16 && p.y < 16) bad.push(p.id + " overlaps the invalid-texture cell");
    }
    // Overlap, on the shelf structure rather than pairwise over everything: two
    // rects can only collide within a row band, and the rows are what the packer
    // builds.
    const byRow = new Map();
    for (const p of placed) {
      if (!byRow.has(p.y)) byRow.set(p.y, []);
      byRow.get(p.y).push(p);
    }
    for (const row of byRow.values()) {
      row.sort((a, b) => a.x - b.x);
      for (let i = 1; i < row.length; i++) {
        if (row[i].x < row[i - 1].x + row[i - 1].w) {
          bad.push(row[i].id + " overlaps " + row[i - 1].id);
        }
      }
    }
    return bad;
  }

  function buildAtlas(textures, done) {
    const ids = Object.keys(textures).sort();
    const cells = ids.map((id) => ({ id, w: textures[id].w, h: textures[id].ch }));
    let size = 64, placed = null;
    while (size <= ATLAS_MAX) {
      placed = shelfPack(cells, size);
      if (placed) break;
      size *= 2;
    }
    if (!placed) {
      done(null, size, ["no atlas up to " + ATLAS_MAX + "px holds " + ids.length + " textures"]);
      return;
    }
    const complaints = auditPacking(placed, ids, size);

    const cv = document.createElement("canvas");
    cv.width = cv.height = size;
    const ctx = cv.getContext("2d");
    ctx.imageSmoothingEnabled = false;
    ctx.fillStyle = "black"; ctx.fillRect(0, 0, 16, 16);
    ctx.fillStyle = "magenta"; ctx.fillRect(0, 0, 8, 8); ctx.fillRect(8, 8, 8, 8);

    const idMap = {};
    for (const p of placed) {
      idMap[p.id] = [p.x / size, p.y / size, (p.x + p.w) / size, (p.y + p.h) / size];
    }

    let pending = placed.length;
    const finish = () => {
      const img = ctx.getImageData(0, 0, size, size);
      done(new D.TextureAtlas(img, idMap), size, complaints);
    };
    if (pending === 0) { finish(); return; }
    for (const p of placed) {
      const img = new Image();
      img.onload = () => {
        ctx.drawImage(img, 0, 0, p.w, p.h, p.x, p.y, p.w, p.h);
        if (--pending === 0) finish();
      };
      img.onerror = () => {
        complaints.push(p.id + " could not be decoded");
        if (--pending === 0) finish();
      };
      img.src = "data:image/png;base64," + textures[p.id].b64;
    }
  }

  /* -------------------------------------------------------- the resources -- */

  const blockDefs = {}, blockModels = {};
  for (const k of Object.keys(DATA.blockstates)) {
    blockDefs[k] = D.BlockDefinition.fromJson(DATA.blockstates[k]);
  }
  for (const k of Object.keys(DATA.block_models)) {
    blockModels[k] = D.BlockModel.fromJson(DATA.block_models[k]);
  }

  function makeResources(atlas) {
    const res = {
      getBlockDefinition(id) { return blockDefs[id.toString()] || null; },
      getBlockModel(id) { return blockModels[id.toString()] || null; },
      getTextureUV(id) { return atlas.getTextureUV(id); },
      getTextureAtlas() { return atlas.getTextureAtlas(); },
      getPixelSize() { return atlas.getPixelSize(); },
      getBlockFlags(id) { return DATA.flags[id.toString()] || null; },
      // The legal values of every property, from the pinned registry. Nothing on
      // the render path reads it; it is part of the interface and is answered
      // honestly rather than with a null.
      getBlockProperties(id) {
        const d = DATA.defaults[id.toString()];
        if (!d) return null;
        const out = {};
        for (const k of Object.keys(d)) out[k] = [d[k]];
        return out;
      },
      // What the game fills an unwritten property with. From the pinned block
      // registry, never from a guess: a bare cobblestone_wall is a POST, and
      // "the first legal value" would make it something else.
      getDefaultBlockProperties(id) { return DATA.defaults[id.toString()] || null; },
    };
    for (const k of Object.keys(blockModels)) blockModels[k].flatten(res);
    return res;
  }

  /* ------------------------------------------------------------- overlays -- */
  /*
   * Anchors, the bounding box and the ground grid, drawn by this page's own
   * program on the same context. deepslate re-binds its program and its
   * attributes on every draw call, so the two coexist as long as this one puts
   * its attribute arrays back when it is done.
   */
  let ovProg = null, ovPos = 0, ovCol = 0, ovMVP = null, ovPosBuf = null, ovColBuf = null;

  function compile(type, src) {
    const s = gl.createShader(type);
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
      throw new Error(gl.getShaderInfoLog(s) || "shader compile failed");
    }
    return s;
  }

  function initOverlay() {
    const vs = compile(gl.VERTEX_SHADER,
      "attribute vec3 aPos;attribute vec4 aCol;uniform mat4 uMVP;varying vec4 vCol;"
      + "void main(){vCol=aCol;gl_Position=uMVP*vec4(aPos,1.0);}");
    const fs = compile(gl.FRAGMENT_SHADER,
      "precision mediump float;varying vec4 vCol;void main(){gl_FragColor=vCol;}");
    ovProg = gl.createProgram();
    gl.attachShader(ovProg, vs); gl.attachShader(ovProg, fs); gl.linkProgram(ovProg);
    if (!gl.getProgramParameter(ovProg, gl.LINK_STATUS)) {
      throw new Error(gl.getProgramInfoLog(ovProg) || "overlay program link failed");
    }
    ovPos = gl.getAttribLocation(ovProg, "aPos");
    ovCol = gl.getAttribLocation(ovProg, "aCol");
    ovMVP = gl.getUniformLocation(ovProg, "uMVP");
    ovPosBuf = gl.createBuffer();
    ovColBuf = gl.createBuffer();
  }

  function drawOverlay(mode, positions, colors, count, mvp) {
    if (!ovProg || !count) return;
    gl.useProgram(ovProg);
    gl.uniformMatrix4fv(ovMVP, false, mvp);
    gl.bindBuffer(gl.ARRAY_BUFFER, ovPosBuf);
    gl.bufferData(gl.ARRAY_BUFFER, positions, gl.DYNAMIC_DRAW);
    gl.enableVertexAttribArray(ovPos);
    gl.vertexAttribPointer(ovPos, 3, gl.FLOAT, false, 0, 0);
    gl.bindBuffer(gl.ARRAY_BUFFER, ovColBuf);
    gl.bufferData(gl.ARRAY_BUFFER, colors, gl.DYNAMIC_DRAW);
    gl.enableVertexAttribArray(ovCol);
    gl.vertexAttribPointer(ovCol, 4, gl.UNSIGNED_BYTE, true, 0, 0);
    gl.drawArrays(mode, 0, count);
    gl.disableVertexAttribArray(ovPos);
    gl.disableVertexAttribArray(ovCol);
  }

  /* --------------------------------------------------------------- state -- */

  const state = {
    model: null,
    renderer: null,
    resources: null,
    cutY: 0,
    // "walk" is the page's resting state and its opening state. Orbit exists,
    // but only because a button that says so was pressed — never as the mode a
    // reviewer discovers by finding that W does nothing.
    mode: "walk",
    target: [0, 0, 0],
    dist: 30,
    // shared angles: yaw 0 looks toward +Z, pitch is up-positive
    yaw: Math.PI * 0.25,
    pitch: 0.5,
    eye: [0, 0, 0],
    preset: "",
    show: { anchors: true, labels: true, bounds: false, ground: true },
    atlasSize: 0,
    atlasComplaints: [],
    noGeometry: [],
  };

  const FACING_YAW = { south: 0, west: -Math.PI / 2, north: Math.PI, east: Math.PI / 2 };

  function cameraEye() {
    if (state.mode === "walk") return state.eye;
    const cp = Math.cos(state.pitch);
    return [
      state.target[0] + state.dist * cp * Math.sin(state.yaw),
      state.target[1] + state.dist * Math.sin(state.pitch),
      state.target[2] + state.dist * cp * Math.cos(state.yaw),
    ];
  }

  function cameraCenter() {
    if (state.mode !== "walk") return state.target;
    const cp = Math.cos(state.pitch);
    return [
      state.eye[0] + cp * Math.sin(state.yaw),
      state.eye[1] + Math.sin(state.pitch),
      state.eye[2] + cp * Math.cos(state.yaw),
    ];
  }

  // The renderer's own field of view, so the overlay geometry and the blocks
  // share one projection instead of two that agree by coincidence.
  const FOV = 70 * Math.PI / 180;

  /**
   * Distance at which the whole box is in frame with a margin.
   *
   * The binding constraint is the NARROWER of the two half-angles: with a tall
   * viewport the horizontal one is smaller, and solving only for the vertical
   * one pushes the model off the sides.
   */
  function fitDistance(model) {
    const [sx, sy, sz] = model.size;
    const r = Math.hypot(sx, sy, sz) / 2;
    const aspect = Math.max(0.2, canvas.clientWidth / Math.max(1, canvas.clientHeight));
    const halfY = FOV / 2;
    const halfX = Math.atan(Math.tan(halfY) * aspect);
    return (r / Math.sin(Math.min(halfX, halfY))) * 1.12;
  }

  /* ------------------------------------------------------------- presets -- */

  function stemOf(name) {
    const last = String(name).split("/").pop();
    return last.replace(/-\d+$/, "");
  }

  /**
   * Is this anchor a way in — the piece's own declaration first, then the
   * reading of its name.
   *
   * One function because the page answers this question in three places (the
   * default preset, the marker colour, the label class) and they must agree:
   * an anchor drawn in way-in blue whose label is styled as an ordinary anchor
   * is the same anchor described two ways. The two spellings in `WAY_IN` are a
   * fallback for pieces admitted before the role existed; a piece that declares
   * the role is never reached by a name.
   */
  function isWayIn(a) {
    return a.role === "entry" || WAY_IN.indexOf(stemOf(a.name)) >= 0;
  }

  function anchorPresets(model) {
    const out = [];
    for (const a of model.anchors) {
      if (!a.pos) continue;
      out.push({
        id: "pov:" + a.name,
        label: (a.socket ? "▸ " : "") + a.name.replace(/^anchor\//, ""),
      });
    }
    return out;
  }

  /** True when a standing player's eye at this anchor would be inside a block. */
  function eyeIsBuried(model, a) {
    const [sx, sy, sz] = model.size;
    const x = a.pos[0], y = Math.floor(a.pos[1] + EYE), z = a.pos[2];
    if (x < 0 || y < 0 || z < 0 || x >= sx || y >= sy || z >= sz) return false;
    const p = model.grid[(y * sz + z) * sx + x];
    return p !== 0 && model.solid[p] === 1;
  }

  /**
   * The way in, by declaration: the first anchor whose name stem is one the
   * engine reserves for the party's arrival, else the first jigsaw socket.
   *
   * An anchor whose eye height lands inside a solid block is skipped for the
   * DEFAULT only — opening the page on the inside of a wall teaches the reviewer
   * nothing. Every anchor still has its own button, including that one, because
   * an anchor buried in rock is itself worth being able to look at.
   */
  function defaultPreset(model) {
    const usable = (a) => a && a.pos && !eyeIsBuried(model, a);
    // What the piece SAYS it is, before any reading of what it is called.
    const declared = model.anchors.find((a) => usable(a) && a.role === "entry");
    if (declared) return "pov:" + declared.name;
    for (const stem of WAY_IN) {
      const hit = model.anchors.find((a) => usable(a) && stemOf(a.name) === stem);
      if (hit) return "pov:" + hit.name;
    }
    const socket = model.anchors.find((a) => usable(a) && a.socket);
    if (socket) return "pov:" + socket.name;
    // A prefab that declares no way in still opens on a pair of feet, standing
    // off the piece at eye height. Opening in orbit instead is what made W do
    // nothing on exactly the prefabs whose interiors most needed walking.
    return "ground";
  }

  /**
   * `#model=<id>&preset=<id>` selects what the page opens on, so a reviewer can
   * be sent straight to the view being discussed — and so an automated check can
   * open any preset without driving the UI.
   */
  function fromHash() {
    const h = (location.hash || "").replace(/^#/, "");
    const out = {};
    for (const part of h.split("&")) {
      const i = part.indexOf("=");
      if (i > 0) out[decodeURIComponent(part.slice(0, i))] = decodeURIComponent(part.slice(i + 1));
    }
    return out;
  }

  function centerOf(model) {
    return [model.size[0] / 2, model.size[1] / 2, model.size[2] / 2];
  }

  function applyPreset(id) {
    const model = state.model;
    state.preset = id;
    if (id === "ground") {
      // Feet on the ground, off the south face, far enough back that the whole
      // piece is in frame — and walkable from the first frame.
      const c = centerOf(model);
      state.mode = "walk";
      state.eye = [c[0], EYE, c[2] - fitDistance(model) * 0.85];
      state.yaw = 0;      // looking +Z, at the piece
      state.pitch = 0.18;
    } else if (id === "exterior") {
      state.mode = "orbit";
      state.target = centerOf(model);
      state.dist = fitDistance(model);
      state.yaw = Math.PI * 0.25;
      state.pitch = 0.48;
    } else if (id === "plan") {
      state.mode = "orbit";
      state.target = centerOf(model);
      state.dist = fitDistance(model);
      state.yaw = Math.PI;
      state.pitch = Math.PI / 2 - 0.001;
    } else if (id.startsWith("pov:")) {
      const name = id.slice(4);
      const a = model.anchors.find((x) => x.name === name);
      if (!a || !a.pos) return;
      state.mode = "walk";
      // Feet in the anchor's cell, eyes 1.62 blocks above its floor — the same
      // offset the game gives a standing player.
      state.eye = [a.pos[0] + 0.5, a.pos[1] + EYE, a.pos[2] + 0.5];
      let yaw = FACING_YAW[a.facing] !== undefined ? FACING_YAW[a.facing] : Math.PI;
      // A jigsaw socket's facing points OUT of the piece; standing in a doorway
      // to review the room means looking the other way.
      if (a.socket) yaw += Math.PI;
      state.yaw = yaw;
      state.pitch = 0;
    }
    syncPresetButtons();
    updateReadout();
  }

  /**
   * Orbit is a switch the reviewer throws, never a state the page puts them in.
   *
   * Turning it on frames the whole piece — that is what the button offers, so
   * that is what it should do. Turning it off leaves the camera exactly where
   * the orbit put it, facing the model: yaw turns through half a circle and
   * pitch inverts, because the orbit's angles describe where the eye sits
   * relative to the target and a walker's describe where they are looking.
   */
  function setOrbit(on) {
    if (on === (state.mode === "orbit")) return;
    if (on) {
      applyPreset("exterior");
    } else {
      const e = cameraEye();
      state.mode = "walk";
      state.eye = [e[0], e[1], e[2]];
      state.yaw += Math.PI;
      state.pitch = C.clampPitch(-state.pitch);
      state.preset = "";
    }
    syncPresetButtons();
    updateReadout();
    invalidate();
  }

  /* ------------------------------------------------------------- drawing -- */

  let mvp = mat4(), proj = mat4(), view = mat4();
  let groundVerts = null, groundColors = null, groundCount = 0;
  let boundsVerts = null, boundsColors = null, boundsCount = 0;

  function buildOverlays(model) {
    const [sx, sy, sz] = model.size;
    const lines = [];
    for (let x = 0; x <= sx; x++) lines.push(x, 0, 0, x, 0, sz);
    for (let z = 0; z <= sz; z++) lines.push(0, 0, z, sx, 0, z);
    groundVerts = new Float32Array(lines);
    groundCount = lines.length / 3;
    groundColors = new Uint8Array(groundCount * 4);
    for (let i = 0; i < groundCount; i++) {
      groundColors[i * 4] = 62; groundColors[i * 4 + 1] = 70;
      groundColors[i * 4 + 2] = 88; groundColors[i * 4 + 3] = 255;
    }

    const c = [[0, 0, 0], [sx, 0, 0], [sx, 0, sz], [0, 0, sz],
    [0, sy, 0], [sx, sy, 0], [sx, sy, sz], [0, sy, sz]];
    const edges = [[0, 1], [1, 2], [2, 3], [3, 0], [4, 5], [5, 6], [6, 7], [7, 4],
    [0, 4], [1, 5], [2, 6], [3, 7]];
    const bl = [];
    for (const [a, b] of edges) bl.push(...c[a], ...c[b]);
    boundsVerts = new Float32Array(bl);
    boundsCount = bl.length / 3;
    boundsColors = new Uint8Array(boundsCount * 4);
    for (let i = 0; i < boundsCount; i++) {
      boundsColors[i * 4] = 110; boundsColors[i * 4 + 1] = 168;
      boundsColors[i * 4 + 2] = 254; boundsColors[i * 4 + 3] = 255;
    }
  }

  function resize() {
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const w = Math.max(1, Math.round(canvas.clientWidth * dpr));
    const h = Math.max(1, Math.round(canvas.clientHeight * dpr));
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w; canvas.height = h;
      if (state.renderer) state.renderer.setViewport(0, 0, w, h);
    }
  }

  let needsDraw = true;
  function invalidate() { needsDraw = true; }

  function draw() {
    if (!gl || !state.renderer) return;
    resize();
    gl.viewport(0, 0, canvas.width, canvas.height);
    gl.clearColor(0.078, 0.086, 0.110, 1);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.enable(gl.DEPTH_TEST);

    const aspect = canvas.width / Math.max(1, canvas.height);
    perspective(proj, FOV, aspect, 0.05, 4000);
    lookAt(view, cameraEye(), cameraCenter(), [0, 1, 0]);
    multiply(mvp, proj, view);

    if (state.show.ground && groundCount) {
      drawOverlay(gl.LINES, groundVerts, groundColors, groundCount, mvp);
    }

    // The renderer keeps its own projection, taken from the canvas' CSS size at
    // the last `setViewport`, so it is handed only the view.
    state.renderer.drawStructure(view);

    if (state.show.bounds && boundsCount) {
      drawOverlay(gl.LINES, boundsVerts, boundsColors, boundsCount, mvp);
    }
    if (state.show.anchors) drawAnchors();
    positionLabels();
  }

  /** Anchors as small octahedra, plus a wireframe box for region anchors. */
  function drawAnchors() {
    const model = state.model;
    if (!model.anchors.length) return;
    const tri = [], col = [], lin = [], lcol = [];
    for (const a of model.anchors) {
      const c = a.socket ? [110, 220, 160] : isWayIn(a) ? [110, 168, 254] : [255, 180, 84];
      if (a.pos) {
        const [x, y, z] = [a.pos[0] + 0.5, a.pos[1] + 0.5, a.pos[2] + 0.5];
        const s = 0.28;
        const p = [[x, y + s, z], [x, y - s, z], [x + s, y, z], [x - s, y, z], [x, y, z + s], [x, y, z - s]];
        const faces = [[0, 2, 4], [0, 4, 3], [0, 3, 5], [0, 5, 2],
        [1, 4, 2], [1, 3, 4], [1, 5, 3], [1, 2, 5]];
        for (const f of faces) {
          for (const k of f) { tri.push(p[k][0], p[k][1], p[k][2]); col.push(c[0], c[1], c[2], 255); }
        }
        // A stalk to the floor, so a marker in mid-air still reads as "here".
        lin.push(x, y, z, x, a.pos[1], z);
        lcol.push(c[0], c[1], c[2], 255, c[0], c[1], c[2], 255);
      }
      if (a.from && a.to) {
        const f = a.from, t = [a.to[0] + 1, a.to[1] + 1, a.to[2] + 1];
        const cs = [[f[0], f[1], f[2]], [t[0], f[1], f[2]], [t[0], f[1], t[2]], [f[0], f[1], t[2]],
        [f[0], t[1], f[2]], [t[0], t[1], f[2]], [t[0], t[1], t[2]], [f[0], t[1], t[2]]];
        const edges = [[0, 1], [1, 2], [2, 3], [3, 0], [4, 5], [5, 6], [6, 7], [7, 4],
        [0, 4], [1, 5], [2, 6], [3, 7]];
        for (const [i, j] of edges) {
          lin.push(...cs[i], ...cs[j]);
          lcol.push(c[0], c[1], c[2], 255, c[0], c[1], c[2], 255);
        }
      }
    }
    if (tri.length) drawOverlay(gl.TRIANGLES, new Float32Array(tri), new Uint8Array(col), tri.length / 3, mvp);
    if (lin.length) drawOverlay(gl.LINES, new Float32Array(lin), new Uint8Array(lcol), lin.length / 3, mvp);
  }

  const labelEls = new Map();
  let labelModel = null;

  /**
   * The label elements belong to the model on screen, not to the page.
   *
   * They are a per-model panel structure exactly like the preset buttons, the
   * anchor list and the legend — and like those three, the whole of them is
   * discarded when the model changes. The difference is where the rebuild can
   * live: those are built once per model in `selectModel`, while these are
   * built lazily inside the draw pass, so `selectModel` is not the place that
   * can empty them.
   *
   * That is deliberately not repaired by clearing the layer at the switch. The
   * pool is asked here, in the pass that draws it, whose model it holds, so a
   * label cannot outlive its scene by ANY route — the picker, a `#model=`
   * fragment at boot, the headless surface, or a front end this page has not
   * grown yet. A clearing call at a switch site would be correct only for the
   * switch sites that exist today.
   *
   * Keying by name is also only sound within one model: names are unique per
   * prefab and not across a page of them (a page of the eight Halgrave zones
   * has 187 anchors under 181 distinct names). Held across a switch, a shared
   * name would hand the new scene the old scene's element — with the old
   * scene's socket and way-in styling frozen in at creation.
   */
  function labelPoolFor(model) {
    if (labelModel === model) return labelEls;
    labelLayer.textContent = "";
    labelEls.clear();
    labelModel = model;
    return labelEls;
  }

  /**
   * Place anchor labels nearest-first, dropping any that would land on one
   * already placed.
   *
   * A prefab can declare thirty anchors in a small box; drawn unconditionally
   * they overlap into an unreadable stack, which is worse than no labels at all
   * because it hides the model too. The markers stay — every anchor is still
   * visible as a marker — and the panel lists all of them by name.
   */
  function positionLabels() {
    const model = state.model;
    labelPoolFor(model);
    const wanted = state.show.anchors && state.show.labels;
    if (!wanted) {
      for (const el of labelEls.values()) el.style.display = "none";
      return;
    }
    const w = canvas.clientWidth, h = canvas.clientHeight;
    const eye = cameraEye();
    const candidates = [];

    for (const a of model.anchors) {
      if (!a.pos) continue;
      let el = labelEls.get(a.name);
      if (!el) {
        el = document.createElement("div");
        el.className = "label" + (a.socket ? " socket" : (isWayIn(a) ? " way-in" : ""));
        el.textContent = a.name.replace(/^anchor\//, "");
        labelLayer.appendChild(el);
        labelEls.set(a.name, el);
      }
      el.style.display = "none";
      const p = [a.pos[0] + 0.5, a.pos[1] + 0.95, a.pos[2] + 0.5];
      const cx = mvp[0] * p[0] + mvp[4] * p[1] + mvp[8] * p[2] + mvp[12];
      const cy = mvp[1] * p[0] + mvp[5] * p[1] + mvp[9] * p[2] + mvp[13];
      const cw = mvp[3] * p[0] + mvp[7] * p[1] + mvp[11] * p[2] + mvp[15];
      if (cw <= 0.001) continue;
      const nx = cx / cw, ny = cy / cw;
      if (nx < -1 || nx > 1 || ny < -1 || ny > 1) continue;
      candidates.push({
        el,
        x: (nx * 0.5 + 0.5) * w,
        y: (1 - (ny * 0.5 + 0.5)) * h,
        d: Math.hypot(p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]),
      });
    }

    candidates.sort((a, b) => a.d - b.d);
    const placed = [];
    for (const c of candidates) {
      const cw2 = Math.max(40, c.el.offsetWidth || 60) / 2;
      const ch2 = Math.max(9, c.el.offsetHeight || 18) / 2;
      let clash = false;
      for (const p of placed) {
        if (Math.abs(c.x - p.x) < cw2 + p.w && Math.abs(c.y - p.y) < ch2 + p.h) { clash = true; break; }
      }
      if (clash) continue;
      placed.push({ x: c.x, y: c.y, w: cw2, h: ch2 });
      c.el.style.display = "";
      c.el.style.left = c.x + "px";
      c.el.style.top = c.y + "px";
    }
  }

  /* --------------------------------------------------------------- input -- */

  // Every key and every gesture resolves through the shared control table, so
  // this page and any other front end the viewer grows answer a reviewer's hands
  // identically. Nothing below decides what a key means; it only carries out
  // what `controls.js` says it means.
  const C = DelveControls;

  const pointers = new Map();
  let lastPinch = 0;

  function onDown(e) {
    canvas.setPointerCapture(e.pointerId);
    // Focus follows the drag, so the keys the reviewer presses next reach the
    // camera rather than whichever panel control was clicked last.
    if (canvas.focus) canvas.focus({ preventScroll: true });
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY, button: e.button });
    lastPinch = 0;
    noteFirstInput();
  }

  function onMove(e) {
    const prev = pointers.get(e.pointerId);
    if (!prev) return;
    const dx = e.clientX - prev.x, dy = e.clientY - prev.y;
    prev.x = e.clientX; prev.y = e.clientY;

    if (pointers.size >= 2) {
      const pts = [...pointers.values()];
      const d = Math.hypot(pts[0].x - pts[1].x, pts[0].y - pts[1].y);
      if (lastPinch > 0) zoom((lastPinch - d) * 0.02);
      lastPinch = d;
      pan(dx * 0.5, dy * 0.5);
      invalidate();
      return;
    }

    // The left button always looks (or orbits, while orbit is switched on).
    // Shift no longer pans: shift is "move faster", and one modifier cannot
    // mean two things without the reviewer having to know which mode they are
    // in — which is the defect this file is being repaired for.
    const panning = prev.button === 1 || prev.button === 2;
    if (panning) pan(dx, dy);
    else look(dx, dy);
    invalidate();
  }

  function onUp(e) {
    pointers.delete(e.pointerId);
    lastPinch = 0;
  }

  function look(dx, dy) {
    // Walking turns the head; orbiting swings the model, which inverts yaw.
    // Both senses come from the shared table so the two gestures cannot drift
    // apart, and pitch is the same in each because tilting the head down and
    // tilting the model down look identical from the reviewer's side.
    const d = state.mode === "walk" ? C.lookStep(dx, dy) : C.orbitStep(dx, dy);
    state.yaw += d.dyaw;
    state.pitch = C.clampPitch(state.pitch + d.dpitch);
    markCustom();
  }

  function pan(dx, dy) {
    const eye = cameraEye(), c = cameraCenter();
    let fx = c[0] - eye[0], fy = c[1] - eye[1], fz = c[2] - eye[2];
    const fl = Math.hypot(fx, fy, fz) || 1; fx /= fl; fy /= fl; fz /= fl;
    // Same right vector the walk uses, from the same place — the duplicated
    // hand-written copy here is what let A/D and the pan axes disagree.
    const b = C.basis(Math.atan2(fx, fz));
    let rx = -b.right[0], rz = -b.right[1];
    const rl = Math.hypot(rx, rz) || 1; rx /= rl; rz /= rl;
    const ux = rz * fy, uy = rx * fz - rz * fx, uz = -rx * fy;
    const scale = state.mode === "walk" ? 0.02 : state.dist * 0.0022;
    const mx = (-dx * rx + dy * ux) * scale;
    const my = (dy * uy) * scale;
    const mz = (-dx * rz + dy * uz) * scale;
    if (state.mode === "walk") {
      state.eye = [state.eye[0] + mx, state.eye[1] + my, state.eye[2] + mz];
    } else {
      state.target = [state.target[0] + mx, state.target[1] + my, state.target[2] + mz];
    }
    markCustom();
  }

  function zoom(amount) {
    if (state.mode === "walk") {
      const c = cameraCenter();
      const dx = c[0] - state.eye[0], dy = c[1] - state.eye[1], dz = c[2] - state.eye[2];
      const step = -amount * 1.2;
      state.eye = [state.eye[0] + dx * step, state.eye[1] + dy * step, state.eye[2] + dz * step];
    } else {
      state.dist = Math.max(0.6, state.dist * Math.exp(amount * 0.28));
    }
    markCustom();
  }

  function markCustom() {
    if (state.preset) { state.preset = ""; syncPresetButtons(); }
    updateReadout();
  }

  function onWheel(e) {
    e.preventDefault();
    zoom(Math.sign(e.deltaY) * (e.ctrlKey ? 0.5 : 1));
    invalidate();
  }

  /** Actions currently held down, never raw keys — the table owns that mapping. */
  const held = new Set();

  /** One frame's worth of whatever is held down. Split out of the animation
   *  loop so a check can advance the camera one tick without a real clock. */
  function applyHeld() {
    if (held.size) {
      // A movement key is never inert. If the reviewer is orbiting and presses
      // W, the page leaves orbit and walks from where the camera was standing —
      // visibly, because the orbit button un-presses. A key that silently does
      // nothing is how this page failed twice.
      if (state.mode !== "walk") {
        for (const a of held) {
          if (C.isMovement(a)) { setOrbit(false); break; }
        }
      }

      const turn = C.lookKeyStep(held);
      if (turn.dyaw || turn.dpitch) {
        state.yaw += turn.dyaw;
        state.pitch = C.clampPitch(state.pitch + turn.dpitch);
        markCustom();
        invalidate();
      }

      if (state.mode === "walk") {
        const m = C.walkStep({ yaw: state.yaw, held: held });
        if (m[0] || m[1] || m[2]) {
          state.eye = [state.eye[0] + m[0], state.eye[1] + m[1], state.eye[2] + m[2]];
          markCustom();
          invalidate();
        }
      }
    }
  }

  function step() {
    applyHeld();
    if (needsDraw) { needsDraw = false; draw(); }
    requestAnimationFrame(step);
  }

  if (gl) {
    canvas.addEventListener("pointerdown", onDown);
    canvas.addEventListener("pointermove", onMove);
    canvas.addEventListener("pointerup", onUp);
    canvas.addEventListener("pointercancel", onUp);
    canvas.addEventListener("wheel", onWheel, { passive: false });
    canvas.addEventListener("contextmenu", (e) => e.preventDefault());
    window.addEventListener("keydown", (e) => {
      // Leave the browser's own chords alone; nothing here binds a modifier
      // combination, so swallowing one could only ever break something.
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      // A focused slider or button keeps its keys. The cutaway slider is
      // steered with the same arrows the camera uses, and a control the
      // reviewer can see but cannot use is worse than one that is absent.
      if (C.isTypingTarget(e.target)) return;
      const a = C.actionFor(e);
      if (!a) return;
      held.add(a);
      noteFirstInput();
      e.preventDefault();
    });
    window.addEventListener("keyup", (e) => {
      const a = C.actionFor(e);
      if (a) held.delete(a);
      // Shift reports two codes and either release should stop the sprint.
      if (e.key === "Shift") held.delete(C.ACTIONS.FASTER);
    });
    // Anything that can strand a key down — tab away, alt-tab, a chord that
    // steals the keyup — clears the whole set rather than leaving the camera
    // drifting on a key nobody is holding.
    window.addEventListener("blur", () => held.clear());
    document.addEventListener("visibilitychange", () => { if (document.hidden) held.clear(); });
    window.addEventListener("resize", invalidate);
  }

  /* ------------------------------------------------------------------ ui -- */

  const els = {
    select: document.getElementById("model-select"),
    stats: document.getElementById("model-stats"),
    presets: document.getElementById("presets"),
    cameraHint: document.getElementById("camera-hint"),
    cut: document.getElementById("cut"),
    cutValue: document.getElementById("cut-value"),
    anchorList: document.getElementById("anchor-list"),
    anchorCount: document.getElementById("anchor-count"),
    legend: document.getElementById("legend"),
    legendCount: document.getElementById("legend-count"),
    findings: document.getElementById("findings"),
    findingsCount: document.getElementById("findings-count"),
    fidelity: document.getElementById("fidelity"),
    panel: document.getElementById("panel"),
    app: document.getElementById("app"),
    panelToggle: document.getElementById("panel-toggle"),
    orbitToggle: document.getElementById("orbit-toggle"),
    controlsHelp: document.getElementById("controls-help"),
    firstHint: document.getElementById("first-hint"),
  };

  let presetButtons = [];

  /** The panel's key list is the control table, printed. Neither can drift from
   *  the other because there is only one of them. */
  function buildControlsHelp() {
    if (!els.controlsHelp) return;
    els.controlsHelp.textContent = "";
    for (const row of C.HELP) {
      const li = document.createElement("li");
      const g = document.createElement("span");
      g.className = "gesture";
      g.textContent = row.gesture;
      const e = document.createElement("span");
      e.className = "effect";
      e.textContent = row.effect;
      li.appendChild(g);
      li.appendChild(e);
      els.controlsHelp.appendChild(li);
    }
  }

  let sawInput = false;
  function noteFirstInput() {
    if (sawInput) return;
    sawInput = true;
    if (els.firstHint) els.firstHint.classList.add("gone");
  }

  function syncPresetButtons() {
    for (const b of presetButtons) {
      b.setAttribute("aria-pressed", String(b.dataset.preset === state.preset));
    }
    const orbiting = state.mode === "orbit";
    if (els.orbitToggle) els.orbitToggle.setAttribute("aria-pressed", String(orbiting));
    els.cameraHint.textContent = orbiting
      ? "Orbiting: dragging now swings the camera around the piece. Any movement key puts you back on your feet."
      : "On foot, eyes " + EYE.toFixed(2) + " blocks off the floor — a standing player's height.";
  }

  function updateReadout() {
    if (!readout || !gl) return;
    const e = cameraEye();
    const fmt = (n) => n.toFixed(1);
    const heading = ((state.yaw * 180 / Math.PI) % 360 + 360) % 360;
    readout.textContent = state.mode === "walk"
      ? "on foot " + fmt(e[0]) + ", " + fmt(e[1]) + ", " + fmt(e[2]) + "  ·  " + heading.toFixed(0) + "°  ·  player eye height " + EYE
      : "orbiting  ·  " + fmt(state.dist) + " blocks out  ·  " + heading.toFixed(0) + "°";
  }

  /* -------------------------------------------------------- the structure -- */

  /**
   * The grid, as a structure the renderer can mesh, up to `maxY`.
   *
   * Built through the public `addBlock` rather than from structure NBT, because
   * a zone reassembled from several templates has no single file — and because
   * the cutaway needs a second structure over the same palette.
   */
  function structureUpTo(model, maxY) {
    const [sx, sy, sz] = model.size;
    const top = Math.min(maxY, sy - 1);
    const st = new D.Structure([sx, sy, sz]);
    const grid = model.grid;
    for (let y = 0; y <= top; y++) {
      for (let z = 0; z < sz; z++) {
        const row = (y * sz + z) * sx;
        for (let x = 0; x < sx; x++) {
          const p = grid[row + x];
          if (p === 0) continue;
          const e = model.palette[p];
          st.addBlock([x, y, z], e.name, e.props || {});
        }
      }
    }
    return st;
  }

  /**
   * What the renderer actually does with each blockstate, meshed one at a time.
   *
   * Two failures, both invisible in a finished picture and neither reachable
   * from the resources alone:
   *
   *   - **nothing is drawn.** Every resource resolved and the block still has no
   *     geometry, because the definition selects no model for these properties.
   *   - **the missing-texture checker is drawn.** A face landed on the atlas cell
   *     reserved for "no such texture", which means some id the renderer asked
   *     for is not an id the page was given. This is the failure mode of the
   *     block-entity texture table: those ids are asked for by code, never by a
   *     model file, so a wrong one renders magenta and says nothing. It cost
   *     three wrong ids to learn, and it is why this probe runs on every page
   *     rather than being a line in a document.
   *
   * Alone, so an empty result is about the block and not about what happens to
   * sit beside it.
   */
  function probeGeometry(model, res) {
    const out = [];
    const seen = new Set();
    const checkerUV = 16 / Math.max(1, state.atlasSize);
    for (let p = 1; p < model.palette.length; p++) {
      const e = model.palette[p];
      if (!e || e.count === 0 || seen.has(e.state)) continue;
      seen.add(e.state);
      const id = D.Identifier.parse(e.name);
      try {
        const def = res.getBlockDefinition(id);
        const props = Object.assign({}, res.getDefaultBlockProperties(id) || {}, e.props || {});
        const mesh = new D.Mesh();
        if (def) mesh.merge(def.getMesh(id, props, res, res, D.Cull.none()));
        mesh.merge(D.SpecialRenderers.getBlockMesh(new D.BlockState(id, props), undefined, res, D.Cull.none()));
        if (mesh.isEmpty()) {
          out.push({
            state: e.state,
            cells: e.count,
            why: def
              ? "draws nothing — the definition selects no model for these properties"
              : "draws nothing — no blockstate definition",
          });
          continue;
        }
        let checker = 0, total = 0;
        for (const q of mesh.quads) {
          for (const vert of q.vertices()) {
            total++;
            if (vert.texture && vert.texture[0] < checkerUV && vert.texture[1] < checkerUV) checker++;
          }
        }
        if (checker > 0) {
          out.push({
            state: e.state,
            cells: e.count,
            why: "draws the missing-texture checker on " + checker + " of " + total
              + " vertices — the renderer asked for a texture this page was not given",
          });
        }
      } catch (err) {
        out.push({ state: e.state, cells: e.count, why: "the renderer threw: " + err });
      }
    }
    return out;
  }

  /* --------------------------------------------------------------- panel -- */

  function buildPresetButtons(model) {
    els.presets.textContent = "";
    presetButtons = [];
    const list = [
      { id: "ground", label: "Ground level" },
      { id: "exterior", label: "Exterior ¾" },
      { id: "plan", label: "Plan" },
    ].concat(anchorPresets(model));
    for (const p of list) {
      const b = document.createElement("button");
      b.type = "button";
      b.textContent = p.label;
      b.dataset.preset = p.id;
      b.setAttribute("aria-pressed", "false");
      b.addEventListener("click", () => {
        applyPreset(p.id);
        // Hand the keyboard straight back to the camera. Left focused, this
        // button would swallow the next Space the reviewer presses.
        b.blur();
        if (canvas.focus) canvas.focus({ preventScroll: true });
        noteFirstInput();
        invalidate();
      });
      els.presets.appendChild(b);
      presetButtons.push(b);
    }
  }

  function buildAnchorList(model) {
    els.anchorList.textContent = "";
    els.anchorCount.textContent = String(model.anchors.length);
    if (!model.anchors.length) {
      const li = document.createElement("li");
      li.className = "hint";
      li.textContent = "This prefab declares none. Anchors come from the "
        + "<name>.json beside the .nbt; without them the page still shows the "
        + "geometry, and offers the exterior and plan views only.";
      els.anchorList.appendChild(li);
      return;
    }
    for (const a of model.anchors) {
      const li = document.createElement("li");
      const b = document.createElement("button");
      b.type = "button";
      const where = a.pos ? a.pos.join(",") : (a.from ? a.from.join(",") + " → " + a.to.join(",") : "?");
      b.textContent = a.name + "  " + where + (a.facing ? "  " + a.facing : "");
      if (a.pos) b.addEventListener("click", () => { applyPreset("pov:" + a.name); invalidate(); });
      else b.disabled = true;
      li.appendChild(b);
      els.anchorList.appendChild(li);
    }
  }

  /**
   * The block list, each row swatched with the colour that block occupies in the
   * atlas — sampled from the picture the reviewer is looking at, so a swatch and
   * its block cannot disagree.
   */
  function buildLegend(model, sample) {
    els.legend.textContent = "";
    const rows = [];
    for (let i = 1; i < model.palette.length; i++) {
      const e = model.palette[i];
      if (e.count > 0) rows.push(e);
    }
    rows.sort((a, b) => b.count - a.count || (a.state < b.state ? -1 : 1));
    els.legendCount.textContent = String(rows.length);
    for (const e of rows) {
      const li = document.createElement("li");
      const sw = document.createElement("span");
      sw.className = "swatch";
      const rgb = sample ? sample(e.name) : null;
      if (rgb) sw.style.background = "rgb(" + rgb[0] + "," + rgb[1] + "," + rgb[2] + ")";
      const nm = document.createElement("span");
      nm.className = "name";
      nm.textContent = e.state.replace(/^minecraft:/, "");
      nm.title = e.state;
      const ct = document.createElement("span");
      ct.className = "count";
      ct.textContent = e.count.toLocaleString();
      li.append(sw, nm, ct);
      els.legend.appendChild(li);
    }
  }

  /**
   * Everything the page knows about how faithful the picture is, in one list,
   * with the binding count of each check beside it.
   *
   * A page that showed nothing here would be claiming a clean bill of health,
   * and a clean bill of health from a check that examined nothing is the failure
   * this section exists to make impossible to miss.
   */
  function buildFindings() {
    const rows = [];
    for (const u of DATA.unresolved || []) {
      rows.push({
        bad: true,
        head: u.state + "  ×" + u.cells,
        why: u.reason.replace(/_/g, " ") + " — " + u.detail
          + ". The pinned version does not have it, so a server pinned to the same version does not either.",
      });
    }
    for (const u of DATA.under_specified || []) {
      const filled = Object.keys(u.filled).map((k) => k + "=" + u.filled[k]).join(", ");
      rows.push({
        bad: u.multipart,
        head: u.state + "  ×" + u.cells,
        why: "leaves " + Object.keys(u.filled).join(", ") + " unwritten; drawn from the "
          + "version's default state (" + filled + ")"
          + (u.multipart
            ? ". This block's definition is multipart, where an unwritten property matches no case at all — what the file says and what you are looking at are different blocks."
            : "."),
      });
    }
    for (const g of state.noGeometry) {
      rows.push({ bad: true, head: g.state + "  ×" + g.cells, why: g.why });
    }
    for (const c of state.atlasComplaints) {
      rows.push({ bad: true, head: "texture atlas", why: c });
    }

    els.findingsCount.textContent = String(rows.length);
    els.findings.textContent = "";
    if (!rows.length) {
      const li = document.createElement("li");
      li.className = "hint";
      li.textContent = "Every blockstate on this page resolved, wrote every property it has, and produced geometry.";
      els.findings.appendChild(li);
    }
    for (const r of rows) {
      const li = document.createElement("li");
      if (r.bad) li.className = "bad";
      const head = document.createElement("span");
      head.textContent = r.head;
      const why = document.createElement("span");
      why.className = "why";
      why.textContent = r.why;
      li.append(head, why);
      els.findings.appendChild(li);
    }

    // The binding counts. Each number is what the corresponding check looked at;
    // a zero means it examined nothing, which is a finding and not a pass.
    const states = countStates();
    const bind = [
      states + " blockstate" + (states === 1 ? "" : "s") + " examined",
      Object.keys(DATA.textures).length + " textures packed into a "
        + state.atlasSize + "×" + state.atlasSize + " atlas",
      DATA.special_bound + " block-entity texture id"
        + (DATA.special_bound === 1 ? "" : "s") + " resolved against the jar",
      "Minecraft " + DATA.mc_version,
    ];
    els.fidelity.textContent = bind.join("  ·  ");
    els.fidelity.classList.toggle("warn", states === 0);
  }

  function countStates() {
    const seen = new Set();
    for (const m of DATA.models) {
      for (let i = 1; i < m.palette.length; i++) {
        if (m.palette[i] && m.palette[i].count > 0) seen.add(m.palette[i].state);
      }
    }
    return seen.size;
  }

  function updateStats() {
    const m = state.model;
    const [sx, sy, sz] = m.size;
    const parts = [
      sx + "×" + sy + "×" + sz,
      m.filled.toLocaleString() + " blocks",
      m.palette.length - 1 + " states",
    ];
    if (m.tiles > 1) parts.push(m.tiles + " templates reassembled");
    els.stats.textContent = parts.join("  ·  ");
  }

  /** The atlas colour of a block, for its legend swatch. */
  function makeSampler(atlasImage, size) {
    const cache = new Map();
    return function (name) {
      if (cache.has(name)) return cache.get(name);
      let rgb = null;
      const model = DATA.blockstates[name];
      if (model && atlasImage) {
        // The first texture any of this block's models names, via the model
        // chain the renderer itself walks.
        const id = firstTexture(name);
        if (id) {
          const uv = state.resources.getTextureUV(D.Identifier.parse(id));
          const px = Math.floor(((uv[0] + uv[2]) / 2) * size);
          const py = Math.floor(((uv[1] + uv[3]) / 2) * size);
          const at = (py * size + px) * 4;
          rgb = [atlasImage.data[at], atlasImage.data[at + 1], atlasImage.data[at + 2]];
        }
      }
      cache.set(name, rgb);
      return rgb;
    };
  }

  function firstTexture(blockName) {
    const def = DATA.blockstates[blockName];
    if (!def) return null;
    const refs = [];
    const push = (v) => {
      if (!v) return;
      if (Array.isArray(v)) v.forEach((x) => x && x.model && refs.push(x.model));
      else if (v.model) refs.push(v.model);
    };
    if (def.variants) for (const k of Object.keys(def.variants)) push(def.variants[k]);
    if (def.multipart) for (const p of def.multipart) push(p.apply);
    for (const r of refs) {
      const t = textureOf(qualify(r), 0);
      if (t) return t;
    }
    return null;
  }

  function qualify(id) { return id.indexOf(":") >= 0 ? id : "minecraft:" + id; }

  function textureOf(modelId, depth) {
    if (depth > 8) return null;
    const m = DATA.block_models[modelId];
    if (!m) return null;
    if (m.textures) {
      for (const key of ["all", "texture", "side", "top", "particle", "end", "cross", "wall", "bottom"]) {
        const v = m.textures[key];
        if (typeof v === "string" && v.charAt(0) !== "#" && DATA.textures[qualify(v)]) return qualify(v);
      }
      for (const key of Object.keys(m.textures)) {
        const v = m.textures[key];
        if (typeof v === "string" && v.charAt(0) !== "#" && DATA.textures[qualify(v)]) return qualify(v);
      }
    }
    return m.parent ? textureOf(qualify(m.parent), depth + 1) : null;
  }

  /* ---------------------------------------------------------------- boot -- */

  function selectModel(index) {
    const m = DATA.models[index];
    const [sx, sy, sz] = m.size;
    if (!m.grid) {
      m.grid = decodeGrid(m.voxels, sx * sy * sz);
      m.solid = new Uint8Array(m.palette.length);
      for (let i = 1; i < m.palette.length; i++) {
        const f = DATA.flags[m.palette[i].name];
        m.solid[i] = f && f.opaque ? 1 : 0;
      }
    }
    state.model = m;
    els.select.value = String(index);
    state.cutY = sy - 1;
    els.cut.max = String(sy - 1);
    els.cut.value = String(sy - 1);
    els.cutValue.textContent = String(sy - 1);
    buildOverlays(m);
    buildPresetButtons(m);
    buildAnchorList(m);

    const hash = fromHash();
    if (hash.cut !== undefined && hash.cut !== "") {
      const y = Math.max(0, Math.min(sy - 1, Number(hash.cut)));
      if (Number.isFinite(y)) {
        state.cutY = y;
        els.cut.value = String(y);
        els.cutValue.textContent = String(y);
      }
    }

    if (state.resources) {
      const st = structureUpTo(m, state.cutY);
      if (state.renderer) {
        state.renderer.setStructure(st);
        state.renderer.updateStructureBuffers();
      } else {
        state.renderer = new D.StructureRenderer(gl, st, state.resources, {
          chunkSize: 16,
          // The invisible-block markers walk the whole volume and draw a wire
          // cube per empty cell — 42,000 of them on a zone, over a review that
          // is about the building rather than about where the air is.
          useInvisibleBlockBuffer: false,
        });
        state.renderer.setViewport(0, 0, canvas.width, canvas.height);
      }
      state.noGeometry = probeGeometry(m, state.resources);
      buildFindings();
    }
    buildLegend(m, state.sampler);
    updateStats();

    const want = hash.preset;
    const known = want && (want === "ground" || want === "exterior" || want === "plan"
      || (want.startsWith("pov:") && m.anchors.some((a) => a.name === want.slice(4) && a.pos)));
    applyPreset(known ? want : defaultPreset(m));
    invalidate();
  }

  function rebuildStructure() {
    if (!state.renderer) return;
    state.renderer.setStructure(structureUpTo(state.model, state.cutY));
    state.renderer.updateStructureBuffers();
    invalidate();
  }

  buildControlsHelp();

  if (els.orbitToggle) {
    els.orbitToggle.addEventListener("click", () => {
      setOrbit(state.mode !== "orbit");
      els.orbitToggle.blur();
      if (canvas.focus) canvas.focus({ preventScroll: true });
      noteFirstInput();
    });
  }

  els.cut.addEventListener("input", () => {
    state.cutY = Number(els.cut.value);
    els.cutValue.textContent = els.cut.value;
    rebuildStructure();
  });

  for (const [id, key] of [["opt-anchors", "anchors"], ["opt-labels", "labels"],
  ["opt-bounds", "bounds"], ["opt-ground", "ground"]]) {
    const el = document.getElementById(id);
    el.addEventListener("change", () => { state.show[key] = el.checked; invalidate(); });
  }

  els.panelToggle.addEventListener("click", () => {
    const hidden = els.app.classList.toggle("panel-hidden");
    els.panelToggle.setAttribute("aria-expanded", String(!hidden));
    invalidate();
  });

  for (let i = 0; i < DATA.models.length; i++) {
    const o = document.createElement("option");
    o.value = String(i);
    o.textContent = DATA.models[i].id;
    els.select.appendChild(o);
  }
  els.select.hidden = DATA.models.length < 2;
  els.select.addEventListener("change", () => selectModel(Number(els.select.value)));

  const wantModel = fromHash().model;
  let startIndex = 0;
  if (wantModel) {
    const i = DATA.models.findIndex((m) => m.id === wantModel);
    if (i >= 0) startIndex = i;
  }
  els.select.value = String(startIndex);

  let ready = false;
  function boot() {
    if (!gl) { selectModel(startIndex); buildFindings(); return; }
    initOverlay();
    resize();
    buildAtlas(DATA.textures, (atlas, size, complaints) => {
      try {
        state.atlasSize = size;
        state.atlasComplaints = complaints;
        if (!atlas) throw new Error(complaints.join("; "));
        state.resources = makeResources(atlas);
        state.sampler = makeSampler(atlas.getTextureAtlas(), size);
        selectModel(startIndex);
        ready = true;
        invalidate();
        requestAnimationFrame(step);
      } catch (err) {
        canvas.hidden = true;
        fallback.hidden = false;
        fallback.textContent = "The model could not be drawn: " + err
          + " — the block list and the findings in the panel are still readable.";
        selectModel(startIndex);
        buildFindings();
      }
    });
  }

  try {
    boot();
  } catch (err) {
    if (fallback) {
      canvas.hidden = true;
      fallback.hidden = false;
      fallback.textContent = "The model could not be drawn: " + err;
    }
  }

  // A headless check needs to read what was built without a screenshot, so the
  // page's own measurements are reachable from it rather than only on screen.
  window.delveViewer = {
    data: DATA,
    state: state,
    ready: () => ready,
    stats: () => ({
      id: state.model && state.model.id,
      size: state.model && state.model.size,
      filled: state.model && state.model.filled,
      runs: state.model && state.model.runs,
      tiles: state.model && state.model.tiles,
      anchors: state.model ? state.model.anchors.length : 0,
      states: countStates(),
      textures: Object.keys(DATA.textures).length,
      atlas: state.atlasSize,
      atlasComplaints: state.atlasComplaints,
      specialBound: DATA.special_bound,
      unresolved: (DATA.unresolved || []).length,
      underSpecified: (DATA.under_specified || []).length,
      noGeometry: state.noGeometry,
      preset: state.preset,
      mode: state.mode,
      eye: cameraEye(),
      webgl: !!gl,
    }),
    applyPreset: (id) => { applyPreset(id); draw(); },
    selectModel: (i) => { selectModel(i); draw(); },
    // The label pass, and what it left in the layer. `draw` cannot stand in for
    // it: `draw` returns at once without a WebGL context, and a check that can
    // only run where there is one is a check CI cannot run. The pass itself
    // needs no context — it is the projection matrix, the DOM and nothing else.
    positionLabels: () => { positionLabels(); },
    labels: () => [...labelLayer.children].map((el) => ({
      text: el.textContent,
      className: el.className,
      shown: el.style.display !== "none",
    })),
    // The control surface, reachable without synthesising events, so a headless
    // check can drive exactly what a pair of hands drives.
    controls: C,
    held: held,
    setOrbit: (on) => { setOrbit(on); draw(); },
    press: (code) => {
      const a = C.actionFor({ code: code });
      if (a) { held.add(a); noteFirstInput(); }
      return a;
    },
    release: (code) => {
      const a = C.actionFor({ code: code });
      if (a) held.delete(a);
      return a;
    },
    tick: () => { applyHeld(); draw(); },
  };
})();
