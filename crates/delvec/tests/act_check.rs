//! spec-0114: an objective says the act that completes it (`DW1045`), and an
//! item laid at a place leaves the hand (`interact.takes_item`).
//!
//! The act check is read over hello-world with its objectives replaced, so each
//! test states exactly the objectives it judges; the Muster Loft shape and its
//! repairs are read and built over the v0.4 showcase, which carries a `collect`.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvec::compiler::telling::{self, ACTS_EN, DW_ACT_UNTRUE, Lexicon, MAX_HEAD_WORDS};
use delvewright_dsl::{Act, Campaign, RawCampaign, parse_campaign};
use serde_json::{Value, json};

fn read(dir: &std::path::Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).unwrap()
}

fn parse_in(dir: &std::path::Path, quests: &Value) -> Campaign {
    let raw = RawCampaign {
        world: read(dir, "world.json"),
        npcs: read(dir, "npcs.json"),
        classes: read(dir, "classes.json"),
        quest_plan: read(dir, "quest-plan.json"),
        quests: quests.to_string(),
        dialogue: read(dir, "dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

/// hello-world whose one quest carries `objectives`, and `effects` as its
/// `on_objective_complete`.
fn with_objectives(objectives: Vec<Value>, effects: Value) -> Campaign {
    let dir = common::hello_world_dir();
    let mut q: Value = serde_json::from_str(&read(&dir, "quests.json")).unwrap();
    q["content"]["quests"][0]["objectives"] = Value::Array(objectives);
    q["content"]["quests"][0]["on_objective_complete"] = effects;
    parse_in(&dir, &q)
}

fn talk(title: &str) -> Value {
    json!({"type": "talk-to", "id": "obj/talk", "npc": "npc/keeper", "title": title})
}
fn reach(title: &str) -> Value {
    json!({"type": "reach-anchor", "id": "obj/reach", "anchor": "anchor/exit", "radius": 2, "title": title})
}
fn kill(title: &str) -> Value {
    json!({"type": "kill", "id": "obj/kill", "wave": "wave/guards", "title": title})
}
fn collect(title: &str) -> Value {
    json!({"type": "collect", "id": "obj/collect", "item": "minecraft:paper", "count": 1,
           "anchor": "anchor/exit", "title": title})
}
fn interact(title: &str) -> Value {
    json!({"type": "interact", "id": "obj/use", "anchor": "anchor/exit", "title": title})
}
fn presents(title: &str) -> Value {
    let mut o = interact(title);
    o["requires_item"] = json!("minecraft:paper");
    o
}
fn takes(title: &str) -> Value {
    let mut o = presents(title);
    o["takes_item"] = json!(true);
    o
}

/// The verdict over one objective: `(refused paths, judged, unknown)`.
fn judge(o: Value, effects: Value) -> (Vec<String>, usize, usize) {
    let (d, b) = telling::check_acts(&with_objectives(vec![o], effects));
    assert!(d.iter().all(|x| x.code == DW_ACT_UNTRUE.id()));
    (
        d.into_iter().map(|x| x.path).collect(),
        b.judged,
        b.unknown.len(),
    )
}

fn agrees(o: Value) -> bool {
    judge(o, json!({})).0.is_empty()
}

const TITLE: &str = "/content/quests/0/objectives/0/title";

/// A builder of one objective of one shape, from its title.
type Shape = fn(&str) -> Value;

/// Five objectives, one per kind, each titled with a head of its own act set:
/// the run judges all five and refuses none.
#[test]
fn one_objective_per_kind_titled_with_its_own_act_is_green() {
    let c = with_objectives(
        vec![
            talk("Speak to the Keeper"),
            reach("Go to the door"),
            kill("Clear the guard"),
            collect("Take the roll"),
            interact("Pull the lever"),
        ],
        json!({}),
    );
    let (d, b) = telling::check_acts(&c);
    assert!(d.is_empty(), "{d:?}");
    assert_eq!((b.objectives, b.titled, b.judged, b.refused), (5, 5, 5, 0));
    assert_eq!(
        b.line(),
        "act binding (DW1045): 5 of 5 English title(s) read against 5 objective(s); 0 head(s) \
         the lexicon does not know; 0 untitled; 0 refused"
    );
}

/// Each kind, and each of the `interact` shapes, refuses a head of a disjoint
/// act set — naming the title's path and no other.
#[test]
fn a_head_of_a_disjoint_act_is_refused_on_every_shape() {
    let cases: Vec<(&str, Shape, &str, &str)> = vec![
        ("talk-to", talk, "Hear the Keeper", "Climb up to the Keeper"),
        (
            "reach-anchor",
            reach,
            "Climb to the loft",
            "Lay the roll in the loft",
        ),
        ("kill", kill, "Silence the guard", "Take the guard's key"),
        ("collect", collect, "Fetch the roll", "Speak to the roll"),
        (
            "interact",
            interact,
            "Ring the bell",
            "Show the bell the key",
        ),
        (
            "interact + requires_item",
            presents,
            "Hold up the roll",
            "Hang the roll",
        ),
        (
            "interact + requires_item + takes_item",
            takes,
            "Hang the roll",
            "Go to the hook",
        ),
    ];
    for (shape, mk, ok, bad) in cases {
        assert!(agrees(mk(ok)), "{shape}: `{ok}` agrees");
        let (refused, judged, _) = judge(mk(bad), json!({}));
        assert_eq!(judged, 1, "{shape}: `{bad}` is judged");
        assert_eq!(
            refused,
            vec![TITLE.to_string()],
            "{shape}: `{bad}` is refused"
        );
    }
    // In a campaign of several objectives, only the changed one is refused.
    let c = with_objectives(
        vec![
            talk("Speak to the Keeper"),
            reach("Lay the roll in the loft"),
            interact("Pull the lever"),
        ],
        json!({}),
    );
    let (d, _) = telling::check_acts(&c);
    assert_eq!(
        d.iter().map(|x| x.path.as_str()).collect::<Vec<_>>(),
        vec!["/content/quests/0/objectives/1/title"]
    );
}

/// A head the lexicon does not know is counted and listed, never refused; an
/// objective with no title is counted apart.
#[test]
fn an_unknown_head_is_counted_and_listed_not_refused() {
    let mut untitled = reach("x");
    untitled.as_object_mut().unwrap().remove("title");
    let c = with_objectives(vec![talk("Parley with the Keeper"), untitled], json!({}));
    let (d, b) = telling::check_acts(&c);
    assert!(d.is_empty(), "{d:?}");
    assert_eq!((b.titled, b.judged, b.untitled), (1, 0, 1));
    assert_eq!(b.unknown.len(), 1);
    assert!(
        b.line().contains("1 head(s) the lexicon does not know")
            && b.line().contains("\"Parley with the Keeper\""),
        "{}",
        b.line()
    );
}

/// A `give-item` among the completion's effects adds `take` to any kind.
#[test]
fn a_give_item_in_the_completion_adds_take() {
    let gives = json!({"obj/talk": [
        {"type": "give-item", "item": "minecraft:lantern", "count": 1}
    ]});
    assert!(
        judge(talk("Fetch the lantern from the Keeper"), gives)
            .0
            .is_empty()
    );
    assert_eq!(
        judge(talk("Fetch the lantern from the Keeper"), json!({})).0,
        vec![TITLE.to_string()]
    );
    // …including one handed later in the same bundle.
    let nested = json!({"obj/talk": [{"type": "sequence", "steps": [
        {"at_ticks": 20, "effects": [{"type": "give-item", "item": "minecraft:lantern", "count": 1}]}
    ]}]});
    assert!(judge(talk("Take the lantern"), nested).0.is_empty());
}

/// `give` is true of an `interact` only when it takes the presented item.
#[test]
fn give_needs_takes_item() {
    assert!(agrees(takes("Give the Keeper the roll")));
    assert_eq!(
        judge(presents("Give the Keeper the roll"), json!({})).0,
        vec![TITLE.to_string()]
    );
}

/// The six heads whose object decides their act pass on every kind in their set
/// and refuse on one outside it.
#[test]
fn object_decided_heads_pass_in_their_set_and_refuse_outside_it() {
    let shape_for = |a: Act| -> Shape {
        match a {
            Act::Reach => reach,
            Act::Talk => talk,
            Act::Kill => kill,
            Act::Take => collect,
            Act::Use => interact,
            Act::Present => presents,
            Act::Give => takes,
            Act::Rest | Act::Save | Act::Quit => unreachable!("a label act, never an objective's"),
        }
    };
    let lex = Lexicon::english();
    let cases = [
        ("find", Act::Kill),
        ("read", Act::Talk),
        ("try", Act::Talk),
        ("board", Act::Talk),
        ("return", Act::Talk),
        ("break", Act::Talk),
    ];
    for (head, outside) in cases {
        let acts = lex.phrases().get(head).expect("in the lexicon");
        assert!(acts.len() > 1, "`{head}` is object-decided: {acts:?}");
        assert!(!acts.contains(&outside));
        let title = format!("{head} the thing");
        for a in acts {
            assert!(agrees(shape_for(*a)(&title)), "`{head}` on {a:?}");
        }
        assert_eq!(
            judge(shape_for(outside)(&title), json!({})).0,
            vec![TITLE.to_string()],
            "`{head}` on {outside:?}"
        );
    }
}

/// The head is the longest phrase, read case-folded with punctuation as space.
#[test]
fn the_head_is_the_longest_phrase_read_as_drawn() {
    let lex = Lexicon::english();
    assert_eq!(lex.head("Take your place").unwrap().0, "take your place");
    assert_eq!(lex.head("Take the key").unwrap().0, "take");
    assert_eq!(lex.head("CLIMB—up to Oru!").unwrap().0, "climb up");
    assert_eq!(lex.head("[[bold|Ring]] the bell").unwrap().0, "ring");
    assert!(lex.head("Parley").is_none());
}

/// The lexicon is one file, every phrase lowercase and at most four words, and
/// the file sorted is the file as committed.
#[test]
fn the_lexicon_is_one_sorted_file() {
    let raw: BTreeMap<String, Vec<String>> = serde_json::from_str(ACTS_EN).unwrap();
    let mut n = 0;
    for (act, list) in &raw {
        assert!(Act::parse(act).is_some(), "`{act}` is an act");
        for p in list {
            n += 1;
            assert_eq!(p, &p.to_lowercase(), "`{p}` is lowercase");
            assert!(p.split(' ').count() <= MAX_HEAD_WORDS, "`{p}`");
        }
    }
    let sorted: BTreeMap<String, Vec<String>> = raw
        .iter()
        .map(|(k, v)| {
            let mut v = v.clone();
            v.sort();
            v.dedup();
            (k.clone(), v)
        })
        .collect();
    let canonical = serde_json::to_string_pretty(&sorted).unwrap() + "\n";
    assert_eq!(canonical, ACTS_EN, "acts-en.json is written sorted");
    assert_eq!(n, 249, "the seed's 230 entries and spec-0124's 19 label heads");
    assert_eq!(
        Lexicon::english().phrases().len(),
        240,
        "over 240 distinct heads"
    );
    for a in Act::ALL {
        assert!(
            raw[a.as_str()].len() >= 6 && raw[a.as_str()].iter().any(|p| p == a.as_str()),
            "every act has at least six heads, its own name among them: {a:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// The Muster Loft shape, refused and repaired (spec-0114 §9.4)
// ---------------------------------------------------------------------------

fn showcase() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v04-showcase")
}

/// The v0.4 showcase with its key made a roll of paper, the door presenting
/// it, and its last objective replaced by `last`.
fn muster_loft(last: Value) -> Campaign {
    let mut q: Value = serde_json::from_str(&read(&showcase(), "quests.json")).unwrap();
    let trial = &mut q["content"]["quests"][1];
    trial["objectives"][1]["item"] = json!("minecraft:paper");
    trial["objectives"][2]["requires_item"] = json!("minecraft:paper");
    trial["objectives"][3] = last;
    // The Keeper no longer walks onto the place the roll is laid.
    trial["on_objective_complete"]["obj/slay"]
        .as_array_mut()
        .unwrap()
        .retain(|e| e["type"] != "move-npc");
    parse_in(&showcase(), &q)
}

fn lay_reach(title: &str) -> Value {
    json!({"type": "reach-anchor", "id": "obj/shrine", "anchor": "anchor/objective", "radius": 2,
           "after": ["obj/door"], "requires_flags": ["flag/opened"], "title": title,
           "happening": {"text": "the roll is laid in the loft", "verb": "arrives"}})
}

fn lay_interact() -> Value {
    json!({"type": "interact", "id": "obj/shrine", "anchor": "anchor/objective",
           "prop": {"block": "minecraft:lectern"}, "requires_item": "minecraft:paper",
           "takes_item": true, "after": ["obj/door"], "requires_flags": ["flag/opened"],
           "title": "Lay the roll in the loft",
           "hint": "The lectern stands at the top of the loft stair.",
           "happening": {"text": "the roll is laid in the loft", "verb": "arrives"}})
}

#[test]
fn the_muster_loft_shape_is_refused_naming_both_act_sets() {
    let (d, b) = telling::check_acts(&muster_loft(lay_reach("Lay the roll in the loft")));
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(b.refused, 1);
    assert_eq!(d[0].path, "/content/quests/1/objectives/3/title");
    let m = &d[0].message;
    assert!(m.contains("`lay` → give"), "{m}");
    assert!(m.contains("`reach-anchor` → reach"), "{m}");
    // A placing word over a non-conversation: the completion repair comes first.
    let first = m
        .find("make it an `interact`")
        .expect("the completion repair is named");
    let second = m
        .find("re-head the title")
        .expect("the re-titling is named");
    assert!(
        first < second,
        "the take is offered before the re-titling: {m}"
    );
}

#[test]
fn the_muster_loft_shape_is_repaired_either_way() {
    assert!(
        telling::check_acts(&muster_loft(lay_reach("Climb to the loft with the roll")))
            .0
            .is_empty()
    );
    assert!(
        telling::check_acts(&muster_loft(lay_interact()))
            .0
            .is_empty()
    );
}

/// A placing word over an interact that presents and does not take: the first
/// repair is to add `takes_item: true`.
#[test]
fn hang_over_a_presenting_interact_offers_takes_item_first() {
    let (d, _) = telling::check_acts(&with_objectives(
        vec![presents("Hang the lantern on the hook")],
        json!({}),
    ));
    let m = &d[0].message;
    let first = m.find("add `takes_item: true`").expect("named");
    assert!(first < m.find("re-head the title").unwrap(), "{m}");
    // Everywhere else the re-titling is first.
    let (d, _) = telling::check_acts(&with_objectives(vec![talk("Climb up to Oru")], json!({})));
    let m = &d[0].message;
    assert!(m.find("re-head the title").unwrap() < m.find("change the completion").unwrap());
    // A placing word on a conversation names the gap.
    let (d, _) = telling::check_acts(&with_objectives(vec![talk("Bring both to Oru")], json!({})));
    assert!(d[0].message.contains("spec-0114 §6.4"), "{}", d[0].message);
}

/// **A `collect` from an item holder is judged as a take** (spec-0111 ×
/// spec-0114): the act check reads the completion, not the hand that performs
/// it, so a collect that adopts a frame or a stand is a `take` like any collect —
/// "Take the seal" agrees, and a head of another act set over it is refused
/// with the collect's own remedy, exactly as over a conjured chest.
#[test]
fn a_collect_from_a_holder_is_judged_as_a_take() {
    let from_frame = |title: &str| {
        let mut o = collect(title);
        o["holder"] = json!("anchor/exit");
        o["slot"] = json!("contents");
        o
    };
    let (refused, judged, unknown) = judge(from_frame("Take the seal"), json!({}));
    assert_eq!((refused.len(), judged, unknown), (0, 1, 0));
    let (d, b) = telling::check_acts(&with_objectives(
        vec![from_frame("Climb to the seal")],
        json!({}),
    ));
    assert_eq!((b.judged, b.refused), (1, 1));
    assert_eq!(d[0].path, TITLE);
    assert!(
        d[0].message.contains("its completion `collect` → take"),
        "{}",
        d[0].message
    );
}

/// **A gated connection's opener says the opening act** (spec-0107 ×
/// spec-0114): the bar in a gated seam is opened by the `open-gate` an
/// objective's completion fires. Over an `interact` at the bar, "Open the
/// hatch", "Lift the grating" and "Unbar the door" all agree; the same titles
/// over a walk to the seam, or over the conversation whose beat opens it, are
/// refused — the player is told to work a bar the game opens for them.
#[test]
fn a_gated_seams_opener_is_titled_with_the_act_that_opens_it() {
    let opens = json!({"obj/use": [{"type": "open-gate", "anchor": "anchor/exit"}],
                       "obj/reach": [{"type": "open-gate", "anchor": "anchor/exit"}],
                       "obj/talk": [{"type": "open-gate", "anchor": "anchor/exit"}]});
    for title in ["Open the hatch", "Lift the grating", "Unbar the door"] {
        let (refused, judged, _) = judge(interact(title), opens.clone());
        assert_eq!(
            (refused.len(), judged),
            (0, 1),
            "`{title}` over an interact"
        );
        for (shape, mk) in [("reach-anchor", reach as Shape), ("talk-to", talk)] {
            let (refused, judged, _) = judge(mk(title), opens.clone());
            assert_eq!(
                (refused, judged),
                (vec![TITLE.to_string()], 1),
                "`{title}` over a {shape} that opens the gate"
            );
        }
    }
}

fn build(campaign: &Campaign) -> BuildOutput {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("every emitted command validates")
}

fn file(out: &BuildOutput, suffix: &str) -> String {
    let (_, bytes) = out
        .iter()
        .find(|(p, _)| p.ends_with(suffix))
        .unwrap_or_else(|| panic!("no emitted file ends with `{suffix}`"));
    String::from_utf8(bytes.clone()).unwrap()
}

/// Repair (b) builds: its completion takes one roll from the presenting
/// player before anything else, the take's PackTest asserts one fewer, and the
/// critical path carries the interact step holding the paper.
#[test]
fn the_laid_roll_leaves_the_hand() {
    let out = build(&muster_loft(lay_interact()));
    let complete = file(&out, "/function/complete_o_shrine.mcfunction");
    assert_eq!(
        complete.lines().next(),
        Some("clear @s minecraft:paper 1"),
        "the take opens the completion: {complete}"
    );
    // The door presents the paper and keeps it: no take there.
    let door = file(&out, "/function/complete_o_door.mcfunction");
    assert!(!door.contains("clear @s"), "{door}");
    let t = file(&out, "test/verb_interact_takes.mcfunction");
    let sel = "@a[tag=dw_t_vtake,limit=1]";
    assert!(
        t.contains(&format!(
            "item replace entity {sel} inventory.0 with minecraft:paper"
        )),
        "{t}"
    );
    let before = t
        .find("assert score #take_before dw.sys matches 2")
        .expect("before");
    let after = t
        .find("assert score #take_after dw.sys matches 1")
        .expect("after");
    assert!(before < after, "{t}");
    let path: Value = serde_json::from_slice(out.get("critical-path.json").unwrap()).unwrap();
    let step = path["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["objective"] == "obj/shrine")
        .expect("the step");
    assert_eq!(step["action"], "interact", "{step}");
    assert_eq!(step["requires_item"], "minecraft:paper", "{step}");
    // The act ledger is written beside the build.
    let ledger: Value =
        serde_json::from_slice(out.get("validation/act-gate.json").unwrap()).unwrap();
    assert_eq!(ledger["code"], "DW1045");
    assert_eq!(ledger["refused"], 0);
    assert!(ledger["examined"].as_u64().unwrap() > 0, "{ledger}");
}

/// The same campaign without `takes_item` emits nothing for it: the field
/// costs one line, and only where declared.
#[test]
fn an_interact_that_does_not_take_emits_no_take() {
    let mut o = lay_interact();
    o.as_object_mut().unwrap().remove("takes_item");
    o["title"] = json!("Hold up the roll in the loft");
    let out = build(&muster_loft(o));
    let complete = file(&out, "/function/complete_o_shrine.mcfunction");
    assert!(!complete.contains("clear @s"), "{complete}");
    assert!(
        !out.keys()
            .any(|k| k.ends_with("test/verb_interact_takes.mcfunction")),
        "no take, no take test"
    );
}

// ---------------------------------------------------------------------------
// The gallery (spec-0114 §9.5, §9.6)
// ---------------------------------------------------------------------------

/// The gallery's titles all agree, and changing one agreeing title's head to a
/// disjoint act's word turns the run red with `DW1045` naming that title.
#[test]
fn a_perturbed_gallery_title_is_refused_by_name() {
    let dir = common::repo_root().join("gallery");
    let c = common::campaign_at(&dir);
    let (d, b) = telling::check_acts(&c);
    assert!(d.is_empty(), "the gallery's titles all agree: {d:?}");
    assert!(b.judged > 0 && b.judged == b.titled, "{}", b.line());

    let mut c = c;
    // `obj/climb-to-the-mezzanine` (a reach-anchor) titled with a `give` head.
    let (qi, oi) = c
        .quests
        .content
        .quests
        .iter()
        .enumerate()
        .find_map(|(qi, q)| {
            q.objectives
                .iter()
                .position(|o| o.title() == Some("Climb to the mezzanine"))
                .map(|oi| (qi, oi))
        })
        .expect("the gallery's mezzanine objective");
    *c.quests.content.quests[qi].objectives[oi].title_mut() =
        Some("Lay the bone on the mezzanine".to_string());
    let (d, b) = telling::check_acts(&c);
    assert_eq!(b.refused, 1);
    assert_eq!(d.len(), 1);
    assert_eq!(
        d[0].path,
        format!("/content/quests/{qi}/objectives/{oi}/title")
    );
    assert!(d[0].message.contains("Lay the bone on the mezzanine"));
}

/// The translator reads the act beside every title (spec-0114 §5).
#[test]
fn the_title_row_situation_carries_the_act() {
    let c = with_objectives(
        vec![takes("Hang the roll"), reach("Go to the door")],
        json!({}),
    );
    let s = delvewright_dsl::l10n::key_situations(&c);
    let row = |k: &str| s.get(k).unwrap_or_else(|| panic!("{k}: {s:?}"));
    assert!(
        row("obj.open-the-door.use.title").contains(&"act: use, present, give".to_string()),
        "{:?}",
        row("obj.open-the-door.use.title")
    );
    assert!(row("obj.open-the-door.reach.title").contains(&"act: reach".to_string()));
    assert!(
        !s.iter()
            .any(|(k, v)| !k.ends_with(".title") && v.iter().any(|l| l.starts_with("act: "))),
        "only title rows carry the act"
    );
}

// ---------------------------------------------------------------------------
// spec-0124 §6.4: a rest point's caption names only the acts its button performs
// ---------------------------------------------------------------------------

/// hello-world whose quest completes on a bonfire carrying `labels`.
fn with_bonfire(labels: Value) -> Campaign {
    let dir = common::hello_world_dir();
    let mut q: Value = serde_json::from_str(&read(&dir, "quests.json")).unwrap();
    let mut b = json!({"type": "bonfire", "anchor": "anchor/exit",
                       "prop": {"block": "minecraft:campfire[lit=true]"}});
    for (k, v) in labels.as_object().unwrap() {
        b[k] = v.clone();
    }
    q["content"]["quests"][0]["on_complete"] = json!([b]);
    parse_in(&dir, &q)
}

/// **spec-0124 criterion 9**: `Save and quit` on the save option is refused
/// `DW1045` on `quit`; the engine's defaults and `Rest`, `Save` pass; a label
/// with no lexicon verb passes unjudged and is counted as such.
#[test]
fn a_caption_names_only_what_its_button_does_dw1045() {
    let (d, b) = telling::check_labels(&with_bonfire(json!({"save_label": "Save and quit"})));
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code, DW_ACT_UNTRUE.id());
    assert!(d[0].path.ends_with("/save_label"), "{}", d[0].path);
    assert!(d[0].message.contains("`quit` → quit"), "{}", d[0].message);
    assert!(d[0].message.contains("performs save"), "{}", d[0].message);
    assert_eq!((b.bonfires, b.labels, b.refused), (1, 2, 1));
    assert!(b.line().contains("2 label(s) judged over 1 bonfire(s)"), "{}", b.line());
    for labels in [
        json!({}),
        json!({"rest_label": "Rest and save", "save_label": "Save only"}),
        json!({"rest_label": "Rest", "save_label": "Save"}),
    ] {
        let (d, b) = telling::check_labels(&with_bonfire(labels.clone()));
        assert!(d.is_empty(), "{labels}: {d:?}");
        assert_eq!((b.bonfires, b.labels, b.unjudged), (1, 2, 0), "{labels}");
    }
    let (d, b) = telling::check_labels(&with_bonfire(json!({"rest_label": "By the fire"})));
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(b.unjudged, 1, "{}", b.line());
    // The spec's specimen of an unjudged caption is not lexicon-free on this
    // tree: `light` is a `use` head, so the caption is judged, and a rest
    // option performs no `use`.
    let (d, _) = telling::check_labels(&with_bonfire(json!({"rest_label": "Light the candles"})));
    assert_eq!(d.len(), 1, "{d:?}");
    // The save option does not rest: a rest word on it is refused.
    let (d, _) = telling::check_labels(&with_bonfire(json!({"save_label": "Sit a while"})));
    assert_eq!(d.len(), 1, "{d:?}");
    // A campaign with no bonfire states zeroes.
    let (d, b) = telling::check_labels(&with_objectives(vec![interact("Pull the lever")], json!({})));
    assert!(d.is_empty());
    assert_eq!(
        b.line(),
        "label act binding (DW1045): 0 label(s) judged over 0 bonfire(s); 0 lexicon verb(s) \
         found, 0 label(s) with none; 0 refused"
    );
}

/// The zh sidecar is not read (spec-0116's English-only rule): the label arm
/// reads the campaign's English, and a translation's words never reach it.
#[test]
fn the_label_arm_reads_the_english_alone() {
    let c = with_bonfire(json!({"save_label": "Save"}));
    let (d, _) = telling::check_labels(&c);
    assert!(d.is_empty());
}
