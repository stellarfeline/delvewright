//! What the text tells the player: a question gets an answer (`DW0981`), a
//! name reaches a line only after the player has been told it (`DW0982`), a
//! name tag marks a person, never one of a crowd (`DW0983`), an objective's
//! title says the act that completes it (`DW1045`, spec-0114), a hint that
//! names an act names that one (a `DW1045` warning, spec-0104 §5), and a
//! hidden way's secret is kept until the shot that shows it (`DW1060`,
//! spec-0118).
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

use delvewright_dsl::l10n::{
    L10nDoc, TextKind, effect_string_sites, inventory, key_kind, local_id,
};
use delvewright_dsl::{Act, Campaign, Diagnostic, acts_line};

use crate::compiler::flow::Flow;
use delvewright_dsl::{DwCode, ExitTier};

delvewright_dsl::dw_code! {
    /// `DW0981`: a dialogue option whose label or tooltip is a question leads to
    /// no line that could answer it.
    pub const DW_QUESTION_UNANSWERED: DwCode = DwCode::new("DW0981", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0982`: a declared name reaches an NPC's line, an option, or an
    /// announced objective's title or hint (spec-0104 §7) before the play order
    /// has told the player what it is.
    pub const DW_NAME_UNTOLD: DwCode = DwCode::new("DW0982", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0983`: a custom name tags a body that is one of many — the members of a
    /// wave, or any body sharing a name that tags a group.
    pub const DW_NAME_ON_A_CROWD: DwCode = DwCode::new("DW0983", ExitTier::Build);
}

// ---------------------------------------------------------------------------
// DW0983 — a name tag is a person
// ---------------------------------------------------------------------------

/// What `DW0983` examined, zeroes included.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CrowdBinding {
    /// Waves declared.
    pub waves: usize,
    /// Waves with at least one named member.
    pub named_waves: usize,
    /// Named actors.
    pub named_actors: usize,
    /// Names that tag a group.
    pub group_names: usize,
    /// Waves refused.
    pub refused_waves: usize,
    /// Actors refused.
    pub refused_actors: usize,
}

impl CrowdBinding {
    /// The binding line `delvec validate` prints.
    pub fn line(&self) -> String {
        format!(
            "name-tag binding (DW0983): {} of {} wave(s) carry a named member, {} named \
             actor(s); {} name(s) tag a group; {} wave(s) and {} actor(s) refused",
            self.named_waves,
            self.waves,
            self.named_actors,
            self.group_names,
            self.refused_waves,
            self.refused_actors
        )
    }
}

/// `DW0983`: a name tag belongs to a unique character. The object decides
/// which bodies are unique: a name is a **group's** when one wave entry with
/// that name spawns more than one body, or when two entries of one wave carry
/// it; a group's name names no one, so every wave member and every actor that
/// carries it is refused, in every wave. A name borne by exactly one body of
/// each encounter it appears in (an elite or a boss inside a wave, a named
/// actor, the same character as a wave member and later as an actor) is a
/// person, and stands.
pub fn check_name_tags(c: &Campaign) -> (Vec<Diagnostic>, CrowdBinding) {
    let q = &c.quests.content;
    let mut bind = CrowdBinding {
        waves: q.waves.len(),
        ..CrowdBinding::default()
    };
    // A name is compared as it is drawn: a styled span (spec-0096) is its own
    // text, so `[[bold|Footman]]` and `Footman` are one name over two bodies.
    let shown = |n: &str| delvewright_dsl::textstyle::visible(n).into_owned();
    let mut group: BTreeMap<String, String> = BTreeMap::new();
    for w in &q.waves {
        let mut bodies: BTreeMap<String, (u32, usize)> = BTreeMap::new();
        for m in &w.mobs {
            if let Some(n) = m.name.as_deref() {
                let e = bodies.entry(shown(n)).or_default();
                e.0 += m.count;
                e.1 += 1;
            }
        }
        if !bodies.is_empty() {
            bind.named_waves += 1;
        }
        for (n, (count, rows)) in bodies {
            if count > 1 {
                group.entry(n).or_insert_with(|| {
                    format!(
                        "`{}` spawns {count} bodies under it{}",
                        w.id,
                        if rows > 1 {
                            format!(" across {rows} entries")
                        } else {
                            String::new()
                        }
                    )
                });
            }
        }
    }
    bind.named_actors = q.actors.iter().filter(|a| a.name.is_some()).count();
    bind.group_names = group.len();
    let mut d = Vec::new();
    for (wi, w) in q.waves.iter().enumerate() {
        let mut hit = false;
        for (mi, m) in w.mobs.iter().enumerate() {
            let Some(n) = m.name.as_deref() else { continue };
            let Some(why) = group.get(&shown(n)) else {
                continue;
            };
            hit = true;
            d.push(Diagnostic::error(
                DW_NAME_ON_A_CROWD,
                "quests",
                format!("/content/waves/{wi}/mobs/{mi}/name"),
                format!(
                    "`{}`'s entry {mi} (`{}` × {}) wears the name `{n}`, and that name tags a \
                     group: {why}. A name tag belongs to a character the story treats as a \
                     person — a boss, an elite, a named actor, an NPC — and a crowd of \
                     ordinary bodies wears none (game-writing.md §3 N6). Remove `name` from the \
                     entry; if the fight needs a heading, state the wave's `health_bar.title`. \
                     If one of these bodies is a character, give it its own entry of `count: 1` \
                     and its own name",
                    w.id, m.entity, m.count
                ),
            ));
        }
        if hit {
            bind.refused_waves += 1;
        }
    }
    for (ai, a) in q.actors.iter().enumerate() {
        let Some(n) = a.name.as_deref() else { continue };
        let Some(why) = group.get(&shown(n)) else {
            continue;
        };
        bind.refused_actors += 1;
        d.push(Diagnostic::error(
            DW_NAME_ON_A_CROWD,
            "quests",
            format!("/content/actors/{ai}/name"),
            format!(
                "actor `{}` wears the name `{n}`, and that name tags a group: {why}. An actor is \
                 one body, and its name tag says it is a person; a name a crowd also wears says \
                 it is one of them (game-writing.md §3 N6). Give the actor a name of its own, or \
                 none",
                a.id
            ),
        ));
    }
    (d, bind)
}

// ---------------------------------------------------------------------------
// DW0981 — a question gets an answer
// ---------------------------------------------------------------------------

