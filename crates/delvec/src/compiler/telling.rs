//! What the dialogue tells the player: a question gets an answer (`DW0981`),
//! and a name reaches a line only after the player has been told it (`DW0982`).
//!
//! Both are `docs/reference/game-writing.md` rules made mechanical: §2 S1/S5 (the
//! critical path is plain) for the first, §3 N1/N3 (introduce a name before you
//! use it; the player never asks about a name they have not heard) for the
//! second. Everything here reads the documents and the compiler's own play order;
//! nothing reaches the datapack.
//!
//! ## A question gets an answer (`DW0981`)
//!
//! An option whose label or tooltip is a question — it ends in `?`, or in `？` in
//! any sidecar rendition — is the player asking. The dialog closes when the
//! option has no `next`, so the player asks and the speaker turns away. The
//! option must lead to a node with a line in it, and not back to the node it
//! stands in (the speaker repeating the line they just said is not an answer).
//! Whether the line answers the question is the writer's judgement; that some
//! line follows is the machine's.
//!
//! ## A name is told before it is used (`DW0982`)
//!
//! The names are the inventory's `name` and item-name rows — the same declared
//! set the transcreation fact check holds to one rendering
//! (`docs/reference/i18n.md` § The fact check): a person, a body or wave, a
//! place, a counter, an item. A **use** is an occurrence in an NPC's line or in
//! an option's label or tooltip, written as the name (its capitalisation) or as a
//! definite reference to it (`the watch`, `my stand`): `keep watch` is a word,
//! `the watch` is the name.
//!
//! A name is **told** by:
//!
//! - the person it belongs to speaking to the player — their dialog carries their
//!   name over their line;
//! - the class the player picks, and the kit it hands over;
//! - an item the player is given;
//! - a sentence the player reads that says what it is: the name after `a`/`an`
//!   (`a watch of dead soldiers`), after `called`/`named`/`known as` or `this
//!   is`/`I am` (`This is the Quiet Keep.`), followed by
//!   an apposition (`the Watch, the keep's dead garrison,`) or by `is`/`are`/
//!   `was`/`were`.
//!
//! A nameplate over a body the party fights, a counter on the sidebar, a bar's
//! title and an area's name are labels: they show the name and do not say what
//! it is, and a definite mention (`the last of the watch`) presupposes it. None
//! of them tells.
//!
//! Each string is placed at the first state of the compiler's play order at
//! which the player can read it — the walk [`Flow::journal`] takes, along the
//! exported critical path and every realised branch's own: a dialogue line or
//! option where [`crate::compiler::flow::Walk::dialogue_on_screen`] draws it (and,
//! after a `talk-to` step, wherever the node its option opens leads), an
//! objective's title and hint once its `after` is done, a quest's goal and its
//! barks once it runs, an effect's strings at the step that fires it. A string
//! the walk cannot date (an environment trigger, a refusal, a reaction bundle,
//! loot) is placed at the start, so it can tell a name early and never makes a
//! use look early: the dating errs toward silence, never toward a refusal.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{
    Campaign, Diagnostic, L10nDoc, TextKind, effect_string_sites, key_kind, l10n_inventory,
    local_id,
};

use crate::compiler::flow::Flow;
use delvewright_dsl::{DwCode, ExitTier};

