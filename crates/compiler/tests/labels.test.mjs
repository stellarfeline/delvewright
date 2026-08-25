/*
 * Executable coverage for the viewer's anchor labels across a scene switch.
 *
 * Why this file exists. Switching prefab left the previous prefab's labels
 * drawn over the new one, and it was reported by the one reader whose hands
 * are the gate — the same way the control mapping was, twice. Nothing in CI
 * could have caught it: the label pass lives in a page that only a browser
 * ever ran.
 *
 * What it asserts, and what it does not. It asserts the STATE TRANSITION, not
 * the pixels: that after the displayed model changes, the label layer holds
 * the new model's anchors and nothing else. It does not assert where a label
 * lands on screen or which survive the overlap pass — placement needs a
 * projection matrix, which is written only inside `draw`, which needs a WebGL
 * context. The leak was a property of the element pool, and the pool is what
 * this reads.
 *
 * How it runs the real thing. `page.js` is the file the page actually gets,
 * evaluated here under a DOM small enough to read. The canvas reports no WebGL
 * context, which is a path the page already supports — it hides the canvas and
 * carries on with the panel — and the label pass itself uses no context.
 *
 * The fixture is hand-built, and the direction that matters is which way drift
 * fails: a `page.js` that grows a need for some DATA field the fixture lacks
 * throws here and reds. There is no way for it to pass quietly against a page
 * that has moved on.
 *
 * Run: node --test crates/compiler/tests/labels.test.mjs
 * No dependencies, no build step, no browser.
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const HERE = dirname(fileURLToPath(import.meta.url));
const VIEWER = join(HERE, "..", "src", "view", "viewer");

/* ------------------------------------------------------------------- dom -- */

/* Enough DOM for the page to boot and for the label pass to run. Two of these
 * behaviours are load-bearing and are the ones to distrust, because a shim
 * that got them wrong would decide the result by itself:
 *
 *   - setting `textContent` on an element DISCARDS its children, which is how
 *     the page empties every one of its per-model containers, the label layer
 *     included;
 *   - `children` is the live list, so what the test reads back is what the
 *     page left there.
 *
 * The perturbation check at the bottom of this file is what establishes that
 * the shim separates the two states rather than manufacturing a pass: with the
 * repair removed, these same assertions fail. */
class El {
  constructor(tag) {
    this.tagName = String(tag).toUpperCase();
    this.children = [];
    this.style = {};
    this.dataset = {};
    this.className = "";
    this.id = "";
    this.hidden = false;
    this.value = "";
    this.checked = false;
    this.disabled = false;
    this._text = "";
    this.classList = {
      _el: this,
      add() {},
      remove() {},
      toggle: () => false,
      contains: () => false,
    };
  }
  get textContent() {
    return this.children.length
      ? this.children.map((c) => c.textContent).join("")
      : this._text;
  }
  set textContent(v) {
    // The real setter replaces every child with a single text node.
    this.children = [];
    this._text = String(v);
  }
  appendChild(c) {
    this.children.push(c);
    return c;
  }
  addEventListener() {}
  removeEventListener() {}
  setAttribute() {}
  getAttribute() {
    return null;
  }
  focus() {}
  blur() {}
  getContext() {
    // No WebGL. The page supports this and says so in the fallback.
    return null;
  }
  getBoundingClientRect() {
    return { left: 0, top: 0, width: 1200, height: 800 };
  }
  get clientWidth() {
    return 1200;
  }
  get clientHeight() {
    return 800;
  }
  get offsetWidth() {
    return 60;
  }
  get offsetHeight() {
    return 18;
  }
}

/* The ids are DERIVED from the page's own markup rather than listed here, so a
 * new element in `page.html` cannot leave this shim quietly short of one. */
function idsFromMarkup() {
  const html = readFileSync(join(VIEWER, "page.html"), "utf8");
  return [...html.matchAll(/id="([^"]+)"/g)].map((m) => m[1]);
}

function makeDocument(data) {
  const byId = new Map();
  for (const id of idsFromMarkup()) {
    const el = new El("div");
    el.id = id;
    byId.set(id, el);
  }
  // The page reads its model out of a script block the compiler writes in.
  const model = new El("script");
  model.id = "delve-model";
  model.textContent = JSON.stringify(data);
  byId.set("delve-model", model);

  return {
    getElementById: (id) => byId.get(id) || null,
    createElement: (tag) => new El(tag),
    addEventListener() {},
    hidden: false,
    _byId: byId,
  };
}

/* ---------------------------------------------------------------- fixture -- */

