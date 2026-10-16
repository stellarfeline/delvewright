//! The words a sign says, as the commands that write them (spec-0115 §4).
//!
//! One value per lettered sign, [`SignCompound`], rendered twice: as the SNBT
//! a `data merge block` line carries, and as the NBT the written world lays
//! as the cell's block entity. One producer, so the line and the world cannot
//! disagree.

use super::*;

use delvewright_dsl::{Dye, SignFace, SignKind};

/// One face as the game holds it: exactly four line components, the dye, and
/// whether the text glows.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FaceValue {
    /// The four lines, top to bottom, each a text component.
    pub(crate) messages: [Value; 4],
    /// The dye.
    pub(crate) color: Dye,
    /// `has_glowing_text`.
    pub(crate) glowing: bool,
}

/// A line as a text component: a translation-tagged line lowers to its
/// `translate` component (styled spans as `with`), an untagged styled line to
/// its styled component, anything else to the bare string the game writes for
/// plain text — `""` for a blank line.
fn component(s: &str) -> Value {
    if s.is_empty() {
        return Value::String(String::new());
    }
    if delvewright_dsl::l10n::untag(s).is_some() || delvewright_dsl::textstyle::lower(s).is_some() {
        return tr(s);
    }
    Value::String(s.to_string())
}

impl FaceValue {
    /// The face the game gives a sign nobody lettered: four empty lines in
    /// black, not glowing.
    pub(crate) fn blank() -> FaceValue {
        FaceValue {
            messages: std::array::from_fn(|_| Value::String(String::new())),
            color: Dye::Black,
            glowing: false,
        }
    }

    /// A declared face. `keybase` re-tags an untagged line with its l10n key
    /// (`<keybase>.<i>`), for a reader holding the English campaign rather
    /// than the build's tagged one; `None` takes each line as the campaign
    /// holds it.
    pub(crate) fn of(face: &SignFace, keybase: Option<&str>) -> FaceValue {
        let messages = std::array::from_fn(|i| match face.lines.get(i) {
            None => Value::String(String::new()),
            Some(l) if l.is_empty() => Value::String(String::new()),
            Some(l) => match keybase {
                Some(k) if delvewright_dsl::l10n::untag(l).is_none() => {
                    component(&delvewright_dsl::l10n::tag(&format!("{k}.{i}"), l))
                }
                _ => component(l),
            },
        });
        FaceValue {
            messages,
            color: face.dye(),
            glowing: face.glowing,
        }
    }

    /// `{messages:[…],color:"…",has_glowing_text:<0b|1b>}`.
    fn snbt(&self) -> String {
        format!(
            "{{messages:[{}],color:{},has_glowing_text:{}}}",
            self.messages
                .iter()
                .map(snbt_of)
                .collect::<Vec<_>>()
                .join(","),
            snbt_string(self.color.token()),
            if self.glowing { "1b" } else { "0b" }
        )
    }

    fn nbt(&self) -> crate::compiler::view::world::Nbt {
        use crate::compiler::view::world::Nbt;
        Nbt::Compound(vec![
            (
                "messages".to_string(),
                Nbt::list(self.messages.iter().map(Nbt::of_json).collect()),
            ),
            (
                "color".to_string(),
                Nbt::Str(self.color.token().to_string()),
            ),
            (
                "has_glowing_text".to_string(),
                Nbt::Byte(i8::from(self.glowing)),
            ),
        ])
    }
}

/// **What a lettered sign's block entity holds** (spec-0115 §4.1): both faces,
/// and `is_waxed: 1b`, always.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SignCompound {
    /// `front_text`.
    pub(crate) front: FaceValue,
    /// `back_text`; the blank face when the row declares none.
    pub(crate) back: FaceValue,
}

impl SignCompound {
    /// The compound a [`delvewright_dsl::Sign`] row declares. `ns` is the
    /// pack key prefix to re-tag an untagged line with (see
    /// [`FaceValue::of`]); `None` takes each line as the campaign holds it.
    pub(crate) fn declared(sign: &delvewright_dsl::Sign, ns: Option<&str>) -> SignCompound {
        let sl = delvewright_dsl::l10n::local_id(sign.id.as_str());
        let front_key = ns.map(|n| format!("{n}sign.{sl}.front"));
        let back_key = ns.map(|n| format!("{n}sign.{sl}.back"));
        SignCompound {
            front: FaceValue::of(&sign.front, front_key.as_deref()),
            back: sign
                .back
                .as_ref()
                .map_or_else(FaceValue::blank, |b| FaceValue::of(b, back_key.as_deref())),
        }
    }