delvewright_dsl::dw_code! {
    /// `DW0981`: a dialogue option whose label or tooltip is a question leads to
    /// no line that could answer it.
    pub const DW_QUESTION_UNANSWERED: DwCode = DwCode::new("DW0981", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0982`: a declared name reaches an NPC's line or an option before the
    /// play order has told the player what it is.
    pub const DW_NAME_UNTOLD: DwCode = DwCode::new("DW0982", ExitTier::Build);
}

// ---------------------------------------------------------------------------
// DW0981 — a question gets an answer
// ---------------------------------------------------------------------------

/// Whether `text` is a question: its last visible character is `?` or `？`.
pub fn is_question(text: &str) -> bool {
    let t = text.trim_end_matches(|c: char| {
        c.is_whitespace() || matches!(c, '"' | '\'' | '”' | '’' | '」' | '』')
    });
    t.ends_with('?') || t.ends_with('？')
}

/// `DW0981` over every dialogue option, in the English source and every
/// declared-language sidecar. One diagnostic per option, naming every rendition
/// that asks.
pub fn check_questions(c: &Campaign, sidecars: &BTreeMap<String, L10nDoc>) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    for (ti, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        let np = local_id(tree.npc.as_str());
        for (ni, node) in tree.nodes.iter().enumerate() {
            let nd = local_id(node.id.as_str());
            for (oi, opt) in node.options.iter().enumerate() {
                let key = format!("dlg.{np}.{nd}.opt.{oi}");
                let mut asks: Vec<String> = Vec::new();
                if is_question(&opt.label) {
                    asks.push(format!("label `{}`", opt.label));
                }
                if let Some(t) = &opt.tooltip
                    && is_question(t)
                {
                    asks.push(format!("tooltip `{t}`"));
                }
                for lang in &c.world.content.languages {
                    let Some(doc) = sidecars.get(lang) else {
                        continue;
                    };
                    for field in ["label", "tooltip"] {
                        if let Some(t) = doc.content.get(&format!("{key}.{field}"))
                            && is_question(t)
                        {
                            asks.push(format!("{lang} {field} `{t}`"));
                        }
                    }
                }
                if asks.is_empty() {
                    continue;
                }
                let answer = opt
                    .next
                    .as_ref()
                    .filter(|n| n.as_str() != node.id.as_str())
                    .and_then(|n| tree.nodes.iter().find(|x| x.id.as_str() == n.as_str()))
                    .filter(|x| !x.text.trim().is_empty());
                if answer.is_some() {
                    continue;
                }
                let why = match &opt.next {
                    None => "it has no `next`, so choosing it closes the dialog: the player asks \
                             and the conversation ends"
                        .to_string(),
                    Some(n) if n.as_str() == node.id.as_str() => format!(
                        "its `next` is `{n}`, the node it stands in, so the speaker repeats the \
                         line the player just asked about"
                    ),
                    Some(n) => format!("its `next` `{n}` carries no line"),
                };
                d.push(Diagnostic::error(
                    DW_QUESTION_UNANSWERED,
                    "dialogue",
                    format!("/content/dialogues/{ti}/nodes/{ni}/options/{oi}"),
                    format!(
                        "option {oi} of `{}` is a question ({}) and nothing answers it: {why}. \
                         Give it a `next` node in which `{}` answers the question plainly, in \
                         their own voice (game-writing.md §2); keep the option's effects — a \
                         `complete-objective` fires when the option is chosen, whatever node it \
                         opens. If the player is not asking, word the label as what they say \
                         (`Goodbye.`, `I'll go.`), not as a question",
                        node.id,
                        asks.join(", "),
                        tree.npc,
                    ),
                ));
            }
        }
    }
    d
}

// ---------------------------------------------------------------------------
// DW0982 — a name is told before it is used
// ---------------------------------------------------------------------------

/// How many earlier labels and mentions a `DW0982` message names before it
/// counts the rest.
const SHOWN_LISTED: usize = 3;

/// When a string first reaches the player on one walk: the state index (`0`
/// before the first step, `k` after the `k`-th).
type At = usize;

/// One declared name, by the core a sentence repeats (`The Watch` → `Watch`).
#[derive(Clone, Debug)]
struct Name {
    /// The declared text (`The Watch`).
    text: String,
    /// The inventory key that declares it.
    key: String,
    /// The words a mention repeats, article dropped.
    core: String,
}

/// What told a name, or what showed it without telling, for the message.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Shown {
    /// A label: a nameplate, counter, bar or area name (the inventory key).
    Label(String),
    /// A definite mention in a string the player read earlier (the key).
    Mention(String),
}

/// The article-free core of a declared name.
fn core_of(name: &str) -> String {
    let t = name.trim();
    for a in ["The ", "the ", "A ", "a ", "An ", "an "] {
        if let Some(rest) = t.strip_prefix(a) {
            return rest.trim().to_string();
        }
    }
    t.to_string()
}

