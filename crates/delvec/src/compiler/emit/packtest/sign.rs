use super::*;

use crate::compiler::emit::sign::{FaceValue, letter_fn, letterings, reletter_fn};
use delvewright_dsl::SignFace;

/// The assertion a face's first non-blank line makes: the component's key,
/// which is what the server holds (`{translate:"…"}`), or the literal under a
/// `--lang` build. `None` for a face with no visible line.
fn first_line(face: &FaceValue) -> Option<String> {
    face.messages.iter().find_map(|m| match m {
        Value::String(s) if s.is_empty() => None,
        Value::String(s) => Some(snbt_string(s)),
        Value::Object(o) => match o.get("translate") {
            Some(Value::String(k)) => Some(format!("{{translate:{}}}", snbt_string(k))),
            _ => o
                .get("text")
                .and_then(Value::as_str)
                .map(|t| format!("{{text:{}}}", snbt_string(t))),
        },
        _ => None,
    })
}

/// `{color:"…",has_glowing_text:<b>,messages:[<first line>]}` — the face's
/// dye, its glow and its first non-blank line.
fn face_pattern(face: &FaceValue) -> Option<String> {
    let first = first_line(face)?;
    Some(format!(
        "{{color:{},has_glowing_text:{},messages:[{first}]}}",
        snbt_string(face.color.token()),
        if face.glowing { "1b" } else { "0b" }
    ))
}

/// The block a template writes at the cell before lettering it: the block a
/// document puts there (a prop, a `set-block`, a `fill-region`); none for a
/// sign a piece's template lays, which already stands.
fn placed_block(gate: &crate::compiler::sign::SignGate, id: &str) -> Option<String> {
    let row = gate.rows.iter().find(|r| r.id == id)?;
    if gate.lettered_at_load(id) {
        return None;
    }
    row.block.clone()
}

/// Push `execute store success … if data block …` and its `assert` for
/// `pattern` at `cell` under the holder `#<tag>`.
fn assert_holds(b: &mut Vec<String>, cell: [i32; 3], tag: &str, pattern: &str) {
    b.push(format!(
        "execute store success score #{tag} dw.sys if data block {} {} {} {pattern}",
        cell[0], cell[1], cell[2]
    ));
    b.push(format!("assert score #{tag} dw.sys matches 1"));
}

fn declared_face(face: Option<&SignFace>) -> FaceValue {
    face.map_or_else(FaceValue::blank, |f| FaceValue::of(f, None))
}

/// The sign templates (spec-0115 §5.2), synchronous. Per declared sign,
/// `sign_<s>`: stand the sign the way its route does (a template's already
/// stands; a prop's or a write's block is written as the document writes it),
/// run `letter_<s>`, and assert `is_waxed:1b`, each declared face's dye and
/// glow, and its first non-blank line. Per distinct `letter-sign`,
/// `reletter_<s>_<n>`: the same, then the re-lettering body, asserting each
/// face it states and the face it leaves as declared. Emits nothing for a
/// campaign that declares no sign.
pub(super) fn emit_sign_packtests(
    plan: &Plan,
    gate: &crate::compiler::sign::SignGate,
    out: &mut BuildOutput,
) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    for s in &plan.signs {
        let Some(row) = plan
            .campaign
            .quests
            .content
            .signs
            .iter()
            .find(|r| r.id.as_str() == s.id)
        else {
            continue;
        };
        let safe = plan::safe_local(&s.id);
        let c = s.cell;
        let stand =
            placed_block(gate, &s.id).map(|b| format!("setblock {} {} {} {b}", c[0], c[1], c[2]));
        let front = declared_face(Some(&row.front));
        let back = declared_face(row.back.as_ref());
        let mut b = packtest_header(&format!(
            "{title}: sign `{}` says its words, waxed (spec-0115)",
            s.id
        ));
        b.extend(stand.clone());
        b.push(format!("function {ns}:{}", letter_fn(&s.id)));
        assert_holds(&mut b, c, &format!("sgn_{safe}_w"), "{is_waxed:1b}");
        for (side, face) in [("front_text", &front), ("back_text", &back)] {
            if let Some(p) = face_pattern(face) {
                assert_holds(
                    &mut b,
                    c,
                    &format!("sgn_{safe}_{}", &side[..1]),
                    &format!("{{{side}:{p}}}"),
                );
            }
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/test/sign_{safe}.mcfunction"),
            lines(&b).into_bytes(),
        );
        for (n, (sf, sb)) in letterings(plan, &s.id).into_iter().enumerate() {
            let mut t = packtest_header(&format!(
                "{title}: a beat re-letters sign `{}` (`letter-sign` {n}, spec-0115)",
                s.id
            ));
            t.extend(stand.clone());
            t.push(format!("function {ns}:{}", letter_fn(&s.id)));
            t.push(format!("function {ns}:{}", reletter_fn(&s.id, n)));
            let after_front = sf.map_or_else(|| front.clone(), |f| FaceValue::of(f, None));
            let after_back = sb.map_or_else(|| back.clone(), |f| FaceValue::of(f, None));
            for (side, face) in [("front_text", &after_front), ("back_text", &after_back)] {
                if let Some(p) = face_pattern(face) {
                    assert_holds(
                        &mut t,
                        c,
                        &format!("rsg_{safe}_{n}_{}", &side[..1]),
                        &format!("{{{side}:{p}}}"),
                    );
                }
            }
            out.insert(
                format!(
                    "packtest-datapack/data/{ns}/test/{}.mcfunction",
                    reletter_fn(&s.id, n)
                ),
                lines(&t).into_bytes(),
            );
        }
    }
}

/// The sign claims (`DW0811`): every declared sign's `letter_<s>` body and
/// every distinct `letter-sign`'s `reletter_<s>_<n>` body is driven by the
/// suite.
pub(super) fn sign_watch_claims(plan: &Plan) -> Vec<crate::compiler::watch::Claim> {
    let signs: BTreeSet<String> = plan.signs.iter().map(|s| plan::safe_local(&s.id)).collect();
    let letters: BTreeSet<String> = plan
        .signs
        .iter()
        .flat_map(|s| {
            (0..letterings(plan, &s.id).len())
                .map(|n| format!("{}_{n}", plan::safe_local(&s.id)))
                .collect::<Vec<_>>()
        })
        .collect();
    vec![
        crate::compiler::watch::Claim {
            mechanic: "sign",
            families: vec!["letter_".to_string()],
            declared: signs,
        },
        crate::compiler::watch::Claim {
            mechanic: "letter-sign",
            families: vec!["reletter_".to_string()],
            declared: letters,
        },
    ]
}
