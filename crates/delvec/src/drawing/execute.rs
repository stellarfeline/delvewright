//! **The executor** (spec-0072 §5): a drawing plus a box plus a seed gives an
//! [`Expansion`] — the same thing a grammar program derives to, so everything
//! downstream is what already exists.
//!
//! # Order, and what determinism rests on
//!
//! Operations run in document order; a `repeat`'s instances in ascending index;
//! a `mirror`'s unreflected body first; a `use`'s body in order. The canvas
//! holds, per cell, the role last painted and the frame-resolved paint it names.
//! After the last operation, in this order:
//!
//! 1. **weighted paints are drawn, by position** — the draw at a cell is a pure
//!    function of the execution's seed and the cell's linear index in the
//!    place's box, so no stream is consumed and editing one operation
//!    re-textures no other cell;
//! 2. **derived state is written** — every stair's `shape`, from the engine's
//!    own measured derivation;
//! 3. the result is an [`Expansion`].
//!
//! **Geometry has no seed.** The seed reaches weighted paints and nothing else.
//! No clock, no environment, no hash-ordered container: two executions are
//! byte-identical (ADR-0006).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use delvewright_dsl::codes;

use crate::drawing::diag::{
    Address, DW_DEFINE_REACHES_ITSELF, DW_DERIVED_PROPERTY, DW_DOES_NOT_FIT, DW_LEFT_ITS_SCOPE,
    DW_OUT_OF_RANGE, DW_UNKNOWN_NAME, Refusal,
};
use crate::drawing::ir::{
    AIR, Define, Drawing, Face, GRAMMAR, Horizontal, Int, Op, Remainder, Vec3,
};
use crate::drawing::raster::{
    BoxRule, MAX_ROUND_AXIS, RoundRule, Rule, Step, StepRule, line_points,
};
use crate::grammar::block::BlockState;
use crate::grammar::eval::{Env, Scope as EvalScope};
use crate::grammar::expand::{
    Anchor, ExpandOptions, Expansion, Limits, OrientedFillAudit, Overrides, ResolvedRegion, Stats,
    resolve_declared_contract,
};
use crate::grammar::geom::{Axis, Box3, Mirror, Orientation};
use crate::grammar::ir::{Cond, Expr, Paint, States, WeightedBlock};
use crate::grammar::model::VoxelModel;
use crate::grammar::rng::Rng;
use crate::grammar::settle;

/// The largest magnitude a coordinate or an extent may evaluate to.
///
/// The expression algebra saturates at `i64` rather than wrapping, and a
/// saturated value is not a number an author wrote — it is arithmetic that ran
/// off the end. A cell of a Minecraft world fits in an `i32`, so a value past
/// this bound is refused at the operation that evaluated it (`DW0905`), naming
/// the field and the parameters that produced it, instead of being clamped into
/// a box and quietly painting the wrong place.
pub const COORD_LIMIT: i64 = i32::MAX as i64;

/// The palette an integer expression is evaluated against: none. A drawing's
/// expressions are integers over parameters and extents; roles are resolved by
/// the executor, not by the evaluator.
static NO_PALETTE: LazyLock<BTreeMap<String, Paint>> = LazyLock::new(BTreeMap::new);

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// Read a drawing from disk: the schema, then the one `dsl_version`.
///
/// Both refusals are the campaign format's own (`DW0100`, `DW0102`, ADR-0024),
/// because a drawing is a campaign document and an author who has met them in a
/// stage document has met them here.
pub fn load(path: &Path) -> Result<Drawing, Refusal> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        Refusal::at_field(
            codes::SCHEMA,
            "/",
            format!("cannot read {}: {e}", path.display()),
        )
    })?;
    let drawing: Drawing = serde_json::from_str(&text).map_err(|e| {
        Refusal::at_field(
            codes::SCHEMA,
            "/",
            format!("{} is not a drawing: {e}", path.display()),
        )
    })?;
    let engine = crate::compiler::DSL_VERSION;
    if drawing.dsl_version != engine {
        return Err(Refusal::at_field(
            codes::DSL_VERSION,
            "/dsl_version",
            format!(
                "{} declares `dsl_version` {:?}; this engine reads {engine:?}. There is one \
                 campaign format number and every document of a campaign declares it (ADR-0024).",
                path.display(),
                drawing.dsl_version
            ),
        ));
    }
    Ok(drawing)
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// What `delvec drawing check` counts, so a green verdict states what it was
/// over.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Census {
    /// Operations written, over `ops` and every define body.
    pub ops: usize,
    /// Operations written at the top level.
    pub top_level_ops: usize,
    /// Defines declared.
    pub defines: usize,
    /// Defines no `use` names — dead text, reported rather than refused.
    pub unused_defines: Vec<String>,
    /// Palette roles bound.
    pub roles: usize,
    /// Parameters declared.
    pub params: usize,
    /// Contract regions the operations claim.
    pub claimed_regions: usize,
    /// Marks written.
    pub marks: usize,
}

/// **Validate a drawing without executing it**: every name, every define chain,
/// every paint that would write a property the engine derives, and the
/// contract's reference integrity.
///
/// Everything here is decidable from the document alone — no box, no seed, no
/// cell. What is left to the executor is what depends on a scope.
pub fn check(drawing: &Drawing) -> Result<Census, Refusal> {
    let mut census = Census {
        defines: drawing.defines.len(),
        roles: drawing.palette.len(),
        params: drawing.params.len(),
        top_level_ops: drawing.ops.len(),
        ..Census::default()
    };

    // `DW0907`, before anything reads a role: a paint that writes a property the
    // engine derives is a claim about neighbours the author has not got, and the
    // repair is to omit it.
    let registry = crate::schem::blocks::BlockRegistry::v1_21_11();
    for (role, states) in &drawing.palette {
        for state in states.each() {
            if registry.is_stairs(&state.name) && state.properties.contains_key("shape") {
                return Err(Refusal::at_field(
                    DW_DERIVED_PROPERTY,
                    &format!("/palette/{role}"),
                    format!(
                        "the role {role:?} paints {state}, which writes `shape`. A stair's shape \
                         is the engine's: it is derived from the stair's four neighbours after \
                         the last operation, by the rule the pinned server was measured against \
                         (`schem::stairs::derive_shape`). Omit `shape` and the executor writes it."
                    ),
                ));
            }
        }
    }

    // `DW0904`, before any name is resolved: a define that reaches itself has a
    // body nothing can finish, and the chain is what an author repairs.
    check_define_cycles(drawing)?;

    let mut claimed: BTreeMap<String, String> = BTreeMap::new();
    let mut used_defines: BTreeSet<String> = BTreeSet::new();

    let top = Names {
        params: drawing.params.keys().cloned().collect(),
        roles: drawing.palette.keys().cloned().collect(),
    };
    walk_ops(
        drawing,
        &drawing.ops,
        "/ops",
        &top,
        &mut census,
        &mut claimed,
        &mut used_defines,
    )?;
    for (name, define) in &drawing.defines {
        let names = Names {
            params: define
                .params
                .keys()
                .chain(drawing.params.keys())
                .cloned()
                .collect(),
            roles: define.roles.iter().cloned().collect(),
        };
        walk_ops(
            drawing,
            &define.body,
            &format!("/defines/{name}/body"),
            &names,
            &mut census,
            &mut claimed,
            &mut used_defines,
        )?;
    }

    census.claimed_regions = claimed.len();
    census.unused_defines = drawing
        .defines
        .keys()
        .filter(|d| !used_defines.contains(*d))
        .cloned()
        .collect();

    // The contract's reference integrity, through the one checker a program is
    // held to: a name that is two things is two things in both documents.
    if let Some(contract) = &drawing.contract {
        crate::grammar::ir::check_contract_references(contract, &claimed, &|role| {
            drawing.palette.get(role)
        })
        .map_err(|e| {
            Refusal::at_field(
                DW_UNKNOWN_NAME,
                "/contract",
                format!(
                    "{e} (the drawing declares {} role(s))",
                    drawing.palette.len()
                ),
            )
        })?;
    } else if let Some((region, site)) = claimed.iter().next() {
        return Err(Refusal::at_field(
            DW_UNKNOWN_NAME,
            "/contract",
            format!(
                "the operation at {site} claims the region {region:?} and the drawing declares no \
                 `contract`, so nothing says what that name is. A claim the contract does not \
                 classify resolves boxes that belong to nothing."
            ),
        ));
    }
    Ok(census)
}

/// The names a body may read, at the site it is written.
struct Names {
    params: BTreeSet<String>,
    roles: BTreeSet<String>,
}

