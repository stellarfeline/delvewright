//! **A placed piece's outside is buried, or the piece declares it** (`DW0885`).
//!
//! One world, built four ways, so that every arm of the code and both directions
//! of its binding are the same geometry with one thing changed:
//!
//! * a room the party cannot get out of — examined, not judged, and the build
//!   goes on;
//! * the same room with one wall slab missing, declaring nothing — refused,
//!   naming the side, the cells, the first coordinate and the three moves;
//! * the same open room whose piece declares its sides — the check passes and
//!   the NEXT proof speaks, which is what proves the refusal was this code's and
//!   not somebody else's;
//! * a declaration naming a face the piece does not have — refused by the other
//!   arm, off the piece's own bytes.
//!
//! The prefab is synthetic and the campaign is the shipped hello-world, built
//! through the real `emit::build`, so nothing here is a unit test of a private
//! function: the diagnostic is raised where a creator would meet it.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, parse_campaign};

fn hello_world() -> Campaign {
    let loaded = load_campaign_dir(&common::hello_world_dir()).unwrap();
    parse_campaign(&loaded.raw).expect("hello-world parses")
}

/// A stone box, lit from inside so the darkness gate never pre-empts this one.
///
/// `open_x0` drops the whole `x == 0` slab. That is what lets the party's air out
/// of the room — and it is the only difference between a world this check has
/// nothing to say about and a world it refuses.
fn box_nbt(size: [i32; 3], open_x0: bool) -> Vec<u8> {
    use fastnbt::Value;
    let [sx, sy, sz] = size;
    let mut blocks: Vec<Value> = Vec::new();
    let mut push = |x: i32, y: i32, z: i32, state: i32| {
        let mut c = std::collections::HashMap::new();
        c.insert(
            "pos".to_string(),
            Value::List(vec![Value::Int(x), Value::Int(y), Value::Int(z)]),
        );
        c.insert("state".to_string(), Value::Int(state));
        blocks.push(Value::Compound(c));
    };
    for x in 0..sx {
        if open_x0 && x == 0 {
            continue;
        }
        for y in 0..sy {
            for z in 0..sz {
                if y == 0 || y == sy - 1 || x == 0 || x == sx - 1 || z == 0 || z == sz - 1 {
                    push(x, y, z, 1);
                }
            }
        }
    }
    for x in [2, sx / 2, sx - 3] {
        for z in [2, sz / 2, sz - 3] {
            push(x, sy - 2, z, 2);
        }
    }
    let palette = Value::List(vec![
        pal_entry("minecraft:air"),
        pal_entry("minecraft:stone"),
        pal_entry("minecraft:glowstone"),
    ]);
    let mut root = std::collections::HashMap::new();
    root.insert("DataVersion".to_string(), Value::Int(4671));
    root.insert(
        "size".to_string(),
        Value::List(vec![Value::Int(sx), Value::Int(sy), Value::Int(sz)]),
    );
    root.insert("palette".to_string(), palette);
    root.insert("blocks".to_string(), Value::List(blocks));
    root.insert("entities".to_string(), Value::List(vec![]));
    let raw = fastnbt::to_bytes(&Value::Compound(root)).unwrap();
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::Write::write_all(&mut gz, &raw).unwrap();
    gz.finish().unwrap()
}

fn pal_entry(name: &str) -> fastnbt::Value {
    let mut c = std::collections::HashMap::new();
    c.insert("Name".to_string(), fastnbt::Value::String(name.to_string()));
    fastnbt::Value::Compound(c)
}

