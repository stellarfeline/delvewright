# `delvec::compiler::registry`

The reference page for `crates/delvec/src/compiler/registry.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW03xx — build / solver / nav (`compiler`; error; exit 3, `stage:"build"`)

This module's rows of a section whose prose is on the [`delvec::compiler::nav` page](nav.md#dw03xx--build--solver--nav-compiler-error-exit-3-stagebuild).

| Code | Meaning |
|------|---------|
| `DW0346` | A prefab metadata `*.json` (or `pools.json`) in the prefabs dir failed to read or parse. `PrefabRegistry::load_dir` records a per-file diagnostic naming the file and the serde error, folded into every `validate`/`analyze`/`build` at **validation tier (exit 1)**; loading continues for the other files (report-all, not fail-fast). Without it the prefab simply vanished from the registry and the run failed much later as a baffling `DW0300` "prefab not found" (or a `DW0160` binding error) with no hint of why. What reaches this code is a document that is **malformed for this delvec**: a value of the wrong type, an absent required block, unreadable bytes. A key this delvec does not model is deliberately not one of them — see `DW0543`. Prescription: fix the named field. **A gate report is not one of these either**: a file named `*.report.json` — what `delvec grammar expand` and `delvec detail` write beside the piece they froze — is skipped by name, as `pools.json` is read by name, so an expander's output directory is a prefab directory; the suffix is the full `.report.json`, and a metadata file with `report` in its stem is still read. **A tile-set manifest is not one of these**: metadata carrying `structure_set` instead of `structure` is an ordinary prefab document whose blocks arrive as several `.nbt` tiles, and it loads, is indexed under its own prefab id, and is placed (see §4 “A piece's blocks arrive as one template or as a tile set”). What is still `DW0346` about a manifest is the same thing that is `DW0346` about any document: it is malformed — tiles that do not cover the zone exactly (a reassembly with a hole or an overlap), a tile past the declared `part_max`, an empty parts list. A document declaring **neither** structure block, or **both**, is likewise `DW0346`, and the message names both keys: “which shape is this” has two answers and no third, and a bare serde `missing field \u0060structure\u0060` is a true statement about the bytes and a useless one about the situation. |

### DW0543 — a prefab metadata key this delvec does not model (`compiler::registry`)

The prefab metadata document (`docs/reference/prefab-procedure.md` §9) has one
definition, `delvewright_dsl::prefab`, and every reader — `delvec`,
`delvec prefab`, `delvec grammar`, `delvec render` — uses that type rather than a
local copy of the shape. `delvec` reaches it through the DSL crate because it is
published to crates.io and may only depend on published crates.

That type does not carry `deny_unknown_fields`, and the decision is the point.
The attribute is right on a document whose reader is also its owner — every
campaign stage struct keeps it, because a typo there is the bug it catches and
`dsl_version` handles forward compatibility. It is wrong on a **consumer**: a
content library and an engine version move independently, so a key newer than
the reader is the normal state of a mixed-version pair, and refusing it turns a
forward addition into a hard failure at the layer with the least context. One
key would have stopped **every** campaign building.

Ignoring the key is the other wrong answer, because a misspelled key looks
exactly the same from here. So the piece loads, the key is preserved on any
rewrite, and the reader says what it saw.

| Code | Meaning |
|------|---------|
| `DW0543` | **A prefab metadata file carries a key this delvec does not model.** Warning, `compiler::registry::PrefabRegistry::load_dir`, reported per file at every `validate`/`analyze`/`build`. The message names every unknown key, at the document root and per anchor, and states the two things it can be: a library newer than this engine (upgrade `delvec` to consume the key), or a misspelling of a key the document does define, in which case whatever it was meant to say is not being said. The prefab **loads** — it is not skipped, and this is not `DW0346`. Binding: the pinned content library must produce zero of these, which is what makes it a tripwire rather than noise (`tests/registry_load.rs`). |