/// Whether `text` is a question: its last visible character is `?` or `？`.
/// A styled span (spec-0096) is read as the text it draws, so a question
/// wrapped in markup still ends in its own question mark.
pub fn is_question(text: &str) -> bool {
    let shown = delvewright_dsl::textstyle::visible(text);
    let t = shown.trim_end_matches(|c: char| {
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

/// Whether a character continues a word for [`occurrences`]' boundaries: a
/// letter or a digit of a script that spaces its words. A Han, kana or Hangul
/// character is a word of its own, so a word written in one is found inside
/// the unspaced sentence around it.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric()
        && !matches!(c as u32,
            0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF)
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
        let word = |c: Option<char>| c.is_some_and(is_word_char);
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
    /// Objective titles and hints of announced objectives the walks put on
    /// screen, counted once each (spec-0104 §7).
    pub objective_read: usize,
    /// Objective titles and hints of announced objectives the documents declare.
    pub objective_declared: usize,
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
             string(s) and {} of {} objective string(s) on screen on some walk; {} use(s) of a \
             name in them, {} untold",
            self.names,
            self.walks,
            self.dialogue_read,
            self.dialogue_declared,
            self.objective_read,
            self.objective_declared,
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
    // Every string as the player reads it (spec-0096): a styled span is its own
    // text, so a line that uses or glosses a name in markup is matched by the
    // words it draws — except an obfuscated span, which is drawn as noise and
    // tells nothing and uses nothing (`textstyle::legible`). A name itself is
    // what the documents declare it to be, its markup dropped (`visible`): an
    // obfuscated name is still that name when a legible line uses it.
    let raw = inventory(c);
    let inv = read_inventory(c);
    let declared: BTreeMap<String, String> = raw
        .iter()
        .map(|(k, v)| {
            let shown = delvewright_dsl::textstyle::visible(v).into_owned();
            (k.clone(), shown)
        })
        .collect();
    let mut names: Vec<Name> = declared
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

    let Some(placed) = placements(c, &inv) else {
        return (Vec::new(), bind); // DW0201/DW0130 own a campaign with no play order
    };
    let Sites { dlg, npc_local, .. } = &placed.sites;

    bind.walks = placed.walks.len();
    // spec-0104 §7: an announced objective's title and hint are uses too —
    // what the party reads as its instruction names only what it has been
    // told. Keyed like the dialogue map: the key → (quest id, objective id,
    // pointer). An unannounced objective's strings never reach a screen.
    let guidance = &c.quests.content.guidance;
    let mut obj_strings: BTreeMap<String, (String, String)> = BTreeMap::new();
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        let ql = local_id(q.id.as_str());
        for (oi, o) in q.objectives.iter().enumerate() {
            if !o.announced(guidance) {
                continue;
            }
            let ol = local_id(o.id().as_str());
            for f in ["title", "hint"] {
                obj_strings.insert(
                    format!("obj.{ql}.{ol}.{f}"),
                    (
                        o.id().as_str().to_string(),
                        format!("/content/quests/{qi}/objectives/{oi}/{f}"),
                    ),
                );
            }
        }
    }
    bind.dialogue_declared = dlg.keys().filter(|k| inv.contains_key(*k)).count();
    bind.objective_declared = obj_strings.keys().filter(|k| inv.contains_key(*k)).count();
    let mut read_dialogue: BTreeSet<String> = BTreeSet::new();
    let mut read_objective: BTreeSet<String> = BTreeSet::new();
    let mut all_uses: BTreeSet<(String, usize)> = BTreeSet::new();
    let mut worst: BTreeMap<(String, usize), Untold> = BTreeMap::new();
    for walk in &placed.walks {
        let label = &walk.label;
        let journal_states = &walk.states;
        let first: BTreeMap<&str, At> =
            walk.first.iter().map(|(k, p)| (k.as_str(), p.at)).collect();
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
        // Every use in a dialogue line or option, or in an announced
        // objective's title or hint, against when it was told.
        for (key, &at) in &first {
            if dlg.contains_key(*key) {
                if inv.contains_key(*key) {
                    read_dialogue.insert(key.to_string());
                }
            } else if obj_strings.contains_key(*key) {
                if inv.contains_key(*key) {
                    read_objective.insert(key.to_string());
                }
            } else {
                continue;
            }
            let Some(text) = inv.get(*key) else { continue };
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
    bind.objective_read = read_objective.len();
    bind.uses = all_uses.len();
    bind.untold = worst.len();
    let mut d = Vec::new();
    for ((key, i), u) in worst {
        let n = &names[i];
        let (whose, stage, pointer) = if let Some((npc, node, opt, pointer)) = dlg.get(&key) {
            (
                match opt {
                    None => format!("`{npc}`'s line in `{node}`"),
                    Some(o) => format!("option {o} of `{node}` (the player's own words)"),
                },
                "dialogue",
                pointer,
            )
        } else if let Some((oid, pointer)) = obj_strings.get(&key) {
            let field = if key.ends_with(".hint") {
                "hint"
            } else {
                "title"
            };
            (
                format!("the {field} of objective `{oid}` (the party's instruction)"),
                "quests",
                pointer,
            )
        } else {
            continue;
        };
        let text = inv.get(&key).cloned().unwrap_or_default();
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
            stage,
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

/// Every inventoried string as the player reads it (spec-0096): a styled span
/// is its own text, an obfuscated span is drawn as noise and reads as nothing
/// (`textstyle::legible`). The one reading both `DW0982` and `DW1060` place.
pub(crate) fn read_inventory(c: &Campaign) -> BTreeMap<String, String> {
    inventory(c)
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                delvewright_dsl::textstyle::legible(v).into_owned(),
            )
        })
        .collect()
}

/// What a string's key is tied to in the documents, for placing it on a walk:
/// an effect's string by its pointer, an objective's and a quest's by their
/// ids, a dialogue string by its tree, node and option.
pub(crate) struct Sites {
    /// Effect string key base → the pointer of the effect that carries it.
    pub(crate) fx: BTreeMap<String, String>,
    /// `(quest local id, objective local id)` → `(quest id, objective id,
    /// after, announced)`. An objective that is not announced
    /// (`Objective::announced`, spec-0093) never puts its title or
    /// hint on a screen.
    objectives: BTreeMap<(String, String), (String, String, Vec<String>, bool)>,
    /// Quest local id → quest id.
    quests: BTreeMap<String, String>,
    /// Dialogue key → `(npc id, node id, option index, pointer)`.
    dlg: BTreeMap<String, (String, String, Option<usize>, String)>,
    /// NPC local id → NPC id.
    npc_local: BTreeMap<String, String>,
}

impl Sites {
    fn of(c: &Campaign) -> Sites {
        // Effect strings by key base, and the pointer the replay records them under.
        let fx: BTreeMap<String, String> = effect_string_sites(c)
            .into_iter()
            .map(|(_, path, kb)| (kb, path))
            .collect();
        // Objectives and quests by the local ids their keys carry.
        let mut objectives: BTreeMap<(String, String), (String, String, Vec<String>, bool)> =
            BTreeMap::new();
        let mut quests: BTreeMap<String, String> = BTreeMap::new();
        let guidance = &c.quests.content.guidance;
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
                        o.announced(guidance),
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
        Sites {
            fx,
            objectives,
            quests,
            dlg,
            npc_local,
        }
    }
}

/// Where one string first reaches the player on one walk: the state index,
/// and whether the walk dates it at all. An undated string (an environment
/// trigger, a refusal, a reaction bundle, loot, a name or a label) is placed
/// at the start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Placement {
    /// The state index: `0` before the first step, `k` after the `k`-th.
    pub(crate) at: At,
    /// Whether the walk dates it; `false` is placed at the start.
    pub(crate) dated: bool,
}

/// One walk of the play order, every inventoried string placed on it.
pub(crate) struct PlacedWalk {
    /// The walk in words (`the critical path`, ``branch `…` ``).
    pub(crate) label: String,
    /// The walk's states, before the first step and after each.
    states: Vec<State>,
    /// Every readable key, at the first state it can be read.
    pub(crate) first: BTreeMap<String, Placement>,
}

impl PlacedWalk {
    /// The first state at which the effect at `pointer` has fired on this
    /// walk — the rule an effect string is dated by ([`fired_covers`]) — or
    /// `None` when the walk never fires it.
    pub(crate) fn fires_at(&self, pointer: &str) -> Option<At> {
        self.states
            .iter()
            .position(|st| fired_covers(&st.fired, pointer))
    }
}

/// Every walk's string placements, with the sites they were read against.
pub(crate) struct Placements {
    /// The critical path, then every realised branch.
    pub(crate) walks: Vec<PlacedWalk>,
    /// What the keys are tied to.
    pub(crate) sites: Sites,
}

/// **The one placement of every string on the play order** — the walk
/// [`Flow::journal`] takes along the exported critical path and every realised
/// branch, and the first state at which the player can read each inventoried
/// key ([`when_readable`]). `DW0982` (a name told before it is used) and
/// `DW1060` (a secret kept until its shot) both read their strings' steps
/// from here and from no copy. `None` for a campaign with no play order.
pub(crate) fn placements(c: &Campaign, inv: &BTreeMap<String, String>) -> Option<Placements> {
    let flow = Flow::new(c);
    let casts = crate::compiler::cast::npc_casts(c);
    let mut walks: Vec<(String, crate::compiler::flow::Playthrough)> = Vec::new();
    let main = flow.playthrough();
    if main.degenerate || main.cyclic {
        return None;
    }
    walks.push(("the critical path".to_string(), main));
    for r in crate::compiler::branch::realize(c) {
        if let Some(w) = r.world {
            walks.push((
                format!("branch `{}`", r.branch.id),
                flow.story_playthrough(w),
            ));
        }
    }
    let sites = Sites::of(c);
    let walks = walks
        .into_iter()
        .map(|(label, pt)| {
            let states = states_of(&flow, &pt, &casts);
            let mut first: BTreeMap<String, Placement> = BTreeMap::new();
            for (at, st) in states.iter().enumerate() {
                for key in inv.keys() {
                    if first.contains_key(key) {
                        continue;
                    }
                    let placed = match when_readable(key, st, &sites) {
                        Readable::Undated => Some(Placement { at, dated: false }),
                        Readable::Dated(true) => Some(Placement { at, dated: true }),
                        Readable::Dated(false) => None,
                    };
                    if let Some(p) = placed {
                        first.insert(key.clone(), p);
                    }
                }
            }
            PlacedWalk {
                label,
                states,
                first,
            }
        })
        .collect();
    Some(Placements { walks, sites })
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
    let mut w = flow.walk_for(pt);
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

/// Whether the effect at `pointer` has fired by a state whose fired lists
/// are `fired`: the list itself, or one that holds it.
fn fired_covers(fired: &BTreeSet<String>, pointer: &str) -> bool {
    fired.contains(pointer) || fired.iter().any(|f| pointer.starts_with(&format!("{f}/")))
}

/// Whether the player can read a key at a state, or whether the walk dates
/// the key at all.
enum Readable {
    /// The walk does not date the key: readable from the start.
    Undated,
    /// The walk dates the key, and it is readable at this state or not.
    Dated(bool),
}

/// **The one rule a key is dated by** on a walk: a dialogue string while its
/// node or option is on screen, an announced objective's title and hint once
/// its `after` is done — an objective that is not announced never shows them
/// (its item name once it is done), a quest's goal and cast while it
/// runs, an effect's strings once the effect fires. Names, labels, refusals
/// and the world's own text — and an effect outside the quests — are not
/// dated.
fn when_readable(key: &str, st: &State, sites: &Sites) -> Readable {
    let segs: Vec<&str> = key.split('.').collect();
    match segs[0] {
        "dlg" => Readable::Dated(match sites.dlg.get(key) {
            Some((npc, node, None, _)) => st.screen.nodes.contains(&(npc.clone(), node.clone())),
            Some((npc, node, Some(o), _)) => {
                st.screen.options.contains(&(npc.clone(), node.clone(), *o))
            }
            None => false,
        }),
        "obj" if segs.len() == 4 => Readable::Dated(
            match sites
                .objectives
                .get(&(segs[1].to_string(), segs[2].to_string()))
            {
                Some((q, o, after, announced)) => {
                    if segs[3] == "item_name" {
                        st.done.contains(o)
                    } else {
                        // spec-0104 §7: only an announced objective's title and
                        // hint reach a screen (the announcement, the journal,
                        // the compass menu).
                        *announced
                            && (st.active.contains(q) && after.iter().all(|a| st.done.contains(a))
                                || st.done.contains(o))
                    }
                }
                None => false,
            },
        ),
        "quest" | "cast" => Readable::Dated(
            sites
                .quests
                .get(segs.get(1).copied().unwrap_or(""))
                .is_some_and(|q| st.active.contains(q)),
        ),
        "fx" => {
            let base = &key[..key.rfind('.').unwrap_or(key.len())];
            match sites.fx.get(base) {
                Some(path) if path.starts_with("/content/quests/") => {
                    Readable::Dated(fired_covers(&st.fired, path))
                }
                // Not dated by the walk: readable from the start.
                _ => Readable::Undated,
            }
        }
        // Names, labels, refusals and the world's own text: from the start.
        _ => Readable::Undated,
    }
}

// ---------------------------------------------------------------------------
// DW1060 — a secret is kept until the shot that shows it (spec-0118 §6.4)
// ---------------------------------------------------------------------------

delvewright_dsl::dw_code! {
    /// `DW1060`: a word of a hidden way's `secret` is mentioned by a
    /// player-facing string the play order places before the shot that shows
    /// the way, or is mentioned by no string at or after it.
    pub const DW_SECRET_TOLD: DwCode = DwCode::new("DW1060", ExitTier::Build);
}

/// Where a mention of a secret word stands: the string, its language, and
/// where the walk reads it against the shot.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Mention {
    /// The inventory key.
    key: String,
    /// `en`, or the sidecar's language.
    lang: String,
    /// The walk it is read on.
    walk: String,
    /// When it is read: `None` when the walk does not date it.
    at: Option<At>,
}

/// **The spans of a secret word in a text** — `DW0982`'s matcher, shared:
/// the word's article-free core ([`core_of`]), on word boundaries,
/// case-insensitive ([`occurrences`]). A definite reference (`the passage`) is
/// an occurrence of the core.
fn secret_spans(text: &str, word: &str) -> Vec<(usize, usize)> {
    occurrences(text, &core_of(word))
}

/// What `DW1060` examined, zeroes included.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SecretBinding {
    /// Hidden ways (`learned: hidden` on a connection the world builds shut).
    pub hidden_ways: usize,
    /// Secret words across them.
    pub words: usize,
    /// Hidden ways with no shot to read against (`DW1054`'s, at the build).
    pub no_shot: usize,
    /// Distinct `(string, language)` pairs some walk places before a shot.
    pub read_before: usize,
    /// Mentions refused: `(way, word)` told early, plus words never told.
    pub refused: usize,
    /// Words some string at or after the shot mentions, on every walk the shot
    /// fires on.
    pub told_after: usize,
}

