//! **Two delves must not put the same texture id on different faces.**
//!
//! A skin ships twice: as `assets/delvewright/textures/npc/<id>.png` inside the
//! delve's resource pack, and as `delvewright:npc/<id>` inside the `summon` that
//! stands the mannequin up. Both ids land in a space the CLIENT owns — enabled
//! packs merge per path and stay enabled across servers and worlds — so a face
//! baked under a bare `keeper` is the face every other delve's `keeper` wears.
//! The same mechanism, and the same consequence, as two delves defining
//! `world.title` (`i18n_v2::two_delves_share_no_translate_key`, the string half).
//!
//! Run through the SHIPPED path (`delvec build`), because the namespace is stamped
//! there, on the campaign, before an emitter can read a texture id — a test that
//! assembled a `Plan` by hand would prove nothing about the delivery.
//!
//! Every coverage set states its binding count (CLAUDE.md): a gate here that
//! examined zero textures fails rather than passes.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use delvewright_dsl::DSL_VERSION;
use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A delve whose stage-2 npc and stage-5 actor each wear a skin, under the
/// campaign id `id`, with `payload` as the bytes of **both** PNGs.
///
/// The two delves this test builds agree on every `texture_id` and differ in
/// their id and in their face bytes — the collision exactly, as a creator would
/// meet it: `keeper` is an ordinary name for a keeper.
fn delve(dirname: &str, id: &str, payload: &[u8]) -> PathBuf {
    let dir = tmp(dirname);
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    common::patch_file(&dir.join("npcs.json"), |d| {
        d["dsl_version"] = serde_json::json!(DSL_VERSION);
        d["content"]["npcs"][0]["skin"] =
            serde_json::json!({ "texture_id": "keeper", "model": "wide" });
    });
    common::patch_file(&dir.join("quests.json"), |d| {
        d["dsl_version"] = serde_json::json!(DSL_VERSION);
        common::objective_effects(d, 0, "obj/talk").push(serde_json::json!({
            "type": "spawn-actor", "actor": "actor/giant"
        }));
        d["content"]["actors"] = serde_json::json!([
            { "id": "actor/giant", "entity": "minecraft:zombie", "name": "The Sleeper",
              "anchor": "anchor/exit", "facing": "east",
              "skin": { "texture_id": "giant-idle", "model": "slim" } }
        ]);
    });
    common::declare_story_dir(&dir);
    // The campaign id is what the namespace is grained by, so it is set on every
    // document that carries one — the stage docs are cross-checked against each
    // other and a build refuses a mismatch.
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        common::patch_file(&path, |d| {
            if d.get("campaign_id").is_some() {
                d["campaign_id"] = serde_json::json!(id);
            }
        });
    }
    let skins = dir.join("skins");
    std::fs::create_dir_all(&skins).unwrap();
    // Distinctive bytes: the archive is STORE-method, so finding them proves the
    // FILE was baked, and finding the OTHER delve's proves whose face it is.
    std::fs::write(skins.join("keeper.png"), payload).unwrap();
    std::fs::write(skins.join("giant-idle.png"), payload).unwrap();
    dir
}