/// Every occurrence of `core` in `text` on word boundaries, ASCII
/// case-insensitive, as byte spans.
fn occurrences(text: &str, core: &str) -> Vec<(usize, usize)> {
    let hay = text.to_ascii_lowercase();
    let needle = core.to_ascii_lowercase();
    let mut out = Vec::new();
    if needle.is_empty() {
        return out;
    }
    let mut from = 0;
    while let Some(i) = hay[from..].find(&needle) {
        let s = from + i;
        let e = s + needle.len();
        let before = hay[..s].chars().next_back();
        let after = hay[e..].chars().next();
        let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric());
        if !word(before) && !word(after) {
            out.push((s, e));
        }
        from = s + needle.len().max(1);
        if from >= hay.len() {
            break;
        }
    }
    out
}

/// The words before byte `s`, nearest first, lowercased, punctuation trimmed;
/// stops at a sentence break.
fn words_before(text: &str, s: usize, n: usize) -> Vec<String> {
    let mut out = Vec::new();
    for w in text[..s].split_whitespace().rev() {
        let stop = w.ends_with(['.', '!', '?', ';', ':']);
        let clean: String = w
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_ascii_lowercase();
        if stop || clean.is_empty() {
            break;
        }
        out.push(clean);
        if out.len() == n {
            break;
        }
    }
    out
}

/// Whether the occurrence at `(s, e)` is a sentence that says what the name is.
fn introduces(text: &str, s: usize, e: usize) -> bool {
    let before = words_before(text, s, 3);
    // `a watch`, `an old watch`, `a long-dead city watch`.
    let modifiers_then_article = before
        .iter()
        .position(|w| w == "a" || w == "an")
        .is_some_and(|i| {
            before[..i].iter().all(|w| {
                !matches!(
                    w.as_str(),
                    "the"
                        | "my"
                        | "your"
                        | "his"
                        | "her"
                        | "our"
                        | "their"
                        | "its"
                        | "this"
                        | "that"
                )
            })
        });
    if modifiers_then_article {
        return true;
    }
    // `called the Watch`, `named Halvard`, `known as the Undertide`.
    let mut b = before.iter().map(String::as_str);
    let first = b.next();
    let lead = if first == Some("the") {
        b.next()
    } else {
        first
    };
    let second = b.next();
    if matches!(lead, Some("called" | "named")) || (lead == Some("as") && second == Some("known")) {
        return true;
    }
    // `This is the Quiet Keep.`, `I am the Keeper.` — naming what is in front of
    // the player, or the speaker.
    if matches!(
        (lead, second),
        (Some("is"), Some("this" | "here")) | (Some("am"), Some("i")) | (Some("are"), Some("we"))
    ) {
        return true;
    }
    // The gloss forms below say what a name IS; a possessed or pointed-at
    // thing (`your tallow is where you left it`) is presupposed, not glossed.
    if before.first().is_some_and(|w| {
        matches!(
            w.as_str(),
            "my" | "your" | "his" | "her" | "our" | "their" | "its" | "this" | "that"
        )
    }) {
        return false;
    }
    let rest = &text[e..];
    let rest = rest
        .strip_prefix("'s")
        .or_else(|| rest.strip_prefix("’s"))
        .unwrap_or(rest)
        .trim_start();
    // `the Watch, the keep's dead garrison,` / `the Undertide — grey water`.
    if let Some(after) = [',', '—', '–', '(']
        .iter()
        .find_map(|p| rest.strip_prefix(*p))
        && matches!(
            words_after(after, 1).first().map(String::as_str),
            Some("the" | "a" | "an" | "who" | "which" | "what")
        )
    {
        return true;
    }
    // `Vesperhold. A king, a court, a bell.` — the name standing alone, and the
    // next sentence saying what it is with an indefinite.
    if let Some(after) = ['.', ':'].iter().find_map(|p| rest.strip_prefix(*p))
        && matches!(
            words_after(after, 1).first().map(String::as_str),
            Some("a" | "an")
        )
    {
        return true;
    }
    // `Halvard is the bell-warden`, `the Undertide was a river`.
    let next = words_after(rest, 2);
    matches!(
        (
            next.first().map(String::as_str),
            next.get(1).map(String::as_str)
        ),
        (
            Some("is" | "are" | "was" | "were"),
            Some("a" | "an" | "the" | "one")
        )
    )
}

