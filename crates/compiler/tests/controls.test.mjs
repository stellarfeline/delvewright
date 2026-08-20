/*
 * Executable coverage for the viewer's control mapping.
 *
 * Why this file exists. The viewer's interactive layer had none, and could not
 * have had any: the whole camera lived in a page that only a browser ever ran.
 * A and D shipped swapped, were fixed by inspection, and the mapping was
 * reported broken a second time — twice, by the one reader whose hands are the
 * gate. Neither report could have been caught by anything in CI, because
 * nothing in CI could press a key.
 *
 * So the mapping is a dependency-free module and this runs it. Every assertion
 * below is a sentence about what a hand does, checked as arithmetic:
 *
 *   - D moves you to YOUR right, in every one of the four cardinal facings.
 *     Written with the strafe basis inverted — the exact defect that shipped —
 *     four of these fail.
 *   - Movement never consults a mode, because `walkStep` is not given one.
 *   - Keys match on the PHYSICAL key, so an input method cannot silence them.
 *   - The list printed in the panel is the table, not a second copy of it.
 *
 * Run: node --test crates/render/tests/
 * No dependencies, no build step, no browser.
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const HERE = dirname(fileURLToPath(import.meta.url));
const SRC = join(HERE, "..", "src", "view", "viewer", "controls.js");
const PAGE = join(HERE, "..", "src", "view", "viewer", "page.js");

/* The module is an inline browser script, not an ES module — it is concatenated
 * into a `<script>` block by `render_html`. Evaluating the source is therefore
 * the honest way to load it: it proves the file works in the form the page
 * actually gets, rather than a differently-packaged copy of it. */
const C = new Function(
  readFileSync(SRC, "utf8") + "\nreturn globalThis.DelveControls;",
)();

const A = C.ACTIONS;

/* Minecraft's axes: +X east, +Z south, +Y up. Yaw 0 faces south. */
const YAW = { south: 0, west: -Math.PI / 2, north: Math.PI, east: Math.PI / 2 };
const NEAR = 1e-9;

/** The dominant compass direction of a horizontal displacement. */
function heading(v) {
  const [x, , z] = v;
  if (Math.abs(x) < NEAR && Math.abs(z) < NEAR) return "still";
  if (Math.abs(x) > Math.abs(z)) return x > 0 ? "east" : "west";
  return z > 0 ? "south" : "north";
}

function walk(facing, ...actions) {
  return C.walkStep({ yaw: YAW[facing], held: new Set(actions) });
}

/* ---------------------------------------------------------------- walking -- */

test("W walks the way you are facing, in all four facings", () => {
  for (const facing of Object.keys(YAW)) {
    assert.equal(heading(walk(facing, A.WALK_FORWARD)), facing, `W facing ${facing}`);
  }
});

test("S walks backwards, in all four facings", () => {
  const opposite = { north: "south", south: "north", east: "west", west: "east" };
  for (const facing of Object.keys(YAW)) {
    assert.equal(heading(walk(facing, A.WALK_BACK)), opposite[facing], `S facing ${facing}`);
  }
});

test("D steps to YOUR right — the defect that shipped, in all four facings", () => {
  // Face north and your right hand points east; face south and it points west.
  // The inverted basis gives exactly these four answers reversed.
  const right = { north: "east", east: "south", south: "west", west: "north" };
  for (const facing of Object.keys(YAW)) {
    assert.equal(heading(walk(facing, A.STRAFE_RIGHT)), right[facing], `D facing ${facing}`);
  }
});

test("A steps to YOUR left, in all four facings", () => {
  const left = { north: "west", west: "south", south: "east", east: "north" };
  for (const facing of Object.keys(YAW)) {
    assert.equal(heading(walk(facing, A.STRAFE_LEFT)), left[facing], `A facing ${facing}`);
  }
});

test("A and D are exact opposites, at an arbitrary heading too", () => {
  const yaw = 0.7413;
  const l = C.walkStep({ yaw, held: new Set([A.STRAFE_LEFT]) });
  const r = C.walkStep({ yaw, held: new Set([A.STRAFE_RIGHT]) });
  for (let i = 0; i < 3; i++) assert.ok(Math.abs(l[i] + r[i]) < 1e-12, `axis ${i}`);
});