impl Names {
    /// The same names with a `repeat` index bound over them.
    fn with_index(&self, index: &str) -> Names {
        let mut params = self.params.clone();
        params.insert(index.to_string());
        Names {
            params,
            roles: self.roles.clone(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn walk_ops(
    drawing: &Drawing,
    ops: &[Op],
    base: &str,
    names: &Names,
    census: &mut Census,
    claimed: &mut BTreeMap<String, String>,
    used: &mut BTreeSet<String>,
) -> Result<(), Refusal> {
    for (i, op) in ops.iter().enumerate() {
        let at = Address::at(base, i);
        census.ops += 1;
        if let Some(when) = op.when() {
            check_guard(when, &at)?;
            check_exprs_of_cond(when, names, &at)?;
        }
        let (from, to) = op.extent();
        for (field, v) in [("from", from), ("to", to)] {
            if let Some(v) = v {
                check_vec(v, names, &at, field)?;
            }
        }
        if let Some(solid) = op.solid() {
            if solid.role != AIR && !names.roles.contains(solid.role) {
                return Err(unknown_role(&at, "role", solid.role, names));
            }
            for role in solid.where_roles {
                if role != AIR && role != GRAMMAR && !names.roles.contains(role.as_str()) {
                    return Err(unknown_role(&at, "`where` role", role, names));
                }
            }
        }
        match op {
            Op::Box(o) => {
                if let Some(faces) = &o.faces
                    && faces.is_empty()
                {
                    return Err(Refusal::new(
                        codes::SCHEMA,
                        at.clone(),
                        "`faces` is written and empty, so the box names no shell and no solid. \
                         Omit it for the solid box, or list the faces the shell stands at.",
                    ));
                }
                check_opt(o.t.as_ref(), names, &at, "t")?;
            }
            Op::Cylinder(o) => check_opt(o.t.as_ref(), names, &at, "t")?,
            Op::Sphere(o) => check_opt(o.t.as_ref(), names, &at, "t")?,
            Op::Prism(o) => {
                check_opt(o.rise.as_ref(), names, &at, "rise")?;
                check_opt(o.run.as_ref(), names, &at, "run")?;
            }
            Op::Pyramid(o) => {
                check_opt(o.rise.as_ref(), names, &at, "rise")?;
                check_opt(o.run.as_ref(), names, &at, "run")?;
            }
            Op::Line(o) => {
                if let Some(brush) = &o.brush {
                    check_vec(brush, names, &at, "brush")?;
                }
            }
            Op::Use(o) => {
                used.insert(o.define.clone());
                let Some(define) = drawing.defines.get(&o.define) else {
                    return Err(Refusal::new(
                        DW_UNKNOWN_NAME,
                        at.clone(),
                        format!(
                            "no define named {:?}; the drawing declares {}: {}",
                            o.define,
                            drawing.defines.len(),
                            listed(drawing.defines.keys())
                        ),
                    ));
                };
                if o.turn > 3 {
                    return Err(Refusal::new(
                        codes::SCHEMA,
                        at.clone(),
                        format!(
                            "`turn` is {}, and a turn is a quarter-turn count: 0, 1, 2 or 3.",
                            o.turn
                        ),
                    ));
                }
                for (name, value) in &o.params {
                    if !define.params.contains_key(name) {
                        return Err(Refusal::new(
                            DW_UNKNOWN_NAME,
                            at.clone(),
                            format!(
                                "the define {:?} declares no parameter {name:?}; it declares {}: \
                                 {}. A `use` is closed, so a misspelt argument is refused here \
                                 rather than expanding the default.",
                                o.define,
                                define.params.len(),
                                listed(define.params.keys())
                            ),
                        ));
                    }
                    check_int(value, names, &at, name)?;
                }
                let declared: BTreeSet<&String> = define.roles.iter().collect();
                for name in o.roles.keys() {
                    if !declared.contains(name) {
                        return Err(Refusal::new(
                            DW_UNKNOWN_NAME,
                            at.clone(),
                            format!(
                                "the define {:?} lists no role {name:?}; it lists {}: {}",
                                o.define,
                                define.roles.len(),
                                listed(define.roles.iter())
                            ),
                        ));
                    }
                }
                for name in &define.roles {
                    if !o.roles.contains_key(name) {
                        return Err(Refusal::new(
                            DW_UNKNOWN_NAME,
                            at.clone(),
                            format!(
                                "the define {:?} paints the role {name:?} and this `use` binds it \
                                 to nothing. A role has no default the way a parameter does, so \
                                 every role a define lists is bound at every `use`; this one \
                                 binds {}.",
                                o.define,
                                listed(o.roles.keys())
                            ),
                        ));
                    }
                }
                for (local, outer) in &o.roles {
                    if outer != AIR && !names.roles.contains(outer.as_str()) {
                        return Err(unknown_role(
                            &at,
                            &format!("the role {local:?} is bound to"),
                            outer,
                            names,
                        ));
                    }
                }
            }
            Op::Repeat(o) => {
                let spelling = repeat_spelling(o, &at)?;
                let inner = match &o.index {
                    Some(index) => names.with_index(index),
                    None => Names {
                        params: names.params.clone(),
                        roles: names.roles.clone(),
                    },
                };
                match &spelling {
                    Spelling::Counted { step, count } => {
                        check_vec(step, names, &at, "step")?;
                        check_int(count, names, &at, "count")?;
                    }
                    Spelling::Fitted { stride, item, .. } => {
                        check_int(stride, names, &at, "stride")?;
                        check_int(item, names, &at, "item")?;
                    }
                }
                walk_ops(
                    drawing,
                    &o.body,
                    &format!("{}/body", at.pointer),
                    &inner,
                    census,
                    claimed,
                    used,
                )?;
            }
            Op::Mark(o) => {
                census.marks += 1;
                if !crate::grammar::ir::is_kebab(&o.mark.anchor) {
                    return Err(Refusal::new(
                        DW_UNKNOWN_NAME,
                        at.clone(),
                        format!(
                            "the anchor stem {:?} is not kebab-case; an exported anchor is named \
                             `anchor/<kebab>` because that is the id the DSL resolves.",
                            o.mark.anchor
                        ),
                    ));
                }
                if let crate::grammar::ir::MarkAt::Offset { x, y, z } = &o.mark.at {
                    for expr in [x, y, z] {
                        check_expr(expr, names, &at, "the mark's offset")?;
                    }
                }
            }
            Op::Claim(o) => {
                claimed
                    .entry(o.region.clone())
                    .or_insert_with(|| at.pointer.clone());
                walk_ops(
                    drawing,
                    &o.body,
                    &format!("{}/body", at.pointer),
                    names,
                    census,
                    claimed,
                    used,
                )?;
            }
            Op::Scope(_) | Op::Mirror(_) => {
                walk_ops(
                    drawing,
                    op.body(),
                    &format!("{}/body", at.pointer),
                    names,
                    census,
                    claimed,
                    used,
                )?;
            }
            Op::Grammar(_) => {}
        }
    }
    Ok(())
}

fn listed<'a>(names: impl Iterator<Item = &'a String>) -> String {
    let v: Vec<String> = names.map(|n| format!("{n:?}")).collect();
    if v.is_empty() {
        "(none)".to_string()
    } else {
        v.join(", ")
    }
}

fn unknown_role(at: &Address, kind: &str, name: &str, names: &Names) -> Refusal {
    Refusal::new(
        DW_UNKNOWN_NAME,
        at.clone(),
        format!(
            "the {kind} {name:?} is bound by nothing here; the {} role(s) this body may paint are \
             {}, and `air` is the reserved role that clears (`grammar` names the cells a `grammar` \
             operation wrote, in `where` only).",
            names.roles.len(),
            listed(names.roles.iter())
        ),
    )
}

/// A `when` guard is the `cmp` / `all` / `any` / `none_of` members of [`Cond`].
///
/// `otherwise` means "no sibling alternative matched", and a drawing has no
/// alternatives; `orientation` asks which frame the scope stands in, and a
/// drawing's frames are the author's own `turn` and `mirror` rather than a fact
/// about the region. Both are refused where they are written rather than left to
/// evaluate to a constant.
fn check_guard(cond: &Cond, at: &Address) -> Result<(), Refusal> {
    let bad = |what: &str, why: &str| {
        Err(Refusal::new(
            codes::SCHEMA,
            at.clone(),
            format!("`when` writes `{what}`, {why}"),
        ))
    };
    match cond {
        Cond::Otherwise => bad(
            "otherwise",
            "which means \"no sibling alternative matched\". A drawing's operations are an ordered \
             list, not a set of alternatives, so there is no sibling for it to be about. Write the \
             comparison the guard means.",
        ),
        Cond::Orientation { .. } => bad(
            "orientation",
            "which asks which frame the scope stands in. A drawing's frames are the author's own \
             `turn` and `mirror`, so the answer is already written at the `use`; a guard on it \
             would test the document against itself.",
        ),
        Cond::All { of } | Cond::Any { of } | Cond::NoneOf { of } => {
            for c in of {
                check_guard(c, at)?;
            }
            Ok(())
        }
        Cond::Always | Cond::Cmp { .. } => Ok(()),
    }
}

fn check_exprs_of_cond(cond: &Cond, names: &Names, at: &Address) -> Result<(), Refusal> {
    match cond {
        Cond::Cmp { lhs, rhs, .. } => {
            check_expr(lhs, names, at, "`when`")?;
            check_expr(rhs, names, at, "`when`")
        }
        Cond::All { of } | Cond::Any { of } | Cond::NoneOf { of } => {
            for c in of {
                check_exprs_of_cond(c, names, at)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn check_vec(v: &Vec3, names: &Names, at: &Address, field: &str) -> Result<(), Refusal> {
    for i in v {
        check_int(i, names, at, field)?;
    }
    Ok(())
}

fn check_opt(v: Option<&Int>, names: &Names, at: &Address, field: &str) -> Result<(), Refusal> {
    match v {
        Some(i) => check_int(i, names, at, field),
        None => Ok(()),
    }
}

fn check_int(v: &Int, names: &Names, at: &Address, field: &str) -> Result<(), Refusal> {
    match v {
        Int::Literal(_) => Ok(()),
        Int::Expr(e) => check_expr(e, names, at, field),
    }
}

fn check_expr(e: &Expr, names: &Names, at: &Address, field: &str) -> Result<(), Refusal> {
    match e {
        Expr::Param { name } => {
            if names.params.contains(name) {
                Ok(())
            } else {
                Err(Refusal::new(
                    DW_UNKNOWN_NAME,
                    at.clone(),
                    format!(
                        "{field} reads the parameter {name:?}, which is declared by nothing here; \
                         the {} name(s) this body reads are {}. A body reads its own parameters, \
                         the `repeat` indices written around it, and the document's `params`.",
                        names.params.len(),
                        listed(names.params.iter())
                    ),
                ))
            }
        }
        Expr::Arith { lhs, rhs, .. } => {
            check_expr(lhs, names, at, field)?;
            check_expr(rhs, names, at, field)
        }
        Expr::Int { .. } | Expr::Dim { .. } => Ok(()),
    }
}

/// `DW0904`: a define that reaches itself, with the chain named.
fn check_define_cycles(drawing: &Drawing) -> Result<(), Refusal> {
    fn uses(ops: &[Op], into: &mut Vec<String>) {
        for op in ops {
            if let Op::Use(u) = op {
                into.push(u.define.clone());
            }
            uses(op.body(), into);
        }
    }
    let edges: BTreeMap<&String, Vec<String>> = drawing
        .defines
        .iter()
        .map(|(name, define)| {
            let mut out = Vec::new();
            uses(&define.body, &mut out);
            (name, out)
        })
        .collect();

    // Depth-first, carrying the path, so the refusal is the chain rather than
    // the fact that there is one.
    fn walk(
        node: &str,
        edges: &BTreeMap<&String, Vec<String>>,
        path: &mut Vec<String>,
        done: &mut BTreeSet<String>,
    ) -> Option<Vec<String>> {
        if let Some(start) = path.iter().position(|p| p == node) {
            let mut chain = path[start..].to_vec();
            chain.push(node.to_string());
            return Some(chain);
        }
        if done.contains(node) {
            return None;
        }
        path.push(node.to_string());
        if let Some(next) = edges.get(&node.to_string()) {
            for target in next {
                if let Some(chain) = walk(target, edges, path, done) {
                    return Some(chain);
                }
            }
        }
        path.pop();
        done.insert(node.to_string());
        None
    }

    let mut done = BTreeSet::new();
    for name in drawing.defines.keys() {
        let mut path = Vec::new();
        if let Some(chain) = walk(name, &edges, &mut path, &mut done) {
            return Err(Refusal::at_field(
                DW_DEFINE_REACHES_ITSELF,
                &format!("/defines/{}", chain[0]),
                format!(
                    "the define {:?} reaches itself: {}. A drawing has no recursion, so there is \
                     no depth to limit and nothing to finish the body with; break the chain.",
                    chain[0],
                    chain
                        .iter()
                        .map(|c| format!("{c:?}"))
                        .collect::<Vec<_>>()
                        .join(" \u{2192} ")
                ),
            ));
        }
    }
    Ok(())
}

/// Which of the two spellings a `repeat` is written in.
enum Spelling<'o> {
    Counted {
        step: &'o Vec3,
        count: &'o Int,
    },
    Fitted {
        along: Axis,
        stride: &'o Int,
        item: &'o Int,
        remainder: Remainder,
    },
}

/// Resolve a `repeat`'s spelling, or say exactly which half is missing.
///
/// Both spellings are fields of one operation, so serde cannot decide between
/// them: an untagged pair would answer "data did not match any variant", which
/// names neither the field nor the repair. The refusal is `DW0100` all the same
/// — it is the document's shape, decided at load, before anything is evaluated.
fn repeat_spelling<'o>(
    op: &'o crate::drawing::ir::RepeatOp,
    at: &Address,
) -> Result<Spelling<'o>, Refusal> {
    let counted = op.step.is_some() || op.count.is_some();
    let fitted =
        op.along.is_some() || op.stride.is_some() || op.item.is_some() || op.remainder.is_some();
    let bad = |msg: String| Refusal::new(codes::SCHEMA, at.clone(), msg);
    match (counted, fitted) {
        (true, true) => Err(bad(
            "`repeat` is written in both spellings at once. A counted repeat writes `step` and \
             `count`; a fitted one writes `along`, `stride`, `item` and `remainder`. One \
             operation, two spellings of HOW MANY, and a repeat has to be one of them."
                .to_string(),
        )),
        (false, false) => Err(bad(
            "`repeat` says nothing about how many. Write `step` and `count` to stamp the body \
             along a vector, or `along`, `stride`, `item` and `remainder` to fit items into the \
             scope's own axis."
                .to_string(),
        )),
        (true, false) => match (&op.step, &op.count) {
            (Some(step), Some(count)) => Ok(Spelling::Counted { step, count }),
            (None, _) => Err(bad(
                "a counted `repeat` writes `count` and no `step`, so every instance would stand \
                 on the last. Write the offset one instance moves by."
                    .to_string(),
            )),
            (_, None) => Err(bad(
                "a counted `repeat` writes `step` and no `count`, so nothing says how many \
                 instances there are."
                    .to_string(),
            )),
        },
        (false, true) => {
            let missing: Vec<&str> = [
                ("along", op.along.is_none()),
                ("stride", op.stride.is_none()),
                ("item", op.item.is_none()),
                ("remainder", op.remainder.is_none()),
            ]
            .into_iter()
            .filter(|(_, m)| *m)
            .map(|(n, _)| n)
            .collect();
            if !missing.is_empty() {
                return Err(bad(format!(
                    "a fitted `repeat` is missing {}. `along` is the scope axis the items stand \
                     along, `stride` the cells from one item's start to the next's, `item` the \
                     cells each one is long, and `remainder` — which has no default — says where \
                     the cells the items do not cover go: `exact`, `start`, `end` or `middle`.",
                    missing
                        .iter()
                        .map(|m| format!("`{m}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            }
            Ok(Spelling::Fitted {
                along: op.along.expect("checked"),
                stride: op.stride.as_ref().expect("checked"),
                item: op.item.as_ref().expect("checked"),
                remainder: op.remainder.expect("checked"),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Frames
// ---------------------------------------------------------------------------

/// **A scope**: the world box it covers, and the frame its body reads it
/// through.
///
/// One type for the place's own box, a `scope`, a `claim`, a `repeat` instance,
/// a `mirror`'s two passes and a `use`'s turned and mirrored body — because
/// every body-bearing operation makes its box its body's scope, and the only
/// thing that differs is the signed permutation it hands down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    /// The world box.
    pub region: Box3,
    /// Which world axis each local axis names, and which way it runs.
    pub orient: Orientation,
}

impl Frame {
    /// The identity frame over a box — what the place's own drawing starts in.
    pub fn of(region: Box3) -> Frame {
        Frame {
            region,
            orient: Orientation::IDENTITY,
        }
    }

    /// The scope's extents, in its own axis order.
    pub fn size(&self) -> [i64; 3] {
        [
            self.region.extent(self.orient.axis(Axis::X)) as i64,
            self.region.extent(self.orient.axis(Axis::Y)) as i64,
            self.region.extent(self.orient.axis(Axis::Z)) as i64,
        ]
    }

    /// The world cell a local cell of this scope stands at.
    pub fn world(&self, l: [i64; 3]) -> [i32; 3] {
        let mut out = self.region.origin;
        for a in Axis::ALL {
            let w = self.orient.axis(a);
            out[w.index()] = self.region.origin[w.index()]
                + self.orient.offset(a, l[a.index()], self.region.size) as i32;
        }
        out
    }

    /// The child frame a body-bearing operation hands its body: the box
    /// `lo..=hi` in this scope's own coordinates, mirrored and then turned.
    ///
    /// **The vertical never moves.** A turn is a quarter-turn about it and a
    /// mirror is across a horizontal centre plane, so a drawing has gravity and
    /// every `half`, every yaw and every stair keeps its meaning.
    pub fn child(&self, lo: [i64; 3], hi: [i64; 3], turn: u8, mirror: Option<Horizontal>) -> Frame {
        let a = self.world(lo);
        let b = self.world(hi);
        let region = Box3::new(
            [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
            [
                (a[0] - b[0]).unsigned_abs() + 1,
                (a[1] - b[1]).unsigned_abs() + 1,
                (a[2] - b[2]).unsigned_abs() + 1,
            ],
        );
        // Which of THIS scope's axes each child axis names, and with what sign.
        // The vertical is fixed; the two horizontals are the turn's own signed
        // permutation, composed with the mirror, which is applied first and so
        // multiplies the child axis's own sign.
        let (px, sx, pz, sz) = match turn % 4 {
            0 => (Axis::X, 1i8, Axis::Z, 1i8),
            1 => (Axis::Z, 1, Axis::X, -1),
            2 => (Axis::X, -1, Axis::Z, -1),
            _ => (Axis::Z, -1, Axis::X, 1),
        };
        let mut sign = [sx, 1i8, sz];
        if let Some(m) = mirror {
            let i = match m {
                Horizontal::X => 0,
                Horizontal::Z => 2,
            };
            sign[i] = -sign[i];
        }
        let parent = [px, Axis::Y, pz];
        let mut orient = Orientation {
            x: self.orient.axis(parent[0]),
            y: self.orient.axis(parent[1]),
            z: self.orient.axis(parent[2]),
            mirror: Mirror::NONE,
        };
        let mut flips = [false; 3];
        for a in 0..3 {
            flips[a] = self.orient.reversed(parent[a]) != (sign[a] < 0);
        }
        orient.mirror = Mirror::from_axes(flips);
        Frame { region, orient }
    }
}

// ---------------------------------------------------------------------------
// The canvas
// ---------------------------------------------------------------------------

/// What one cell holds after an operation painted it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CellPaint {
    /// One state, already resolved into the world frame.
    One(BlockState),
    /// A weighted list, already resolved into the world frame. The draw is
    /// taken after the last operation, by position.
    Mix(Vec<WeightedBlock>),
}

/// The role last painted at each cell, and the frame-resolved paint it named.
struct Canvas {
    region: Box3,
    role: Vec<u32>,
    paint: Vec<u32>,
    paints: Vec<CellPaint>,
    paint_of: BTreeMap<String, u32>,
    roles: Vec<String>,
    role_of: BTreeMap<String, u32>,
}

impl Canvas {
    fn new(region: Box3) -> Canvas {
        let cells = region.volume() as usize;
        let air = CellPaint::One(BlockState::air());
        Canvas {
            region,
            role: vec![0; cells],
            paint: vec![0; cells],
            paints: vec![air],
            paint_of: BTreeMap::from([(BlockState::air().to_string(), 0)]),
            roles: vec![AIR.to_string()],
            role_of: BTreeMap::from([(AIR.to_string(), 0)]),
        }
    }

    fn offset(&self, pos: [i32; 3]) -> Option<usize> {
        let max = self.region.maximum();
        for axis in 0..3 {
            if pos[axis] < self.region.origin[axis] || pos[axis] >= max[axis] {
                return None;
            }
        }
        let [_, sy, sz] = self.region.size;
        let dx = (pos[0] - self.region.origin[0]) as usize;
        let dy = (pos[1] - self.region.origin[1]) as usize;
        let dz = (pos[2] - self.region.origin[2]) as usize;
        Some((dx * sy as usize + dy) * sz as usize + dz)
    }

    fn role_id(&mut self, name: &str) -> u32 {
        if let Some(id) = self.role_of.get(name) {
            return *id;
        }
        let id = self.roles.len() as u32;
        self.roles.push(name.to_string());
        self.role_of.insert(name.to_string(), id);
        id
    }

    fn paint_id(&mut self, paint: CellPaint) -> u32 {
        let key = match &paint {
            CellPaint::One(b) => b.to_string(),
            CellPaint::Mix(mix) => mix
                .iter()
                .map(|w| format!("{}*{}", w.weight, w.block))
                .collect::<Vec<_>>()
                .join("|"),
        };
        if let Some(id) = self.paint_of.get(&key) {
            return *id;
        }
        let id = self.paints.len() as u32;
        self.paints.push(paint);
        self.paint_of.insert(key, id);
        id
    }
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

/// Everything an execution needs beyond the drawing and the box.
#[derive(Debug, Clone)]
pub struct ExecuteOptions {
    /// The one seed weighted paints draw from. Geometry has none.
    pub seed: u64,
    /// The work budget, shared with the grammar expander.
    pub limits: Limits,
    /// What the caller changed about the document before handing it over.
    pub overrides: Overrides,
    /// The directory a `grammar` operation resolves its `program` against.
    pub root: PathBuf,
}

impl ExecuteOptions {
    /// Default limits and no overrides, with the given seed, resolving a
    /// `grammar` operation's program against `root`.
    pub fn seeded(seed: u64, root: impl Into<PathBuf>) -> ExecuteOptions {
        ExecuteOptions {
            seed,
            limits: Limits::default(),
            overrides: Overrides::none(),
            root: root.into(),
        }
    }
}

/// What one operation of the document did, over every instance of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Activity {
    /// Instances executed.
    pub instances: u64,
    /// Cells painted, counting every write.
    pub cells: u64,
    /// True for an operation that paints: a solid, or a `grammar`.
    pub paints: bool,
}

/// **Not a refusal, and printed on every run** (spec-0072 §8).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunReport {
    /// Operations written, over `ops` and every define body.
    pub ops_written: usize,
    /// Instances executed, over every operation.
    pub instances: u64,
    /// Cells painted, counting every write.
    pub cells_painted: u64,
    /// Cells holding something other than air in the finished model.
    pub cells_surviving: u64,
    /// Cells whose paint was a weighted list, and so took a draw.
    pub weighted_cells: u64,
    /// Stairs whose `shape` the executor derived.
    pub stairs_settled: usize,
    /// **Every written operation that did nothing**, by address: a painting
    /// operation none of whose instances painted a cell, and a non-painting one
    /// no instance of which ran.
    ///
    /// Listed rather than refused, because a `define` written for many sizes may
    /// honestly have an operation with nothing to do at one of them — but it is
    /// dead text in the document of record, so it is said.
    pub silent: Vec<String>,
    /// What each operation did, by address.
    pub activity: BTreeMap<String, Activity>,
}

/// One executed drawing.
#[derive(Debug, Clone)]
pub struct Execution {
    /// The blocks, the anchors, the resolved contract — what every gate reads.
    pub expansion: Expansion,
    /// What the run did, and what did nothing.
    pub report: RunReport,
    /// The programs a `grammar` operation loaded, each with the SHA-256 of its
    /// canonical bytes — the other half of the provenance row.
    pub programs: Vec<(String, String)>,
}

/// The names a body resolves its roles through.
#[derive(Debug, Clone)]
enum RoleFrame {
    /// The document's own palette: a role name is a palette key.
    Document,
    /// A define's vocabulary: a role name is a key of this map, and the value is
    /// the document palette role — or `air` — it means here.
    Bound(BTreeMap<String, String>),
}

/// A scope: its frame, the names in force, and where it is in the document.
#[derive(Debug, Clone)]
struct ScopeState {
    frame: Frame,
    params: BTreeMap<String, i64>,
    roles: RoleFrame,
    /// The container pointer the body's operations are indexed under.
    base: String,
    through: Vec<String>,
    indices: Vec<(String, i64)>,
    mirrored: Option<Horizontal>,
}

impl ScopeState {
    fn address(&self, index: usize) -> Address {
        Address {
            pointer: format!("{}/{}", self.base, index),
            through: self.through.clone(),
            indices: self.indices.clone(),
            mirrored: self.mirrored,
            cell: None,
        }
    }

    fn eval_scope(&self) -> EvalScope<'_> {
        EvalScope {
            region: &self.frame.region,
            orient: self.frame.orient,
            env: Env::root(&self.params, &NO_PALETTE),
        }
    }
}

struct Executor<'d> {
    drawing: &'d Drawing,
    canvas: Canvas,
    anchors: BTreeMap<String, Anchor>,
    marks_seen: BTreeMap<String, u32>,
    regions: BTreeMap<String, ResolvedRegion>,
    limits: Limits,
    stats: Stats,
    oriented: OrientedFillAudit,
    report: RunReport,
    root: PathBuf,
    programs: Vec<(String, String)>,
}

/// **Execute a drawing over `region`.**
///
/// Same drawing + same region + same seed gives a byte-identical model
/// (ADR-0006); nothing here reads the clock, the environment or a hash order.
pub fn execute(
    drawing: &Drawing,
    region: Box3,
    options: &ExecuteOptions,
) -> Result<Execution, Refusal> {
    let census = check(drawing)?;
    // Before the canvas is allocated, not after: the grid is dense, so an
    // absurd box is an allocation the process may not survive rather than a
    // slow run, and a killed process reports nothing.
    let volume = region.volume();
    if volume > options.limits.max_volume {
        return Err(Refusal::at_field(
            DW_OUT_OF_RANGE,
            "/",
            format!(
                "the place's box is {}x{}x{} = {volume} cell(s), past the executor's budget of {}. \
                 A drawing is executed into a dense grid allocated before the first operation \
                 runs, so the budget is refused rather than clamped.",
                region.size[0], region.size[1], region.size[2], options.limits.max_volume
            ),
        ));
    }

    let mut ex = Executor {
        drawing,
        canvas: Canvas::new(region),
        anchors: BTreeMap::new(),
        marks_seen: BTreeMap::new(),
        regions: BTreeMap::new(),
        limits: options.limits,
        stats: Stats::default(),
        oriented: OrientedFillAudit::default(),
        report: RunReport {
            ops_written: census.ops,
            ..RunReport::default()
        },
        root: options.root.clone(),
        programs: Vec::new(),
    };

    let root = ScopeState {
        frame: Frame::of(region),
        params: drawing.params.clone(),
        roles: RoleFrame::Document,
        base: "/ops".to_string(),
        through: Vec::new(),
        indices: Vec::new(),
        mirrored: None,
    };
    ex.run_ops(&drawing.ops, &root, 0)?;

    // 1. The weighted draws, by position.
    let mut model = VoxelModel::new(region);
    let mut rng = Rng::new(options.seed);
    let mut surviving = 0u64;
    let mut weighted = 0u64;
    for pos in region.positions() {
        let draw = rng.next_u64();
        let index = ex.canvas.offset(pos).expect("positions() stays inside");
        let state = match &ex.canvas.paints[ex.canvas.paint[index] as usize] {
            CellPaint::One(b) => b.clone(),
            CellPaint::Mix(mix) => {
                weighted += 1;
                draw_from(mix, draw)
            }
        };
        if !state.is_air() {
            surviving += 1;
        }
        model.set(pos, &state).map_err(|e| {
            Refusal::at_field(
                DW_OUT_OF_RANGE,
                "/palette",
                format!("the drawing paints more distinct block states than a cell can index: {e}"),
            )
        })?;
    }

    // 2. Derived state, after the paints and before anything reads the model.
    let stairs = settle::derive_stair_shapes(&mut model);

    // 3. The expansion.
    for region in ex.regions.values_mut() {
        region
            .boxes
            .sort_by_key(|b| (b.origin, [b.size[0], b.size[1], b.size[2]]));
        region.boxes.dedup();
        region.declared_by.sort();
        region.declared_by.dedup();
    }
    let palette = &drawing.palette;
    let state_of = |role: &str| match palette.get(role) {
        Some(States::One(state)) => state.clone(),
        _ => BlockState::air(),
    };
    let contract = drawing
        .contract
        .as_ref()
        .map(|c| resolve_declared_contract(c, &state_of, &mut ex.regions));

    ex.report.cells_surviving = surviving;
    ex.report.weighted_cells = weighted;
    ex.report.stairs_settled = stairs;
    ex.report.silent = ex
        .report
        .activity
        .iter()
        .filter(|(_, a)| {
            if a.paints {
                a.cells == 0
            } else {
                a.instances == 0
            }
        })
        .map(|(k, _)| k.clone())
        .collect();

    Ok(Execution {
        expansion: Expansion {
            model,
            anchors: ex.anchors,
            contract,
            stats: ex.stats,
            oriented: ex.oriented,
        },
        report: ex.report,
        programs: ex.programs,
    })
}

/// The member a weighted list draws at one cell: `out % total`, over the weights
/// in the order the document writes them.
fn draw_from(mix: &[WeightedBlock], out: u64) -> BlockState {
    let total: u64 = mix.iter().map(|w| w.weight as u64).sum();
    if total == 0 {
        return BlockState::air();
    }
    let mut pick = out % total;
    for w in mix {
        let weight = w.weight as u64;
        if pick < weight {
            return w.block.clone();
        }
        pick -= weight;
    }
    mix.last()
        .map(|w| w.block.clone())
        .unwrap_or_else(BlockState::air)
}

impl Executor<'_> {
    /// Run one body in one scope.
    fn run_ops(&mut self, ops: &[Op], scope: &ScopeState, depth: u32) -> Result<(), Refusal> {
        for (i, op) in ops.iter().enumerate() {
            let at = scope.address(i);
            let entry = self.report.activity.entry(at.pointer.clone()).or_default();
            entry.paints = matches!(
                op,
                Op::Box(_)
                    | Op::Cylinder(_)
                    | Op::Sphere(_)
                    | Op::Prism(_)
                    | Op::Pyramid(_)
                    | Op::Line(_)
                    | Op::Grammar(_)
            );
            if let Some(when) = op.when()
                && !scope.eval_scope().test(when).map_err(|e| {
                    Refusal::new(DW_OUT_OF_RANGE, at.clone(), format!("`when`: {e}"))
                })?
            {
                // A false guard is how a define says *not at this size*. The
                // instance is not counted, which is what puts the operation on
                // the silent list at a size it has nothing to do at.
                continue;
            }
            self.enter(&at, depth)?;
            self.report
                .activity
                .entry(at.pointer.clone())
                .or_default()
                .instances += 1;
            self.report.instances += 1;
            self.run_op(op, &at, scope, depth)?;
        }
        Ok(())
    }

    /// The work budget. A drawing cannot recurse, so this bounds a document that
    /// nests or repeats past what the executor will do rather than one that
    /// cannot finish.
    fn enter(&mut self, at: &Address, depth: u32) -> Result<(), Refusal> {
        if depth > self.limits.max_depth {
            return Err(Refusal::new(
                DW_OUT_OF_RANGE,
                at.clone(),
                format!(
                    "the document nests {} levels deep, past the executor's limit of {}.",
                    depth, self.limits.max_depth
                ),
            ));
        }
        self.stats.scopes += 1;
        self.stats.depth = self.stats.depth.max(depth);
        if self.stats.scopes > self.limits.max_scopes {
            return Err(Refusal::new(
                DW_OUT_OF_RANGE,
                at.clone(),
                format!(
                    "the drawing has executed {} instance(s), past the executor's ceiling of {}. \
                     A `repeat`'s `count` or a nest of them is what reaches it.",
                    self.stats.scopes, self.limits.max_scopes
                ),
            ));
        }
        Ok(())
    }

    fn run_op(
        &mut self,
        op: &Op,
        at: &Address,
        scope: &ScopeState,
        depth: u32,
    ) -> Result<(), Refusal> {
        match op {
            Op::Box(o) => {
                let (lo, hi) = self.op_box(op, at, scope)?;
                let n = extents(lo, hi);
                let t = self.eval_positive(o.t.as_ref(), 1, at, scope, "t")?;
                let rule = Rule::Boxed(BoxRule {
                    n,
                    faces: o.faces.clone(),
                    t,
                });
                self.paint_rule(&rule, lo, &o.role, &o.where_roles, at, scope)
            }
            Op::Cylinder(o) => {
                let (lo, hi) = self.op_box(op, at, scope)?;
                let n = extents(lo, hi);
                let t = self.eval_thickness(o.t.as_ref(), at, scope)?;
                let mut round = [true; 3];
                round[o.axis.index()] = false;
                self.check_round(&round, n, o.flat, at)?;
                let rule = Rule::Round(RoundRule {
                    n,
                    round,
                    flat: o.flat,
                    t,
                });
                self.paint_rule(&rule, lo, &o.role, &o.where_roles, at, scope)
            }
            Op::Sphere(o) => {
                let (lo, hi) = self.op_box(op, at, scope)?;
                let n = extents(lo, hi);
                let t = self.eval_thickness(o.t.as_ref(), at, scope)?;
                self.check_round(&[true; 3], n, o.flat, at)?;
                let rule = Rule::Round(RoundRule::sphere(n, o.flat, t));
                self.paint_rule(&rule, lo, &o.role, &o.where_roles, at, scope)
            }
            Op::Prism(o) => {
                let (lo, hi) = self.op_box(op, at, scope)?;
                let rule = Rule::Stepped(StepRule {
                    n: extents(lo, hi),
                    step: Step::Prism {
                        taper: o.taper,
                        sides: o.sides,
                    },
                    rise: self.eval_positive(o.rise.as_ref(), 1, at, scope, "rise")?,
                    run: self.eval_positive(o.run.as_ref(), 1, at, scope, "run")?,
                    skin: o.skin,
                });
                self.paint_rule(&rule, lo, &o.role, &o.where_roles, at, scope)
            }
            Op::Pyramid(o) => {
                let (lo, hi) = self.op_box(op, at, scope)?;
                let rule = Rule::Stepped(StepRule {
                    n: extents(lo, hi),
                    step: Step::Pyramid { section: o.section },
                    rise: self.eval_positive(o.rise.as_ref(), 1, at, scope, "rise")?,
                    run: self.eval_positive(o.run.as_ref(), 1, at, scope, "run")?,
                    skin: o.skin,
                });
                self.paint_rule(&rule, lo, &o.role, &o.where_roles, at, scope)
            }
            Op::Line(o) => self.run_line(o, at, scope),
            Op::Scope(o) => {
                let (lo, hi) = self.op_box(op, at, scope)?;
                let child = self.enter_box(scope, lo, hi, 0, None, at);
                self.run_ops(&o.body, &child, depth + 1)
            }
            Op::Claim(o) => {
                let (lo, hi) = self.op_box(op, at, scope)?;
                let child = self.enter_box(scope, lo, hi, 0, None, at);
                self.claim(&o.region, &child, at);
                self.run_ops(&o.body, &child, depth + 1)
            }
            Op::Mirror(o) => {
                let (lo, hi) = self.op_box(op, at, scope)?;
                // The unreflected body first: a `mirror` is one operation whose
                // second pass overwrites the first where the two meet, and
                // "first" has to mean the same thing on every run (ADR-0006).
                let plain = self.enter_box(scope, lo, hi, 0, None, at);
                self.run_ops(&o.body, &plain, depth + 1)?;
                let mut reflected = self.enter_box(scope, lo, hi, 0, Some(o.axis), at);
                reflected.mirrored = Some(o.axis);
                self.run_ops(&o.body, &reflected, depth + 1)
            }
            Op::Repeat(o) => self.run_repeat(o, at, scope, depth),
            Op::Use(o) => self.run_use(o, at, scope, depth),
            Op::Mark(o) => self.run_mark(&o.mark, at, scope),
            Op::Grammar(o) => self.run_grammar(o, at, scope),
        }
    }

    /// The box an operation covers, in its scope's own coordinates — and the
    /// refusal when it reaches a cell the scope does not hold.
    ///
    /// `from` omitted is the scope's minimum corner; `to` omitted is `from`
    /// where one was written and the scope's maximum corner where none was, so
    /// both omitted is the whole scope and `from` alone is one cell.
    fn op_box(
        &self,
        op: &Op,
        at: &Address,
        scope: &ScopeState,
    ) -> Result<([i64; 3], [i64; 3]), Refusal> {
        let n = scope.frame.size();
        let (from, to) = op.extent();
        let lo = match from {
            Some(v) => self.eval_vec(v, at, scope, "from")?,
            None => [0, 0, 0],
        };
        let hi = match (to, from) {
            (Some(v), _) => self.eval_vec(v, at, scope, "to")?,
            (None, Some(_)) => lo,
            (None, None) => [n[0] - 1, n[1] - 1, n[2] - 1],
        };
        for a in 0..3 {
            if hi[a] < lo[a] {
                return Err(Refusal::new(
                    DW_OUT_OF_RANGE,
                    at.clone(),
                    format!(
                        "`to` is {},{},{} and `from` is {},{},{}: the {} corner is below the low \
                         one. The two are inclusive corners of one box, so `to` is at or past \
                         `from` on every axis.",
                        hi[0],
                        hi[1],
                        hi[2],
                        lo[0],
                        lo[1],
                        lo[2],
                        ["x", "y", "z"][a]
                    ),
                ));
            }
        }
        self.hold(lo, hi, n, at, op.verb())?;
        Ok((lo, hi))
    }

    /// `DW0902`: a box the scope does not hold.
    fn hold(
        &self,
        lo: [i64; 3],
        hi: [i64; 3],
        n: [i64; 3],
        at: &Address,
        what: &str,
    ) -> Result<(), Refusal> {
        if (0..3).all(|a| lo[a] >= 0 && hi[a] < n[a]) {
            return Ok(());
        }
        Err(Refusal::new(
            DW_LEFT_ITS_SCOPE,
            at.clone(),
            format!(
                "this `{what}` covers {},{},{} .. {},{},{}, and the scope it is written in holds \
                 0,0,0 .. {},{},{} ({}x{}x{}). An operation paints only cells its own scope holds.",
                lo[0],
                lo[1],
                lo[2],
                hi[0],
                hi[1],
                hi[2],
                n[0] - 1,
                n[1] - 1,
                n[2] - 1,
                n[0],
                n[1],
                n[2]
            ),
        ))
    }

    /// The child scope a body-bearing operation hands its body.
    fn enter_box(
        &self,
        scope: &ScopeState,
        lo: [i64; 3],
        hi: [i64; 3],
        turn: u8,
        mirror: Option<Horizontal>,
        at: &Address,
    ) -> ScopeState {
        ScopeState {
            frame: scope.frame.child(lo, hi, turn, mirror),
            params: scope.params.clone(),
            roles: scope.roles.clone(),
            base: format!("{}/body", at.pointer),
            through: scope.through.clone(),
            indices: scope.indices.clone(),
            mirrored: scope.mirrored,
        }
    }
}

/// The extents of an inclusive corner pair.
fn extents(lo: [i64; 3], hi: [i64; 3]) -> [i64; 3] {
    [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1]
}

// ---------------------------------------------------------------------------
// Evaluating what an operation was written with
// ---------------------------------------------------------------------------

impl Executor<'_> {
    fn eval_int(
        &self,
        v: &Int,
        at: &Address,
        scope: &ScopeState,
        field: &str,
    ) -> Result<i64, Refusal> {
        let value = scope.eval_scope().eval(&v.expr()).map_err(|e| {
            Refusal::new(
                DW_OUT_OF_RANGE,
                at.clone(),
                format!(
                    "`{field}`: {e}. The parameters in force here are {}.",
                    bound(scope)
                ),
            )
        })?;
        if value.abs() > COORD_LIMIT {
            return Err(Refusal::new(
                DW_OUT_OF_RANGE,
                at.clone(),
                format!(
                    "`{field}` evaluates to {value}, past what a cell of a world can be \
                     (±{COORD_LIMIT}). The algebra saturates rather than wrapping, so a value this \
                     large is arithmetic that ran off the end rather than a number anyone wrote. \
                     The parameters in force here are {}.",
                    bound(scope)
                ),
            ));
        }
        Ok(value)
    }

    fn eval_vec(
        &self,
        v: &Vec3,
        at: &Address,
        scope: &ScopeState,
        field: &str,
    ) -> Result<[i64; 3], Refusal> {
        Ok([
            self.eval_int(&v[0], at, scope, field)?,
            self.eval_int(&v[1], at, scope, field)?,
            self.eval_int(&v[2], at, scope, field)?,
        ])
    }

    /// A field that is at least 1, with its default.
    fn eval_positive(
        &self,
        v: Option<&Int>,
        default: i64,
        at: &Address,
        scope: &ScopeState,
        field: &str,
    ) -> Result<i64, Refusal> {
        let Some(v) = v else { return Ok(default) };
        let value = self.eval_int(v, at, scope, field)?;
        if value < 1 {
            return Err(Refusal::new(
                DW_OUT_OF_RANGE,
                at.clone(),
                format!(
                    "`{field}` evaluates to {value}, and it is a count of cells or courses: it is \
                     1 or more. The parameters in force here are {}.",
                    bound(scope)
                ),
            ));
        }
        Ok(value)
    }

    /// A wall thickness: absent is solid, written is at least 1.
    fn eval_thickness(
        &self,
        v: Option<&Int>,
        at: &Address,
        scope: &ScopeState,
    ) -> Result<Option<i64>, Refusal> {
        match v {
            None => Ok(None),
            Some(_) => Ok(Some(self.eval_positive(v, 1, at, scope, "t")?)),
        }
    }

    /// The two bounds a round solid carries: `flat` names a face of a round
    /// axis, and a round axis fits the arithmetic.
    fn check_round(
        &self,
        round: &[bool; 3],
        n: [i64; 3],
        flat: Option<Face>,
        at: &Address,
    ) -> Result<(), Refusal> {
        if let Some(f) = flat
            && !round[f.axis().index()]
        {
            return Err(Refusal::new(
                DW_OUT_OF_RANGE,
                at.clone(),
                format!(
                    "`flat` is {f}, which lies on the straight axis. A flat is the cut plane of a \
                     HALF solid, so it names a face of an axis the solid is round on."
                ),
            ));
        }
        for a in 0..3 {
            if round[a] && n[a] > MAX_ROUND_AXIS {
                return Err(Refusal::new(
                    DW_OUT_OF_RANGE,
                    at.clone(),
                    format!(
                        "the round {} axis is {} cells, past the {MAX_ROUND_AXIS} the inclusion \
                         test is exact to. The test multiplies squared doubled extents, and past \
                         this bound the arithmetic would have to wrap — a wrapped comparison is a \
                         solid that is quietly the wrong shape, so the bound is refused instead.",
                        ["x", "y", "z"][a],
                        n[a]
                    ),
                ));
            }
        }
        Ok(())
    }
}

/// The parameters in force at a scope, for a refusal that has to name them.
fn bound(scope: &ScopeState) -> String {
    let v: Vec<String> = scope
        .params
        .iter()
        .map(|(k, n)| format!("{k}={n}"))
        .collect();
    if v.is_empty() {
        "(none)".to_string()
    } else {
        v.join(", ")
    }
}

// ---------------------------------------------------------------------------
// Painting
// ---------------------------------------------------------------------------

impl Executor<'_> {
    /// What a role name means here: the document palette role it resolves to,
    /// or `air`.
    fn resolve_role(
        &self,
        name: &str,
        scope: &ScopeState,
        at: &Address,
    ) -> Result<String, Refusal> {
        if name == AIR {
            return Ok(AIR.to_string());
        }
        let resolved = match &scope.roles {
            RoleFrame::Document => self
                .drawing
                .palette
                .contains_key(name)
                .then(|| name.to_string()),
            RoleFrame::Bound(map) => map.get(name).cloned(),
        };
        resolved.ok_or_else(|| {
            Refusal::new(
                DW_UNKNOWN_NAME,
                at.clone(),
                format!("the role {name:?} is bound by nothing here"),
            )
        })
    }

    /// Paint every cell of a rule, with its box's low corner at `lo` in the
    /// scope's own coordinates.
    fn paint_rule(
        &mut self,
        rule: &Rule,
        lo: [i64; 3],
        role: &str,
        where_roles: &[String],
        at: &Address,
        scope: &ScopeState,
    ) -> Result<(), Refusal> {
        let document_role = self.resolve_role(role, scope, at)?;
        let paint = self.frame_paint(&document_role, scope, at)?;
        let allowed = self.allowed(where_roles, scope, at)?;
        let n = rule.extents();
        let mut cells = 0u64;
        for x in 0..n[0] {
            for y in 0..n[1] {
                for z in 0..n[2] {
                    if !rule.contains([x, y, z]) {
                        continue;
                    }
                    let local = [lo[0] + x, lo[1] + y, lo[2] + z];
                    if self.write(local, &document_role, &paint, allowed.as_ref(), scope) {
                        cells += 1;
                    }
                }
            }
        }
        self.credit(at, cells, &paint);
        Ok(())
    }

    /// The role ids a `where` admits, or `None` for an operation that overwrites
    /// every cell.
    fn allowed(
        &mut self,
        where_roles: &[String],
        scope: &ScopeState,
        at: &Address,
    ) -> Result<Option<BTreeSet<u32>>, Refusal> {
        if where_roles.is_empty() {
            return Ok(None);
        }
        let mut out = BTreeSet::new();
        for name in where_roles {
            // `grammar` is a current role and never a palette one: it is what
            // the cells a `grammar` operation wrote answer to.
            let resolved = if name == GRAMMAR {
                GRAMMAR.to_string()
            } else {
                self.resolve_role(name, scope, at)?
            };
            out.insert(self.canvas.role_id(&resolved));
        }
        Ok(Some(out))
    }

    /// The paint a document role names, resolved into the world through the
    /// frame of the scope that paints it.
    fn frame_paint(
        &mut self,
        document_role: &str,
        scope: &ScopeState,
        at: &Address,
    ) -> Result<CellPaint, Refusal> {
        if document_role == AIR {
            return Ok(CellPaint::One(BlockState::air()));
        }
        let states = self
            .drawing
            .palette
            .get(document_role)
            .expect("check() proved every resolved role is bound");
        let resolved =
            crate::grammar::place::resolve_states(states, scope.frame.orient).map_err(|u| {
                Refusal::new(
                    crate::drawing::diag::DW_UNRESOLVABLE_FRAME,
                    at.clone(),
                    format!(
                        "the role {document_role:?} paints {}, whose {} has no image under the \
                         frame this scope stands in ({}). Every state in a drawing is written in \
                         the frame of the scope that paints it, and a property the pinned \
                         vocabulary cannot map has no correct block to write.",
                        u.state, u.property, u.orientation
                    ),
                )
            })?;
        Ok(match resolved {
            States::One(b) => CellPaint::One(b),
            States::Mix(mix) => CellPaint::Mix(mix),
        })
    }

    /// Write one cell, honouring `where`. Returns whether it landed.
    fn write(
        &mut self,
        local: [i64; 3],
        document_role: &str,
        paint: &CellPaint,
        allowed: Option<&BTreeSet<u32>>,
        scope: &ScopeState,
    ) -> bool {
        let world = scope.frame.world(local);
        let Some(index) = self.canvas.offset(world) else {
            return false;
        };
        if let Some(allowed) = allowed
            && !allowed.contains(&self.canvas.role[index])
        {
            return false;
        }
        let role_id = self.canvas.role_id(document_role);
        let paint_id = self.canvas.paint_id(paint.clone());
        self.canvas.role[index] = role_id;
        self.canvas.paint[index] = paint_id;
        true
    }

    /// Record what an operation painted, and what its paint was for the
    /// `oriented-fills` audit.
    ///
    /// Every painting instance the executor ran is counted, and every one of
    /// them that carries properties was resolved from the frame: a drawing has
    /// no world-frame literal to land wrong, so `resolved` equals `carrying` by
    /// construction and the gate binds to the whole population rather than
    /// quietly stopping.
    fn credit(&mut self, at: &Address, cells: u64, paint: &CellPaint) {
        self.report
            .activity
            .entry(at.pointer.clone())
            .or_default()
            .cells += cells;
        self.report.cells_painted += cells;
        self.oriented.fills += 1;
        let carries = match paint {
            CellPaint::One(b) => !b.properties.is_empty(),
            CellPaint::Mix(mix) => mix.iter().any(|w| !w.block.properties.is_empty()),
        };
        if carries {
            self.oriented.carrying += 1;
            self.oriented.resolved += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// The operations that are not a box of cells
// ---------------------------------------------------------------------------

impl Executor<'_> {
    /// `line`: the integer line of §3.5, each point stamped with the brush.
    ///
    /// `from` and `to` are **end points**, not corners, so the cells the
    /// operation reaches are the points' own bounding box grown by the brush —
    /// and that, not the corner pair, is what the scope must hold.
    fn run_line(
        &mut self,
        op: &crate::drawing::ir::LineOp,
        at: &Address,
        scope: &ScopeState,
    ) -> Result<(), Refusal> {
        let n = scope.frame.size();
        let a = match &op.from {
            Some(v) => self.eval_vec(v, at, scope, "from")?,
            None => [0, 0, 0],
        };
        let b = match &op.to {
            Some(v) => self.eval_vec(v, at, scope, "to")?,
            None => a,
        };
        let brush = match &op.brush {
            Some(v) => {
                let brush = self.eval_vec(v, at, scope, "brush")?;
                for (i, side) in brush.iter().enumerate() {
                    if *side < 1 {
                        return Err(Refusal::new(
                            DW_OUT_OF_RANGE,
                            at.clone(),
                            format!(
                                "`brush` is {},{},{}, and its {} side is {side}: a brush is the \
                                 box painted at each point, so every side is 1 or more.",
                                brush[0],
                                brush[1],
                                brush[2],
                                ["x", "y", "z"][i]
                            ),
                        ));
                    }
                }
                brush
            }
            None => [1, 1, 1],
        };
        let points = line_points(a, b);
        let lo = [
            points.iter().map(|p| p[0]).min().unwrap_or(0),
            points.iter().map(|p| p[1]).min().unwrap_or(0),
            points.iter().map(|p| p[2]).min().unwrap_or(0),
        ];
        let hi = [
            points.iter().map(|p| p[0]).max().unwrap_or(0) + brush[0] - 1,
            points.iter().map(|p| p[1]).max().unwrap_or(0) + brush[1] - 1,
            points.iter().map(|p| p[2]).max().unwrap_or(0) + brush[2] - 1,
        ];
        self.hold(lo, hi, n, at, "line")?;

        let document_role = self.resolve_role(&op.role, scope, at)?;
        let paint = self.frame_paint(&document_role, scope, at)?;
        let allowed = self.allowed(&op.where_roles, scope, at)?;
        // The brush stamps overlap wherever the line turns, so a cell is
        // counted once: the report says cells painted, not writes attempted.
        let mut seen: BTreeSet<[i64; 3]> = BTreeSet::new();
        let mut cells = 0u64;
        for p in &points {
            for dx in 0..brush[0] {
                for dy in 0..brush[1] {
                    for dz in 0..brush[2] {
                        let local = [p[0] + dx, p[1] + dy, p[2] + dz];
                        if !seen.insert(local) {
                            continue;
                        }
                        if self.write(local, &document_role, &paint, allowed.as_ref(), scope) {
                            cells += 1;
                        }
                    }
                }
            }
        }
        self.credit(at, cells, &paint);
        Ok(())
    }

    /// `repeat`, in either spelling.
    fn run_repeat(
        &mut self,
        op: &crate::drawing::ir::RepeatOp,
        at: &Address,
        scope: &ScopeState,
        depth: u32,
    ) -> Result<(), Refusal> {
        let (lo, hi) = self.op_box(&Op::Repeat(op.clone()), at, scope)?;
        let n = scope.frame.size();
        match repeat_spelling(op, at)? {
            Spelling::Counted { step, count } => {
                let step = self.eval_vec(step, at, scope, "step")?;
                let count = self.eval_int(count, at, scope, "count")?;
                if count < 0 {
                    return Err(Refusal::new(
                        DW_OUT_OF_RANGE,
                        at.clone(),
                        format!(
                            "`count` evaluates to {count}, and a count of instances is 0 or more. \
                             The parameters in force here are {}.",
                            bound(scope)
                        ),
                    ));
                }
                for k in 0..count {
                    let ilo = [
                        lo[0] + k * step[0],
                        lo[1] + k * step[1],
                        lo[2] + k * step[2],
                    ];
                    let ihi = [
                        hi[0] + k * step[0],
                        hi[1] + k * step[1],
                        hi[2] + k * step[2],
                    ];
                    let mut named = at.clone();
                    if let Some(index) = &op.index {
                        named.indices.push((index.clone(), k));
                    }
                    self.hold(ilo, ihi, n, &named, "repeat instance")?;
                    let child = self.instance(scope, ilo, ihi, at, op.index.as_deref(), k);
                    self.enter(&named, depth)?;
                    self.run_ops(&op.body, &child, depth + 1)?;
                }
                Ok(())
            }
            Spelling::Fitted {
                along,
                stride,
                item,
                remainder,
            } => {
                let stride = self.eval_positive(Some(stride), 1, at, scope, "stride")?;
                let item = self.eval_positive(Some(item), 1, at, scope, "item")?;
                let axis = along.index();
                let extent = hi[axis] - lo[axis] + 1;
                if extent < item {
                    return Err(Refusal::new(
                        DW_DOES_NOT_FIT,
                        at.clone(),
                        format!(
                            "the scope's {} axis is {extent} cell(s) and one item is {item}: no \
                             whole item fits. A fitted `repeat` stands items in the run it was \
                             given, so a run shorter than one item has nothing to stand in it.",
                            ["x", "y", "z"][axis]
                        ),
                    ));
                }
                let items = (extent - item) / stride + 1;
                let left = extent - ((items - 1) * stride + item);
                if remainder == Remainder::Exact && left != 0 {
                    return Err(Refusal::new(
                        DW_DOES_NOT_FIT,
                        at.clone(),
                        format!(
                            "`remainder` is `exact` and {left} cell(s) are over: the scope's {} \
                             axis is {extent}, `stride` is {stride}, `item` is {item}, so {items} \
                             item(s) cover {} and leave {left}. `start`, `end` and `middle` each \
                             accept it and say where it goes.",
                            ["x", "y", "z"][axis],
                            (items - 1) * stride + item
                        ),
                    ));
                }
                let pad = match remainder {
                    Remainder::Exact | Remainder::End => 0,
                    Remainder::Start => left,
                    // The lower-middle split, which is what `split`'s own
                    // `middle` rounding means and what `mark`'s centres mean:
                    // it has to be one of the two, and it has to be the same one
                    // every time (ADR-0006).
                    Remainder::Middle => left / 2,
                };
                for k in 0..items {
                    let start = lo[axis] + pad + k * stride;
                    let mut ilo = lo;
                    let mut ihi = hi;
                    ilo[axis] = start;
                    ihi[axis] = start + item - 1;
                    let mut named = at.clone();
                    if let Some(index) = &op.index {
                        named.indices.push((index.clone(), k));
                    }
                    self.hold(ilo, ihi, n, &named, "repeat item")?;
                    let child = self.instance(scope, ilo, ihi, at, op.index.as_deref(), k);
                    self.enter(&named, depth)?;
                    self.run_ops(&op.body, &child, depth + 1)?;
                }
                Ok(())
            }
        }
    }

    /// One `repeat` instance's scope: its own box, with the index bound over
    /// the body.
    fn instance(
        &self,
        scope: &ScopeState,
        lo: [i64; 3],
        hi: [i64; 3],
        at: &Address,
        index: Option<&str>,
        k: i64,
    ) -> ScopeState {
        let mut child = self.enter_box(scope, lo, hi, 0, None, at);
        if let Some(index) = index {
            child.params.insert(index.to_string(), k);
            child.indices.push((index.to_string(), k));
        }
        child
    }

    /// `use`: a define's body, in this box, under this frame.
    fn run_use(
        &mut self,
        op: &crate::drawing::ir::UseOp,
        at: &Address,
        scope: &ScopeState,
        depth: u32,
    ) -> Result<(), Refusal> {
        let (lo, hi) = self.op_box(&Op::Use(op.clone()), at, scope)?;
        let define: &Define = self
            .drawing
            .defines
            .get(&op.define)
            .expect("check() proved every `use` names a define");
        // The arguments are evaluated in the ENCLOSING scope, before the frame
        // is pushed: `{"h": {"expr":"dim","dim":"y"}}` means the caller's own
        // height, which is the whole point of passing it.
        let mut params = self.drawing.params.clone();
        for (name, default) in &define.params {
            params.insert(name.clone(), *default);
        }
        for (name, value) in &op.params {
            let v = self.eval_int(value, at, scope, name)?;
            params.insert(name.clone(), v);
        }
        let mut roles = BTreeMap::new();
        for (local, outer) in &op.roles {
            roles.insert(local.clone(), self.resolve_role(outer, scope, at)?);
        }
        let child = ScopeState {
            frame: scope.frame.child(lo, hi, op.turn, op.mirror),
            params,
            roles: RoleFrame::Bound(roles),
            base: format!("/defines/{}/body", op.define),
            through: {
                let mut through = vec![at.pointer.clone()];
                through.extend(scope.through.iter().cloned());
                through
            },
            indices: scope.indices.clone(),
            mirrored: scope.mirrored,
        };
        self.run_ops(&define.body, &child, depth + 1)
    }

    /// `mark`: an anchor, at a cell of the scope it is written in.
    fn run_mark(
        &mut self,
        mark: &crate::grammar::ir::Mark,
        at: &Address,
        scope: &ScopeState,
    ) -> Result<(), Refusal> {
        let eval_scope = scope.eval_scope();
        let mut eval = |expr: &Expr| eval_scope.eval(expr);
        let cell = crate::grammar::place::mark_cell(
            mark,
            scope.frame.region,
            scope.frame.orient,
            &mut eval,
        )
        .map_err(|e| match e {
            crate::grammar::place::MarkError::Eval { error, axis } => Refusal::new(
                DW_OUT_OF_RANGE,
                at.clone(),
                format!(
                    "the mark's offset on {}: {error}. The parameters in force here are {}.",
                    ["x", "y", "z"][axis.index()],
                    bound(scope)
                ),
            ),
            crate::grammar::place::MarkError::Outside { cell } => Refusal::new(
                DW_LEFT_ITS_SCOPE,
                at.clone().at_cell(cell),
                format!(
                    "the mark {:?} names a cell the scope it is written in does not hold; the \
                     scope is {},{},{} size {}x{}x{}.",
                    mark.anchor,
                    scope.frame.region.origin[0],
                    scope.frame.region.origin[1],
                    scope.frame.region.origin[2],
                    scope.frame.region.size[0],
                    scope.frame.region.size[1],
                    scope.frame.region.size[2],
                ),
            ),
        })?;
        let Some(facing) = crate::grammar::place::mark_facing(mark, scope.frame.orient) else {
            return Err(Refusal::new(
                DW_OUT_OF_RANGE,
                at.clone(),
                format!(
                    "the mark {:?} declares no `facing` and the scope it is written in calls the \
                     vertical its local `z`, so there is no cardinal direction to derive. Declare \
                     the facing.",
                    mark.anchor
                ),
            ));
        };
        let seen = self.marks_seen.entry(mark.anchor.clone()).or_insert(0);
        *seen += 1;
        let name = mark.name(*seen);
        let origin = self.canvas.region.origin;
        let anchor = Anchor {
            pos: [
                cell[0] - origin[0],
                cell[1] - origin[1],
                cell[2] - origin[2],
            ],
            facing,
            role: mark.role,
            declared_by: at.pointer.clone(),
        };
        if let Some(first) = self.anchors.get(&name) {
            return Err(Refusal::new(
                DW_UNKNOWN_NAME,
                at.clone(),
                format!(
                    "two marks produce the anchor {name:?} — this one and the one at {}. One name \
                     is one place; write `\"index\": \"auto\"` on both to number them, which is \
                     what a mark inside a `mirror` or a `repeat` wants.",
                    first.declared_by
                ),
            ));
        }
        self.anchors.insert(name, anchor);
        Ok(())
    }

    /// `claim`: this scope's box, for a named region of the contract.
    fn claim(&mut self, region: &str, scope: &ScopeState, at: &Address) {
        let entry = self.regions.entry(region.to_string()).or_default();
        if !entry.declared_by.iter().any(|s| s == &at.pointer) {
            entry.declared_by.push(at.pointer.clone());
        }
        if scope.frame.region.is_empty() {
            return;
        }
        let origin = self.canvas.region.origin;
        entry.boxes.push(Box3::new(
            [
                scope.frame.region.origin[0] - origin[0],
                scope.frame.region.origin[1] - origin[1],
                scope.frame.region.origin[2] - origin[2],
            ],
            scope.frame.region.size,
        ));
    }

    /// `grammar`: a box-split program expanded into the operation's box.
    ///
    /// **Non-air overwrites; air leaves what was drawn.** spec-0072 §4.5 asks
    /// for `fill` and `void` to overwrite and `skip` to leave, and the tree
    /// cannot tell those apart: a grammar expansion is a `VoxelModel` that
    /// starts as air, so a `void`ed cell and a `skip`ped one are the same byte.
    /// The rule here is therefore the one the compiler's own `fragment` stamp
    /// already applies — non-air overwrites, authored air never erases — and it
    /// is stated rather than approximated.
    fn run_grammar(
        &mut self,
        op: &crate::drawing::ir::GrammarOp,
        at: &Address,
        scope: &ScopeState,
    ) -> Result<(), Refusal> {
        let (lo, hi) = self.op_box(&Op::Grammar(op.clone()), at, scope)?;
        let frame = scope.frame.child(lo, hi, 0, None);
        let path = Path::new(&op.program);
        if path.is_absolute() {
            return Err(Refusal::new(
                codes::SCHEMA,
                at.clone(),
                format!(
                    "`program` is {:?}, an absolute path. A program is named relative to the \
                     drawing that writes it, because a document naming one machine's paths builds \
                     on that machine and on no other (ADR-0006).",
                    op.program
                ),
            ));
        }
        let full = self.root.join(path);
        let loaded = crate::grammar::document::load(&full).map_err(|e| {
            Refusal::new(
                codes::SCHEMA,
                at.clone(),
                format!("{}: {e}", full.display()),
            )
        })?;
        let mut program = loaded.program;
        let mut overrides = Overrides::none();
        for (name, value) in &op.params {
            program
                .set_param(name, *value)
                .map_err(|e| Refusal::new(DW_UNKNOWN_NAME, at.clone(), format!("`params`: {e}")))?;
            overrides.params.insert(name.clone(), *value);
        }
        for (role, state) in &op.roles {
            let block: BlockState = state.parse().map_err(|e| {
                Refusal::new(
                    codes::SCHEMA,
                    at.clone(),
                    format!("`roles`: {state:?} is not a block state: {e}"),
                )
            })?;
            // A `--role` override is a RESTYLE: it says which material, and the
            // syntax has no word for an axis frame, so it inherits the frame of
            // the binding it replaces.
            let paint = if program
                .palette
                .get(role.as_str())
                .is_some_and(Paint::is_local)
            {
                Paint::local_block(block)
            } else {
                Paint::block(block)
            };
            program
                .set_role(role, paint)
                .map_err(|e| Refusal::new(DW_UNKNOWN_NAME, at.clone(), format!("`roles`: {e}")))?;
            overrides.roles.insert(role.clone(), state.clone());
        }
        let options = ExpandOptions {
            seed: op.seed,
            limits: self.limits,
            orientation: frame.orient,
            overrides,
        };
        let expansion =
            crate::grammar::expand::expand(&program, frame.region, &options).map_err(|e| {
                Refusal::new(
                    codes::SCHEMA,
                    at.clone(),
                    format!("expanding {}: {e}", full.display()),
                )
            })?;

        // The program's own words about its provenance travel with the drawing's.
        self.programs.push((
            op.program.clone(),
            crate::grammar::export::program_hash(&program),
        ));

        let grammar_role = self.canvas.role_id(GRAMMAR);
        let mut cells = 0u64;
        for pos in frame.region.positions() {
            let Some(state) = expansion.model.get(pos) else {
                continue;
            };
            if state.is_air() {
                continue;
            }
            let Some(index) = self.canvas.offset(pos) else {
                continue;
            };
            let paint = self.canvas.paint_id(CellPaint::One(state.clone()));
            self.canvas.role[index] = grammar_role;
            self.canvas.paint[index] = paint;
            cells += 1;
        }
        // Its marks join the drawing's anchors, rebased onto the place's box.
        for (name, anchor) in &expansion.anchors {
            let world = [
                frame.region.origin[0] + anchor.pos[0],
                frame.region.origin[1] + anchor.pos[1],
                frame.region.origin[2] + anchor.pos[2],
            ];
            let origin = self.canvas.region.origin;
            let moved = Anchor {
                pos: [
                    world[0] - origin[0],
                    world[1] - origin[1],
                    world[2] - origin[2],
                ],
                facing: anchor.facing,
                role: anchor.role,
                declared_by: format!("{} ({})", at.pointer, anchor.declared_by),
            };
            if let Some(first) = self.anchors.get(name) {
                return Err(Refusal::new(
                    DW_UNKNOWN_NAME,
                    at.clone(),
                    format!(
                        "the program's mark produces the anchor {name:?}, which the drawing \
                         already declares at {}. One name is one place.",
                        first.declared_by
                    ),
                ));
            }
            self.anchors.insert(name.clone(), moved);
        }
        // And its own audit joins the drawing's, so the `oriented-fills` gate
        // reads one population.
        self.oriented.fills += expansion.oriented.fills;
        self.oriented.carrying += expansion.oriented.carrying;
        self.oriented.resolved += expansion.oriented.resolved;
        self.oriented
            .unguarded
            .extend(expansion.oriented.unguarded.iter().cloned());
        self.oriented
            .undecided
            .extend(expansion.oriented.undecided.iter().cloned());
        self.oriented.unguarded.sort();
        self.oriented.unguarded.dedup();
        self.oriented.undecided.sort();
        self.oriented.undecided.dedup();

        self.report
            .activity
            .entry(at.pointer.clone())
            .or_default()
            .cells += cells;
        self.report.cells_painted += cells;
        Ok(())
    }
}