impl SecretBinding {
    /// The binding line `delvec validate` prints.
    pub fn line(&self) -> String {
        format!(
            "secrets (DW1060): {} hidden way(s), {} secret word(s); {} string(s) read before the \
             shot(s), {} mention(s) refused; {} word(s) told after; {} hidden way(s) with no shot \
             to read against",
            self.hidden_ways,
            self.words,
            self.read_before,
            self.refused,
            self.told_after,
            self.no_shot,
        )
    }
}

/// One hidden way's text, judged: per secret word, the first mention before
/// the shot and whether some string at or after it tells the word.
struct SecretFinding {
    way: crate::compiler::nav::HiddenWay,
    /// The shot's list and index, or `None` when nothing shows the way.
    shot: Option<(String, usize)>,
    /// Per word: the first early mention, by walk order then step then key.
    early: Vec<Option<Mention>>,
    /// Per word: told at or after the shot on every walk the shot fires on.
    told: Vec<bool>,
    /// `(key, language)` pairs read before the shot on some walk.
    read_before: BTreeSet<(String, String)>,
}

/// Whether the effect string `key` is a string of the shot's own list placed
/// before the shot in it (`{list}/{j}` with `j` < the shot's index).
fn before_in_list(sites: &Sites, key: &str, list: &str, shot: usize) -> bool {
    let Some(base) = key.rfind('.').map(|i| &key[..i]) else {
        return false;
    };
    let Some(path) = sites.fx.get(base) else {
        return false;
    };
    path.strip_prefix(&format!("{list}/"))
        .and_then(|rest| rest.split('/').next())
        .and_then(|j| j.parse::<usize>().ok())
        .is_some_and(|j| j < shot)
}