test("strafing is perpendicular to walking", () => {
  const yaw = 2.11;
  const f = C.walkStep({ yaw, held: new Set([A.WALK_FORWARD]) });
  const r = C.walkStep({ yaw, held: new Set([A.STRAFE_RIGHT]) });
  assert.ok(Math.abs(f[0] * r[0] + f[2] * r[2]) < 1e-12);
});

test("Space rises and C sinks, and neither moves you along the ground", () => {
  const up = walk("north", A.RISE);
  const down = walk("north", A.SINK);
  assert.ok(up[1] > 0 && down[1] < 0);
  assert.equal(heading(up), "still");
  assert.equal(heading(down), "still");
});

test("holding two directions is not faster than holding one", () => {
  const one = walk("north", A.WALK_FORWARD);
  const two = walk("north", A.WALK_FORWARD, A.STRAFE_RIGHT);
  const len = (v) => Math.hypot(v[0], v[2]);
  assert.ok(Math.abs(len(one) - len(two)) < 1e-12, "diagonal is not normalised");
  assert.ok(two[0] > 0 && two[2] < 0, "W+D facing north is not the north-east diagonal");
});

test("Shift moves faster in the same direction, never a different one", () => {
  const slow = walk("east", A.WALK_FORWARD);
  const fast = walk("east", A.WALK_FORWARD, A.FASTER);
  assert.ok(Math.hypot(fast[0], fast[2]) > Math.hypot(slow[0], slow[2]));
  assert.equal(heading(fast), heading(slow));
});

test("no held movement key is inert — every one produces a displacement", () => {
  const movers = [A.WALK_FORWARD, A.WALK_BACK, A.STRAFE_LEFT, A.STRAFE_RIGHT, A.RISE, A.SINK];
  for (const m of movers) {
    const v = C.walkStep({ yaw: 0.3, held: new Set([m]) });
    assert.ok(Math.hypot(v[0], v[1], v[2]) > 0, `${m} produced nothing`);
    assert.ok(C.isMovement(m), `${m} is not classed as movement`);
  }
});

test("walkStep is not given a camera mode, so it cannot gate on one", () => {
  // The regression was structural, not arithmetic: movement asked what mode the
  // page was in and declined. It cannot ask a question it is never told.
  const src = readFileSync(SRC, "utf8");
  const body = src.slice(src.indexOf("function walkStep"), src.indexOf("function lookStep"));
  assert.ok(!/\bmode\b/.test(body), "walkStep mentions a mode");
  assert.ok(!/\borbit\b/i.test(body), "walkStep mentions orbit");
});

/* ---------------------------------------------------------------- looking -- */

test("dragging right turns you right; dragging down looks down", () => {
  const yaw0 = YAW.south;
  const d = C.lookStep(10, 10);
  const before = C.basis(yaw0).forward;
  const after = C.basis(yaw0 + d.dyaw).forward;
  const right = C.basis(yaw0).right;
  assert.ok(after[0] * right[0] + after[1] * right[1] > 0, "drag right did not turn right");
  assert.ok(before[0] * right[0] + before[1] * right[1] < 1e-12, "sanity: forward is not right");
  assert.ok(d.dpitch < 0, "drag down did not look down");
});

test("dragging left and up are the exact inverses", () => {
  const a = C.lookStep(7, 3);
  const b = C.lookStep(-7, -3);
  assert.ok(Math.abs(a.dyaw + b.dyaw) < 1e-15);
  assert.ok(Math.abs(a.dpitch + b.dpitch) < 1e-15);
});

test("orbiting inverts yaw against walking, and leaves pitch alone", () => {
  const w = C.lookStep(9, 4);
  const o = C.orbitStep(9, 4);
  assert.ok(Math.abs(w.dyaw + o.dyaw) < 1e-15, "orbit yaw is not the walk's inverse");
  assert.ok(Math.abs(w.dpitch - o.dpitch) < 1e-15, "orbit pitch differs from the walk's");
});

test("arrow keys look the same way the drag does", () => {
  const left = C.lookKeyStep(new Set([A.LOOK_LEFT]));
  const right = C.lookKeyStep(new Set([A.LOOK_RIGHT]));
  const dragRight = C.lookStep(10, 0);
  assert.ok(right.dyaw < 0 && left.dyaw > 0);
  assert.ok(Math.sign(right.dyaw) === Math.sign(dragRight.dyaw), "arrow and drag disagree");
  assert.ok(C.lookKeyStep(new Set([A.LOOK_DOWN])).dpitch < 0);
  assert.ok(C.lookKeyStep(new Set([A.LOOK_UP])).dpitch > 0);
});

