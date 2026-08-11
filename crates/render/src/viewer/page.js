/* Interactive prefab viewer.
 *
 * A voxel model is axis-aligned boxes, so this draws it directly rather than
 * carrying a general 3D library: the whole renderer is smaller than a minified
 * scene graph's licence header, and the page must inline every byte it uses
 * because the CSP it is reviewed under blocks external hosts outright.
 *
 * Geometry arrives run-length encoded over the grid. Meshing happens here, and
 * only exposed faces become triangles — the interior of a building is the vast
 * majority of its cells and none of it is ever visible.
 */
"use strict";

(function () {
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

  function perspective(out, fovy, aspect, near, far) {
    const f = 1 / Math.tan(fovy / 2), nf = 1 / (near - far);
    out.fill(0);
    out[0] = f / aspect; out[5] = f; out[11] = -1;
    out[10] = (far + near) * nf; out[14] = 2 * far * near * nf;
    return out;
  }

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

  /* -------------------------------------------------------------- meshing -- */

  // Face order: +X, -X, +Y, -Y, +Z, -Z. Shade per normal, the way a voxel game
  // does, so edges read without any lighting model.
  // Ambient floor, so a face turned away from the light keeps its material
  // legible instead of crushing toward black. A reviewer judging a dark
  // interior needs to see what the walls are MADE of; the relative face shades
  // still carry the form.
  const AMBIENT = 0.36;
  const lit = (s) => AMBIENT + (1 - AMBIENT) * s;

  const FACES = [
    { d: [1, 0, 0], shade: lit(0.60), corners: [[1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1]] },
    { d: [-1, 0, 0], shade: lit(0.60), corners: [[0, 0, 1], [0, 1, 1], [0, 1, 0], [0, 0, 0]] },
    { d: [0, 1, 0], shade: lit(1.00), corners: [[0, 1, 0], [0, 1, 1], [1, 1, 1], [1, 1, 0]] },
    { d: [0, -1, 0], shade: lit(0.45), corners: [[0, 0, 1], [0, 0, 0], [1, 0, 0], [1, 0, 1]] },
    { d: [0, 0, 1], shade: lit(0.80), corners: [[1, 0, 1], [1, 1, 1], [0, 1, 1], [0, 0, 1]] },
    { d: [0, 0, -1], shade: lit(0.80), corners: [[0, 0, 0], [0, 1, 0], [1, 1, 0], [1, 0, 0]] },
  ];

  const FULL_CUBE = [0, 0, 0, 16, 16, 16];

  function isFullCube(box) {
    for (let i = 0; i < 6; i++) if (box[i] !== FULL_CUBE[i]) return false;
    return true;
  }

  /**
   * Build vertex + colour arrays for a model at a given cutaway height.
   * Two passes: count the faces, then fill exactly-sized typed arrays.
   */
  function buildMesh(model, cutY) {
    const [sx, sy, sz] = model.size;
    const grid = model.grid;
    const pal = model.palette;
    const solid = model.solid;   // per palette index: opaque full cube
    const full = model.full;     // per palette index: full cube (any coverage)
    const alpha = model.alpha;   // per palette index: 0..1

    const topY = Math.min(cutY, sy - 1);
    const at = (x, y, z) => (y * sz + z) * sx + x;

    // A neighbour hides a face only when this block fills its cell, the
    // neighbour is an opaque full cube, and the neighbour is not cut away.
    function hidden(x, y, z) {
      if (x < 0 || y < 0 || z < 0 || x >= sx || y >= sy || z >= sz) return false;
      if (y > topY) return false;
      return solid[grid[at(x, y, z)]] === 1;
    }

    let faceCount = 0;
    const emit = [];
    for (let y = 0; y <= topY; y++) {
      for (let z = 0; z < sz; z++) {
        for (let x = 0; x < sx; x++) {
          const p = grid[at(x, y, z)];
          if (p === 0) continue;
          const canCull = full[p] === 1;
          for (let f = 0; f < 6; f++) {
            const d = FACES[f].d;
            if (canCull && hidden(x + d[0], y + d[1], z + d[2])) continue;
            faceCount++;
            emit.push((at(x, y, z) << 3) | f);
          }
        }
      }
    }

    const positions = new Float32Array(faceCount * 6 * 3);
    const colors = new Uint8Array(faceCount * 6 * 4);
    // Where each vertex sits on its own quad, so the fragment shader can draw
    // the block edge. Without it a wall of one material is a single flat
    // expanse and the reviewer cannot read its form at all.
    const quadUV = new Float32Array(faceCount * 6 * 2);
    const QUAD = [[0, 0], [0, 1], [1, 1], [1, 0]];
    let vi = 0, ci = 0, ui = 0, opaqueFaces = 0;

    // Opaque faces first, then translucent, so one buffer can be drawn as two
    // passes without sorting per frame.
    for (const pass of [0, 1]) {
      for (const packed of emit) {
        const f = packed & 7;
        const cell = packed >> 3;
        const x = cell % sx, z = ((cell / sx) | 0) % sz, y = ((cell / (sx * sz)) | 0);
        const p = grid[at(x, y, z)];
        const translucent = alpha[p] < 0.99 ? 1 : 0;
        if (translucent !== pass) continue;
        if (pass === 0) opaqueFaces++;

        const e = pal[p];
        const b = e.box;
        const x0 = x + b[0] / 16, y0 = y + b[1] / 16, z0 = z + b[2] / 16;
        const x1 = x + b[3] / 16, y1 = y + b[4] / 16, z1 = z + b[5] / 16;
        const face = FACES[f];
        const s = face.shade;
        const r = Math.round(e.rgb[0] * s), g = Math.round(e.rgb[1] * s), bl = Math.round(e.rgb[2] * s);
        const a = Math.round(Math.max(alpha[p], 0.25) * 255);

        // Two triangles from the quad's four corners.
        const q = face.corners;
        const order = [0, 1, 2, 0, 2, 3];
        for (const k of order) {
          const c = q[k];
          positions[vi++] = c[0] ? x1 : x0;
          positions[vi++] = c[1] ? y1 : y0;
          positions[vi++] = c[2] ? z1 : z0;
          colors[ci++] = r; colors[ci++] = g; colors[ci++] = bl; colors[ci++] = a;
          quadUV[ui++] = QUAD[k][0]; quadUV[ui++] = QUAD[k][1];
        }
      }
    }

    return { positions, colors, quadUV, faces: faceCount, opaqueVerts: opaqueFaces * 6 };
  }

  /* ------------------------------------------------------------- program -- */

  let prog = null, aPos = 0, aCol = 0, aUV = 0, uMVP = null;
  let posBuf = null, colBuf = null, uvBuf = null;
  // A quad coordinate every non-block primitive can share: lines and markers
  // want no edge darkening, so they bind the middle of a quad everywhere.
  let flatUV = null, flatUVCount = 0;

  function compile(type, src) {
    const s = gl.createShader(type);
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
      throw new Error(gl.getShaderInfoLog(s) || "shader compile failed");
    }
    return s;
  }

  function initGL() {
    const vs = compile(gl.VERTEX_SHADER,
      "attribute vec3 aPos;attribute vec4 aCol;attribute vec2 aUV;uniform mat4 uMVP;"
      + "varying vec4 vCol;varying vec2 vUV;"
      + "void main(){vCol=aCol;vUV=aUV;gl_Position=uMVP*vec4(aPos,1.0);}");
    // Darken the rim of every quad. The seam between two blocks of one material
    // is otherwise invisible, and a wall the reviewer cannot count the courses
    // of is the grey box this tool exists to replace. `fwidth` keeps the line
    // one pixel wide at any distance where the extension is available; without
    // it a fixed inset still separates the blocks, only less evenly.
    const deriv = gl.getExtension("OES_standard_derivatives");
    const fs = compile(gl.FRAGMENT_SHADER,
      (deriv ? "#extension GL_OES_standard_derivatives : enable\n" : "")
      + "precision mediump float;varying vec4 vCol;varying vec2 vUV;"
      + "void main(){"
      + "vec2 d=min(vUV,1.0-vUV);float e=min(d.x,d.y);"
      + (deriv
        ? "float w=min(max(fwidth(vUV.x),fwidth(vUV.y)),0.06);float k=smoothstep(0.0,w*1.5,e);"
        : "float k=smoothstep(0.0,0.045,e);")
      + "gl_FragColor=vec4(vCol.rgb*mix(0.74,1.0,k),vCol.a);}");
    prog = gl.createProgram();
    gl.attachShader(prog, vs); gl.attachShader(prog, fs); gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
      throw new Error(gl.getProgramInfoLog(prog) || "program link failed");
    }
    gl.useProgram(prog);
    aPos = gl.getAttribLocation(prog, "aPos");
    aCol = gl.getAttribLocation(prog, "aCol");
    aUV = gl.getAttribLocation(prog, "aUV");
    uMVP = gl.getUniformLocation(prog, "uMVP");
    posBuf = gl.createBuffer();
    colBuf = gl.createBuffer();
    uvBuf = gl.createBuffer();
    gl.enable(gl.DEPTH_TEST);
    gl.enable(gl.CULL_FACE);
    gl.cullFace(gl.BACK);
    gl.clearColor(0.078, 0.086, 0.110, 1);
  }

  /* --------------------------------------------------------------- state -- */

  const state = {
    model: null,
    mesh: null,
    cutY: 0,
    mode: "orbit",
    // orbit
    target: [0, 0, 0],
    dist: 30,
    // shared angles: yaw 0 looks toward +Z, pitch is up-positive
    yaw: Math.PI * 0.25,
    pitch: 0.5,
    eye: [0, 0, 0],
    preset: "",
    show: { anchors: true, labels: true, bounds: false, ground: true },
  };

  const FACING_YAW = { south: 0, west: -Math.PI / 2, north: Math.PI, east: Math.PI / 2 };

  function cameraEye() {
    if (state.mode === "pov") return state.eye;
    const cp = Math.cos(state.pitch);
    return [
      state.target[0] + state.dist * cp * Math.sin(state.yaw),
      state.target[1] + state.dist * Math.sin(state.pitch),
      state.target[2] + state.dist * cp * Math.cos(state.yaw),
    ];
  }

  function cameraCenter() {
    if (state.mode !== "pov") return state.target;
    const cp = Math.cos(state.pitch);
    return [
      state.eye[0] + cp * Math.sin(state.yaw),
      state.eye[1] + Math.sin(state.pitch),
      state.eye[2] + cp * Math.cos(state.yaw),
    ];
  }

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

  function anchorPresets(model) {
    const out = [];
    for (const a of model.anchors) {
      if (!a.pos) continue;
      out.push({
        id: "pov:" + a.name,
        label: (a.socket ? "▸ " : "") + a.name.replace(/^anchor\//, ""),
        kind: "pov",
        anchor: a,
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
    for (const stem of WAY_IN) {
      const hit = model.anchors.find((a) => usable(a) && stemOf(a.name) === stem);
      if (hit) return "pov:" + hit.name;
    }
    const socket = model.anchors.find((a) => usable(a) && a.socket);
    if (socket) return "pov:" + socket.name;
    return "exterior";
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

  function applyPreset(id) {
    const model = state.model;
    state.preset = id;
    if (id === "exterior") {
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
      state.mode = "pov";
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

  function centerOf(model) {
    return [model.size[0] / 2, model.size[1] / 2, model.size[2] / 2];
  }

  /* ------------------------------------------------------------- drawing -- */

  let mvp = mat4(), proj = mat4(), view = mat4();
  let groundVerts = null, groundColors = null, groundCount = 0;
  let boundsVerts = null, boundsColors = null, boundsCount = 0;

  function buildOverlays(model) {
    const [sx, sy, sz] = model.size;
    // Ground grid at y=0, one line per block boundary.
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

  /** Bind position, colour and quad coordinate, then draw. `uv` may be null,
   *  in which case the primitive is drawn without edge darkening. */
  function drawArray(mode, positions, colors, count, uv) {
    gl.bindBuffer(gl.ARRAY_BUFFER, posBuf);
    gl.bufferData(gl.ARRAY_BUFFER, positions, gl.DYNAMIC_DRAW);
    gl.enableVertexAttribArray(aPos);
    gl.vertexAttribPointer(aPos, 3, gl.FLOAT, false, 0, 0);
    gl.bindBuffer(gl.ARRAY_BUFFER, colBuf);
    gl.bufferData(gl.ARRAY_BUFFER, colors, gl.DYNAMIC_DRAW);
    gl.enableVertexAttribArray(aCol);
    gl.vertexAttribPointer(aCol, 4, gl.UNSIGNED_BYTE, true, 0, 0);
    if (!uv) {
      if (!flatUV || flatUVCount < count) {
        flatUVCount = Math.max(count, 1024);
        flatUV = new Float32Array(flatUVCount * 2).fill(0.5);
      }
      uv = flatUV;
    }
    gl.bindBuffer(gl.ARRAY_BUFFER, uvBuf);
    gl.bufferData(gl.ARRAY_BUFFER, uv, gl.DYNAMIC_DRAW);
    gl.enableVertexAttribArray(aUV);
    gl.vertexAttribPointer(aUV, 2, gl.FLOAT, false, 0, 0);
    gl.drawArrays(mode, 0, count);
  }

  function resize() {
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const w = Math.max(1, Math.round(canvas.clientWidth * dpr));
    const h = Math.max(1, Math.round(canvas.clientHeight * dpr));
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w; canvas.height = h;
    }
  }

  let needsDraw = true;
  function invalidate() { needsDraw = true; }

  function draw() {
    if (!gl || !state.mesh) return;
    resize();
    gl.viewport(0, 0, canvas.width, canvas.height);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);

    const aspect = canvas.width / Math.max(1, canvas.height);
    perspective(proj, FOV, aspect, 0.05, 4000);
    lookAt(view, cameraEye(), cameraCenter(), [0, 1, 0]);
    multiply(mvp, proj, view);
    gl.uniformMatrix4fv(uMVP, false, mvp);

    if (state.show.ground && groundCount) {
      gl.depthMask(true);
      gl.disable(gl.BLEND);
      drawArray(gl.LINES, groundVerts, groundColors, groundCount);
    }

    const m = state.mesh;
    if (m.faces > 0) {
      gl.depthMask(true);
      gl.disable(gl.BLEND);
      if (m.opaqueVerts > 0) drawArray(gl.TRIANGLES, m.positions, m.colors, m.opaqueVerts, m.quadUV);
      const rest = m.faces * 6 - m.opaqueVerts;
      if (rest > 0) {
        // Translucent faces last, without writing depth, so glass shows what is
        // behind it without needing a per-frame sort.
        gl.enable(gl.BLEND);
        gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
        gl.depthMask(false);
        gl.bindBuffer(gl.ARRAY_BUFFER, posBuf);
        gl.bufferData(gl.ARRAY_BUFFER, m.positions, gl.DYNAMIC_DRAW);
        gl.enableVertexAttribArray(aPos);
        gl.vertexAttribPointer(aPos, 3, gl.FLOAT, false, 0, 0);
        gl.bindBuffer(gl.ARRAY_BUFFER, colBuf);
        gl.bufferData(gl.ARRAY_BUFFER, m.colors, gl.DYNAMIC_DRAW);
        gl.enableVertexAttribArray(aCol);
        gl.vertexAttribPointer(aCol, 4, gl.UNSIGNED_BYTE, true, 0, 0);
        gl.bindBuffer(gl.ARRAY_BUFFER, uvBuf);
        gl.bufferData(gl.ARRAY_BUFFER, m.quadUV, gl.DYNAMIC_DRAW);
        gl.enableVertexAttribArray(aUV);
        gl.vertexAttribPointer(aUV, 2, gl.FLOAT, false, 0, 0);
        gl.drawArrays(gl.TRIANGLES, m.opaqueVerts, rest);
        gl.depthMask(true);
        gl.disable(gl.BLEND);
      }
    }

    if (state.show.bounds && boundsCount) {
      drawArray(gl.LINES, boundsVerts, boundsColors, boundsCount);
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
      const wayIn = WAY_IN.indexOf(stemOf(a.name)) >= 0;
      const c = a.socket ? [110, 220, 160] : wayIn ? [110, 168, 254] : [255, 180, 84];
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
    gl.depthMask(true);
    gl.disable(gl.BLEND);
    if (tri.length) drawArray(gl.TRIANGLES, new Float32Array(tri), new Uint8Array(col), tri.length / 3);
    if (lin.length) drawArray(gl.LINES, new Float32Array(lin), new Uint8Array(lcol), lin.length / 3);
  }

  const labelEls = new Map();

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
        el.className = "label" + (a.socket ? " socket" : (WAY_IN.indexOf(stemOf(a.name)) >= 0 ? " way-in" : ""));
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

  const pointers = new Map();
  let lastPinch = 0;

  function onDown(e) {
    canvas.setPointerCapture(e.pointerId);
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY, button: e.button, shift: e.shiftKey });
    lastPinch = 0;
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

    const panning = prev.button === 1 || prev.button === 2 || e.shiftKey;
    if (panning) pan(dx, dy);
    else look(dx, dy);
    invalidate();
  }

  function onUp(e) {
    pointers.delete(e.pointerId);
    lastPinch = 0;
  }

  function look(dx, dy) {
    const k = 0.005;
    // In a point of view the camera turns its head; in orbit it swings around
    // the model, which is the opposite sense.
    const sign = state.mode === "pov" ? 1 : -1;
    state.yaw -= sign * dx * k;
    state.pitch += sign * dy * k * (state.mode === "pov" ? -1 : 1);
    const lim = Math.PI / 2 - 0.001;
    state.pitch = Math.max(-lim, Math.min(lim, state.pitch));
    markCustom();
  }

  function pan(dx, dy) {
    const eye = cameraEye(), c = cameraCenter();
    let fx = c[0] - eye[0], fy = c[1] - eye[1], fz = c[2] - eye[2];
    const fl = Math.hypot(fx, fy, fz) || 1; fx /= fl; fy /= fl; fz /= fl;
    let rx = fz, rz = -fx;
    const rl = Math.hypot(rx, rz) || 1; rx /= rl; rz /= rl;
    const ux = rz * fy, uy = rx * fz - rz * fx, uz = -rx * fy;
    const scale = state.mode === "pov" ? 0.02 : state.dist * 0.0022;
    const mx = (-dx * rx + dy * ux) * scale;
    const my = (dy * uy) * scale;
    const mz = (-dx * rz + dy * uz) * scale;
    if (state.mode === "pov") {
      state.eye = [state.eye[0] + mx, state.eye[1] + my, state.eye[2] + mz];
    } else {
      state.target = [state.target[0] + mx, state.target[1] + my, state.target[2] + mz];
    }
    markCustom();
  }

  function zoom(amount) {
    if (state.mode === "pov") {
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

  const keys = new Set();
  function step() {
    if (state.mode === "pov" && keys.size) {
      const c = cameraCenter();
      let fx = c[0] - state.eye[0], fz = c[2] - state.eye[2];
      const fl = Math.hypot(fx, fz) || 1; fx /= fl; fz /= fl;
      const rx = fz, rz = -fx;
      let mx = 0, my = 0, mz = 0;
      const v = keys.has("shift") ? 0.34 : 0.13;
      if (keys.has("w")) { mx += fx * v; mz += fz * v; }
      if (keys.has("s")) { mx -= fx * v; mz -= fz * v; }
      if (keys.has("d")) { mx += rx * v; mz += rz * v; }
      if (keys.has("a")) { mx -= rx * v; mz -= rz * v; }
      if (keys.has(" ")) my += v;
      if (keys.has("c")) my -= v;
      if (mx || my || mz) {
        state.eye = [state.eye[0] + mx, state.eye[1] + my, state.eye[2] + mz];
        markCustom();
        invalidate();
      }
    }
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
      const k = e.key.toLowerCase();
      if ("wasdc ".indexOf(k) >= 0) { keys.add(k === " " ? " " : k); e.preventDefault(); }
      if (e.key === "Shift") keys.add("shift");
    });
    window.addEventListener("keyup", (e) => {
      const k = e.key.toLowerCase();
      keys.delete(k === " " ? " " : k);
      if (e.key === "Shift") keys.delete("shift");
    });
    window.addEventListener("blur", () => keys.clear());
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
    anchorSection: document.getElementById("anchor-section"),
    legend: document.getElementById("legend"),
    legendCount: document.getElementById("legend-count"),
    unresolved: document.getElementById("unresolved"),
    unresolvedSection: document.getElementById("unresolved-section"),
    panel: document.getElementById("panel"),
    app: document.getElementById("app"),
    panelToggle: document.getElementById("panel-toggle"),
  };

  let presetButtons = [];

  function syncPresetButtons() {
    for (const b of presetButtons) {
      b.setAttribute("aria-pressed", String(b.dataset.preset === state.preset));
    }
    els.cameraHint.textContent = state.mode === "pov"
      ? "Eyes " + EYE.toFixed(2) + " blocks above the floor of that cell. Drag to look, W/A/S/D to walk, space and C for up and down, scroll to step forward."
      : "Drag to orbit, shift-drag or right-drag to pan, scroll to zoom.";
  }

  function updateReadout() {
    if (!readout || !gl) return;
    const e = cameraEye();
    const fmt = (n) => n.toFixed(1);
    const heading = ((state.yaw * 180 / Math.PI) % 360 + 360) % 360;
    readout.textContent = state.mode === "pov"
      ? "eye " + fmt(e[0]) + ", " + fmt(e[1]) + ", " + fmt(e[2]) + "  ·  " + heading.toFixed(0) + "°  ·  player eye height " + EYE
      : "orbit  ·  " + fmt(state.dist) + " blocks out  ·  " + heading.toFixed(0) + "°";
  }

  function rebuildMesh() {
    if (!gl) return;
    state.mesh = buildMesh(state.model, state.cutY);
    updateStats();
    invalidate();
  }

  function updateStats() {
    const m = state.model;
    const [sx, sy, sz] = m.size;
    const parts = [
      sx + "×" + sy + "×" + sz,
      m.filled.toLocaleString() + " blocks",
      m.palette.length - 1 + " states",
    ];
    if (state.mesh) parts.push(state.mesh.faces.toLocaleString() + " faces drawn");
    els.stats.textContent = parts.join("  ·  ");
  }

  function selectModel(index) {
    const m = DATA.models[index];
    const [sx, sy, sz] = m.size;
    if (!m.grid) {
      m.grid = decodeGrid(m.voxels, sx * sy * sz);
      m.solid = new Uint8Array(m.palette.length);
      m.full = new Uint8Array(m.palette.length);
      m.alpha = new Float32Array(m.palette.length);
      for (let i = 1; i < m.palette.length; i++) {
        const e = m.palette[i];
        const fullCube = isFullCube(e.box);
        m.full[i] = fullCube ? 1 : 0;
        m.solid[i] = fullCube && e.cov >= 250 ? 1 : 0;
        m.alpha[i] = Math.min(1, e.cov / 255);
      }
    }
    state.model = m;
    state.cutY = sy - 1;
    els.cut.max = String(sy - 1);
    els.cut.value = String(sy - 1);
    els.cutValue.textContent = String(sy - 1);
    buildOverlays(m);
    buildPresetButtons(m);
    buildAnchorList(m);
    buildLegend(m);
    buildUnresolved(m);
    const hash = fromHash();
    if (hash.cut !== undefined && hash.cut !== "") {
      const y = Math.max(0, Math.min(sy - 1, Number(hash.cut)));
      if (Number.isFinite(y)) {
        state.cutY = y;
        els.cut.value = String(y);
        els.cutValue.textContent = String(y);
      }
    }
    rebuildMesh();
    const want = hash.preset;
    const known = want && (want === "exterior" || want === "plan"
      || (want.startsWith("pov:") && m.anchors.some((a) => a.name === want.slice(4) && a.pos)));
    applyPreset(known ? want : defaultPreset(m));
  }

  function buildPresetButtons(model) {
    els.presets.textContent = "";
    presetButtons = [];
    const list = [
      { id: "exterior", label: "Exterior ¾" },
      { id: "plan", label: "Plan" },
    ].concat(anchorPresets(model));
    for (const p of list) {
      const b = document.createElement("button");
      b.type = "button";
      b.textContent = p.label;
      b.dataset.preset = p.id;
      b.setAttribute("aria-pressed", "false");
      b.addEventListener("click", () => { applyPreset(p.id); invalidate(); });
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

  function buildLegend(model) {
    els.legend.textContent = "";
    const rows = [];
    for (let i = 1; i < model.palette.length; i++) {
      const e = model.palette[i];
      if (e.count > 0) rows.push(e);
    }
    rows.sort((a, b) => b.count - a.count || (a.name < b.name ? -1 : 1));
    els.legendCount.textContent = String(rows.length);
    for (const e of rows) {
      const li = document.createElement("li");
      const sw = document.createElement("span");
      sw.className = "swatch";
      sw.style.background = "rgb(" + e.rgb[0] + "," + e.rgb[1] + "," + e.rgb[2] + ")";
      const nm = document.createElement("span");
      nm.className = "name";
      nm.textContent = e.name.replace(/^minecraft:/, "");
      nm.title = e.name;
      const ct = document.createElement("span");
      ct.className = "count";
      ct.textContent = e.count.toLocaleString();
      li.append(sw, nm, ct);
      els.legend.appendChild(li);
    }
  }

  function buildUnresolved(model) {
    const keys = Object.keys(model.unresolved || {});
    els.unresolvedSection.hidden = keys.length === 0;
    els.unresolved.textContent = "";
    for (const k of keys) {
      const u = model.unresolved[k];
      const li = document.createElement("li");
      const head = document.createElement("span");
      head.textContent = k + "  ×" + u.count;
      const why = document.createElement("span");
      why.className = "why";
      why.textContent = u.detail;
      li.append(head, why);
      els.unresolved.appendChild(li);
    }
  }

  els.cut.addEventListener("input", () => {
    state.cutY = Number(els.cut.value);
    els.cutValue.textContent = els.cut.value;
    rebuildMesh();
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

  try {
    if (gl) initGL();
    selectModel(startIndex);
    if (gl) { invalidate(); requestAnimationFrame(step); }
  } catch (err) {
    if (fallback) {
      canvas.hidden = true;
      fallback.hidden = false;
      fallback.textContent = "The model could not be drawn: " + err.message;
    }
  }

  // A headless check needs to read what was built without a screenshot, so the
  // mesh statistics are reachable from the page rather than only on screen.
  window.delveViewer = {
    data: DATA,
    state: state,
    stats: () => ({
      id: state.model && state.model.id,
      size: state.model && state.model.size,
      filled: state.model && state.model.filled,
      runs: state.model && state.model.runs,
      faces: state.mesh ? state.mesh.faces : 0,
      anchors: state.model ? state.model.anchors.length : 0,
      unresolved: state.model ? Object.keys(state.model.unresolved || {}).length : 0,
      preset: state.preset,
      mode: state.mode,
      eye: cameraEye(),
      webgl: !!gl,
    }),
    applyPreset: (id) => { applyPreset(id); draw(); },
    selectModel: (i) => { selectModel(i); draw(); },
  };
})();