/// Judge every hidden way's secret against the play order (spec-0118 §6.4).
fn judge_secrets(c: &Campaign, sidecars: &BTreeMap<String, L10nDoc>) -> Option<Vec<SecretFinding>> {
    let ways = crate::compiler::nav::hidden_ways(c);
    if ways.is_empty() {
        return Some(Vec::new());
    }
    let inv = read_inventory(c);
    let placed = placements(c, &inv)?;
    let mut langs: Vec<(&str, &L10nDoc)> = Vec::new();
    for lang in &c.world.content.languages {
        if let Some(doc) = sidecars.get(lang) {
            langs.push((lang.as_str(), doc));
        }
    }
    // Every rendition of a key: the English source, then each sidecar's.
    let texts = |key: &str| -> Vec<(String, String)> {
        let mut out = Vec::new();
        if let Some(t) = inv.get(key) {
            out.push(("en".to_string(), t.clone()));
        }
        for (lang, doc) in &langs {
            if let Some(t) = doc.content.get(key) {
                out.push((
                    (*lang).to_string(),
                    delvewright_dsl::textstyle::legible(t).into_owned(),
                ));
            }
        }
        out
    };
    let mut out = Vec::new();
    for way in ways {
        let shot = crate::compiler::nav::opening_lists(c, &way.seam)
            .into_iter()
            .find_map(|l| l.shot.map(|i| (l.list, i)));
        let n = way.secret.len();
        let mut f = SecretFinding {
            way,
            shot: shot.clone(),
            early: vec![None; n],
            told: vec![false; n],
            read_before: BTreeSet::new(),
        };
        let Some((list, idx)) = shot else {
            out.push(f);
            continue;
        };
        let pointer = format!("{list}/{idx}");
        let mut told_on_every: Vec<Option<bool>> = vec![None; n];
        for walk in &placed.walks {
            let shot_at = walk.fires_at(&pointer);
            let mut told_here = vec![false; n];
            for (key, p) in &walk.first {
                let before = !p.dated
                    || shot_at.is_none_or(|s| p.at < s)
                    || (Some(p.at) == shot_at && before_in_list(&placed.sites, key, &list, idx));
                for (lang, text) in texts(key) {
                    if before {
                        f.read_before.insert((key.clone(), lang.clone()));
                    }
                    for (k, word) in f.way.secret.iter().enumerate() {
                        if secret_spans(&text, word).is_empty() {
                            continue;
                        }
                        if before {
                            let m = Mention {
                                key: key.clone(),
                                lang: lang.clone(),
                                walk: walk.label.clone(),
                                at: p.dated.then_some(p.at),
                            };
                            if f.early[k].is_none() {
                                f.early[k] = Some(m);
                            }
                        } else {
                            told_here[k] = true;
                        }
                    }
                }
            }
            if shot_at.is_some() {
                for k in 0..n {
                    let prev = told_on_every[k].unwrap_or(true);
                    told_on_every[k] = Some(prev && told_here[k]);
                }
            }
        }
        f.told = told_on_every
            .into_iter()
            .map(|t| t.unwrap_or(false))
            .collect();
        out.push(f);
    }
    Some(out)
}

