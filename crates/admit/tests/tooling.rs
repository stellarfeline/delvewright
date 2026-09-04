//! Admission tooling: structure round-trip determinism, socket carving, and the
//! static light probe.

use delvewright_admit::fixtures;
use delvewright_admit::jigsaw;
use delvewright_admit::light::{self, Zone};
use delvewright_admit::meta::{self, License, LightingProfile, PrefabMeta};
use delvewright_admit::socket::{self, SocketDecl};
use delvewright_admit::structure::{Structure, roundtrip};

fn license() -> License {
    License {
        source: "original".into(),
        spdx: "CC0-1.0".into(),
        note: "test".into(),
        provenance: "test".into(),
        generated_by: None,
    }
}

#[test]
fn structure_write_is_deterministic() {
    let s = fixtures::clean_room();
    assert_eq!(s.write(), s.write(), "same structure -> same bytes");
    // round-trip preserves size + palette content.
    let back = roundtrip(&s).unwrap();
    assert_eq!(back.size, s.size);
    assert_eq!(back.block_names(), s.block_names());
    // re-serializing the round-tripped structure is byte-stable too.
    assert_eq!(back.write(), back.write());
}

#[test]
fn socket_carving_places_jigsaw_and_carves_opening() {
    let mut s = fixtures::clean_room(); // 7x5x7, wall at z=0
    let mut meta = PrefabMeta::skeleton("clean", s.size, s.data_version, "test", license());
    // North wall socket at bottom-centre of the opening: x=3, y=1, z=0.
    let decl = SocketDecl::new([3, 1, 0], "north");
    socket::carve(&mut s, &mut meta, &decl).unwrap();

    // jigsaw marker at the socket cell.
    let entry = s.entry_at([3, 1, 0]).unwrap();
    assert_eq!(entry.name, "minecraft:jigsaw");
    assert_eq!(
        entry.properties.get("orientation").map(String::as_str),
        Some("north_up")
    );
    // the block entity is carried.
    assert!(s.block_at([3, 1, 0]).unwrap().nbt.is_some());
    // opening carved to air (a flanking cell of the 3x3, not the jigsaw cell).
    assert_eq!(s.entry_at([2, 1, 0]).unwrap().name, "minecraft:air");
    assert_eq!(s.entry_at([3, 3, 0]).unwrap().name, "minecraft:air");
    // connector recorded once.
    assert_eq!(meta.connectors.len(), 1);
    assert_eq!(meta.connectors[0].facing, "north");
    assert_eq!(meta.connectors[0].local_pos, [3, 1, 0]);

    // idempotent metadata; carving is deterministic.
    let bytes1 = s.write();
    let mut s2 = fixtures::clean_room();
    let mut meta2 = PrefabMeta::skeleton("clean", s2.size, s2.data_version, "test", license());
    socket::carve(&mut s2, &mut meta2, &decl).unwrap();
    assert_eq!(bytes1, s2.write(), "carving is deterministic");

    // the carved piece re-parses.
    let reparsed = Structure::read(&bytes1).unwrap();
    assert_eq!(
        reparsed.entry_at([3, 1, 0]).unwrap().name,
        "minecraft:jigsaw"
    );
}

#[test]
fn resolve_jigsaw_replaces_with_final_state_and_prunes() {
    let mut s = fixtures::foreign_jigsaw_piece();
    assert!(s.block_names().contains("minecraft:jigsaw"));

    let resolved = jigsaw::resolve(&mut s);
    assert_eq!(resolved.len(), 3, "three jigsaw markers resolved");

    // plain final_state -> that block.
    assert_eq!(
        s.entry_at([2, 1, 2]).unwrap().name,
        "minecraft:stone_bricks"
    );
    // final_state with block-state properties is parsed.
    let stairs = s.entry_at([4, 1, 4]).unwrap();
    assert_eq!(stairs.name, "minecraft:oak_stairs");
    assert_eq!(
        stairs.properties.get("facing").map(String::as_str),
        Some("east")
    );
    assert_eq!(
        stairs.properties.get("half").map(String::as_str),
        Some("bottom")
    );
    // no final_state -> air fallback.
    assert_eq!(s.entry_at([2, 1, 4]).unwrap().name, "minecraft:air");

    // the jigsaw block entities are gone, and the palette no longer lists jigsaw.
    assert!(s.block_at([2, 1, 2]).unwrap().nbt.is_none());
    assert!(
        !s.block_names().contains("minecraft:jigsaw"),
        "pruned the orphaned jigsaw palette entry"
    );

    // the resolved piece round-trips and is byte-stable.
    let bytes = s.write();
    let back = Structure::read(&bytes).unwrap();
    assert_eq!(back.block_names(), s.block_names());
    assert_eq!(bytes, back.write());

    // idempotent: a second resolve is a no-op.
    let again = jigsaw::resolve(&mut s);
    assert!(again.is_empty());
}