/// The first `n` words of `text`, lowercased and stripped of punctuation.
fn words_after(text: &str, n: usize) -> Vec<String> {
    text.split_whitespace()
        .take(n)
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_ascii_lowercase()
        })
        .collect()
}

/// Whether the occurrence at `(s, e)` is a use of the name: written with the
/// name's own capitalisation, or after an article or a possessive. A name
/// declared with its article (`The Keep`, `articled`) is written with it, so its
/// bare word opening a sentence (`Keep your road.`) is a capital, not the name.
fn is_use(text: &str, core: &str, articled: bool, s: usize, e: usize) -> bool {
    let opens_sentence = text[..s]
        .trim_end_matches(|c: char| c.is_whitespace() || matches!(c, '"' | '“' | '\'' | '‘'))
        .chars()
        .next_back()
        .is_none_or(|c| matches!(c, '.' | '!' | '?' | ':' | ';'));
    if &text[s..e] == core
        && core.chars().next().is_some_and(char::is_uppercase)
        && !(articled && opens_sentence)
    {
        return true;
    }
    // A demonstrative points at what is in front of the player (`this keep`),
    // so it presupposes nothing; an article or a possessive does.
    words_before(text, s, 1).first().is_some_and(|w| {
        matches!(
            w.as_str(),
            "the" | "my" | "your" | "his" | "her" | "our" | "their" | "its"
        )
    })
}

/// The spans of `names` in `text`, each held to the longest name covering it
/// (`Warden` inside `Warden's Door` is a mention of the door), as
/// `(name index, start, end)`.
fn mentions(text: &str, names: &[Name]) -> Vec<(usize, usize, usize)> {
    let mut all: Vec<(usize, usize, usize)> = Vec::new();
    for (i, n) in names.iter().enumerate() {
        for (s, e) in occurrences(text, &n.core) {
            all.push((i, s, e));
        }
    }
    all.iter()
        .copied()
        .filter(|&(i, s, e)| {
            !all.iter()
                .any(|&(j, s2, e2)| j != i && s2 <= s && e <= e2 && (e2 - s2) > (e - s))
        })
        .collect()
}

/// Where a declared name is told by its own source, by inventory key: the
/// person speaking to the player, the class and kit, an item handed over.
/// `None` for a label (nameplate, counter, bar, area), which never tells.
enum OwnTelling {
    /// When this NPC's dialog is first on screen.
    Npc(String),
    /// From the start.
    Start,
    /// When this string's own effect or objective first reaches the player.
    WithString,
    /// A label: the name shown, not told.
    Label,
}

fn own_telling(key: &str) -> OwnTelling {
    let segs: Vec<&str> = key.split('.').collect();
    match segs.first().copied() {
        Some("npc") => OwnTelling::Npc(segs.get(1).copied().unwrap_or("").to_string()),
        Some("class") => OwnTelling::Start,
        Some("obj") | Some("fx") => OwnTelling::WithString,
        // Loot and a body's drop are held as soon as they can be had; the walk
        // cannot date that, so it tells from the start (the safe direction).
        Some("loot") => OwnTelling::Start,
        Some("wave") | Some("actor") if segs.contains(&"drop") => OwnTelling::Start,
        _ => OwnTelling::Label,
    }
}

/// One use of an untold name, at its earliest state on any walk.
#[derive(Clone, Debug)]
struct Untold {
    at: At,
    walk: String,
    shown: Vec<Shown>,
}

/// What `DW0982` examined on one campaign, zeroes included.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NameBinding {
    /// Declared names (inventory `name` and item-name rows) the check holds.
    pub names: usize,
    /// Walks taken: the critical path and every realised branch.
    pub walks: usize,
    /// Dialogue strings (lines, labels, tooltips) the walks put on screen, counted
    /// once each.
    pub dialogue_read: usize,
    /// Dialogue strings the documents declare.
    pub dialogue_declared: usize,
    /// Uses of a declared name in those strings, counted per string and name.
    pub uses: usize,
    /// Uses refused: the name was not told before it on some walk.
    pub untold: usize,
}