/// Build hello-world against `nbt`, with `hello-room.json` carrying exactly
/// `shown_faces`.
///
/// `None` is the shipped state of every piece in the library: no declaration at
/// all, which is what a piece authored to be buried writes.
fn build_declaring(
    tag: &str,
    nbt: &[u8],
    shown: Option<&[&str]>,
) -> Result<BuildOutput, BuildFailure> {
    let dir = common::shown_prefabs_dir(tag);
    std::fs::write(dir.join("hello-room.nbt"), nbt).unwrap();
    set_shown(&dir, shown);
    let prefabs = PrefabRegistry::load_dir(&dir).unwrap();
    let campaign = hello_world();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for t in area.pieces.iter().flat_map(|p| &p.templates) {
            structures.insert(t.structure_file.clone(), nbt.to_vec());
        }
    }
    emit::build(
        &plan,
        &load_campaign_dir(&common::hello_world_dir())
            .unwrap()
            .inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

fn set_shown(dir: &Path, shown: Option<&[&str]>) {
    let path = dir.join("hello-room.json");
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let obj = doc.as_object_mut().unwrap();
    match shown {
        Some(sides) => {
            obj.insert("shown_faces".to_string(), serde_json::json!(sides));
        }
        None => {
            obj.remove("shown_faces");
        }
    }
    std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
}

fn refusal(r: Result<BuildOutput, BuildFailure>) -> (String, String) {
    match r {
        Err(BuildFailure::Diagnostic { code, message }) => (code.to_string(), message),
        Err(other) => panic!("expected a diagnostic failure, got {other:?}"),
        Ok(_) => panic!("expected a refusal, the build succeeded"),
    }
}

/// **The room nobody can get out of is examined and not judged.**
///
/// Every one of its six sides is solid and every one of them has nothing in
/// front of it: under `horizon: void` there is nothing outside the placed
/// geometry at all. It is still not a finding, and this is the discrimination
/// the whole check rests on — a side no body can be in front of is neither shown
/// nor hidden, and demanding a declaration for it would make every sealed
/// one-room delve in the library red for a face nobody will ever look at.
///
/// The zero is asserted from the LEDGER rather than from the build's silence,
/// because a check that never ran and a check that found nothing are the same
/// silence. `exposed > 0` beside `judged == 0` is what says the question was put.
#[test]
fn a_room_the_party_cannot_leave_is_examined_and_not_judged() {
    let out = build_declaring("exposure-sealed", &box_nbt([11, 6, 11], false), None)
        .expect("a sealed room builds");
    let ledger: serde_json::Value = serde_json::from_slice(
        out.get("validation/piece-exposure.json")
            .expect("every assembled world emits the piece-exposure ledger"),
    )
    .unwrap();
    assert_eq!(ledger["placed"], 1, "{ledger}");
    assert_eq!(ledger["examined"], 1, "{ledger}");
    assert!(
        ledger["boundary_cells"].as_u64().unwrap() > 0,
        "the piece put solid cells on its own boundary: {ledger}"
    );
    assert!(
        ledger["exposed_cells"].as_u64().unwrap() > 0,
        "and nothing is in front of them — a void world buries nothing: {ledger}"
    );
    assert_eq!(
        ledger["judged_cells"], 0,
        "but the party's air never reaches them: {ledger}"
    );
    assert_eq!(ledger["party_air_cut_off"], false, "{ledger}");
    assert_eq!(ledger["horizon"], "void", "{ledger}");
}

/// **Open one wall and the piece owes an answer for the other five.**
///
/// The missing `x == 0` slab is what lets the party's air out of the room; from
/// there it runs along the outside of the box, and the piece is judged on every
/// side. The message is asserted for what an author needs to act: which side,
/// how many cells, where the first one is, what the document says, and each of
/// the three moves with the one this horizon does not offer said so in words.
#[test]
fn an_open_room_that_declares_nothing_is_dw0885() {
    let (code, message) = refusal(build_declaring(
        "exposure-open-silent",
        &box_nbt([11, 6, 11], true),
        None,
    ));
    assert_eq!(code, "DW0885", "{message}");
    assert!(message.contains("prefab/hello-room"), "{message}");
    assert!(
        message.contains("standing in open air the party can be in"),
        "{message}"
    );
    assert!(
        message.contains("no `shown_faces` at all"),
        "names what the document says: {message}"
    );
    assert!(
        message.contains("solid cell(s) of that side"),
        "counts the cells: {message}"
    );
    for move_ in [
        "BURY it",
        "PLACE something against it",
        "DECLARE the side shown",
    ] {
        assert!(
            message.contains(move_),
            "owes the author {move_}: {message}"
        );
    }
    assert!(
        message.contains("`void` is the horizon that declares there is nothing outside"),
        "says which move this horizon does not leave open: {message}"
    );
}

/// **The same world, declared, gets past this check** — and is then refused by
/// the proof that owns the actual hazard.
///
/// This is the direction that makes the test above evidence. Without it, a
/// refusal on the open room proves only that SOMETHING reds; with it, the one
/// thing that changed is a line in the prefab document, and what comes back is a
/// different code from a different check about a different fact (the walkable
/// floor beside a bottomless column). `DW0885` runs before `DW0322` in
/// `emit::build`, so reaching `DW0322` at all is this check having passed.
#[test]
fn the_same_room_with_its_sides_declared_reaches_the_next_proof() {
    let (code, message) = refusal(build_declaring(
        "exposure-open-declared",
        &box_nbt([11, 6, 11], true),
        Some(&["down", "east", "north", "south", "up"]),
    ));
    assert_eq!(
        code, "DW0322",
        "the exposure check passed and the boundary proof spoke: {message}"
    );
}

/// **A declaration naming a face the piece does not have is refused.**
///
/// The open box has no `x == 0` slab at all, so `west` names pure air. The demand
/// is a fact about the piece's own template bytes rather than about the
/// assembled world, which is what keeps a campaign that carves a doorway from
/// invalidating a claim about the asset — and it is what stops `shown_faces`
/// from being free typing.
#[test]
fn a_declaration_naming_a_face_of_pure_air_is_dw0885() {
    let (code, message) = refusal(build_declaring(
        "exposure-empty-side",
        &box_nbt([11, 6, 11], true),
        Some(&["down", "east", "north", "south", "up", "west"]),
    ));
    assert_eq!(code, "DW0885", "{message}");
    assert!(
        message.contains("not one cell of its own west face holds a block"),
        "{message}"
    );
    assert!(
        message.contains("Drop it from `shown_faces`"),
        "owes the author a move: {message}"
    );
}

/// **A side that is not a side is refused where it is written.**
///
/// `top` is a word a person would reach for and the engine does not have. It is
/// refused rather than counted as a declaration that binds to nothing, because
/// nothing can be judged against a side that does not exist — and a silently
/// ignored spelling is the shape that leaves an author certain they declared
/// something.
#[test]
fn a_shown_face_that_is_not_a_side_is_dw0885() {
    let (code, message) = refusal(build_declaring(
        "exposure-bad-side",
        &box_nbt([11, 6, 11], false),
        Some(&["top"]),
    ));
    assert_eq!(code, "DW0885", "{message}");
    assert!(message.contains("is not a side of a piece"), "{message}");
    assert!(
        message.contains("`east`, `west`, `up`, `down`, `north`, `south`"),
        "names the vocabulary: {message}"
    );
}