/// `delvec build <dir> -o <out>` with the shipped prefab library, panicking with
/// the compiler's own diagnostics so a red reads like a build log.
fn build(dir: &Path, out_name: &str) -> PathBuf {
    let out = tmp(out_name);
    let pf = common::prefabs_dir();
    let r = Command::new(BIN)
        .args([
            "build",
            dir.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
        ])
        .output()
        .expect("run delvec");
    assert!(
        r.status.success(),
        "build {}: {}{}",
        dir.display(),
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    out
}

/// Every skin texture path the built resource pack carries. The zip is written
/// with the STORE method (`resourcepack.rs`), so member names appear verbatim and
/// no zip crate is needed — the technique `resourcepack`'s own `pack.mcmeta` test
/// uses.
fn baked_texture_paths(out: &Path) -> BTreeSet<String> {
    const PREFIX: &str = "assets/delvewright/textures/npc/";
    let bytes = std::fs::read(out.join("resourcepack.zip")).expect("a skinned delve ships a pack");
    let text = String::from_utf8_lossy(&bytes).to_string();
    let mut found = BTreeSet::new();
    let mut at = 0usize;
    while let Some(i) = text[at..].find(PREFIX) {
        let start = at + i;
        let end = text[start..]
            .find(".png")
            .expect("a texture member ends .png")
            + start
            + 4;
        found.insert(text[start..end].to_string());
        at = end;
    }
    found
}

/// Every texture a summon in the built tree points at (`delvewright:npc/<id>`).
fn summoned_texture_refs(out: &Path) -> BTreeSet<String> {
    const PREFIX: &str = "delvewright:npc/";
    let mut found = BTreeSet::new();
    let mut stack = vec![out.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let mut at = 0usize;
            while let Some(i) = text[at..].find(PREFIX) {
                let start = at + i;
                let end = text[start..].find('"').expect("a texture ref is quoted") + start;
                found.insert(text[start..end].to_string());
                at = end;
            }
        }
    }
    found
}

/// The texture directory the delve in `dir` writes into, read from the campaign's
/// own `world.json` rather than written as a literal — the test states the rule,
/// not a copy of one fixture's id.
///
/// **It must NAME the delve.** Asking the authority is what keeps the assertions
/// honest about the rule, and it is also what would let an empty namespace make a
/// "every texture is under this delve's directory" assertion true of every texture
/// there is. So the answer is checked here, once, for the property the whole file
/// rests on: a directory that does not carry the campaign id separates nothing.
fn texture_dir(dir: &Path) -> String {
    let doc: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("world.json")).unwrap()).unwrap();
    let id = doc["campaign_id"].as_str().expect("a campaign id");
    let out = delvewright_dsl::pack_texture_dir(id);
    assert!(
        out.contains(id) && out.ends_with('/'),
        "`pack_texture_dir({id})` = `{out}`, which does not name the delve — every texture would \
         land in a directory every other delve also writes into"
    );
    out
}