/// `DW1060` over every hidden way's secret, in English and every sidecar, with
/// what it examined. A hidden way with no shot is counted and judged no
/// further: `DW1054` refuses it at the build.
pub fn check_secrets(
    c: &Campaign,
    sidecars: &BTreeMap<String, L10nDoc>,
) -> (Vec<Diagnostic>, SecretBinding) {
    let mut bind = SecretBinding::default();
    let Some(findings) = judge_secrets(c, sidecars) else {
        return (Vec::new(), bind); // DW0201/DW0130 own a campaign with no play order
    };
    let mut read_before: BTreeSet<(String, String)> = BTreeSet::new();
    let mut d = Vec::new();
    for f in &findings {
        bind.hidden_ways += 1;
        bind.words += f.way.secret.len();
        let Some((list, idx)) = &f.shot else {
            bind.no_shot += 1;
            continue;
        };
        read_before.extend(f.read_before.iter().cloned());
        for (k, word) in f.way.secret.iter().enumerate() {
            let at = format!("/content/edges/{}/gating/secret/{k}", f.way.index);
            if let Some(m) = &f.early[k] {
                bind.refused += 1;
                let when = match m.at {
                    None => "before the shot (undated, read as the start)".to_string(),
                    Some(0) => format!("before the shot, from the start of {}", m.walk),
                    Some(s) => format!("before the shot, after step {s} of {}", m.walk),
                };
                let lang = if m.lang == "en" {
                    "in English".to_string()
                } else {
                    format!("in the `{}` sidecar", m.lang)
                };
                d.push(Diagnostic::error(
                    DW_SECRET_TOLD,
                    "layout-graph",
                    at.clone(),
                    format!(
                        "`{edge}` is declared `learned: hidden`, and its secret word `{word}` is \
                         mentioned by `{key}` {lang}, which the player reads {when}; the shot \
                         that shows the way is the `cutscene` at `{list}/{idx}`. A way the level \
                         reveals is told by that shot and by nothing before it \
                         (game-writing.md §8 L4). Cut the word from the string, or move the \
                         string after the shot.",
                        edge = f.way.edge,
                        key = m.key,
                    ),
                ));
            }
            if f.told[k] {
                bind.told_after += 1;
            } else {
                bind.refused += 1;
                d.push(Diagnostic::error(
                    DW_SECRET_TOLD,
                    "layout-graph",
                    at,
                    format!(
                        "`{edge}` keeps the secret word `{word}`, and no string the player reads \
                         at or after the shot that shows the way (`{list}/{idx}`) mentions it. A \
                         secret nobody ever tells keeps nothing: tell it after the shot — a line \
                         in the shot's own beat, after the `cutscene` — or remove the word.",
                        edge = f.way.edge,
                    ),
                ));
            }
        }
    }
    bind.read_before = read_before.len();
    (d, bind)
}