    /// `{front_text:{…},back_text:{…},is_waxed:1b}`.
    pub(crate) fn snbt(&self) -> String {
        faces_snbt(Some(&self.front), Some(&self.back))
    }

    /// The block entity's own keys as NBT, in the order the SNBT states them.
    pub(crate) fn nbt(&self) -> Vec<(String, crate::compiler::view::world::Nbt)> {
        use crate::compiler::view::world::Nbt;
        vec![
            ("front_text".to_string(), self.front.nbt()),
            ("back_text".to_string(), self.back.nbt()),
            ("is_waxed".to_string(), Nbt::Byte(1)),
        ]
    }
}

/// The compound a `data merge block` writes: the faces given, and
/// `is_waxed:1b`. A face not given is left as the sign holds it, because
/// `data merge` keeps every key the compound does not name.
fn faces_snbt(front: Option<&FaceValue>, back: Option<&FaceValue>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(f) = front {
        parts.push(format!("front_text:{}", f.snbt()));
    }
    if let Some(b) = back {
        parts.push(format!("back_text:{}", b.snbt()));
    }
    parts.push("is_waxed:1b".to_string());
    format!("{{{}}}", parts.join(","))
}

/// The one shape a sign is lettered by: `data merge block <x> <y> <z> <compound>`.
fn merge_line(cell: [i32; 3], compound: &str) -> String {
    format!(
        "data merge block {} {} {} {compound}",
        cell[0], cell[1], cell[2]
    )
}

/// The declared row for a resolved sign.
fn row<'a>(plan: &'a Plan, id: &str) -> Option<&'a delvewright_dsl::Sign> {
    plan.campaign
        .quests
        .content
        .signs
        .iter()
        .find(|s| s.id.as_str() == id)
}

/// The function that letters sign `id` with its declared words:
/// `letter_<s>`. One body per declared sign, called from every place that
/// letters it, so the generated suite drives the very lines the delve runs.
pub(crate) fn letter_fn(id: &str) -> String {
    format!("letter_{}", plan::safe_local(id))
}

/// The function a `letter-sign` re-letters sign `id` through:
/// `reletter_<s>_<n>`, `n` the index of its stated faces among the distinct
/// ones the campaign states for that sign, in walk order — two identical
/// `letter-sign`s share one body.
pub(crate) fn reletter_fn(id: &str, n: usize) -> String {
    format!("reletter_{}_{n}", plan::safe_local(id))
}

/// Every distinct `(front, back)` the campaign's `letter-sign`s state for
/// `id`, in walk order — the index is [`reletter_fn`]'s `n`.
pub(crate) fn letterings<'a>(
    plan: &'a Plan,
    id: &str,
) -> Vec<(Option<&'a SignFace>, Option<&'a SignFace>)> {
    let mut out: Vec<(Option<&SignFace>, Option<&SignFace>)> = Vec::new();
    crate::compiler::plan::for_each_gate_effect(plan.campaign, &mut |_site, e| {
        if let delvewright_dsl::Verb::LetterSign { sign, front, back } = &e.verb
            && sign.as_str() == id
        {
            let pair = (front.as_ref(), back.as_ref());
            if !out.contains(&pair) {
                out.push(pair);
            }
        }
    });
    out
}

/// The sign functions (spec-0115 §4.2): per declared sign `letter_<s>`, and
/// per distinct `letter-sign` of it `reletter_<s>_<n>`. Empty for a campaign
/// that declares no sign.
pub(super) fn emit_sign_functions(plan: &Plan) -> Vec<(String, String)> {
    let mut fns = Vec::new();
    for s in &plan.signs {
        let Some(sign) = row(plan, &s.id) else {
            continue;
        };
        fns.push((
            letter_fn(&s.id),
            lines(&[merge_line(
                s.cell,
                &SignCompound::declared(sign, None).snbt(),
            )]),
        ));
        for (n, (front, back)) in letterings(plan, &s.id).into_iter().enumerate() {
            let f = front.map(|f| FaceValue::of(f, None));
            let b = back.map(|b| FaceValue::of(b, None));
            fns.push((
                reletter_fn(&s.id, n),
                lines(&[merge_line(s.cell, &faces_snbt(f.as_ref(), b.as_ref()))]),
            ));
        }
    }
    fns
}