#[test]
fn socket_out_of_bounds_errors() {
    let mut s = fixtures::clean_room();
    let mut meta = PrefabMeta::skeleton("clean", s.size, s.data_version, "test", license());
    let decl = SocketDecl::new([99, 1, 0], "north");
    assert!(socket::carve(&mut s, &mut meta, &decl).is_err());
}

#[test]
fn light_probe_calls_lit_room_lit_and_dark_room_dark() {
    let room = fixtures::clean_room();
    let lit = light::probe(
        &Zone::single(&room),
        light::DEFAULT_DARK_THRESHOLD,
        light::SkyClaim::OpenAir,
    );
    assert_eq!(
        lit.profile, "lit",
        "ceiling glowstone -> lit ({:?})",
        lit.measured_min_light
    );
    assert!(
        lit.measured_cells > 0,
        "the probe states what it bound to, and a zero binding is a finding"
    );
    assert!(lit.entry_cells > 0, "the doorway is the way in");
    assert!(lit.measured_min_light.unwrap() >= light::DEFAULT_DARK_THRESHOLD);

    let unlit = fixtures::dark_room();
    let dark = light::probe(
        &Zone::single(&unlit),
        light::DEFAULT_DARK_THRESHOLD,
        light::SkyClaim::OpenAir,
    );
    assert_eq!(dark.profile, "dark", "no light source -> dark");
    assert!(dark.is_dark());
    assert_eq!(dark.measured_min_light, Some(0));
}

/// **A roofed-but-open building is not a sealed box.**
///
/// The pavilion has no light source in it and a roof over every cell of its
/// floor, so a model with no sky term measures it at zero everywhere and calls
/// it `dark` — at exit 0, with a plausible number and a plausible verdict.
///
/// The perturbation is chosen so that only the sky term can move it: there is no
/// emitter in the fixture at all, so every level the probe reports is sky light
/// and nothing else in the pipeline can supply one. A block-light change, an
/// opacity change or a binding change all leave this at zero.
#[test]
fn a_roofed_but_open_pavilion_is_not_measured_as_a_sealed_box() {
    let p = light::probe(
        &Zone::single(&fixtures::pavilion()),
        light::DEFAULT_DARK_THRESHOLD,
        light::SkyClaim::OpenAir,
    );
    assert!(
        p.measured_cells > 0,
        "the pavilion has a floor a body can walk onto"
    );
    assert!(
        p.measured_min_light.unwrap() > 0,
        "an unlit pavilion under the night sky is not pitch black: the sky reaches \
         under its roof from every side. min={:?} over {} cell(s)",
        p.measured_min_light,
        p.measured_cells
    );
    assert!(
        p.min_light_daylight.unwrap() >= light::DEFAULT_DARK_THRESHOLD,
        "and by day it is lit, which is the sentence a reviewer needs: min={:?}",
        p.min_light_daylight
    );
    // The verdict states the sky it was taken at, or it states nothing.
    assert_eq!(p.sky_light, light::night_sky());
    assert_eq!(p.daylight_sky_light, light::daylight_sky());
    assert!(p.daylight_sky_light > p.sky_light);
}

