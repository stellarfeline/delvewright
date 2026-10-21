// The face a block press clicks: never a zero direction, and for a bell one of
// the faces vanilla rings it on.

import { test } from "node:test";
import assert from "node:assert/strict";
import { FACE_NORMALS, pressFace, usableFaces } from "../src/press-face.ts";

const CEILING = "minecraft:bell[attachment=ceiling,facing=north,powered=false]";

test("an eye straight under a ceiling bell still gets a horizontal face", () => {
  const f = pressFace(CEILING, 0, 0);
  const [nx, nz] = FACE_NORMALS[f];
  assert.notDeepEqual([nx, nz], [0, 0]);
  assert.equal(f, "north");
});

test("the face turned toward the eye is chosen among those that ring", () => {
  assert.equal(pressFace(CEILING, 0.2, 3), "south");
  assert.equal(pressFace(CEILING, -2, 0.5), "west");
  // A floor bell facing north rings only on its north and south faces.
  const floor = "minecraft:bell[attachment=floor,facing=north,powered=false]";
  assert.deepEqual(usableFaces(floor), ["north", "south"]);
  assert.equal(pressFace(floor, 3, 0.1), "south");
  // A wall bell facing east rings only across its axis.
  const wall = "minecraft:bell[attachment=single_wall,facing=east,powered=false]";
  assert.deepEqual(usableFaces(wall), ["north", "south"]);
  assert.deepEqual(usableFaces("minecraft:bell[attachment=double_wall,facing=north]"), ["east", "west"]);
});

test("a lever or a button takes the face toward the eye", () => {
  assert.deepEqual(usableFaces("minecraft:lever[face=wall,facing=east,powered=false]").length, 4);
  assert.equal(pressFace("minecraft:stone_button[face=floor,facing=north]", 0, -1), "north");
});
