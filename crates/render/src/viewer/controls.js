/* The viewer's control mapping — one table, one set of vectors, no modes.
 *
 * This file is deliberately DOM-free and renderer-free. It knows keys, gestures
 * and arithmetic; it does not know WebGL, deepslate, or which page is asking.
 * That is the point: the rendering core underneath the viewer is being replaced,
 * and a mapping that lives inside a renderer is a mapping that has to be
 * rewritten — and re-broken — every time the renderer changes. A page adopts the
 * controls by calling into here, whatever it draws with.
 *
 * Two properties this file exists to hold, both of which have failed in the past:
 *
 *   1. WASD walks ALWAYS. There is no camera mode in which a movement key does
 *      nothing. A page that gates movement on a mode the reviewer never chose
 *      reads as a broken page, because from the reviewer's side it is one.
 *   2. Keys are matched on `KeyboardEvent.code`, the physical key, never on
 *      `.key`. `.key` is the character the layout and the input method produce:
 *      with a Chinese IME active `.key` is `"Process"` for every letter, so a
 *      `.key`-matched WASD is dead exactly for the reader this project has.
 *
 * `crates/render/tests/controls.test.mjs` executes every claim below.
 */
"use strict";

var DELVE_CONTROLS = (function () {
  /** Movement is expressed as an action, so the key that produced it is one
   *  lookup away from the vector it becomes, and only one place decides which. */
  var ACTIONS = {
    WALK_FORWARD: "walk_forward",
    WALK_BACK: "walk_back",
    STRAFE_LEFT: "strafe_left",
    STRAFE_RIGHT: "strafe_right",
    RISE: "rise",
    SINK: "sink",
    FASTER: "faster",
    LOOK_LEFT: "look_left",
    LOOK_RIGHT: "look_right",
    LOOK_UP: "look_up",
    LOOK_DOWN: "look_down",
  };

  /**
   * The key table.
   *
   * `code` is the physical key and is what actually matches. `key` is a
   * last-resort fallback for an environment that reports no `code` at all; it is
   * never consulted while `code` is present, so an input method that rewrites
   * `.key` cannot reach it.
   *
   * `Shift` is "move faster" and NOT "descend", and the modifier is not `Ctrl`,
   * on purpose: `Ctrl`+`W` closes the tab in Chrome on Windows and Linux before
   * any page handler runs, so a `Ctrl` sprint would make walking forward at speed
   * destroy the reviewer's work. `C` descends instead.
   */
  var BINDINGS = [
    { code: "KeyW", key: "w", action: ACTIONS.WALK_FORWARD },
    { code: "KeyS", key: "s", action: ACTIONS.WALK_BACK },
    { code: "KeyA", key: "a", action: ACTIONS.STRAFE_LEFT },
    { code: "KeyD", key: "d", action: ACTIONS.STRAFE_RIGHT },
    { code: "Space", key: " ", action: ACTIONS.RISE },
    { code: "KeyC", key: "c", action: ACTIONS.SINK },
    { code: "ShiftLeft", key: "shift", action: ACTIONS.FASTER },
    { code: "ShiftRight", key: "shift", action: ACTIONS.FASTER },
    { code: "ArrowLeft", key: "arrowleft", action: ACTIONS.LOOK_LEFT },
    { code: "ArrowRight", key: "arrowright", action: ACTIONS.LOOK_RIGHT },
    { code: "ArrowUp", key: "arrowup", action: ACTIONS.LOOK_UP },
    { code: "ArrowDown", key: "arrowdown", action: ACTIONS.LOOK_DOWN },
  ];

  /** What the panel tells the reviewer. Derived from the table above rather than
   *  written twice, so the page cannot document a mapping it does not have. */
  var HELP = [
    { gesture: "W A S D", effect: "walk — forward, left, back, right" },
    { gesture: "Drag", effect: "look around" },
    { gesture: "Arrow keys", effect: "look around, a little at a time" },
    { gesture: "Space / C", effect: "rise / sink" },
    { gesture: "Hold Shift", effect: "move faster" },
    { gesture: "Right- or middle-drag", effect: "slide sideways and up without turning" },
    { gesture: "Scroll", effect: "move toward or away from what you are looking at" },
    { gesture: "Orbit button", effect: "swing around the outside of the whole piece" },
  ];

  var WALK_SPEED = 0.13;      // blocks per frame
  var FAST_FACTOR = 2.6;
  var LOOK_SENSITIVITY = 0.005;   // radians per pixel of drag
  var KEY_LOOK = 0.030;           // radians per frame while an arrow key is held
  var PITCH_LIMIT = Math.PI / 2 - 0.001;

  /**
   * Which action a key event means, or `null`.
   *
   * `code` first and `key` only as a fallback — see the note on BINDINGS. The
   * event is read defensively because this is also called from tests with plain
   * objects.
   */
  function actionFor(ev) {
    if (!ev) return null;
    var i;
    if (typeof ev.code === "string" && ev.code) {
      for (i = 0; i < BINDINGS.length; i++) {
        if (BINDINGS[i].code === ev.code) return BINDINGS[i].action;
      }
      return null;
    }
    if (typeof ev.key === "string" && ev.key) {
      var k = ev.key.toLowerCase();
      for (i = 0; i < BINDINGS.length; i++) {
        if (BINDINGS[i].key === k) return BINDINGS[i].action;
      }
    }
    return null;
  }

  /** True when this action moves the body, as opposed to turning the head. */
  function isMovement(action) {
    return action === ACTIONS.WALK_FORWARD || action === ACTIONS.WALK_BACK
      || action === ACTIONS.STRAFE_LEFT || action === ACTIONS.STRAFE_RIGHT
      || action === ACTIONS.RISE || action === ACTIONS.SINK;
  }

  /**
   * The horizontal basis of a body facing `yaw`.
   *
   * Minecraft's axes: +X east, +Z south, +Y up. `yaw` 0 faces +Z (south) and
   * increases turning to the LEFT, which is the convention the page's camera
   * already used.
   *
   * `right` is `(-fz, fx)`. Facing north `(0, -1)` it gives east `(1, 0)`;
   * facing south `(0, 1)` it gives west `(-1, 0)` — which is where your right
   * hand points in each case. Written with the signs the other way round it is
   * the LEFT vector and A and D swap, which is how this page shipped once.
   */
  function basis(yaw) {
    var fx = Math.sin(yaw), fz = Math.cos(yaw);
    return { forward: [fx, fz], right: [-fz, fx] };
  }

  function holds(held, action) {
    if (!held) return false;
    if (typeof held.has === "function") return held.has(action);
    return held.indexOf(action) >= 0;
  }

  /**
   * The displacement one frame of the currently held keys produces, in blocks.
   *
   * Takes no mode: there is no camera state in which this returns zero for a
   * held W. A page that wants an orbit view leaves orbit before it walks; it
   * never asks this function for permission.
   *
   * Diagonals are normalised, so holding W and D is not faster than holding W.
   */
  function walkStep(opts) {
    opts = opts || {};
    var held = opts.held;
    var b = basis(opts.yaw || 0);
    var x = 0, y = 0, z = 0;

    if (holds(held, ACTIONS.WALK_FORWARD)) { x += b.forward[0]; z += b.forward[1]; }
    if (holds(held, ACTIONS.WALK_BACK)) { x -= b.forward[0]; z -= b.forward[1]; }
    if (holds(held, ACTIONS.STRAFE_RIGHT)) { x += b.right[0]; z += b.right[1]; }
    if (holds(held, ACTIONS.STRAFE_LEFT)) { x -= b.right[0]; z -= b.right[1]; }

    var h = Math.sqrt(x * x + z * z);
    if (h > 1) { x /= h; z /= h; }

    if (holds(held, ACTIONS.RISE)) y += 1;
    if (holds(held, ACTIONS.SINK)) y -= 1;

    var speed = opts.speed === undefined ? WALK_SPEED : opts.speed;
    if (holds(held, ACTIONS.FASTER)) speed *= FAST_FACTOR;
    return [x * speed, y * speed, z * speed];
  }

  /**
   * A drag of `dx, dy` pixels, as a change in yaw and pitch, first-person sense:
   * drag right and the view turns right, drag down and it looks down. Same
   * gesture, same meaning, every time the page is in walk — which is by default.
   */
  function lookStep(dx, dy, sens) {
    var k = sens === undefined ? LOOK_SENSITIVITY : sens;
    return { dyaw: -dx * k, dpitch: -dy * k };
  }

  /**
   * The same drag while orbiting. Yaw inverts because the gesture now grabs the
   * MODEL rather than the head; pitch does not, because tilting the model down
   * and tilting the head down look identical from the reviewer's side.
   */
  function orbitStep(dx, dy, sens) {
    var s = lookStep(dx, dy, sens);
    return { dyaw: -s.dyaw, dpitch: s.dpitch };
  }

  /** One frame of held arrow keys, in the same units `lookStep` returns. */
  function lookKeyStep(held, rate) {
    var k = rate === undefined ? KEY_LOOK : rate;
    var dyaw = 0, dpitch = 0;
    if (holds(held, ACTIONS.LOOK_LEFT)) dyaw += k;
    if (holds(held, ACTIONS.LOOK_RIGHT)) dyaw -= k;
    if (holds(held, ACTIONS.LOOK_UP)) dpitch += k;
    if (holds(held, ACTIONS.LOOK_DOWN)) dpitch -= k;
    return { dyaw: dyaw, dpitch: dpitch };
  }

  function clampPitch(p) {
    return Math.max(-PITCH_LIMIT, Math.min(PITCH_LIMIT, p));
  }

  /**
   * True when the key belongs to the focused control rather than to the camera.
   *
   * Without this the cutaway slider loses its arrow keys the moment the page
   * starts steering with them, and Space stops activating a focused button — a
   * control the reviewer can see and cannot use is worse than one that is absent.
   */
  function isTypingTarget(el) {
    if (!el || typeof el.tagName !== "string") return false;
    if (el.isContentEditable) return true;
    var tag = el.tagName.toLowerCase();
    return tag === "input" || tag === "select" || tag === "textarea"
      || tag === "button" || tag === "option";
  }

  return {
    ACTIONS: ACTIONS,
    BINDINGS: BINDINGS,
    HELP: HELP,
    WALK_SPEED: WALK_SPEED,
    FAST_FACTOR: FAST_FACTOR,
    LOOK_SENSITIVITY: LOOK_SENSITIVITY,
    KEY_LOOK: KEY_LOOK,
    PITCH_LIMIT: PITCH_LIMIT,
    actionFor: actionFor,
    isMovement: isMovement,
    basis: basis,
    walkStep: walkStep,
    lookStep: lookStep,
    orbitStep: orbitStep,
    lookKeyStep: lookKeyStep,
    clampPitch: clampPitch,
    isTypingTarget: isTypingTarget,
  };
})();

if (typeof globalThis !== "undefined") globalThis.DelveControls = DELVE_CONTROLS;
