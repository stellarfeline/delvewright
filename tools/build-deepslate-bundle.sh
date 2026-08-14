#!/usr/bin/env bash
# Rebuild the vendored deepslate bundle the prefab review page embeds.
#
# The page is one self-contained file with no external references of any kind, so
# the renderer's whole browser side ships inside it. `crates/render/src/viewer/
# deepslate.bundle.js` is that byte block; this script is how it is produced, and
# running it is the only sanctioned way to change it.
#
#     tools/build-deepslate-bundle.sh
#
# Needs `npm` and network access. Everything is installed into a scratch
# directory, never into the repo. Rerunning on the same pins yields the same
# bytes (ADR-0006 applies to the page, and the bundle is part of the page).
#
# ## The local patch, and why it is not a fork
#
# deepslate 0.26.0 asks for two block-entity base textures at paths no Minecraft
# version has ever shipped: `entity/banner/banner_base` and
# `entity/shield/shield_base_nopattern`. 1.21.11 has `entity/banner_base.png` and
# `entity/shield_base_nopattern.png` at the top level, and `entity/banner/` holds
# only the 43 pattern textures. Unpatched, every banner and every shield renders
# as the missing-texture checker. The two ids are rewritten below with an exact
# expected hit count, so an upstream release that moves them again fails this
# script instead of silently shipping magenta.
#
# We look after our own build and do not undertake to maintain upstream's: the
# defect is reported there, and the patch is dropped the moment a release carries
# paths the jar supplies.
set -euo pipefail

DEEPSLATE_VERSION="0.26.0"
GL_MATRIX_VERSION="3.4.4"
ESBUILD_VERSION="0.28.2"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="$repo_root/crates/render/src/viewer/deepslate.bundle.js"
work="$(mktemp -d "${TMPDIR:-/tmp}/delve-deepslate.XXXXXX")"
trap 'rm -rf "$work"' EXIT

cat >"$work/package.json" <<JSON
{
  "name": "delvewright-viewer-bundle",
  "private": true,
  "type": "module",
  "dependencies": {
    "deepslate": "$DEEPSLATE_VERSION",
    "gl-matrix": "$GL_MATRIX_VERSION"
  },
  "devDependencies": {
    "esbuild": "$ESBUILD_VERSION"
  }
}
JSON

# The page's whole import surface, in one place. `gl-matrix` rides along because
# deepslate's view matrices are `mat4`s and the page builds its own camera.
cat >"$work/entry.js" <<'JS'
export * from 'deepslate'
export { mat4, vec3 } from 'gl-matrix'
JS

echo "installing deepslate@$DEEPSLATE_VERSION …" >&2
(cd "$work" && npm install --silent --no-audit --no-fund >/dev/null)

echo "bundling …" >&2
(cd "$work" && npx --no-install esbuild entry.js \
	--bundle --minify --format=iife --global-name=deepslate \
	--target=es2020 --legal-comments=inline \
	--outfile=bundle.raw.js >/dev/null)

python3 - "$work/bundle.raw.js" "$out" <<'PY'
import sys, hashlib, pathlib

src, dst = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
text = src.read_text(encoding="utf-8")

# (wrong id, right id, exactly how many occurrences to expect)
PATCHES = [
    ("entity/banner/banner_base", "entity/banner_base", 1),
    ("entity/shield/shield_base_nopattern", "entity/shield_base_nopattern", 1),
]

for wrong, right, expect in PATCHES:
    got = text.count(wrong)
    if got != expect:
        sys.exit(
            f"error: expected {expect} occurrence(s) of {wrong!r} in the bundle, found {got}.\n"
            "Either upstream moved the texture id again, or it has taken the base textures back "
            "to the paths the jar supplies — in which case drop this patch rather than widen it."
        )
    text = text.replace(wrong, right)

dst.write_text(text, encoding="utf-8")
print(f"{dst}: {len(text.encode())} bytes, sha256 {hashlib.sha256(text.encode()).hexdigest()}")
PY

echo "licences of everything in the bundle:" >&2
(cd "$work" && node -e '
const lock = JSON.parse(require("fs").readFileSync("package-lock.json", "utf8"));
const rows = Object.entries(lock.packages)
  .filter(([k, v]) => k.startsWith("node_modules/") && !v.dev && !v.optional)
  .map(([k, v]) => `  ${k.replace("node_modules/", "")}@${v.version} — ${v.license}`)
  .sort();
console.error(rows.join("\n"));
')