/// Each hidden way's text verdict, by connection, as the shut-way ledger
/// carries it beside the eye (`validation/shut-ways.json`): the shot, the
/// strings read before it, the first early mention per word, and the words
/// told after. The same judgement [`check_secrets`] refuses on.
pub(crate) fn secret_verdicts(
    c: &Campaign,
    sidecars: &BTreeMap<String, L10nDoc>,
) -> BTreeMap<String, serde_json::Value> {
    let Some(findings) = judge_secrets(c, sidecars) else {
        return BTreeMap::new();
    };
    let mut langs: Vec<String> = vec!["en".to_string()];
    langs.extend(
        c.world
            .content
            .languages
            .iter()
            .filter(|l| sidecars.contains_key(*l))
            .cloned(),
    );
    findings
        .into_iter()
        .map(|f| {
            let words: Vec<serde_json::Value> = f
                .way
                .secret
                .iter()
                .enumerate()
                .map(|(k, w)| {
                    serde_json::json!({
                        "word": w,
                        "told_before": f.early[k].as_ref().map(|m| serde_json::json!({
                            "key": m.key, "language": m.lang, "walk": m.walk, "step": m.at,
                        })),
                        "told_after": f.told[k],
                    })
                })
                .collect();
            (
                f.way.edge.clone(),
                serde_json::json!({
                    "shot": f.shot.as_ref().map(|(l, i)| format!("{l}/{i}")),
                    "languages": langs,
                    "strings_read_before": f.read_before.len(),
                    "words": words,
                }),
            )
        })
        .collect()
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

    /// A small deterministic generator (ADR-0006): no RNG crate, a fixed LCG.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self, n: u64) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 33) % n
        }
    }

    /// spec-0118 acceptance 20: `DW1060`'s matcher is `DW0982`'s — over
    /// generated strings and words, the spans a secret word finds are the
    /// spans the name matcher finds for a name of that text.
    #[test]
    fn the_secret_matcher_is_the_name_matcher() {
        let vocab = [
            "the",
            "passage",
            "Passage",
            "hearth",
            "behind",
            "a",
            "records",
            "sluice",
            "passages",
            "the records",
            "密道",
            "火炉",
            "后面",
            ",",
            ".",
            "an",
            "flue",
        ];
        let words = [
            "passage",
            "the records",
            "behind the hearth",
            "密道",
            "flue",
        ];
        let mut r = Lcg(7);
        let mut compared = 0usize;
        for _ in 0..400 {
            let n = 1 + r.next(9) as usize;
            let mut text = String::new();
            for i in 0..n {
                let w = vocab[r.next(vocab.len() as u64) as usize];
                if i > 0 && !w.chars().all(|c| c as u32 > 0x2E80) && r.next(4) > 0 {
                    text.push(' ');
                }
                text.push_str(w);
            }
            for w in words {
                let name = [Name {
                    text: w.to_string(),
                    key: "k".to_string(),
                    core: core_of(w),
                }];
                let by_name: Vec<(usize, usize)> = mentions(&text, &name)
                    .into_iter()
                    .map(|(_, s, e)| (s, e))
                    .collect();
                assert_eq!(secret_spans(&text, w), by_name, "{text:?} / {w:?}");
                compared += 1;
            }
        }
        assert_eq!(compared, 2000);
    }

    /// A word in a script that does not space its words is found inside the
    /// sentence around it; an English word is held to word boundaries.
    #[test]
    fn a_han_word_is_found_inside_its_sentence() {
        assert_eq!(secret_spans("火炉后面有一条密道。", "密道").len(), 1);
        assert!(secret_spans("passages", "passage").is_empty());
        assert_eq!(secret_spans("The passage, behind.", "the passage").len(), 1);
    }

    fn blockout(hidden: bool) -> Campaign {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/blockout");
        let mut loaded = crate::compiler::load::load_campaign_dir(&dir).expect("readable");
        if hidden {
            let mut graph: serde_json::Value =
                serde_json::from_str(loaded.raw.layout_graph.as_deref().unwrap()).unwrap();
            for e in graph["content"]["edges"].as_array_mut().unwrap() {
                if e["id"] == "edge/hall-cell" {
                    e["gating"]["learned"] = serde_json::json!("hidden");
                    e["gating"]["secret"] = serde_json::json!(["vault"]);
                }
            }
            loaded.raw.layout_graph = Some(graph.to_string());
            let mut q: serde_json::Value = serde_json::from_str(&loaded.raw.quests).unwrap();
            q["content"]["quests"][0]["on_objective_complete"]["obj/ask"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "type": "cutscene", "seconds": 3,
                    "path": [{"anchor": "anchor/node-hall"}],
                    "look_at": {"anchor": "anchor/seam-hall-cell"},
                }));
            loaded.raw.quests = q.to_string();
        }
        delvewright_dsl::parse_campaign(&loaded.raw).expect("parses")
    }

    /// spec-0118 acceptance 21, **one walk**: `DW0982` and `DW1060` read the
    /// step of every string from the one placement — the undated strings
    /// included — and neither keeps a placement of its own: the names check's
    /// walk and on-screen counts, and the secrets check's strings read before
    /// the shot, are each re-derived here from [`placements`] alone.
    #[test]
    fn both_checks_place_every_string_by_one_walk() {
        let c = blockout(true);
        let inv = read_inventory(&c);
        let placed = placements(&c, &inv).expect("a play order");
        let (_, names) = check_names_told_bound(&c);
        assert_eq!(names.walks, placed.walks.len());
        let on_screen: BTreeSet<&str> = placed
            .walks
            .iter()
            .flat_map(|w| w.first.keys())
            .filter(|k| placed.sites.dlg.contains_key(k.as_str()))
            .map(String::as_str)
            .collect();
        assert_eq!(names.dialogue_read, on_screen.len());
        assert!(
            placed.walks[0].first.values().any(|p| !p.dated),
            "the undated case is exercised"
        );

        let list = "/content/quests/0/on_objective_complete/obj/ask";
        let shot = crate::compiler::nav::opening_lists(&c, "anchor/seam-hall-cell");
        assert_eq!(shot.len(), 1);
        assert_eq!(shot[0].list, list);
        let pointer = shot[0].shot_pointer().expect("the shot");
        let mut before: BTreeSet<&str> = BTreeSet::new();
        for w in &placed.walks {
            let at = w.fires_at(&pointer);
            for (k, p) in &w.first {
                if !p.dated
                    || at.is_none_or(|s| p.at < s)
                    || (Some(p.at) == at
                        && before_in_list(&placed.sites, k, list, shot[0].shot.unwrap()))
                {
                    before.insert(k.as_str());
                }
            }
        }
        let (_, secrets) = check_secrets(&c, &BTreeMap::new());
        assert_eq!(secrets.read_before, before.len(), "{}", secrets.line());
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

// ---------------------------------------------------------------------------
// DW1045 — an objective says the act that completes it (spec-0114)
// ---------------------------------------------------------------------------

delvewright_dsl::dw_code! {
    /// `DW1045`: an objective's title opens on a word naming an act its
    /// completion does not perform.
    pub const DW_ACT_UNTRUE: DwCode = DwCode::new("DW1045", ExitTier::Build);
}

/// The longest head phrase the lexicon may hold, in words (spec-0114 §3).
pub const MAX_HEAD_WORDS: usize = 4;

/// The committed English act lexicon (`crates/delvec/data/acts-en.json`): the
/// one place a head phrase is written. Keys are act names, values the head
/// phrases that state that act; a phrase whose object decides its act appears
/// under every act it can take.
pub const ACTS_EN: &str = include_str!("../../data/acts-en.json");

/// The act lexicon, read once from [`ACTS_EN`].
#[derive(Debug)]
pub struct Lexicon {
    /// Head phrase → the acts it states.
    phrases: BTreeMap<String, BTreeSet<Act>>,
    /// Act → its head phrases, sorted.
    by_act: BTreeMap<Act, Vec<String>>,
}

impl Lexicon {
    /// Parse a lexicon document. Every key must name an [`Act`] and every
    /// phrase must be a lowercase phrase of at most [`MAX_HEAD_WORDS`] words.
    pub fn parse(text: &str) -> Result<Lexicon, String> {
        let raw: BTreeMap<String, Vec<String>> =
            serde_json::from_str(text).map_err(|e| format!("not a lexicon: {e}"))?;
        let mut phrases: BTreeMap<String, BTreeSet<Act>> = BTreeMap::new();
        let mut by_act: BTreeMap<Act, Vec<String>> = BTreeMap::new();
        for (k, list) in raw {
            let act = Act::parse(&k).ok_or_else(|| format!("`{k}` is not an act"))?;
            for p in list {
                let words = title_words(&p);
                if words.join(" ") != p || words.is_empty() || words.len() > MAX_HEAD_WORDS {
                    return Err(format!(
                        "`{p}` (under `{k}`) is not a lowercase phrase of 1–{MAX_HEAD_WORDS} words"
                    ));
                }
                phrases.entry(p.clone()).or_default().insert(act);
                by_act.entry(act).or_default().push(p);
            }
        }
        for v in by_act.values_mut() {
            v.sort();
        }
        Ok(Lexicon { phrases, by_act })
    }

    /// The committed English lexicon.
    pub fn english() -> &'static Lexicon {
        static LEX: std::sync::OnceLock<Lexicon> = std::sync::OnceLock::new();
        LEX.get_or_init(|| Lexicon::parse(ACTS_EN).expect("acts-en.json is a valid lexicon"))
    }

    /// Every head phrase, with the acts it states.
    pub fn phrases(&self) -> &BTreeMap<String, BTreeSet<Act>> {
        &self.phrases
    }

    /// The **head** of a title (spec-0114 §3): the longest lexicon phrase, up
    /// to [`MAX_HEAD_WORDS`] words, the title begins with, and the acts it
    /// states. `None` when the title opens on no phrase the lexicon knows.
    pub fn head(&self, title: &str) -> Option<(String, &BTreeSet<Act>)> {
        let words = title_words(title);
        (1..=MAX_HEAD_WORDS.min(words.len())).rev().find_map(|n| {
            let key = words[..n].join(" ");
            self.phrases.get(&key).map(|acts| (key, acts))
        })
    }

    /// The head of every clause of a line (spec-0104 §5, the hint arm): the
    /// line as drawn (`dsl::l10n::plain`) is cut at every `.` `,` `;` `:` `!`
    /// `?` and dash, and each clause is read by [`Lexicon::head`] — the rule a
    /// title is read by, so *Past the bell, left of the arch: pull the lever*
    /// names `pull` and *At the far end of the nave* names nothing. A clause
    /// whose head the lexicon does not know contributes nothing.
    pub fn clause_heads(&self, line: &str) -> Vec<(String, &BTreeSet<Act>)> {
        let drawn = delvewright_dsl::l10n::plain(line);
        drawn
            .split(['.', ',', ';', ':', '!', '?', '\u{2014}', '\u{2013}'])
            .filter_map(|clause| self.head(clause))
            .collect()
    }

    /// Up to `n` example heads for an act set: each act's own name first (every
    /// act's name is one of its heads), then its one-word heads in order,
    /// taken round-robin across the acts.
    pub fn examples(&self, acts: &BTreeSet<Act>, n: usize) -> Vec<String> {
        let mut queues: Vec<Vec<&String>> = acts
            .iter()
            .map(|a| {
                let all = self.by_act.get(a).map(Vec::as_slice).unwrap_or(&[]);
                let mut q: Vec<&String> = all.iter().filter(|p| p.as_str() == a.as_str()).collect();
                q.extend(
                    all.iter()
                        .filter(|p| p.as_str() != a.as_str() && !p.contains(' ')),
                );
                q
            })
            .collect();
        let mut out: Vec<String> = Vec::new();
        let mut i = 0;
        while out.len() < n && queues.iter().any(|q| i < q.len()) {
            for q in queues.iter_mut() {
                if let Some(p) = q.get(i)
                    && out.len() < n
                    && !out.contains(p)
                {
                    out.push((*p).clone());
                }
            }
            i += 1;
        }
        out
    }
}

