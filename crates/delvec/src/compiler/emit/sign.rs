//! Sign texts (spec-0128 §8).

use super::*;

/// `setup_finish` commands for sign texts: write each declared side into the
/// sign the piece placed with `data merge block … {front_text:…}`, through
/// the placements ledger's one writer ([`crate::compiler::placed::Placed`]),
/// so the review worlds hold the text the server holds.
///
/// A side is written whole — vanilla's own defaults for its colour and glow
/// beside its four messages, a line the author left out written as an empty
/// component — so the compound written is the compound the server saves.
/// Each line is a text component through the one SNBT renderer
/// ([`snbt_component`]): a translated line is a `translate` with its English
/// `fallback`. A side the entry does not declare is left as the piece left it.
/// The sign itself is never placed here; `DW1075` has already proven one is
/// there.
pub(super) fn sign_setup(signs: &[crate::compiler::plan::SignPlan]) -> Vec<String> {
    signs
        .iter()
        .map(|s| {
            let side = |lines: &[String]| {
                let messages: Vec<String> = (0..delvewright_dsl::SIGN_LINES)
                    .map(|i| snbt_component(lines.get(i).map_or("", String::as_str)))
                    .collect();
                format!(
                    "{{color:\"black\",has_glowing_text:0b,messages:[{}]}}",
                    messages.join(",")
                )
            };
            let mut keys = vec![format!("front_text:{}", side(&s.front))];
            if !s.back.is_empty() {
                keys.push(format!("back_text:{}", side(&s.back)));
            }
            crate::compiler::placed::Placed::data(
                crate::compiler::placed::Firing::Setup,
                s.cell,
                &format!("{{{}}}", keys.join(",")),
            )
            .command()
        })
        .collect()
}

#[cfg(test)]
mod sign_emit_tests {
    use super::*;
    use crate::compiler::plan::SignPlan;

    /// One line per sign, both sides whole, a blank line an empty component,
    /// a translated line a `translate` with its fallback; a back left out is
    /// not written; the ledger reads the line back as the row it was made from.
    #[test]
    fn a_sign_text_is_one_data_merge_of_whole_sides() {
        let tagged = delvewright_dsl::l10n::tag("sign.bakery.front.1", "Bakery");
        let out = sign_setup(&[
            SignPlan {
                id: "sign/bakery".into(),
                anchor: "anchor/sign-bakery".into(),
                cell: [3, 70, -2],
                front: vec![String::new(), tagged],
                back: vec!["Bread".into()],
            },
            SignPlan {
                id: "sign/plain".into(),
                anchor: "anchor/plain".into(),
                cell: [0, 64, 0],
                front: vec!["Hi".into()],
                back: Vec::new(),
            },
        ]);
        assert_eq!(out.len(), 2);
        assert!(
            out[0].starts_with(
                "data merge block 3 70 -2 {front_text:{color:\"black\",has_glowing_text:0b,messages:[\"\",{fallback:\"Bakery\",translate:"
            ),
            "{}",
            out[0]
        );
        assert!(
            out[0].ends_with(
                "\"\",\"\"]},back_text:{color:\"black\",has_glowing_text:0b,messages:[\"Bread\",\"\",\"\",\"\"]}}"
            ),
            "{}",
            out[0]
        );
        assert_eq!(
            out[1],
            "data merge block 0 64 0 {front_text:{color:\"black\",has_glowing_text:0b,messages:[\"Hi\",\"\",\"\",\"\"]}}"
        );
        let row = crate::compiler::placed::Placed::of_block_line(
            crate::compiler::placed::Firing::Setup,
            &out[1],
        )
        .expect("the ledger reads a data merge");
        assert_eq!(row.command(), out[1]);
        assert!(
            crate::compiler::placed::parse_snbt(match &row.object {
                crate::compiler::placed::PlacedObject::Data { nbt, .. } => nbt,
                _ => unreachable!(),
            })
            .is_ok()
        );
    }
}