/**
 * Geometry as the page decodes it: run-length pairs of (value u16, length u16),
 * little-endian, base64. Written out here rather than pasted as a magic string
 * so the fixture states the format it is claiming to be in.
 */
function rle(runs) {
  const b = new Uint8Array(runs.length * 4);
  runs.forEach(([v, n], i) => {
    b[i * 4] = v & 0xff;
    b[i * 4 + 1] = (v >> 8) & 0xff;
    b[i * 4 + 2] = n & 0xff;
    b[i * 4 + 3] = (n >> 8) & 0xff;
  });
  return Buffer.from(b).toString("base64");
}

/**
 * A prefab of `n` anchors named `<id>-1 … <id>-n`, in a one-cell box, plus any
 * `shared` names verbatim — a name another prefab on the page also declares.
 */
function prefabModel(id, n, shared = []) {
  const anchor = (name) => ({ name: "anchor/" + name, pos: [0, 0, 0], facing: "north" });
  return {
    id,
    size: [1, 1, 1],
    voxels: rle([[0, 1]]), // one cell of palette entry 0 — air
    palette: [{ name: "minecraft:air" }],
    filled: 0,
    runs: 1,
    tiles: 1,
    anchors: [
      ...Array.from({ length: n }, (_, i) => anchor(id + "-" + (i + 1))),
      ...shared.map(anchor),
    ],
  };
}

function makeData(models) {
  return {
    eye_height: 1.62,
    way_in_stems: ["way-in"],
    models,
    flags: {},
    textures: {},
    block_models: {},
    blockstates: {},
    special_bound: 0,
    unresolved: [],
    under_specified: [],
  };
}

/* ------------------------------------------------------------------- run -- */

/**
 * Boot the real `page.js` over the shim and hand back its test surface.
 *
 * `mutate` is applied to the source before evaluation. It exists for the
 * perturbation check: a gate that stays green once the safety is taken out is
 * pointed the wrong way, so this file removes the repair and requires a red.
 */
function boot(data, mutate = (s) => s) {
  const doc = makeDocument(data);
  const win = {};

  // The control table is a global the page reads by bare name, exactly as the
  // browser gives it: `controls.js` is its own `<script>` block ahead of this
  // one. Loading it the same way keeps this a test of the shipped files rather
  // than of a differently-packaged copy.
  const DelveControls = new Function(
    readFileSync(join(VIEWER, "controls.js"), "utf8")
      + "\nreturn globalThis.DelveControls;",
  )();

  const page = mutate(readFileSync(join(VIEWER, "page.js"), "utf8"));
  const run = new Function(
    "document",
    "window",
    "deepslate",
    "requestAnimationFrame",
    "devicePixelRatio",
    "DelveControls",
    "location",
    page + "\nreturn window.delveViewer;",
  );
  const viewer = run(
    doc,
    win,
    /* deepslate: never reached without a WebGL context */ {},
    () => 0,
    1,
    DelveControls,
    // No fragment: the page's `#model=` route is a way INTO a model, and this
    // file is about leaving one.
    { hash: "" },
  );
  return { viewer, doc, layer: doc.getElementById("labels") };
}

/** What the label layer holds, as plain names. */
function layerNames(layer) {
  return layer.children.map((el) => el.textContent).sort();
}