/// A title as the head is read from it (spec-0114 §3): drawn as the player
/// reads it (a styled span is its own text, a translation tag its English —
/// `dsl::l10n::plain`), case-folded, every character
/// outside letters, apostrophes and spaces read as a space, and split into
/// words. A typographic apostrophe reads as `'`.
pub fn title_words(title: &str) -> Vec<String> {
    let drawn = delvewright_dsl::l10n::plain(title);
    let folded: String = drawn
        .chars()
        .flat_map(char::to_lowercase)
        .map(|ch| match ch {
            '\u{2019}' => '\'',
            c if c.is_alphabetic() || c == '\'' => c,
            _ => ' ',
        })
        .collect();
    folded.split_whitespace().map(str::to_string).collect()
}

/// What `DW1045` examined, zeroes included (spec-0114 §5).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActBinding {
    /// Objectives in the campaign.
    pub objectives: usize,
    /// Objectives carrying a non-blank English title.
    pub titled: usize,
    /// Titled objectives whose head the lexicon knows — the judged ones.
    pub judged: usize,
    /// Titled objectives whose head the lexicon does not know, by path and title.
    pub unknown: Vec<(String, String)>,
    /// Objectives with no title.
    pub untitled: usize,
    /// Judged titles refused.
    pub refused: usize,
}

impl ActBinding {
    /// The binding line `delvec validate` prints.
    pub fn line(&self) -> String {
        let mut s = format!(
            "act binding (DW1045): {} of {} English title(s) read against {} objective(s); \
             {} head(s) the lexicon does not know; {} untitled; {} refused",
            self.judged,
            self.titled,
            self.objectives,
            self.unknown.len(),
            self.untitled,
            self.refused
        );
        if !self.unknown.is_empty() {
            let listed: Vec<String> = self
                .unknown
                .iter()
                .map(|(p, t)| format!("\"{t}\" ({p})"))
                .collect();
            s.push_str(&format!(" — not known: {}", listed.join(", ")));
        }
        s
    }

    /// The ledger `validation/act-gate.json` carries. `examined` is the judged
    /// count, so a build that judged no title reads as a zero binding.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "code": DW_ACT_UNTRUE.id(),
            "objectives": self.objectives,
            "titled": self.titled,
            "judged": self.judged,
            "examined": self.judged,
            "unknown": self.unknown.iter().map(|(p, t)| serde_json::json!({"path": p, "title": t})).collect::<Vec<_>>(),
            "untitled": self.untitled,
            "refused": self.refused,
        })
    }
}