/// The same defect where it changes the **verdict**, not only the number: a
/// colonnade one bay deep is `lit` under the vanilla night sky and `dark` the
/// moment its open side is modelled as a wall.
#[test]
fn a_colonnade_one_bay_deep_is_lit_by_the_sky_alone() {
    let p = light::probe(
        &Zone::single(&fixtures::colonnade()),
        light::DEFAULT_DARK_THRESHOLD,
        light::SkyClaim::OpenAir,
    );
    assert!(p.measured_cells > 0, "the walk is walkable");
    assert_eq!(
        p.profile, "lit",
        "every cell of the walk is one step from open air (min={:?} over {} cell(s))",
        p.measured_min_light, p.measured_cells
    );
}

/// **An open-air piece can carry a measured profile.** A piece with no roofed
/// cell anywhere used to bind zero and fail: there was no honest way to grade a
/// courtyard, a jetty or a meadow, because the only thing the model could say
/// about an open-sky cell was that no lantern reached it.
#[test]
fn an_open_air_piece_is_measurable() {
    // A bare floor slab: nothing over it anywhere.
    let mut cells = Vec::new();
    for x in 0..5 {
        for z in 0..5 {
            cells.push((
                [x, 0, z],
                delvewright_admit::structure::PaletteEntry::simple("minecraft:stone_bricks"),
                None,
            ));
        }
    }
    let yard = delvewright_admit::structure::synth([5, 3, 5], &cells);
    let p = light::probe(
        &Zone::single(&yard),
        light::DEFAULT_DARK_THRESHOLD,
        light::SkyClaim::OpenAir,
    );
    assert!(
        !p.is_unbound(),
        "an open yard has somewhere to stand and something to measure: {}",
        p.unbound_reason()
    );
    assert_eq!(
        p.measured_min_light,
        Some(light::night_sky()),
        "open ground measures the night sky exactly"
    );
}

/// A spatial contract declaring one space with `envelope`, over the whole piece.
fn contract_with(envelopes: &[(&str, &str)]) -> delvewright_dsl::prefab::SpatialContract {
    delvewright_dsl::prefab::SpatialContract {
        entry: envelopes
            .first()
            .map(|(n, _)| (*n).to_string())
            .unwrap_or_default(),
        spaces: envelopes
            .iter()
            .map(|(name, env)| {
                (
                    (*name).to_string(),
                    delvewright_dsl::prefab::ContractSpace {
                        envelope: (*env).to_string(),
                        boxes: vec![delvewright_dsl::prefab::Region {
                            from: [0, 1, 0],
                            to: [4, 3, 4],
                        }],
                    },
                )
            })
            .collect(),
        no_body: Default::default(),
        edges: Vec::new(),
        faces: Vec::new(),
        no_body_majority_ack: None,
    }
}

/// **The sky a piece is probed under is the piece's own claim** (spec-0036's
/// `envelope`, read by `light::SkyClaim`).
///
/// The four answers, and the two that must stay `OpenAir` are the ones that keep
/// the library still: a contractless piece — which is every one of the 36 prefab
/// documents on the content library — and a contract that declares no space at
/// all are not claims of enclosure, and a piece answering `Enclosed` for either
/// would silently re-grade a population nobody touched.
#[test]
fn the_sky_a_piece_is_probed_under_is_read_off_its_own_contract() {
    assert_eq!(light::SkyClaim::of(None), light::SkyClaim::OpenAir);
    assert_eq!(
        light::SkyClaim::of(Some(&contract_with(&[]))),
        light::SkyClaim::OpenAir,
        "a contract with no space is not a claim of enclosure"
    );
    assert_eq!(
        light::SkyClaim::of(Some(&contract_with(&[("room", "enclosed")]))),
        light::SkyClaim::Enclosed
    );
    assert_eq!(
        light::SkyClaim::of(Some(&contract_with(&[("yard", "open")]))),
        light::SkyClaim::OpenAir
    );
    assert_eq!(
        light::SkyClaim::of(Some(&contract_with(&[("well", "open_top")]))),
        light::SkyClaim::OpenAir
    );
    assert_eq!(
        light::SkyClaim::of(Some(&contract_with(&[
            ("room", "enclosed"),
            ("yard", "open"),
        ]))),
        light::SkyClaim::OpenAir,
        "one open space is enough: the sky reaches this piece by its own design"
    );
    assert!(light::SkyClaim::OpenAir.admits_sky());
    assert!(!light::SkyClaim::Enclosed.admits_sky());
}