/** The anchor names of the model the viewer is showing, as the layer spells them. */
function modelNames(viewer) {
  return viewer.state.model.anchors
    .map((a) => a.name.replace(/^anchor\//, ""))
    .sort();
}

/* ----------------------------------------------------------------- tests -- */

/* Four prefabs of four different anchor counts, so a switch changes the size of
 * the pool as well as its contents, in both directions — a repair that grew the
 * layer but never shrank it would pass on same-sized neighbours. `alpha` and
 * `beta` are the pair the single-switch tests name; the pair sweep uses all
 * four, which is twelve ordered pairs rather than two. Names are shared across
 * prefabs on purpose (`shared-1`): the pool is keyed by name, and a name that
 * appears in two prefabs is the case that hands the new scene the old scene's
 * element. A page of the eight Halgrave zones has six such anchors. */
const DATA = makeData([
  prefabModel("alpha", 5, ["shared-1"]),
  prefabModel("beta", 3),
  prefabModel("gamma", 8, ["shared-1"]),
  prefabModel("delta", 1),
]);

test("the label layer holds the anchors of the model on screen", () => {
  const { viewer, layer } = boot(DATA);

  viewer.selectModel(0);
  viewer.positionLabels();
  assert.deepEqual(layerNames(layer), modelNames(viewer));
  assert.equal(layer.children.length, DATA.models[0].anchors.length,
    "one element per anchor the model declares");
});

test("switching scene leaves none of the previous scene's labels behind", () => {
  const { viewer, layer } = boot(DATA);

  viewer.selectModel(0);
  viewer.positionLabels();
  const gone = layerNames(layer);
  assert.equal(gone.length, DATA.models[0].anchors.length);

  // The defect was visible only for labels that were on screen at the moment
  // of the switch — the pass hid the ones it visited and never visited these.
  // Showing them all is the worst case, and the one she saw.
  for (const el of layer.children) el.style.display = "";

  viewer.selectModel(1);
  viewer.positionLabels();

  const left = layerNames(layer);
  assert.deepEqual(left, modelNames(viewer), "the layer is the new model's anchors");
  assert.equal(layer.children.length, DATA.models[1].anchors.length,
    "one element per anchor the NEW model declares");
  for (const name of gone) {
    assert.ok(!left.includes(name), `${name} belongs to the model we left`);
  }
});

test("a label never outlives its scene, over every ordered pair of models", () => {
  const { viewer, layer } = boot(DATA);
  let pairs = 0;
  for (let a = 0; a < DATA.models.length; a++) {
    for (let b = 0; b < DATA.models.length; b++) {
      if (a === b) continue;
      viewer.selectModel(a);
      viewer.positionLabels();
      for (const el of layer.children) el.style.display = "";
      viewer.selectModel(b);
      viewer.positionLabels();
      assert.deepEqual(layerNames(layer), modelNames(viewer),
        `${DATA.models[a].id} -> ${DATA.models[b].id}`);
      pairs++;
    }
  }
  // Binding count, computed from the models rather than written down beside
  // them — a count that is a constant is green on a sweep that swept nothing.
  const n = DATA.models.length;
  assert.equal(pairs, n * (n - 1));
  console.log(`bound ${pairs} ordered pair(s) over ${n} model(s)`);
});

test("labels turned off do not survive a scene switch either", () => {
  const { viewer, layer } = boot(DATA);

  viewer.selectModel(0);
  viewer.positionLabels();
  const alpha = layerNames(layer);
  assert.equal(alpha.length, DATA.models[0].anchors.length);
  for (const el of layer.children) el.style.display = "";

  // The pass returns early when the reviewer has the labels switched off. The
  // pool is still the previous model's at that point, so this is the case a
  // repair placed AFTER the early return would miss: it must be discarded on
  // the way past, and nothing put in its place while labels are off.
  viewer.state.show.labels = false;
  viewer.selectModel(1);
  viewer.positionLabels();

  assert.deepEqual(layerNames(layer), [],
    "labels are switched off, so the layer holds nothing at all");
});

/* ---------------------------------------------------- the perturbation -- */

test("PERTURBATION: with the pool no longer bound to the model, the leak returns", () => {
  // Not a claim about the page as it ships. This takes the repair out of the
  // source in memory and requires the assertions above to FAIL, which is what
  // establishes that they bind to the repair and not to the DOM shim. Two
  // shapes, because they fail for different reasons:
  //
  //   1. the clearing dropped   — the pool is never emptied;
  //   2. the authority lookup made constant — it is emptied once and then
  //      believes it always holds the right model.
  const shapes = {
    "clearing dropped": (s) => {
      const before = s;
      const after = s.replace(
        "    labelLayer.textContent = \"\";\n    labelEls.clear();\n",
        "",
      );
      assert.notEqual(after, before, "the clearing is no longer where this expects it");
      return after;
    },
    "authority lookup made constant": (s) => {
      const before = s;
      const after = s.replace(
        "    if (labelModel === model) return labelEls;",
        "    if (true) return labelEls;",
      );
      assert.notEqual(after, before, "the authority check is no longer where this expects it");
      return after;
    },
  };

  for (const [name, mutate] of Object.entries(shapes)) {
    const { viewer, layer } = boot(DATA, mutate);
    viewer.selectModel(0);
    viewer.positionLabels();
    const alpha = layerNames(layer);
    for (const el of layer.children) el.style.display = "";

    viewer.selectModel(1);
    viewer.positionLabels();
    const left = layerNames(layer);

    const stale = alpha.filter((n) => left.includes(n));
    assert.equal(stale.length, DATA.models[0].anchors.length,
      `${name}: every one of the previous model's labels should still be there`);
    assert.notDeepEqual(left, modelNames(viewer),
      `${name}: the layer should NOT be just beta's anchors`);
  }
});