impl NameBinding {
    /// The binding line `delvec validate` prints.
    pub fn line(&self) -> String {
        format!(
            "name-told binding (DW0982): {} declared name(s); {} walk(s); {} of {} dialogue \
             string(s) on screen on some walk; {} use(s) of a name in them, {} untold",
            self.names,
            self.walks,
            self.dialogue_read,
            self.dialogue_declared,
            self.uses,
            self.untold
        )
    }
}

/// `DW0982` over the exported critical path and every realised branch, without
/// its binding.
pub fn check_names_told(c: &Campaign) -> Vec<Diagnostic> {
    check_names_told_bound(c).0
}

/// `DW0982` over the exported critical path and every realised branch, with
/// what it examined.
pub fn check_names_told_bound(c: &Campaign) -> (Vec<Diagnostic>, NameBinding) {
    let mut bind = NameBinding::default();
    let inv = l10n_inventory(c);
    let mut names: Vec<Name> = inv
        .iter()
        .filter(|(k, _)| matches!(key_kind(k), Some(TextKind::Name | TextKind::ItemName)))
        .map(|(k, v)| Name {
            text: v.clone(),
            key: k.clone(),
            core: core_of(v),
        })
        .filter(|n| n.core.chars().count() >= 3)
        .collect();
    names.sort_by(|a, b| a.key.cmp(&b.key));
    bind.names = names.len();

    let flow = Flow::new(c);
    let casts = crate::compiler::cast::npc_casts(c);
    let mut walks: Vec<(String, crate::compiler::flow::Playthrough)> = Vec::new();
    let main = flow.playthrough();
    if main.degenerate || main.cyclic {
        return (Vec::new(), bind); // DW0201/DW0130 own a campaign with no play order
    }
    walks.push(("the critical path".to_string(), main));
    for r in crate::compiler::branch::realize(c) {
        if let Some(w) = r.world {
            walks.push((format!("branch `{}`", r.branch.id), flow.playthrough_in(w)));
        }
    }

    // Effect strings by key base, and the pointer the replay records them under.
    let sites: BTreeMap<String, String> = effect_string_sites(c)
        .into_iter()
        .map(|(_, path, kb)| (kb, path))
        .collect();
    // Objectives and quests by the local ids their keys carry.
    let mut objectives: BTreeMap<(String, String), (String, String, Vec<String>)> = BTreeMap::new();
    let mut quests: BTreeMap<String, String> = BTreeMap::new();
    for q in &c.quests.content.quests {
        let ql = local_id(q.id.as_str()).to_string();
        quests.insert(ql.clone(), q.id.as_str().to_string());
        for o in &q.objectives {
            objectives.insert(
                (ql.clone(), local_id(o.id().as_str()).to_string()),
                (
                    q.id.as_str().to_string(),
                    o.id().as_str().to_string(),
                    o.after().iter().map(|a| a.as_str().to_string()).collect(),
                ),
            );
        }
    }
    // Dialogue keys → (npc id, node id, Some(option index)) and their pointers.
    let mut dlg: BTreeMap<String, (String, String, Option<usize>, String)> = BTreeMap::new();
    for (ti, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        let np = local_id(tree.npc.as_str());
        for (ni, node) in tree.nodes.iter().enumerate() {
            let nd = local_id(node.id.as_str());
            let base = format!("/content/dialogues/{ti}/nodes/{ni}");
            dlg.insert(
                format!("dlg.{np}.{nd}.text"),
                (
                    tree.npc.as_str().to_string(),
                    node.id.as_str().to_string(),
                    None,
                    format!("{base}/text"),
                ),
            );
            for (oi, _) in node.options.iter().enumerate() {
                for f in ["label", "tooltip"] {
                    dlg.insert(
                        format!("dlg.{np}.{nd}.opt.{oi}.{f}"),
                        (
                            tree.npc.as_str().to_string(),
                            node.id.as_str().to_string(),
                            Some(oi),
                            format!("{base}/options/{oi}/{f}"),
                        ),
                    );
                }
            }
        }
    }
    let npc_local: BTreeMap<String, String> = c
        .npcs
        .content
        .npcs
        .iter()
        .map(|n| {
            (
                local_id(n.id.as_str()).to_string(),
                n.id.as_str().to_string(),
            )
        })
        .collect();

    bind.walks = walks.len();
    bind.dialogue_declared = dlg.keys().filter(|k| inv.contains_key(*k)).count();
    let mut read_dialogue: BTreeSet<String> = BTreeSet::new();
    let mut all_uses: BTreeSet<(String, usize)> = BTreeSet::new();
    let mut worst: BTreeMap<(String, usize), Untold> = BTreeMap::new();
    for (label, pt) in &walks {
        let journal_states = states_of(&flow, pt, &casts);
        // When each key is first readable on this walk.
        let mut first: BTreeMap<&str, At> = BTreeMap::new();
        for (at, st) in journal_states.iter().enumerate() {
            for key in readable(&inv, st, &sites, &objectives, &quests, &dlg) {
                first.entry(key).or_insert(at);
            }
        }
        // When each name is told.
        let mut told: Vec<Option<At>> = vec![None; names.len()];
        let mut shown: Vec<Vec<(At, Shown)>> = vec![Vec::new(); names.len()];
        for (i, n) in names.iter().enumerate() {
            let own = match own_telling(&n.key) {
                OwnTelling::Npc(np) => npc_local.get(&np).and_then(|npc| {
                    journal_states
                        .iter()
                        .position(|st| st.screen.nodes.iter().any(|(who, _)| who == npc))
                }),
                OwnTelling::Start => Some(0),
                OwnTelling::WithString => first.get(n.key.as_str()).copied(),
                OwnTelling::Label => {
                    if let Some(&at) = first.get(n.key.as_str()) {
                        shown[i].push((at, Shown::Label(n.key.clone())));
                    } else {
                        shown[i].push((0, Shown::Label(n.key.clone())));
                    }
                    None
                }
            };
            told[i] = own;
        }
        for (key, &at) in &first {
            let Some(text) = inv.get(*key) else { continue };
            if matches!(key_kind(key), Some(TextKind::Name | TextKind::ItemName)) {
                continue; // a name's own label is not a sentence about it
            }
            for (i, s, e) in mentions(text, &names) {
                if introduces(text, s, e) {
                    told[i] = Some(told[i].map_or(at, |t| t.min(at)));
                } else if is_use(text, &names[i].core, names[i].text != names[i].core, s, e) {
                    shown[i].push((at, Shown::Mention(key.to_string())));
                }
            }
        }
        // Every use in a dialogue line or option, against when it was told.
        for (key, &at) in &first {
            if !dlg.contains_key(*key) {
                continue;
            }
            let Some(text) = inv.get(*key) else { continue };
            read_dialogue.insert(key.to_string());
            for (i, s, e) in mentions(text, &names) {
                if !is_use(text, &names[i].core, names[i].text != names[i].core, s, e)
                    || introduces(text, s, e)
                {
                    continue;
                }
                all_uses.insert((key.to_string(), i));
                if told[i].is_some_and(|t| t <= at) {
                    continue;
                }
                let mut before: Vec<Shown> = shown[i]
                    .iter()
                    .filter(|(a, s)| *a <= at && !matches!(s, Shown::Mention(k) if k == key))
                    .map(|(_, s)| s.clone())
                    .collect();
                before.sort();
                before.dedup();
                let slot = (key.to_string(), i);
                let keep = worst.get(&slot).is_none_or(|u| at < u.at);
                if keep {
                    worst.insert(
                        slot,
                        Untold {
                            at,
                            walk: label.clone(),
                            shown: before,
                        },
                    );
                }
            }
        }
    }

    bind.dialogue_read = read_dialogue.len();
    bind.uses = all_uses.len();
    bind.untold = worst.len();
    let mut d = Vec::new();
    for ((key, i), u) in worst {
        let n = &names[i];
        let Some((npc, node, opt, pointer)) = dlg.get(&key) else {
            continue;
        };
        let text = inv.get(&key).cloned().unwrap_or_default();
        let whose = match opt {
            None => format!("`{npc}`'s line in `{node}`"),
            Some(o) => format!("option {o} of `{node}` (the player's own words)"),
        };
        let when = if u.at == 0 {
            "before the first step".to_string()
        } else {
            format!("after step {} of {}", u.at, u.walk)
        };
        let seen = if u.shown.is_empty() {
            "nothing the player read before it carries the name".to_string()
        } else {
            let mut parts: Vec<String> = u
                .shown
                .iter()
                .take(SHOWN_LISTED)
                .map(|s| match s {
                    Shown::Label(k) => format!("the label `{k}` shows it"),
                    Shown::Mention(k) => format!("`{k}` mentions it as if known"),
                })
                .collect();
            if u.shown.len() > SHOWN_LISTED {
                parts.push(format!(
                    "{} more string(s) mention it",
                    u.shown.len() - SHOWN_LISTED
                ));
            }
            format!(
                "{} — a label shows a name and a definite mention presupposes it; neither says \
                 what it is",
                parts.join("; ")
            )
        };
        d.push(Diagnostic::error(
            DW_NAME_UNTOLD,
            "dialogue",
            pointer.clone(),
            format!(
                "{whose} uses `{}` (declared by `{}`) {when}, and the player has not been told \
                 what it is: {seen}. Text: `{text}`. Tell it first, in a line the player reads \
                 before this one: the name with what it is (`{core}, <what it is>,` or \
                 `{core} is <what it is>`), `called {core}`, or an indefinite that brings it \
                 in (`a … {core}`) — or have the speaker say it in words the player already \
                 has (game-writing.md §3 N1, N3)",
                n.text,
                n.key,
                core = n.core
            ),
        ));
    }
    (d, bind)
}