/// **A profile the piece can have in no world it will be placed in.**
///
/// A `detail-plan` piece stands inside the box a site plan gave it: its frame is
/// the play space plus one floor course (spec-0050 §3), the roof over it is the
/// whole's, and it never meets the sky. Probed as if it stood in open air, an
/// emitterless one measures the night floor at every cell and is written `lit` —
/// while `DW0210` reds the same piece the moment it is built. Two instruments,
/// one piece, opposite answers.
///
/// The colonnade is the fixture that separates them, and it separates ONLY them:
/// it holds no emitter at all, so every level the probe reports is sky light and
/// nothing else in the pipeline can supply one — and one bay deep it sits exactly
/// at the threshold, so the claim moves the VERDICT and not merely a number. Same
/// bytes, same binding, same threshold.
#[test]
fn an_enclosed_piece_is_not_credited_with_a_sky_it_will_never_stand_under() {
    let piece = fixtures::colonnade();

    let open = light::probe(
        &Zone::single(&piece),
        light::DEFAULT_DARK_THRESHOLD,
        light::SkyClaim::OpenAir,
    );
    assert_eq!(
        open.profile, "lit",
        "the open-air answer is unchanged, and it is right for a piece that stands \
         in the open: min={:?}",
        open.measured_min_light
    );

    let closed = light::probe(
        &Zone::single(&piece),
        light::DEFAULT_DARK_THRESHOLD,
        light::SkyClaim::Enclosed,
    );
    assert_eq!(
        closed.measured_cells, open.measured_cells,
        "the binding does not move: this is one piece measured twice, and a changed \
         binding would mean the two verdicts are about different sets of cells"
    );
    assert_eq!(
        closed.profile, "dark",
        "with no sky, an emitterless piece brings no light of its own: min={:?}",
        closed.measured_min_light
    );
    assert_eq!(closed.sky_light, 0);
    assert_eq!(
        closed.daylight_sky_light, 0,
        "and there is no brighter hour to fall back on"
    );
    assert_eq!(
        closed.measured_min_light, closed.min_light_daylight,
        "so the two figures are one figure"
    );

    // The change reaches exactly the borrowed verdicts. A piece carrying its own
    // light is `lit` either way, which is what keeps this from being a blanket
    // re-grade of every enclosed room in the library.
    let room = fixtures::clean_room();
    for claim in [light::SkyClaim::OpenAir, light::SkyClaim::Enclosed] {
        let p = light::probe(&Zone::single(&room), light::DEFAULT_DARK_THRESHOLD, claim);
        assert_eq!(
            p.profile, "lit",
            "a room with a glowstone in its ceiling is lit by the glowstone: {claim:?}"
        );
    }
}

/// The record a closed measurement writes says which sky it was taken under, and
/// does not repeat the open-air sentence it is the correction to.
///
/// A figure filed under the wrong sky is worse than no figure: it reads as a
/// measurement and cannot be re-derived by anyone who believes it.
#[test]
fn the_written_method_states_the_sky_the_figure_was_taken_under() {
    let s = fixtures::colonnade();
    let probe = light::probe(
        &Zone::single(&s),
        light::DEFAULT_DARK_THRESHOLD,
        light::SkyClaim::Enclosed,
    );
    let mut doc = PrefabMeta::skeleton("colonnade", s.size, s.data_version, "test", license());
    meta::set_lighting_from_probe(&mut doc, &probe);
    let method = doc
        .lighting
        .as_ref()
        .and_then(|l| l.method.clone())
        .expect("a probe writes a lighting block with a method");
    assert!(
        method.contains("`enclosed`"),
        "the record names WHY no sky was applied: {method}"
    );
    assert!(
        !method.contains("stands in open air"),
        "and does not also claim the opposite: {method}"
    );
    assert!(
        method.contains("effective sky 0"),
        "and states the sky as a number, so the figure can be re-derived: {method}"
    );
}

