#!/usr/bin/env -S npx tsx
/**
 * Delvewright sandbox driver over the MineBench (MIT) harness.
 *
 * Runs a `voxel.exec` tool-call envelope through the harness's OWN runtime
 * (`runVoxelExec`), validator (`parseVoxelBuildSpec` / `validateVoxelBuild`) and
 * Sponge-schematic exporter (`buildSpongeSchematic`), then gzips the schematic
 * exactly as the browser export worker does (mtime 0).
 *
 * Usage: dw-run-build.ts <call.json> <out-basename>
 *   writes <out-basename>.schem and prints stats JSON.
 *
 * Nothing in the MineBench tree is modified; this file is additive and lives
 * only in the sandbox clone.
 */
import * as fs from "node:fs";
import * as path from "node:path";
import { gzipSync } from "node:zlib";

import { runVoxelExec, voxelExecToolCallSchema } from "../lib/ai/tools/voxelExec";
import { parseVoxelBuildSpec, validateVoxelBuild } from "../lib/voxel/validate";
import { getPalette } from "../lib/blocks/palettes";
import { maxBlocksForGrid, minBlocksForGrid, type GridSize } from "../lib/ai/limits";
import { buildSpongeSchematic } from "../lib/voxel/export/schematic";

const [, , callPath, outBase] = process.argv;
if (!callPath || !outBase) {
  console.error("usage: dw-run-build.ts <call.json> <out-basename>");
  process.exit(2);
}

const raw = JSON.parse(fs.readFileSync(callPath, "utf-8"));
const parsedCall = voxelExecToolCallSchema.safeParse(raw);
if (!parsedCall.success) {
  console.error(`invalid tool call: ${parsedCall.error.message}`);
  process.exit(2);
}
const call = parsedCall.data;

const run = runVoxelExec({
  code: call.input.code,
  gridSize: call.input.gridSize,
  palette: call.input.palette,
  seed: call.input.seed,
  outputDir: path.dirname(path.resolve(outBase)),
});

const spec = parseVoxelBuildSpec(run.build);
if (!spec.ok) {
  console.error(`spec invalid: ${spec.error}`);
  process.exit(2);
}

const paletteDefs = getPalette(call.input.palette);
const validated = validateVoxelBuild(spec.value, {
  gridSize: call.input.gridSize,
  palette: paletteDefs,
  maxBlocks: maxBlocksForGrid(call.input.gridSize as GridSize),
});
if (!validated.ok) {
  console.error(`validation failed: ${validated.error}`);
  process.exit(2);
}

const schem = buildSpongeSchematic(validated.value.build, paletteDefs);
const gz = gzipSync(Buffer.from(schem.bytes), { level: 9 });
fs.writeFileSync(`${outBase}.schem`, gz);
fs.writeFileSync(
  `${outBase}.expanded.json`,
  JSON.stringify(validated.value.build),
);

const minBlocks = minBlocksForGrid(call.input.gridSize as GridSize);
const report = {
  out: `${outBase}.schem`,
  seed: call.input.seed,
  gridSize: call.input.gridSize,
  palette: call.input.palette,
  primitives: { blocks: run.blockCount, boxes: run.boxCount, lines: run.lineCount },
  expandedBlocks: validated.value.build.blocks.length,
  minBlocksRequired: minBlocks,
  meetsMinimum: validated.value.build.blocks.length >= minBlocks,
  bbox: {
    width: schem.stats.width,
    height: schem.stats.height,
    length: schem.stats.length,
  },
  paletteSize: schem.stats.paletteSize,
  warnings: validated.value.warnings.slice(0, 8),
  warningCount: validated.value.warnings.length,
};
console.log(JSON.stringify(report, null, 2));