test("pitch cannot pass straight up or straight down", () => {
  assert.ok(C.clampPitch(99) < Math.PI / 2);
  assert.ok(C.clampPitch(-99) > -Math.PI / 2);
  assert.equal(C.clampPitch(0.2), 0.2);
});

/* ------------------------------------------------------------------ keys -- */

test("every binding matches the PHYSICAL key", () => {
  assert.ok(C.BINDINGS.length > 0, "the key table is empty");
  const physical = /^(Key[A-Z]|Arrow(Up|Down|Left|Right)|Space|Shift(Left|Right))$/;
  for (const b of C.BINDINGS) {
    assert.match(b.code, physical, `binding ${JSON.stringify(b)} is not a physical key`);
    assert.ok(Object.values(A).includes(b.action), `unknown action ${b.action}`);
  }
});

test("an input method that rewrites .key cannot silence WASD", () => {
  // With a Chinese IME composing, every letter arrives as key "Process". A
  // mapping keyed on `.key` is dead for exactly the reader this page is for.
  for (const [code, action] of [
    ["KeyW", A.WALK_FORWARD], ["KeyA", A.STRAFE_LEFT],
    ["KeyS", A.WALK_BACK], ["KeyD", A.STRAFE_RIGHT],
  ]) {
    assert.equal(C.actionFor({ code, key: "Process" }), action, `${code} under an IME`);
  }
});

test("a layout that puts other letters under WASD still walks", () => {
  // Dvorak: the physical W key reports ",". AZERTY: the physical A key is "q".
  assert.equal(C.actionFor({ code: "KeyW", key: "," }), A.WALK_FORWARD);
  assert.equal(C.actionFor({ code: "KeyA", key: "q" }), A.STRAFE_LEFT);
});

test("both shift keys mean the same thing and no key is bound twice", () => {
  assert.equal(C.actionFor({ code: "ShiftLeft" }), A.FASTER);
  assert.equal(C.actionFor({ code: "ShiftRight" }), A.FASTER);
  const codes = C.BINDINGS.map((b) => b.code);
  assert.equal(new Set(codes).size, codes.length, "a physical key is bound twice");
});

test("Ctrl is bound to nothing — Ctrl+W closes the tab before the page sees it", () => {
  for (const b of C.BINDINGS) {
    assert.ok(!/^Control/.test(b.code), `${b.code} is bound; Ctrl+W would kill the tab`);
  }
});

test("an unbound key is not claimed", () => {
  assert.equal(C.actionFor({ code: "KeyQ", key: "q" }), null);
  assert.equal(C.actionFor({ code: "F5", key: "F5" }), null);
  assert.equal(C.actionFor(null), null);
  assert.equal(C.actionFor({}), null);
});

test("a focused form control keeps its own keys, so the cutaway slider works", () => {
  for (const tag of ["INPUT", "SELECT", "TEXTAREA", "BUTTON"]) {
    assert.equal(C.isTypingTarget({ tagName: tag }), true, tag);
  }
  assert.equal(C.isTypingTarget({ tagName: "CANVAS" }), false);
  assert.equal(C.isTypingTarget({ tagName: "DIV", isContentEditable: true }), true);
  assert.equal(C.isTypingTarget(null), false);
});

/* -------------------------------------------------------------- the page -- */

test("the panel prints the table rather than a second copy of it", () => {
  const page = readFileSync(PAGE, "utf8");
  assert.match(page, /C\.HELP/, "the help list is not built from the table");
  assert.ok(C.HELP.length > 0, "the help list is empty");
  for (const row of C.HELP) {
    assert.equal(typeof row.gesture, "string");
    assert.equal(typeof row.effect, "string");
    assert.ok(row.gesture.length > 0 && row.effect.length > 0);
  }
  // Every movement key the table binds is named somewhere in the printed list.
  const printed = C.HELP.map((r) => r.gesture).join(" ").toUpperCase();
  for (const letter of ["W", "A", "S", "D"]) {
    assert.ok(printed.includes(letter), `${letter} is bound but never shown`);
  }
});