/// One walk state: the dialogue on screen and the walk's own progress, with the
/// keys of the effects that fired to reach it.
struct State {
    screen: crate::compiler::flow::DialogueOnScreen,
    active: BTreeSet<String>,
    done: BTreeSet<String>,
    fired: BTreeSet<String>,
}

/// The states of walking `pt`: before the first step, then after each.
fn states_of(
    flow: &Flow<'_>,
    pt: &crate::compiler::flow::Playthrough,
    casts: &BTreeMap<String, crate::compiler::cast::NpcCast>,
) -> Vec<State> {
    let mut w = flow.walk();
    let mut out = Vec::new();
    let mut done: BTreeSet<String> = BTreeSet::new();
    let mut fired: BTreeSet<String> = BTreeSet::new();
    out.push(State {
        screen: w.dialogue_on_screen(casts),
        active: w.active_quests().clone(),
        done: done.clone(),
        fired: fired.clone(),
    });
    for step in &pt.steps {
        if w.ended() {
            break;
        }
        let j = w.take(step);
        done.insert(step.objective.clone());
        fired.extend(j.fired);
        out.push(State {
            screen: w.dialogue_on_screen_after(casts, step),
            active: w.active_quests().clone(),
            done: done.clone(),
            fired: fired.clone(),
        });
    }
    out
}

