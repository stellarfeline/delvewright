// The face a block press clicks (spec-0093 §6.5): the harness right-clicks a
// lever, a button or a bell exactly as a player does, and the server judges
// the face it is handed.
//
// Vanilla's rule for a bell, read once against the pinned 1.21.11 server jar
// (`BellBlock`'s proper-hit test, obfuscated `dzm.a(eoh, iz, double)`;
// recorded in `docs/reference/compiler.md`, the `triggers[]` prop row): a hit
// on a vertical face (up or down), or above 0.8124 of the block's height, never
// rings; otherwise a `floor` bell rings on a face along its own `facing` axis,
// a `single_wall` or `double_wall` bell on a face across it, and a `ceiling`
// bell on any side. A lever and a button take any face. A non-ringing hit
// passes, so vanilla reports no use and the act is never performed.

import { parseBlockState } from "./block-state.ts";

/** A horizontal face of a block. */
export type Face = "north" | "east" | "south" | "west";

/** Each face's outward normal, `[dx, dz]`, in the order ties are broken in. */
export const FACE_NORMALS: Readonly<Record<Face, readonly [number, number]>> = {
  north: [0, -1],
  east: [1, 0],
  south: [0, 1],
  west: [-1, 0],
};

const FACES = Object.keys(FACE_NORMALS) as Face[];

const axis = (f: string): "x" | "z" | undefined =>
  f === "east" || f === "west" ? "x" : f === "north" || f === "south" ? "z" : undefined;

/** The horizontal faces a right-click on the block `state` performs its use on. */
export function usableFaces(state: string): Face[] {
  const s = parseBlockState(state);
  if (s.name !== "minecraft:bell") return [...FACES];
  const facing = axis(s.properties.facing ?? "");
  switch (s.properties.attachment) {
    case "floor":
      return FACES.filter((f) => axis(f) === facing);
    case "single_wall":
    case "double_wall":
      return FACES.filter((f) => axis(f) !== facing);
    case "ceiling":
      return [...FACES];
    default:
      return [...FACES];
  }
}

/**
 * The face to click on the block `state` from an eye `dx`, `dz` off the block's
 * centre: of the faces that perform the use, the one turned most toward the
 * eye, ties (an eye straight above or below, or square to two faces) broken in
 * north, east, south, west order. Never a zero direction.
 */
export function pressFace(state: string, dx: number, dz: number): Face {
  const faces = usableFaces(state);
  let best = faces[0] ?? "north";
  let score = -Infinity;
  for (const f of faces) {
    const [nx, nz] = FACE_NORMALS[f];
    const d = nx * dx + nz * dz;
    if (d > score + 1e-9) {
      best = f;
      score = d;
    }
  }
  return best;
}