test("the page resolves keys through the table and nowhere else", () => {
  const page = readFileSync(PAGE, "utf8");
  assert.match(page, /C\.actionFor\(/, "the page does not use the shared table");
  assert.match(page, /C\.walkStep\(/, "the page does not use the shared walk");
  // The old hand-rolled dispatch, in every form it took.
  assert.ok(!/"wasdc ?"\.indexOf/.test(page), "hand-rolled key string survives");
  assert.ok(!/keys\.has\("[wasdc]"\)/.test(page), "hand-rolled key set survives");
  assert.ok(!/e\.key\.toLowerCase\(\)/.test(page), "the page still dispatches on .key");
});

/* ------------------------------------------------------- the mouse's path -- */

/*
 * The other half of the owner's report was the mouse, and it was not a mapping
 * bug at all: `#fallback` — the no-WebGL notice — is `hidden` in the markup and
 * `position: absolute; inset: 0; display: grid` in the stylesheet. An author
 * `display` beats the user agent's `[hidden] { display: none }`, so that empty
 * paragraph was laid out over the whole stage, invisible and hit-testable.
 * `document.elementFromPoint` in the middle of the model returned `fallback`.
 * Every drag and every wheel event in the tool's life had landed on it.
 *
 * A mapping test cannot see this: the handlers were correct and were never
 * called. So the checks below are about the page's geometry, not its arithmetic.
 */

const CSS = readFileSync(join(HERE, "..", "src", "view", "viewer", "page.css"), "utf8");
const HTML = readFileSync(join(HERE, "..", "src", "view", "viewer", "page.html"), "utf8");

test("the [hidden] attribute actually hides, against any author display", () => {
  assert.match(
    CSS,
    /\[hidden\]\s*\{[^}]*display:\s*none\s*!important/,
    "nothing makes [hidden] beat an author display rule",
  );
});

test("no overlay over the canvas can swallow a gesture", () => {
  // Driven from the STYLESHEET, not from a slice of the markup: an earlier
  // version of this test walked `#stage`'s children and stopped at the first
  // `</div>`, which is the empty `#labels` element — so it examined one node,
  // passed, and would have missed the very defect it was written for. Every
  // absolutely positioned element the page names is a candidate here, wherever
  // it sits in the tree.
  const positioned = [...CSS.matchAll(/#([\w-]+)\s*\{([^}]*)\}/g)]
    .map((m) => ({ id: m[1], decl: m[2] }))
    .filter((r) => /position:\s*(absolute|fixed)/.test(r.decl))
    .filter((r) => r.id !== "view");

  // Binding count, stated: a scan that matched nothing is a finding, not a pass.
  assert.ok(
    positioned.length >= 3,
    `only ${positioned.length} positioned elements found — the scan has come unbound`,
  );

  for (const el of positioned) {
    const inert = /pointer-events:\s*none/.test(el.decl);
    // `hidden` counts only because the previous test proves the attribute is
    // honoured. Without that rule this escape is exactly what the defect
    // supplied for itself: `#fallback` was hidden AND swallowing every gesture.
    const declaredHidden = new RegExp(`id="${el.id}"[^>]*\\shidden`).test(HTML);
    assert.ok(
      inert || declaredHidden,
      `#${el.id} is laid over the page and can take a pointer event`,
    );
  }
});

test("the canvas can hold focus, so keys have somewhere to land", () => {
  assert.match(HTML, /<canvas id="view"[^>]*tabindex="0"/, "the canvas is not focusable");
});

test("the walk is reachable from load — no preset defaults to orbit", () => {
  const page = readFileSync(PAGE, "utf8");
  const decl = page.slice(page.indexOf("function defaultPreset"));
  const body = decl.slice(0, decl.indexOf("\n  }"));
  assert.ok(!/"exterior"/.test(body), "the default camera can still be an orbit");
  assert.match(page, /mode: "walk"/, "the page does not open on foot");
});

test("a piece that SAYS which anchor is the way in is believed before its names", () => {
  // The stems are a reading of a name its author chose for other reasons; the
  // role is the piece's own statement. A grammar-exported zone can only make the
  // statement — every key it writes is `anchor/<stem>` — so a page that consults
  // the stems first opens such a zone on whatever it happens to be called.
  const page = readFileSync(PAGE, "utf8");
  const decl = page.slice(page.indexOf("function defaultPreset"));
  const body = decl.slice(0, decl.indexOf("\n  }"));
  const declared = body.indexOf('a.role === "entry"');
  const guessed = body.indexOf("for (const stem of WAY_IN)");
  assert.ok(declared >= 0, "the page never asks what the piece declared");
  assert.ok(guessed >= 0, "the stem guess has gone — this test now proves nothing");
  assert.ok(declared < guessed, "a guessed name stem is consulted before the declaration");
});