/// **The lines that letter every declared sign a write puts a sign block
/// into** (spec-0115 §4.2): asked by every block writer after the line it
/// writes, with the inclusive box it wrote and the block. Empty when the
/// block is no sign or the box holds no lettered cell.
pub(super) fn after_write(plan: &Plan, lo: [i32; 3], hi: [i32; 3], block: &str) -> Vec<String> {
    if SignKind::of(block).is_none() {
        return Vec::new();
    }
    let ns = &plan.namespace;
    let inside = |c: [i32; 3]| (0..3).all(|k| lo[k].min(hi[k]) <= c[k] && c[k] <= lo[k].max(hi[k]));
    plan.signs
        .iter()
        .filter(|s| inside(s.cell) && row(plan, &s.id).is_some())
        .map(|s| format!("function {ns}:{}", letter_fn(&s.id)))
        .collect()
}

/// The `setup_finish` lines that letter each sign a piece's template places
/// (spec-0115 §4.2), in declaration order: the rows the build found standing
/// in the load world as the template laid it.
pub(super) fn sign_setup(plan: &Plan, gate: &crate::compiler::sign::SignGate) -> Vec<String> {
    let ns = &plan.namespace;
    plan.signs
        .iter()
        .filter(|s| gate.lettered_at_load(&s.id) && row(plan, &s.id).is_some())
        .map(|s| format!("function {ns}:{}", letter_fn(&s.id)))
        .collect()
}

/// A `letter-sign`'s line, at its position in its bundle: the call to the
/// body that writes the faces it states over the faces the sign had.
pub(super) fn emit_letter_sign(
    plan: &Plan,
    sign: &str,
    front: Option<&SignFace>,
    back: Option<&SignFace>,
    body: &mut Vec<String>,
) {
    if !plan.signs.iter().any(|s| s.id == sign) {
        return;
    }
    let Some(n) = letterings(plan, sign)
        .into_iter()
        .position(|p| p == (front, back))
    else {
        return;
    };
    body.push(format!(
        "function {}:{}",
        plan.namespace,
        reletter_fn(sign, n)
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(lines: &[&str]) -> SignFace {
        SignFace {
            lines: lines.iter().map(|s| s.to_string()).collect(),
            color: None,
            glowing: false,
        }
    }

    /// A declared face renders four components, a blank line as `""`, a
    /// tagged line as its `translate` component, the dye and the glow; the
    /// compound always carries `is_waxed:1b`.
    #[test]
    fn a_face_renders_as_the_game_holds_it() {
        let tagged = delvewright_dsl::l10n::tag("sign.x.front.0", "EVERY SURFACE");
        let f = SignFace {
            lines: vec![tagged, String::new(), "written once".into()],
            color: Some(Dye::Red),
            glowing: true,
        };
        let c = SignCompound {
            front: FaceValue::of(&f, None),
            back: FaceValue::blank(),
        };
        assert_eq!(
            c.snbt(),
            "{front_text:{messages:[{fallback:\"EVERY SURFACE\",translate:\"sign.x.front.0\"},\
             \"\",\"written once\",\"\"],color:\"red\",has_glowing_text:1b},back_text:\
             {messages:[\"\",\"\",\"\",\"\"],color:\"black\",has_glowing_text:0b},is_waxed:1b}"
        );
        // Retagging an English line names its key.
        let r = FaceValue::of(&face(&["NOTICE"]), Some("sign.notice.front"));
        assert_eq!(
            snbt_of(&r.messages[0]),
            "{fallback:\"NOTICE\",translate:\"sign.notice.front.0\"}"
        );
    }

    /// A partial merge names only the face it rewrites.
    #[test]
    fn a_letter_names_only_the_faces_it_states() {
        let f = FaceValue::of(&face(&["", "SOLD"]), None);
        assert_eq!(
            faces_snbt(Some(&f), None),
            "{front_text:{messages:[\"\",\"SOLD\",\"\",\"\"],color:\"black\",\
             has_glowing_text:0b},is_waxed:1b}"
        );
    }
}