/// **The defect, as a property.** Two delves that cast the same-named characters
/// share no texture path and no texture reference; strip each delve's own
/// directory and what is left is the SAME two ids — which is both the collision
/// and the perturbation this test would catch.
#[test]
fn two_delves_share_no_skin_texture() {
    let a_dir = delve("skin-ns-alpha", "delve-alpha", b"ALPHA-FACE-PAYLOAD");
    let b_dir = delve("skin-ns-beta", "delve-beta", b"BETA-FACE-PAYLOAD");
    let (a_out, b_out) = (
        build(&a_dir, "skin-ns-alpha-out"),
        build(&b_dir, "skin-ns-beta-out"),
    );

    let (pa, pb) = (baked_texture_paths(&a_out), baked_texture_paths(&b_out));
    let (ra, rb) = (summoned_texture_refs(&a_out), summoned_texture_refs(&b_out));
    assert!(
        !pa.is_empty() && !pb.is_empty(),
        "no baked textures examined"
    );
    assert!(!ra.is_empty() && !rb.is_empty(), "no texture refs examined");
    println!(
        "texture binding: {} / {} baked, {} / {} referenced",
        pa.len(),
        pb.len(),
        ra.len(),
        rb.len()
    );

    let shared_paths: Vec<&String> = pa.intersection(&pb).collect();
    assert!(
        shared_paths.is_empty(),
        "two delves bake {} texture(s) to the same path — whichever pack a client applied last \
         is the face BOTH delves wear: {shared_paths:#?}",
        shared_paths.len()
    );
    let shared_refs: Vec<&String> = ra.intersection(&rb).collect();
    assert!(
        shared_refs.is_empty(),
        "two delves summon mannequins pointing at {} texture(s) in common: {shared_refs:#?}",
        shared_refs.len()
    );

    // They are the same two skins, separated only by the namespace. Without it
    // these campaigns would both bake `keeper` and `giant-idle`, over different
    // bytes, which is the defect verbatim.
    let strip = |ids: &BTreeSet<String>, prefix: &str, dir: &str| -> BTreeSet<String> {
        ids.iter()
            .map(|id| {
                let rest = id
                    .strip_prefix(prefix)
                    .unwrap_or_else(|| panic!("`{id}` is not a `{prefix}` texture"));
                rest.strip_prefix(dir)
                    .unwrap_or_else(|| panic!("`{id}` is outside the delve's directory `{dir}`"))
                    .trim_end_matches(".png")
                    .to_string()
            })
            .collect()
    };
    let (da, db) = (texture_dir(&a_dir), texture_dir(&b_dir));
    let baked_a = strip(&pa, "assets/delvewright/textures/npc/", &da);
    let baked_b = strip(&pb, "assets/delvewright/textures/npc/", &db);
    assert_eq!(
        baked_a, baked_b,
        "the two delves are the same skins under two directories"
    );
    assert_eq!(
        baked_a,
        BTreeSet::from(["giant-idle".to_string(), "keeper".to_string()]),
        "the creator's own ids are what the namespace is stripped back to — one skinned body \
         of each class"
    );

    // What a summon points at is what the pack ships, per delve: a reference with
    // no member behind it renders as the client's default skin.
    for (label, refs, paths, dir) in [("alpha", &ra, &pa, &da), ("beta", &rb, &pb, &db)] {
        assert_eq!(
            strip(refs, "delvewright:npc/", dir),
            strip(paths, "assets/delvewright/textures/npc/", dir),
            "{label}: the textures its mannequins point at and the textures its pack bakes are \
             not the same set"
        );
    }

    // …and the faces really do differ, so the collision above had something to
    // collide: each pack carries its own bytes and neither carries the other's.
    let packs: BTreeMap<&str, Vec<u8>> = [
        (
            "alpha",
            std::fs::read(a_out.join("resourcepack.zip")).unwrap(),
        ),
        (
            "beta",
            std::fs::read(b_out.join("resourcepack.zip")).unwrap(),
        ),
    ]
    .into_iter()
    .collect();
    for (label, mine, theirs) in [
        (
            "alpha",
            &b"ALPHA-FACE-PAYLOAD"[..],
            &b"BETA-FACE-PAYLOAD"[..],
        ),
        (
            "beta",
            &b"BETA-FACE-PAYLOAD"[..],
            &b"ALPHA-FACE-PAYLOAD"[..],
        ),
    ] {
        let zip = &packs[label];
        assert!(
            zip.windows(mine.len()).any(|w| w == mine),
            "{label}'s pack does not carry its own face bytes"
        );
        assert!(
            !zip.windows(theirs.len()).any(|w| w == theirs),
            "{label}'s pack carries the other delve's face bytes"
        );
    }
}

/// The other half of the same property, over **one** delve: every texture it
/// references is its own. A reference outside the delve's directory is a face any
/// other delve's pack can answer — and a bare `delvewright:npc/keeper` is exactly
/// that, which is how this shipped.
#[test]
fn every_texture_a_delve_references_is_its_own() {
    let dir = delve("skin-ns-own", "delve-solo", b"SOLO-FACE-PAYLOAD");
    let out = build(&dir, "skin-ns-own-out");
    let prefix = format!("delvewright:npc/{}", texture_dir(&dir));
    let refs = summoned_texture_refs(&out);
    let baked = baked_texture_paths(&out);
    assert!(!refs.is_empty(), "no texture refs examined");
    println!(
        "own-texture binding: {} referenced, {} baked, all under `{prefix}`",
        refs.len(),
        baked.len()
    );
    // No exemption list. The delve references no vanilla or shared texture today,
    // so there is nothing here for a defect to hide behind: a reference outside
    // the namespace is a finding, and a future emitter with a reason to make one
    // says so by turning this red.
    let foreign: Vec<&String> = refs.iter().filter(|r| !r.starts_with(&prefix)).collect();
    assert!(
        foreign.is_empty(),
        "{} mannequin texture(s) point outside this delve's own directory — a client renders \
         whichever applied pack answers the name: {foreign:#?}",
        foreign.len()
    );
}
