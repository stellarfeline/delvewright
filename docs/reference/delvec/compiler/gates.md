# `delvec::compiler::gates`

The reference page for `crates/delvec/src/compiler/gates.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW01xx — validation (`dsl`; severity error; exit 1)

This module's rows of a section whose prose is on the [`delvewright_dsl::diagnostic` page](../../dsl/diagnostic.md#dw01xx--validation-dsl-severity-error-exit-1).

| Code | Meaning |
|------|---------|
| `DW0343` | A verb that needs a gate anchor's **fill block** targets an anchor that declares none (or is not a gate region at all): `close-gate`, which fills the region back in, or a stage-5 `shortcut` (spec-0016 §2), whose unlock clears exactly that block and whose gate is sealed by it from world-load. Compiler-side (needs prefab metadata the DSL anchor registry does not carry), reported at **validation tier (exit 1)** like the atmos `DW032x` checks. **The scan is over the pieces this campaign's areas can place** — each area's bare `prefab`, or every member of its `prefab_pool` — and **all** of them that declare the anchor as a gate must declare a fillable one. A piece the campaign binds no area to has no standing to answer: it cannot be placed, so what it says about a gate is a fact about a different building. What the anchor declares is asked of one authority, `PrefabMeta::gate_anchor`, so an anchor carrying an explicit `region` + `block` and one carrying a `resolves_to` of `bar:<region>` resolve identically; where that authority refuses (the two forms disagree, the named bar is not in the piece's contract, or the bar's boxes do not fill their own bounding box) its reason is quoted into this diagnostic. Prescription: the placement authority's (see the remediation contract above) — on a prefab campaign, declare the gate on an anchor of a piece an area binds, as a `region` plus a `block` or as a `bar:` the piece's spatial contract carries; on a site-plan campaign, name an `anchor/seam-<edge>` the derivation cuts over a barred connection the layout graph declares. Either way, or remove the verb. A gate whose block the derivation supplies never reaches this check at all — `siteplan::synthesized_gate_block` answers first, and a site-plan campaign with no `layout-graph.json` reaches it no further, because a derivation with no graph to read names no gate to ask about (`dsl::placement::anchor_vocabulary_unknowable`; `DW0824` is the finding). |

### DW03xx — build / solver / nav (`compiler`; error; exit 3, `stage:"build"`)

This module's rows of a section whose prose is on the [`delvec::compiler::nav` page](nav.md#dw03xx--build--solver--nav-compiler-error-exit-3-stagebuild).

| Code | Meaning |
|------|---------|
| `DW0423` | Two `close-gate` effects seal the **same** gate anchor with different `sealed_hint` wordings. A seal's answer belongs to the PLACE: one anchor carries one set of `dw_seal_<anchor>` hitboxes and one reward function, so a second wording has nowhere to live and would be silently dropped — a line an author wrote and a player can never read, which is the same silence class the verb exists to close. A firing that authors no hint is compatible with anything; only two *authored, different* lines conflict. `compiler::gates::check_seal_hints`, validation tier (exit 1). Prescription: give both firings the same line, or seal two different gate anchors. **This polices the SUGAR, not a mechanism**: `sealed_hint` is the wording of the press answer synthesized for that anchor, and one anchor still gets one answer. It has no shortcut analogue and needs none — a `shortcut` carries no wording field, so there is nothing for two declarations to disagree about. |
