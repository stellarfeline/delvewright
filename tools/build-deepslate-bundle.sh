#!/usr/bin/env bash
# Rebuild the vendored deepslate bundle the prefab review page embeds.
#
# The page is one self-contained file with no external references of any kind, so
# the renderer's whole browser side ships inside it. `crates/compiler/src/view/viewer/
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
out="$repo_root/crates/compiler/src/view/viewer/deepslate.bundle.js"
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

# Patch, write, digest and list — one python step rather than one per job, so the
# lockfile that produced the bytes and the bytes themselves are read by the same
# program. `newline` is pinned because a carriage return from a text-mode stdout
# survives command substitution and makes a digest compare unequal to itself on
# exactly one runner.
python3 - "$work/bundle.raw.js" "$out" "$work/package-lock.json" <<'PY'
import hashlib
import json
import pathlib
import sys

sys.stdout.reconfigure(newline="\n")
sys.stderr.reconfigure(newline="\n")

src, dst, lockfile = (pathlib.Path(a) for a in sys.argv[1:4])
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

data = text.encode("utf-8")
dst.write_bytes(data)
print(f"{dst}: {len(data)} bytes, sha256 {hashlib.sha256(data).hexdigest()}")
print("pin this digest in versions.toml [render].deepslate_bundle_sha256")

# Everything that ends up in those bytes, with its licence, read from the
# lockfile that produced them. A new transitive dependency cannot arrive
# unnamed: the ACKNOWLEDGEMENTS entry is written from this list.
lock = json.loads(lockfile.read_text(encoding="utf-8"))
rows = sorted(
    "  {}@{} — {}".format(
        name.removeprefix("node_modules/"), meta.get("version"), meta.get("license")
    )
    for name, meta in lock.get("packages", {}).items()
    if name.startswith("node_modules/") and not meta.get("dev") and not meta.get("optional")
)
print("licences of everything in the bundle:", file=sys.stderr)
print("\n".join(rows), file=sys.stderr)
PY