/// The inventory keys the player can read at `st`.
fn readable<'k>(
    inv: &'k BTreeMap<String, String>,
    st: &State,
    sites: &BTreeMap<String, String>,
    objectives: &BTreeMap<(String, String), (String, String, Vec<String>)>,
    quests: &BTreeMap<String, String>,
    dlg: &BTreeMap<String, (String, String, Option<usize>, String)>,
) -> Vec<&'k str> {
    let mut out = Vec::new();
    for key in inv.keys() {
        let segs: Vec<&str> = key.split('.').collect();
        let now = match segs[0] {
            "dlg" => match dlg.get(key) {
                Some((npc, node, None, _)) => {
                    st.screen.nodes.contains(&(npc.clone(), node.clone()))
                }
                Some((npc, node, Some(o), _)) => {
                    st.screen.options.contains(&(npc.clone(), node.clone(), *o))
                }
                None => false,
            },
            "obj" if segs.len() == 4 => {
                match objectives.get(&(segs[1].to_string(), segs[2].to_string())) {
                    Some((q, o, after)) => {
                        if segs[3] == "item_name" {
                            st.done.contains(o)
                        } else {
                            st.active.contains(q) && after.iter().all(|a| st.done.contains(a))
                                || st.done.contains(o)
                        }
                    }
                    None => false,
                }
            }
            "quest" | "cast" => quests
                .get(segs.get(1).copied().unwrap_or(""))
                .is_some_and(|q| st.active.contains(q)),
            "fx" => {
                let base = &key[..key.rfind('.').unwrap_or(key.len())];
                match sites.get(base) {
                    Some(path) if path.starts_with("/content/quests/") => {
                        st.fired.contains(path)
                            || st.fired.iter().any(|f| path.starts_with(&format!("{f}/")))
                    }
                    // Not dated by the walk: readable from the start.
                    _ => true,
                }
            }
            // Names, labels, refusals and the world's own text: from the start.
            _ => true,
        };
        if now {
            out.push(key.as_str());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_question_ends_in_a_question_mark_in_either_script() {
        assert!(is_question("What was this keep keeping?"));
        assert!(is_question("这里是什么地方？"));
        assert!(is_question("\"Who are you?\" "));
        assert!(!is_question("Read the label."));
        assert!(!is_question("Why? Never mind."));
    }

    #[test]
    fn an_indefinite_or_a_gloss_tells_and_a_definite_mention_does_not() {
        let t = "A watch of dead soldiers stands in the side room.";
        let (s, e) = occurrences(t, "Watch")[0];
        assert!(introduces(t, s, e));
        let t = "The Watch, the keep's dead garrison, comes up out of the floor.";
        let (s, e) = occurrences(t, "Watch")[0];
        assert!(introduces(t, s, e));
        let t = "The last of the watch goes still.";
        let (s, e) = occurrences(t, "Watch")[0];
        assert!(!introduces(t, s, e));
        assert!(is_use(t, "Watch", true, s, e));
    }

    #[test]
    fn a_copula_glosses_and_a_possession_presupposes() {
        let t = "Halvard is the bell-warden.";
        let (s, e) = occurrences(t, "Halvard")[0];
        assert!(introduces(t, s, e));
        let t = "Your tallow is where you left it, still warm.";
        let (s, e) = occurrences(t, "Tallow")[0];
        assert!(!introduces(t, s, e));
        let t = "Vesperhold. A king, a court, a bell.";
        let (s, e) = occurrences(t, "Vesperhold")[0];
        assert!(introduces(t, s, e));
        let t = "This is the Quiet Keep.";
        let (s, e) = occurrences(t, "Quiet Keep")[0];
        assert!(introduces(t, s, e));
        let t = "The watch was put down.";
        let (s, e) = occurrences(t, "Watch")[0];
        assert!(!introduces(t, s, e));
    }

    #[test]
    fn a_common_word_is_not_the_name() {
        let t = "I keep watch here.";
        let (s, e) = occurrences(t, "Watch")[0];
        assert!(!is_use(t, "Watch", true, s, e));
        let t = "This keep is mine to guard.";
        let (s, e) = occurrences(t, "Keep")[0];
        assert!(!is_use(t, "Keep", true, s, e));
        let t = "I run. Keep your road.";
        let (s, e) = occurrences(t, "Keep")[0];
        assert!(!is_use(t, "Keep", true, s, e));
        let t = "My blanket's by the counter. Tallow only.";
        let (s, e) = occurrences(t, "Tallow")[0];
        assert!(is_use(t, "Tallow", false, s, e));
        let t = "You pulled the lever at my stand.";
        let (s, e) = occurrences(t, "Stand")[0];
        assert!(is_use(t, "Stand", true, s, e));
    }

    #[test]
    fn a_shorter_name_inside_a_longer_one_is_the_longer_one() {
        let names = vec![
            Name {
                text: "Warden".into(),
                key: "a".into(),
                core: "Warden".into(),
            },
            Name {
                text: "Warden's Door".into(),
                key: "b".into(),
                core: "Warden's Door".into(),
            },
        ];
        let m = mentions("Open the Warden's Door.", &names);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].0, 1);
    }
}