#[test]
fn light_probe_writes_estimate_method_into_metadata() {
    let s = fixtures::clean_room();
    let probe = light::probe(
        &Zone::single(&s),
        light::DEFAULT_DARK_THRESHOLD,
        light::SkyClaim::OpenAir,
    );
    let mut doc = PrefabMeta::skeleton("clean", s.size, s.data_version, "test", license());
    meta::set_lighting_from_probe(&mut doc, &probe);
    let lighting = doc
        .lighting
        .as_ref()
        .expect("a probe writes a lighting block")
        .clone();
    assert_eq!(lighting.profile, LightingProfile::Lit);
    // honest: the method string marks this as a static estimate, not a live probe.
    assert!(lighting.method.as_deref().unwrap().contains("static"));
    assert!(
        lighting
            .method
            .as_deref()
            .unwrap()
            .to_lowercase()
            .contains("not a live")
    );
}

/// **`delve-admit` must be able to read what the generators write.**
///
/// The grammar back end (spec-0027 §2) exports `{"profile": "unmeasured"}` and
/// no measurement, because a piece that has not been probed carries no probe
/// result — which is exactly what `delvewright_dsl::registry::Lighting` demands
/// and what the compiler's `PrefabRegistry` accepts. This crate's own metadata
/// copy required all four lighting fields, so every admission subcommand
/// (`socket`, `anchor`, `lighting`, `catalog`) refused a grammar-exported prefab
/// at `DW0732` before it did anything: the admission half of the pipeline was
/// closed to the generated half, and nothing said so.
#[test]
fn an_unmeasured_lighting_block_parses_the_way_the_generators_write_it() {
    let json = r#"{
      "prefab_id": "prefab/arcade-undercroft",
      "structure": {
        "file": "arcade-undercroft.nbt",
        "id": "arcade-undercroft",
        "size": [9, 6, 21],
        "data_version": 4671,
        "generator": "crates/grammar"
      },
      "anchors": { "anchor/undercroft-floor": { "pos": [4, 1, 10], "facing": "north" } },
      "lighting": { "profile": "unmeasured" },
      "license": {
        "source": "original",
        "spdx": "GPL-3.0-or-later",
        "note": "n",
        "provenance": "p"
      }
    }"#;
    let meta = PrefabMeta::from_json(json).expect("a grammar prefab's metadata must parse");
    let lighting = meta.lighting.clone().expect("the block is declared");
    assert_eq!(lighting.profile, LightingProfile::Unmeasured);
    assert_eq!(lighting.measured_min_light, None);
    assert_eq!(lighting.measured, None);
    assert!(meta.connectors.is_empty(), "the export emits no connectors");
}

/// ...and a probe still writes a measurement, because the DSL refuses a
/// `lit`/`dim`/`dark` profile that does not carry one. The optional fields are a
/// widening of what can be READ, never of what a measured claim must say.
#[test]
fn a_probed_profile_still_carries_its_measurement() {
    let meta = PrefabMeta::skeleton(
        "probed",
        [5, 5, 5],
        4671,
        "test",
        License {
            source: "original".into(),
            spdx: "GPL-3.0-or-later".into(),
            note: "n".into(),
            provenance: "p".into(),
            generated_by: None,
        },
    );
    assert_eq!(
        meta.lighting.as_ref().map(|l| l.profile),
        Some(LightingProfile::Unmeasured),
        "a skeleton has not been probed and must say so in a profile the DSL has"
    );
    let unlit = fixtures::dark_room();
    let probe = light::probe(
        &Zone::single(&unlit),
        light::DEFAULT_DARK_THRESHOLD,
        light::SkyClaim::OpenAir,
    );
    let mut doc = meta;
    meta::set_lighting_from_probe(&mut doc, &probe);
    let lighting = doc
        .lighting
        .as_ref()
        .expect("a probe writes a lighting block");
    assert!(lighting.measured_min_light.is_some());
    assert!(lighting.measured.is_some());
}
