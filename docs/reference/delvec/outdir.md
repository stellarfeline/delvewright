# `delvec::outdir`

The reference page for `crates/delvec/src/outdir.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0967 — an output directory holds exactly the tree last written into it (`delvec::outdir`; error; exit 3)

| Code | Meaning |
|------|---------|
| `DW0967` | **The output directory holds something no `delvec` tree wrote, or is not a directory.** Build-tier (exit 3), `delvec::outdir::replace`, raised by `delvec build -o` and `delvec prefab gallery --out` after emission and before anything in the directory moves. **Ownership** is the tree's own `manifest.json`: an object stating `delvec_version` whose `outputs` names every other file by a plain relative path. A write into an absent or empty directory, or into one whose manifest parses as such, removes every file the old manifest names and the new tree does not, prunes emptied directories, writes the new `manifest.json`, then the rest, so the directory holds exactly the new tree and stays owned at every instant of the write. Three root entries are **derived artifacts** of the build in the directory and are removed by the next write (`delvec::outdir::DERIVED`): `world/` (`validation/world-save.sh`), `shots/` (`validation/render-shots.sh`'s default) and `staging-admission.json` (`tools/creator/staging-gate.py`). **Refused, with nothing written or removed:** a path that exists and is not a directory; a non-empty directory with no manifest, a manifest that is not one (no `delvec_version`, no `outputs` object, an `outputs` key that is absolute, carries `.`/`..` or names the manifest itself); a file the manifest does not name; a symlink or a non-UTF-8 name anywhere under the root, named or not; a derived name of the wrong kind (a file called `world`). The message names the directory, the count of foreign entries and the first ten. A write that removed anything prints one line on stderr: files written, files of the previous tree removed, and each derived artifact removed with its file count and writer. Prescription: move the named files out, or delete the directory, and build again; never point `-o` at a directory holding anything else. |