/// How the completion could be made to perform `act` instead (spec-0114 §6.2),
/// or the gap that stops it (§6.4).
fn completion_remedy(act: Act, o: &delvewright_dsl::Objective) -> String {
    use delvewright_dsl::Objective as O;
    let talk = matches!(o, O::TalkTo { .. });
    match act {
        Act::Reach => "make it a `reach-anchor` at the place the title names".into(),
        Act::Talk => "make it a `talk-to` of the body the title names".into(),
        Act::Kill => "make it a `kill` of the wave the title names".into(),
        Act::Take => "make it a `collect` of the item, or hand the item with a `give-item` \
                      in its `on_objective_complete`"
            .into(),
        Act::Use => "make it an `interact` at the thing the title names".into(),
        Act::Present if talk => "showing a thing to a body is not a completion the engine has \
                                 (spec-0114 §6.4: a dialogue option reads no hand)"
            .into(),
        Act::Present => "make it an `interact` at the thing with `requires_item` — the item \
                         held in the hand when it is used"
            .into(),
        Act::Give if talk => "giving a thing to a body is not a completion the engine has \
                              (spec-0114 §6.4: a dialogue option reads no hand and takes nothing)"
            .into(),
        Act::Give => match o {
            O::Interact {
                requires_item: Some(it),
                takes_item: false,
                ..
            } => format!("add `takes_item: true`, so one `{it}` leaves the hand that presents it"),
            _ => "make it an `interact` at the place, with a `prop`, the `requires_item` laid \
                  there and `takes_item: true`, so one of it leaves the hand that presents it"
                .into(),
        },
    }
}

/// `DW1045` over every objective of every quest, with what it examined.
pub fn check_acts(c: &Campaign) -> (Vec<Diagnostic>, ActBinding) {
    check_acts_with(c, Lexicon::english())
}

/// [`check_acts`] against a given lexicon.
pub fn check_acts_with(c: &Campaign, lex: &Lexicon) -> (Vec<Diagnostic>, ActBinding) {
    let mut bind = ActBinding::default();
    let mut d = Vec::new();
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            bind.objectives += 1;
            let path = format!("/content/quests/{qi}/objectives/{oi}/title");
            let Some(title) = o
                .title()
                .map(|t| delvewright_dsl::l10n::plain(t).into_owned())
                .filter(|t| !t.trim().is_empty())
            else {
                bind.untitled += 1;
                continue;
            };
            bind.titled += 1;
            let Some((head, said)) = lex.head(&title) else {
                bind.unknown.push((path, title));
                continue;
            };
            bind.judged += 1;
            let done = q.objective_acts(o);
            if said.intersection(&done).next().is_some() {
                continue;
            }
            bind.refused += 1;
            let shape = q.objective_shape(o);
            let words = lex.examples(&done, 3);
            let retitle = format!(
                "re-head the title with a word of what the completion performs ({}: {})",
                acts_line(&done),
                words
                    .iter()
                    .map(|w| format!("`{w}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let recomplete: Vec<String> = said.iter().map(|a| completion_remedy(*a, o)).collect();
            let recomplete = format!(
                "change the completion to the act the title names: {}",
                recomplete.join("; or ")
            );
            // spec-0114 §6: a placing word over anything but a conversation is
            // repaired by bringing the completion up to the text first.
            let placing = said.len() == 1
                && said.contains(&Act::Give)
                && !matches!(o, delvewright_dsl::Objective::TalkTo { .. });
            let (first, second) = if placing {
                (recomplete, retitle)
            } else {
                (retitle, recomplete)
            };
            d.push(Diagnostic::error(
                DW_ACT_UNTRUE,
                "quests",
                path,
                format!(
                    "objective `{}` is titled \"{title}\": its head `{head}` → {}, and its \
                     completion `{shape}` → {}, so the title tells the player to do something \
                     the game does not reward (spec-0114). Either {first}; or {second}",
                    o.id(),
                    acts_line(said),
                    acts_line(&done),
                ),
            ));
        }
    }
    (d, bind)
}

/// What the hint arm of `DW1045` read, zeroes included.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HintActBinding {
    /// Objectives in the campaign.
    pub objectives: usize,
    /// Objectives carrying a non-blank English hint.
    pub hinted: usize,
    /// Of those, hints naming at least one act the lexicon knows — the judged ones.
    pub naming: usize,
    /// Judged hints naming only acts the completion does not perform.
    pub warned: usize,
}

impl HintActBinding {
    /// The binding line `delvec validate` prints.
    pub fn line(&self) -> String {
        format!(
            "hint act binding (DW1045): {} of {} English hint(s) name an act, over {} \
             objective(s); {} warned",
            self.naming, self.hinted, self.objectives, self.warned
        )
    }
}

/// **A hint that names an act names this objective's** (spec-0104 §5): the
/// hint arm of `DW1045`, a **warning**. A hint may be a place with no act at
/// all, and passes; one whose every clause head, read in its English by the
/// one act lexicon ([`Lexicon::clause_heads`]), states an act the completion
/// does not perform is reported for the creator to adjudicate. A hint is free
/// prose, where a closed lexicon cannot tell a noun from a verb or reported
/// speech from an instruction, so this arm never refuses; a title's head is
/// read by [`check_acts`], which does.
pub fn check_hint_acts(c: &Campaign) -> (Vec<Diagnostic>, HintActBinding) {
    check_hint_acts_with(c, Lexicon::english())
}

/// [`check_hint_acts`] against a given lexicon.
pub fn check_hint_acts_with(c: &Campaign, lex: &Lexicon) -> (Vec<Diagnostic>, HintActBinding) {
    let mut bind = HintActBinding::default();
    let mut d = Vec::new();
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            bind.objectives += 1;
            let Some(hint) = o
                .hint()
                .map(|t| delvewright_dsl::l10n::plain(t).into_owned())
                .filter(|t| !t.trim().is_empty())
            else {
                continue;
            };
            bind.hinted += 1;
            let found = lex.clause_heads(&hint);
            if found.is_empty() {
                continue;
            }
            bind.naming += 1;
            let done = q.objective_acts(o);
            if found
                .iter()
                .any(|(_, said)| said.intersection(&done).next().is_some())
            {
                continue;
            }
            bind.warned += 1;
            let (phrase, said) = &found[0];
            let words = lex.examples(&done, 3);
            d.push(Diagnostic::warning(
                DW_ACT_UNTRUE,
                "quests",
                format!("/content/quests/{qi}/objectives/{oi}/hint"),
                format!(
                    "the hint of objective `{}` (\"{hint}\") reads as another act: `{phrase}` → \
                     {}, and its completion `{}` → {}, and the hint names none of those. \
                     Decide whether it sends the party to do something the game does not \
                     reward: if the word is a noun or reported speech, keep it; if not, name \
                     the act the completion performs ({}), or write the hint as a place \
                     (`Past the bell, left of the arch.`). The lexicon reads words, not \
                     meaning, so a hint is a question for you, never a refusal.",
                    o.id(),
                    acts_line(said),
                    q.objective_shape(o),
                    acts_line(&done),
                    words
                        .iter()
                        .map(|w| format!("`{w}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
    }
    (d, bind)
}
