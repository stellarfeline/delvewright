//! Compiler **chrome**: the player-visible strings the compiler writes itself,
//! which no campaign authored and no translator was ever asked for
//! (spec-0029 addendum).
//!
//! A delve's on-screen text has two authors. The campaign writes its own lines,
//! and those travel through the l10n inventory ([`crate::l10n`]) into a sidecar a
//! translator fills in. The **compiler** writes the rest — `New objective: `,
//! `Delve Complete`, `Choose your class`, and the default a `bonfire` shows when
//! the campaign authors no label — and without a module like this one they have
//! no key, no sidecar entry and no way to be anything but English, so a player
//! reading a fully translated delve still sees English chrome wrapped around it.
//!
//! ## Why chrome is compiler-owned rather than authored
//!
//! The obvious fix — inventory them like any other string — charges every campaign
//! for text it did not write: a translator answering the same keys,
//! identically, once per delve, forever. And it would be answering them for
//! **product chrome**: `New objective: `, `Delve Complete`, `Choose your class`
//! are not lines any campaign author wants to write. That nine of them never grew
//! an authored override was a correct judgment, not an oversight, which is exactly
//! why the answer is not to add one — it would move the engine's maintenance cost
//! onto content.
//!
//! The other five are **diegetic**: in-world text a campaign legitimately wants in
//! its own voice, and each already has its authored override (`boundary.message`,
//! `close-gate.sealed_hint`, a bonfire's `prompt`/`rest_label`/`save_label`). Those
//! overrides are unchanged and still win; what lives here is only the **default**
//! the compiler bakes when the campaign authors nothing, which used to be English
//! in every language.
//!
//! ## Key space
//!
//! Chrome keys are `delvewright.ui.<area>.<name>`. The prefix makes collision
//! impossible in both directions, by construction rather than by policing:
//!
//! * a **campaign** key is always one of the fixed kinds the l10n key scheme
//!   derives (`world.` / `area.` / `class.` / `npc.` / `quest.` / `obj.` / `dlg.` /
//!   `wave.` / `actor.` / `loot.` / `cast.` / `fx.`), so no campaign string can
//!   produce a `delvewright.` key — and a sidecar that writes one anyway is
//!   `DW0186` ([`validate_chrome_namespace`]), the guard that sits beside the other
//!   reserved-channel guards;
//! * **vanilla** owns every other namespace in a language file and will never
//!   define `delvewright.*`, so our entries cannot shadow a Minecraft string.
//!
//! ## Sentences, not fragments
//!
//! Four chrome strings frame a value — an objective title, the delve's name, the
//! live party count. They are **one key with `%s` placeholders**, never a key
//! concatenated with a component, because a concatenation freezes English word
//! order into every language: `"%s — complete."` lets a translator put the title
//! where the sentence needs it. Vanilla's `translate` component takes `with`
//! arguments for exactly this, so the placeholder is the intended primitive rather
//! than a workaround (CLAUDE.md no-hack doctrine). A [`ChromeString`] declares how
//! many arguments it takes, and a language table whose placeholder count disagrees
//! with the English fails a unit test rather than a player's screen.
//!
//! ## Delivery, and the honest fallback
//!
//! Chrome rides the same road the campaign's strings do (spec-0029): each chrome
//! string enters emission as a translation tag ([`ChromeString::tagged`]), an
//! emitter lowers it into `{"translate": key, "fallback": <text>, "with": […]}`,
//! and `delvec build` writes the chrome entries into the resource pack's language
//! files beside the campaign's.
//!
//! Chrome is **carried for every language it has a table for and emitted only into
//! the language files the delve already writes** — `en_us` plus the campaign's
//! declared languages. Emitting every table would give, say, a French client on a
//! Chinese-only campaign French chrome around English story: partial in a way that
//! reads as broken, where uniform English does not. The tables still pay off with
//! no engine change: they activate the moment a campaign declares that language.
//!
//! For a language with no table the key is simply **absent** from that file: the
//! client resolves it through `en_us.json` (or, for a player who declined the pack,
//! through the component's own `fallback`) and reads English. Absent, never faked —
//! writing English into `fr_fr.json` would render the same glyphs while claiming to
//! be a translation.
//!
//! ## Translation provenance
//!
//! English is canonical (CLAUDE.md language policy); every table below is derived
//! from it. **These are unreviewed translations** — machine-produced, not
//! checked by a native speaker — which is defensible for short functional
//! strings and is recorded here so nobody mistakes them for reviewed work. A
//! correction is a one-line edit to a table.

use std::collections::BTreeMap;

use crate::diagnostic::{Diagnostic, codes};
use crate::l10n::{L10nDoc, tag};

/// One compiler-authored player-visible string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChromeString {
    /// The stable `delvewright.ui.…` translation key.
    pub key: &'static str,
    /// The canonical English text, `%s` for each argument.
    pub en: &'static str,
    /// How many `with` arguments the component supplies.
    pub args: usize,
    /// This string's index in [`ALL`] — the slot a language table fills.
    pub slot: usize,
}

impl ChromeString {
    /// This string as a **translation tag** over `text` — the in-band form
    /// spec-0029 carries a key on, so an emitter lowers chrome through exactly the
    /// helpers an authored string uses, and a site that fails to lower it trips
    /// `DW0185` exactly as it would for authored text.
    fn tagged_as(&self, text: &str) -> String {
        tag(self.key, text)
    }

    /// This string as a translation tag over its canonical English, under the
    /// **bare** chrome key.
    ///
    /// The plan-time form, for a default the compiler bakes before it knows the
    /// build's language or the delve's key namespace. It is not emittable as it
    /// stands: [`Chrome::rebind`] is what turns it into the delve's own pack key,
    /// and `crates/delvec/tests/i18n_v2.rs` fails the build of any tree that emits
    /// a bare key, so a site that forgets to rebind is a red rather than a string
    /// another delve's pack can answer.
    pub fn tagged(&self) -> String {
        self.tagged_as(self.en)
    }
}

/// Declare a chrome string and its slot in [`ALL`] in one place, so the two can
/// never disagree.
macro_rules! chrome {
    ($( $(#[$m:meta])* $name:ident = $slot:literal, $key:literal, $args:literal, $en:expr; )*) => {
        $(
            $(#[$m])*
            pub const $name: ChromeString = ChromeString {
                key: $key, en: $en, args: $args, slot: $slot,
            };
        )*
        /// Every compiler-authored player-visible string, in slot order. The single
        /// authority: the language tables are indexed by it, the lang-file writer
        /// walks it, and the coverage report counts it.
        pub const ALL: &[ChromeString] = &[$($name),*];
    };
}

chrome! {
    /// An objective's activation announcement. `%s` is its title.
    OBJECTIVE_NEW = 0, "delvewright.ui.objective.new", 1, "New objective: %s";
    /// An objective's completion confirmation. `%s` is its title.
    OBJECTIVE_COMPLETE = 1, "delvewright.ui.objective.complete", 1, "Objective complete: %s";
    /// The campaign-completion chat line. `%s` is the delve's title.
    CAMPAIGN_COMPLETE = 2, "delvewright.ui.campaign.complete", 1, "%s — complete.";
    /// The signature under the campaign-completion line.
    CAMPAIGN_SIGNATURE = 3, "delvewright.ui.campaign.signature", 0, "A Delvewright delve.";
    /// The finale title banner.
    CAMPAIGN_BANNER = 4, "delvewright.ui.campaign.banner", 0, "Delve Complete";
    /// The lobby actionbar (`world.min_players`). `%s` is the live party count,
    /// then the size the delve requires.
    LOBBY_WAITING = 5, "delvewright.ui.lobby.waiting", 2, "Waiting for the party — %s / %s";
    /// The class-selection dialog's title.
    CLASS_TITLE = 6, "delvewright.ui.class.title", 0, "Choose your class";
    /// The class-selection dialog's body line.
    CLASS_BODY = 7, "delvewright.ui.class.body", 0, "Pick the kit you will carry.";
    /// The playable-region return message when the campaign authors no
    /// `world.boundary.message` (spec-0013).
    BOUNDARY_MESSAGE = 8, "delvewright.ui.boundary.message", 0,
        "The tide turns you back — the delve lies behind you.";
    /// What a sealed gate answers a right-click with when the `close-gate` authors
    /// no `sealed_hint` (DSL v0.8).
    GATE_SEALED = 9, "delvewright.ui.gate.sealed", 0, "The way is sealed.";
    /// The bonfire rest dialog's title when the effect authors no `prompt`.
    BONFIRE_TITLE = 10, "delvewright.ui.bonfire.title", 0, crate::BONFIRE_PROMPT_EN;
    /// The bonfire's **rest and save** button when no `rest_label` is authored.
    BONFIRE_REST = 11, "delvewright.ui.bonfire.rest", 0, crate::BONFIRE_REST_LABEL_EN;
    /// The bonfire's **save only** button when no `save_label` is authored.
    BONFIRE_SAVE = 12, "delvewright.ui.bonfire.save", 0, crate::BONFIRE_SAVE_LABEL_EN;
    /// The countdown a waiting player sees on the action bar
    /// (`world.respawn_wait`, spec-0077). `%s` is the seconds left.
    RESPAWN_WAIT = 13, "delvewright.ui.respawn.wait", 1, "Back in %s";
    /// The engine's compass's name in the hand (spec-0104 §6.2).
    COMPASS_NAME = 14, "delvewright.ui.compass.name", 0, "Compass";
    /// The line a player reads when their compass starts following an objective. `%s` is its title.
    COMPASS_FOLLOWING = 15, "delvewright.ui.compass.following", 1, "Following: %s";
    /// The line that follows it when more than one tracked objective is open. `%s` is the count of the others, then the bound sneak key.
    COMPASS_OTHERS = 16, "delvewright.ui.compass.others", 2, "Other open tasks: %s — crouch (%s) with the compass in hand to switch.";
    /// The line a player reads when nothing is open to follow.
    COMPASS_NOTHING = 17, "delvewright.ui.compass.nothing", 0, "Nothing to follow yet.";
    /// The journal's name when the campaign writes no `guidance.manual.title` (spec-0104 §8.1).
    JOURNAL_TITLE = 18, "delvewright.ui.journal.title", 0, "Journal";
    /// The journal's compass page: its heading.
    JOURNAL_COMPASS_HEADING = 19, "delvewright.ui.journal.compass.heading", 0, "Reading the compass";
    /// The journal's compass page: what the needle shows.
    JOURNAL_COMPASS_FOLLOWS = 20, "delvewright.ui.journal.compass.follows", 0, "Its needle points to the task you follow.";
    /// The journal's compass page: the gesture. `%s` is the bound sneak key.
    JOURNAL_COMPASS_SWITCH = 21, "delvewright.ui.journal.compass.switch", 1, "Crouch (%s) with the compass in hand to follow the next open task.";
    /// The journal's compass page: the spinning needle.
    JOURNAL_COMPASS_SPIN = 22, "delvewright.ui.journal.compass.spin", 0, "A spinning needle means nothing is open.";
    /// The journal's compass page: where the open tasks are.
    JOURNAL_COMPASS_PAGES = 23, "delvewright.ui.journal.compass.pages", 0, "The open tasks are on the pages that follow.";
    /// The heading of an open objective's journal page.
    JOURNAL_OPEN = 24, "delvewright.ui.journal.open", 0, "To do";
    /// The heading of a done objective's journal page.
    JOURNAL_DONE = 25, "delvewright.ui.journal.done", 0, "Done";
    /// The mark on the open page of the objective this player's compass follows.
    JOURNAL_FOLLOWING = 26, "delvewright.ui.journal.following", 0, "◆ You are following this.";
}

/// One language's rendition of [`ALL`], in slot order.
type Table = [&'static str; 27];

// --- the tables ------------------------------------------------------------
// Slot order is ALL's: objective.new, objective.complete, campaign.complete,
// campaign.signature, campaign.banner, lobby.waiting, class.title, class.body,
// boundary.message, gate.sealed, bonfire.title, bonfire.rest, bonfire.save,
// respawn.wait, compass.name, compass.following, compass.others,
// compass.nothing, journal.title, journal.compass.heading,
// journal.compass.follows, journal.compass.switch, journal.compass.spin,
// journal.compass.pages, journal.open, journal.done, journal.following.

const ZH_HANS: Table = [
    "新目标：%s",
    "目标完成：%s",
    "%s——完成。",
    "一场 Delvewright 秘境。",
    "秘境通关",
    "等待队伍集结 —— %s / %s",
    "选择你的职业",
    "挑选你要携带的装备。",
    "潮水将你送回——秘境在你身后。",
    "此路已封。",
    "篝火",
    "休息并存档",
    "仅存档",
    "%s 秒后归队",
    "指南针",
    "正在追踪：%s",
    "其他进行中的任务：%s——手持指南针潜行（%s）即可切换。",
    "暂无可追踪的任务。",
    "日志",
    "读懂指南针",
    "指针指向你正在追踪的任务。",
    "手持指南针潜行（%s），追踪下一个进行中的任务。",
    "指针打转，说明没有进行中的任务。",
    "进行中的任务列在后面几页。",
    "待办",
    "已完成",
    "◆ 你正在追踪此任务。",
];

const ZH_HANT: Table = [
    "新目標：%s",
    "目標完成：%s",
    "%s——完成。",
    "一場 Delvewright 秘境。",
    "秘境通關",
    "等待隊伍集結 —— %s / %s",
    "選擇你的職業",
    "挑選你要攜帶的裝備。",
    "潮水將你送回——秘境在你身後。",
    "此路已封。",
    "篝火",
    "休息並存檔",
    "僅存檔",
    "%s 秒後歸隊",
    "指南針",
    "正在追蹤：%s",
    "其他進行中的任務：%s——手持指南針潛行（%s）即可切換。",
    "暫無可追蹤的任務。",
    "日誌",
    "讀懂指南針",
    "指針指向你正在追蹤的任務。",
    "手持指南針潛行（%s），追蹤下一個進行中的任務。",
    "指針打轉，表示沒有進行中的任務。",
    "進行中的任務列在後面幾頁。",
    "待辦",
    "已完成",
    "◆ 你正在追蹤此任務。",
];

const JA: Table = [
    "新しい目標: %s",
    "目標達成: %s",
    "%s — クリア。",
    "Delvewright の探索です。",
    "探索完了",
    "パーティを待っています — %s / %s",
    "クラスを選択",
    "携行する装備を選ぼう。",
    "潮に押し戻された — 探索地は後ろだ。",
    "道は閉ざされている。",
    "焚き火",
    "休息してセーブ",
    "セーブのみ",
    "復帰まで %s",
    "コンパス",
    "追跡中: %s",
    "ほかの未完了タスク: %s — コンパスを持ってスニーク（%s）で切り替え。",
    "まだ追跡するものはない。",
    "日誌",
    "コンパスの読み方",
    "針は追跡中のタスクを指す。",
    "コンパスを持ってスニーク（%s）すると、次の未完了タスクを追跡する。",
    "針が回り続けるなら、未完了のタスクはない。",
    "未完了のタスクは次のページから。",
    "やること",
    "完了",
    "◆ これを追跡中。",
];

const KO: Table = [
    "새 목표: %s",
    "목표 완료: %s",
    "%s — 완료.",
    "Delvewright 탐험.",
    "탐험 완료",
    "일행을 기다리는 중 — %s / %s",
    "직업을 선택하세요",
    "가져갈 장비를 고르세요.",
    "조류가 당신을 되돌려 보낸다 — 탐험지는 뒤에 있다.",
    "길이 막혀 있다.",
    "모닥불",
    "휴식하고 저장",
    "저장만",
    "%s초 후 복귀",
    "나침반",
    "추적 중: %s",
    "다른 진행 중인 임무: %s — 나침반을 들고 웅크리기(%s)로 전환하세요.",
    "아직 추적할 것이 없다.",
    "일지",
    "나침반 읽는 법",
    "바늘은 추적 중인 임무를 가리킨다.",
    "나침반을 들고 웅크리면(%s) 다음 진행 중인 임무를 추적한다.",
    "바늘이 빙빙 돌면 진행 중인 임무가 없다는 뜻이다.",
    "진행 중인 임무는 다음 페이지에 있다.",
    "할 일",
    "완료",
    "◆ 이 임무를 추적 중이다.",
];

const DE: Table = [
    "Neues Ziel: %s",
    "Ziel erfüllt: %s",
    "%s – abgeschlossen.",
    "Ein Delvewright-Abenteuer.",
    "Abenteuer abgeschlossen",
    "Warten auf die Gruppe – %s / %s",
    "Wähle deine Klasse",
    "Wähle die Ausrüstung, die du mitnimmst.",
    "Die Flut trägt dich zurück – das Abenteuer liegt hinter dir.",
    "Der Weg ist versperrt.",
    "Lagerfeuer",
    "Rasten und speichern",
    "Nur speichern",
    "Zurück in %s",
    "Kompass",
    "Du folgst: %s",
    "Weitere offene Aufgaben: %s – schleiche (%s) mit dem Kompass in der Hand, um zu wechseln.",
    "Noch nichts zu verfolgen.",
    "Tagebuch",
    "Den Kompass lesen",
    "Seine Nadel zeigt auf die Aufgabe, der du folgst.",
    "Schleiche (%s) mit dem Kompass in der Hand, um der nächsten offenen Aufgabe zu folgen.",
    "Dreht sich die Nadel, ist nichts offen.",
    "Die offenen Aufgaben stehen auf den folgenden Seiten.",
    "Zu erledigen",
    "Erledigt",
    "◆ Du folgst dieser Aufgabe.",
];

const FR: Table = [
    "Nouvel objectif : %s",
    "Objectif accompli : %s",
    "%s — terminé.",
    "Une aventure Delvewright.",
    "Aventure terminée",
    "En attente du groupe — %s / %s",
    "Choisissez votre classe",
    "Choisissez l'équipement que vous emporterez.",
    "La marée vous repousse — l'aventure est derrière vous.",
    "Le passage est scellé.",
    "Feu de camp",
    "Se reposer et sauvegarder",
    "Sauvegarder seulement",
    "Retour dans %s",
    "Boussole",
    "Suivi : %s",
    "Autres tâches en cours : %s — accroupissez-vous (%s) boussole en main pour changer.",
    "Rien à suivre pour l'instant.",
    "Journal",
    "Lire la boussole",
    "Son aiguille indique la tâche que vous suivez.",
    "Accroupissez-vous (%s) boussole en main pour suivre la tâche en cours suivante.",
    "Une aiguille qui tourne signifie qu'aucune tâche n'est en cours.",
    "Les tâches en cours figurent sur les pages suivantes.",
    "À faire",
    "Terminée",
    "◆ Vous suivez cette tâche.",
];

const ES: Table = [
    "Nuevo objetivo: %s",
    "Objetivo completado: %s",
    "%s: completada.",
    "Una aventura de Delvewright.",
    "Aventura completada",
    "Esperando al grupo — %s / %s",
    "Elige tu clase",
    "Elige el equipo que llevarás.",
    "La marea te hace retroceder: la aventura queda a tus espaldas.",
    "El paso está sellado.",
    "Hoguera",
    "Descansar y guardar",
    "Solo guardar",
    "Vuelves en %s",
    "Brújula",
    "Siguiendo: %s",
    "Otras tareas abiertas: %s — agáchate (%s) con la brújula en la mano para cambiar.",
    "Aún no hay nada que seguir.",
    "Diario",
    "Cómo leer la brújula",
    "Su aguja señala la tarea que sigues.",
    "Agáchate (%s) con la brújula en la mano para seguir la siguiente tarea abierta.",
    "Si la aguja gira, no hay nada abierto.",
    "Las tareas abiertas están en las páginas siguientes.",
    "Por hacer",
    "Hecha",
    "◆ Estás siguiendo esta tarea.",
];

const PT_BR: Table = [
    "Novo objetivo: %s",
    "Objetivo concluído: %s",
    "%s — concluída.",
    "Uma aventura Delvewright.",
    "Aventura concluída",
    "Aguardando o grupo — %s / %s",
    "Escolha sua classe",
    "Escolha o equipamento que vai levar.",
    "A maré te empurra de volta — a aventura ficou para trás.",
    "A passagem está selada.",
    "Fogueira",
    "Descansar e salvar",
    "Apenas salvar",
    "De volta em %s",
    "Bússola",
    "Seguindo: %s",
    "Outras tarefas abertas: %s — agache (%s) com a bússola na mão para trocar.",
    "Nada para seguir ainda.",
    "Diário",
    "Como ler a bússola",
    "A agulha aponta para a tarefa que você segue.",
    "Agache (%s) com a bússola na mão para seguir a próxima tarefa aberta.",
    "Agulha girando significa que nada está aberto.",
    "As tarefas abertas estão nas próximas páginas.",
    "A fazer",
    "Concluída",
    "◆ Você está seguindo esta tarefa.",
];

const PT_PT: Table = [
    "Novo objetivo: %s",
    "Objetivo concluído: %s",
    "%s — concluída.",
    "Uma aventura Delvewright.",
    "Aventura concluída",
    "À espera do grupo — %s / %s",
    "Escolhe a tua classe",
    "Escolhe o equipamento que vais levar.",
    "A maré empurra-te de volta — a aventura ficou para trás.",
    "A passagem está selada.",
    "Fogueira",
    "Descansar e guardar",
    "Apenas guardar",
    "De volta em %s",
    "Bússola",
    "A seguir: %s",
    "Outras tarefas em aberto: %s — agacha-te (%s) com a bússola na mão para mudar.",
    "Ainda nada para seguir.",
    "Diário",
    "Como ler a bússola",
    "A agulha aponta para a tarefa que segues.",
    "Agacha-te (%s) com a bússola na mão para seguir a próxima tarefa em aberto.",
    "Se a agulha gira, não há nada em aberto.",
    "As tarefas em aberto estão nas páginas seguintes.",
    "Por fazer",
    "Concluída",
    "◆ Estás a seguir esta tarefa.",
];

const RU: Table = [
    "Новая цель: %s",
    "Цель выполнена: %s",
    "%s — пройдено.",
    "Приключение Delvewright.",
    "Приключение пройдено",
    "Ожидание отряда — %s / %s",
    "Выберите класс",
    "Выберите снаряжение, которое возьмёте с собой.",
    "Прилив относит вас назад — приключение позади.",
    "Путь запечатан.",
    "Костёр",
    "Отдохнуть и сохранить",
    "Только сохранить",
    "Возвращение через %s",
    "Компас",
    "Отслеживается: %s",
    "Других открытых задач: %s — крадитесь (%s) с компасом в руке, чтобы переключить.",
    "Пока нечего отслеживать.",
    "Дневник",
    "Как читать компас",
    "Его стрелка указывает на отслеживаемую задачу.",
    "Крадитесь (%s) с компасом в руке, чтобы отслеживать следующую открытую задачу.",
    "Если стрелка вращается, открытых задач нет.",
    "Открытые задачи — на следующих страницах.",
    "Сделать",
    "Выполнена",
    "◆ Вы отслеживаете эту задачу.",
];

const IT: Table = [
    "Nuovo obiettivo: %s",
    "Obiettivo completato: %s",
    "%s — completata.",
    "Un'avventura Delvewright.",
    "Avventura completata",
    "In attesa del gruppo — %s / %s",
    "Scegli la tua classe",
    "Scegli l'equipaggiamento che porterai.",
    "La marea ti respinge — l'avventura è alle tue spalle.",
    "Il passaggio è sigillato.",
    "Falò",
    "Riposa e salva",
    "Salva soltanto",
    "Di ritorno tra %s",
    "Bussola",
    "Stai seguendo: %s",
    "Altri incarichi aperti: %s — accovacciati (%s) con la bussola in mano per cambiare.",
    "Ancora niente da seguire.",
    "Diario",
    "Leggere la bussola",
    "L'ago indica l'incarico che segui.",
    "Accovacciati (%s) con la bussola in mano per seguire il prossimo incarico aperto.",
    "Se l'ago gira, non c'è nulla di aperto.",
    "Gli incarichi aperti sono nelle pagine seguenti.",
    "Da fare",
    "Completato",
    "◆ Stai seguendo questo incarico.",
];

const NL: Table = [
    "Nieuw doel: %s",
    "Doel voltooid: %s",
    "%s — voltooid.",
    "Een Delvewright-avontuur.",
    "Avontuur voltooid",
    "Wachten op het gezelschap — %s / %s",
    "Kies je klasse",
    "Kies de uitrusting die je meeneemt.",
    "Het tij drijft je terug — het avontuur ligt achter je.",
    "De doorgang is verzegeld.",
    "Kampvuur",
    "Rusten en opslaan",
    "Alleen opslaan",
    "Terug over %s",
    "Kompas",
    "Je volgt: %s",
    "Andere open taken: %s — sluip (%s) met het kompas in je hand om te wisselen.",
    "Nog niets om te volgen.",
    "Dagboek",
    "Het kompas lezen",
    "De naald wijst naar de taak die je volgt.",
    "Sluip (%s) met het kompas in je hand om de volgende open taak te volgen.",
    "Een draaiende naald betekent dat er niets open is.",
    "De open taken staan op de volgende pagina's.",
    "Te doen",
    "Voltooid",
    "◆ Je volgt deze taak.",
];

const PL: Table = [
    "Nowy cel: %s",
    "Cel ukończony: %s",
    "%s — ukończona.",
    "Przygoda Delvewright.",
    "Przygoda ukończona",
    "Oczekiwanie na drużynę — %s / %s",
    "Wybierz swoją klasę",
    "Wybierz ekwipunek, który zabierzesz.",
    "Przypływ zawraca cię — przygoda została za tobą.",
    "Przejście jest zapieczętowane.",
    "Ognisko",
    "Odpocznij i zapisz",
    "Tylko zapisz",
    "Powrót za %s",
    "Kompas",
    "Śledzisz: %s",
    "Inne otwarte zadania: %s — skradaj się (%s) z kompasem w ręce, aby przełączyć.",
    "Na razie nie ma czego śledzić.",
    "Dziennik",
    "Jak czytać kompas",
    "Jego igła wskazuje śledzone zadanie.",
    "Skradaj się (%s) z kompasem w ręce, aby śledzić następne otwarte zadanie.",
    "Wirująca igła oznacza, że nic nie jest otwarte.",
    "Otwarte zadania są na kolejnych stronach.",
    "Do zrobienia",
    "Ukończone",
    "◆ Śledzisz to zadanie.",
];

const TR: Table = [
    "Yeni hedef: %s",
    "Hedef tamamlandı: %s",
    "%s — tamamlandı.",
    "Bir Delvewright macerası.",
    "Macera tamamlandı",
    "Takım bekleniyor — %s / %s",
    "Sınıfını seç",
    "Yanına alacağın teçhizatı seç.",
    "Gelgit seni geri getiriyor — macera arkanda kaldı.",
    "Yol mühürlendi.",
    "Kamp ateşi",
    "Dinlen ve kaydet",
    "Sadece kaydet",
    "Dönüşe kalan: %s",
    "Pusula",
    "Takip ediliyor: %s",
    "Diğer açık görevler: %s — değiştirmek için pusula elindeyken eğil (%s).",
    "Henüz takip edilecek bir şey yok.",
    "Günlük",
    "Pusulayı okumak",
    "İğnesi takip ettiğin görevi gösterir.",
    "Sıradaki açık görevi takip etmek için pusula elindeyken eğil (%s).",
    "İğne dönüyorsa açık görev yok demektir.",
    "Açık görevler sonraki sayfalarda.",
    "Yapılacak",
    "Tamamlandı",
    "◆ Bunu takip ediyorsun.",
];

const UK: Table = [
    "Нова ціль: %s",
    "Ціль виконано: %s",
    "%s — пройдено.",
    "Пригода Delvewright.",
    "Пригоду пройдено",
    "Очікування загону — %s / %s",
    "Оберіть клас",
    "Оберіть спорядження, яке візьмете з собою.",
    "Приплив відносить вас назад — пригода позаду.",
    "Шлях запечатано.",
    "Багаття",
    "Відпочити та зберегти",
    "Лише зберегти",
    "Повернення через %s",
    "Компас",
    "Відстежується: %s",
    "Інших відкритих завдань: %s — крадіться (%s) з компасом у руці, щоб перемкнути.",
    "Поки нічого відстежувати.",
    "Щоденник",
    "Як читати компас",
    "Його стрілка вказує на завдання, яке ви відстежуєте.",
    "Крадіться (%s) з компасом у руці, щоб відстежувати наступне відкрите завдання.",
    "Якщо стрілка обертається, відкритих завдань немає.",
    "Відкриті завдання — на наступних сторінках.",
    "Зробити",
    "Виконане",
    "◆ Ви відстежуєте це завдання.",
];

const CS: Table = [
    "Nový cíl: %s",
    "Cíl splněn: %s",
    "%s — dokončeno.",
    "Dobrodružství Delvewright.",
    "Dobrodružství dokončeno",
    "Čekání na družinu — %s / %s",
    "Vyber si třídu",
    "Vyber si výbavu, kterou si vezmeš.",
    "Příliv tě vrací zpět — dobrodružství leží za tebou.",
    "Cesta je zapečetěná.",
    "Táborák",
    "Odpočinout a uložit",
    "Jen uložit",
    "Návrat za %s",
    "Kompas",
    "Sleduješ: %s",
    "Další otevřené úkoly: %s — plíž se (%s) s kompasem v ruce a přepneš.",
    "Zatím není co sledovat.",
    "Deník",
    "Jak číst kompas",
    "Jeho střelka ukazuje na úkol, který sleduješ.",
    "Plíž se (%s) s kompasem v ruce a budeš sledovat další otevřený úkol.",
    "Točící se střelka znamená, že nic není otevřené.",
    "Otevřené úkoly jsou na následujících stránkách.",
    "Zbývá udělat",
    "Splněný",
    "◆ Tento úkol sleduješ.",
];

const SK: Table = [
    "Nový cieľ: %s",
    "Cieľ splnený: %s",
    "%s — dokončené.",
    "Dobrodružstvo Delvewright.",
    "Dobrodružstvo dokončené",
    "Čaká sa na družinu — %s / %s",
    "Vyber si triedu",
    "Vyber si výstroj, ktorú si vezmeš.",
    "Príliv ťa vracia späť — dobrodružstvo je za tebou.",
    "Cesta je zapečatená.",
    "Táborák",
    "Oddýchnuť si a uložiť",
    "Len uložiť",
    "Návrat o %s",
    "Kompas",
    "Sleduješ: %s",
    "Ďalšie otvorené úlohy: %s — zakrádaj sa (%s) s kompasom v ruke a prepneš.",
    "Zatiaľ nie je čo sledovať.",
    "Denník",
    "Ako čítať kompas",
    "Jeho ihla ukazuje na úlohu, ktorú sleduješ.",
    "Zakrádaj sa (%s) s kompasom v ruke a budeš sledovať ďalšiu otvorenú úlohu.",
    "Točiaca sa ihla znamená, že nič nie je otvorené.",
    "Otvorené úlohy sú na nasledujúcich stranách.",
    "Treba urobiť",
    "Splnená",
    "◆ Túto úlohu sleduješ.",
];

const SV: Table = [
    "Nytt mål: %s",
    "Mål avklarat: %s",
    "%s — avklarat.",
    "Ett Delvewright-äventyr.",
    "Äventyret avklarat",
    "Väntar på sällskapet — %s / %s",
    "Välj din klass",
    "Välj utrustningen du tar med dig.",
    "Tidvattnet för dig tillbaka — äventyret ligger bakom dig.",
    "Vägen är förseglad.",
    "Lägereld",
    "Vila och spara",
    "Spara endast",
    "Tillbaka om %s",
    "Kompass",
    "Du följer: %s",
    "Andra öppna uppgifter: %s — smyg (%s) med kompassen i handen för att byta.",
    "Inget att följa än.",
    "Dagbok",
    "Att läsa kompassen",
    "Nålen pekar mot uppgiften du följer.",
    "Smyg (%s) med kompassen i handen för att följa nästa öppna uppgift.",
    "En snurrande nål betyder att inget är öppet.",
    "De öppna uppgifterna finns på sidorna som följer.",
    "Att göra",
    "Klar",
    "◆ Du följer den här.",
];

const DA: Table = [
    "Nyt mål: %s",
    "Mål fuldført: %s",
    "%s — fuldført.",
    "Et Delvewright-eventyr.",
    "Eventyret fuldført",
    "Venter på selskabet — %s / %s",
    "Vælg din klasse",
    "Vælg det udstyr, du tager med.",
    "Tidevandet fører dig tilbage — eventyret ligger bag dig.",
    "Vejen er forseglet.",
    "Bål",
    "Hvil og gem",
    "Gem kun",
    "Tilbage om %s",
    "Kompas",
    "Du følger: %s",
    "Andre åbne opgaver: %s — snig (%s) med kompasset i hånden for at skifte.",
    "Intet at følge endnu.",
    "Dagbog",
    "Sådan læser du kompasset",
    "Nålen peger mod den opgave, du følger.",
    "Snig (%s) med kompasset i hånden for at følge den næste åbne opgave.",
    "En snurrende nål betyder, at intet er åbent.",
    "De åbne opgaver står på de følgende sider.",
    "Skal gøres",
    "Fuldført",
    "◆ Du følger denne.",
];

const NB: Table = [
    "Nytt mål: %s",
    "Mål fullført: %s",
    "%s — fullført.",
    "Et Delvewright-eventyr.",
    "Eventyret fullført",
    "Venter på følget — %s / %s",
    "Velg klassen din",
    "Velg utstyret du tar med deg.",
    "Tidevannet fører deg tilbake — eventyret ligger bak deg.",
    "Veien er forseglet.",
    "Bål",
    "Hvil og lagre",
    "Bare lagre",
    "Tilbake om %s",
    "Kompass",
    "Du følger: %s",
    "Andre åpne oppgaver: %s — snik (%s) med kompasset i hånden for å bytte.",
    "Ingenting å følge ennå.",
    "Dagbok",
    "Slik leser du kompasset",
    "Nålen peker mot oppgaven du følger.",
    "Snik (%s) med kompasset i hånden for å følge neste åpne oppgave.",
    "En nål som snurrer betyr at ingenting er åpent.",
    "De åpne oppgavene står på sidene som følger.",
    "Å gjøre",
    "Fullført",
    "◆ Du følger denne.",
];

const FI: Table = [
    "Uusi tavoite: %s",
    "Tavoite suoritettu: %s",
    "%s — suoritettu.",
    "Delvewright-seikkailu.",
    "Seikkailu suoritettu",
    "Odotetaan ryhmää — %s / %s",
    "Valitse luokkasi",
    "Valitse mukaan otettavat varusteet.",
    "Vuorovesi kääntää sinut takaisin — seikkailu jää taaksesi.",
    "Tie on sinetöity.",
    "Nuotio",
    "Lepää ja tallenna",
    "Vain tallennus",
    "Paluu %s kuluttua",
    "Kompassi",
    "Seuraat: %s",
    "Muita avoimia tehtäviä: %s — vaihda hiipimällä (%s) kompassi kädessä.",
    "Ei vielä mitään seurattavaa.",
    "Päiväkirja",
    "Kompassin lukeminen",
    "Sen neula osoittaa seuraamaasi tehtävää.",
    "Hiivi (%s) kompassi kädessä seurataksesi seuraavaa avointa tehtävää.",
    "Pyörivä neula tarkoittaa, ettei mitään ole avoinna.",
    "Avoimet tehtävät ovat seuraavilla sivuilla.",
    "Tehtävänä",
    "Valmis",
    "◆ Seuraat tätä.",
];

const HU: Table = [
    "Új cél: %s",
    "Cél teljesítve: %s",
    "%s — teljesítve.",
    "Egy Delvewright-kaland.",
    "A kaland teljesítve",
    "Várakozás a csapatra — %s / %s",
    "Válaszd ki a kasztodat",
    "Válaszd ki a magaddal vitt felszerelést.",
    "Az ár visszasodor — a kaland mögötted maradt.",
    "Az út le van pecsételve.",
    "Tábortűz",
    "Pihenés és mentés",
    "Csak mentés",
    "Visszatérés: %s",
    "Iránytű",
    "Követed: %s",
    "További nyitott feladatok: %s — lopakodj (%s) iránytűvel a kezedben a váltáshoz.",
    "Még nincs mit követni.",
    "Napló",
    "Az iránytű olvasása",
    "A tűje a követett feladatra mutat.",
    "Lopakodj (%s) iránytűvel a kezedben, hogy a következő nyitott feladatot kövesd.",
    "Ha a tű pörög, nincs nyitott feladat.",
    "A nyitott feladatok a következő oldalakon vannak.",
    "Teendő",
    "Kész",
    "◆ Ezt követed.",
];

const RO: Table = [
    "Obiectiv nou: %s",
    "Obiectiv îndeplinit: %s",
    "%s — încheiată.",
    "O aventură Delvewright.",
    "Aventură încheiată",
    "Se așteaptă grupul — %s / %s",
    "Alege-ți clasa",
    "Alege echipamentul pe care îl vei lua.",
    "Mareea te împinge înapoi — aventura a rămas în urmă.",
    "Trecerea este pecetluită.",
    "Foc de tabără",
    "Odihnește-te și salvează",
    "Doar salvează",
    "Revii în %s",
    "Busolă",
    "Urmărești: %s",
    "Alte sarcini deschise: %s — furișează-te (%s) cu busola în mână ca să schimbi.",
    "Încă nimic de urmărit.",
    "Jurnal",
    "Cum citești busola",
    "Acul ei arată sarcina pe care o urmărești.",
    "Furișează-te (%s) cu busola în mână ca să urmărești următoarea sarcină deschisă.",
    "Un ac care se rotește înseamnă că nimic nu e deschis.",
    "Sarcinile deschise sunt în paginile următoare.",
    "De făcut",
    "Încheiată",
    "◆ Urmărești această sarcină.",
];

const EL: Table = [
    "Νέος στόχος: %s",
    "Ο στόχος ολοκληρώθηκε: %s",
    "%s — ολοκληρώθηκε.",
    "Μια περιπέτεια Delvewright.",
    "Η περιπέτεια ολοκληρώθηκε",
    "Αναμονή για την ομάδα — %s / %s",
    "Διάλεξε την κλάση σου",
    "Διάλεξε τον εξοπλισμό που θα πάρεις μαζί σου.",
    "Η παλίρροια σε γυρίζει πίσω — η περιπέτεια είναι πίσω σου.",
    "Το πέρασμα είναι σφραγισμένο.",
    "Φωτιά",
    "Ξεκουράσου και αποθήκευσε",
    "Μόνο αποθήκευση",
    "Επιστροφή σε %s",
    "Πυξίδα",
    "Ακολουθείς: %s",
    "Άλλες ανοιχτές αποστολές: %s — σκύψε (%s) με την πυξίδα στο χέρι για εναλλαγή.",
    "Τίποτα να ακολουθήσεις ακόμα.",
    "Ημερολόγιο",
    "Πώς διαβάζεται η πυξίδα",
    "Η βελόνα της δείχνει την αποστολή που ακολουθείς.",
    "Σκύψε (%s) με την πυξίδα στο χέρι για να ακολουθήσεις την επόμενη ανοιχτή αποστολή.",
    "Αν η βελόνα γυρίζει, δεν υπάρχει τίποτα ανοιχτό.",
    "Οι ανοιχτές αποστολές βρίσκονται στις επόμενες σελίδες.",
    "Προς εκτέλεση",
    "Ολοκληρώθηκε",
    "◆ Ακολουθείς αυτήν.",
];

const BG: Table = [
    "Нова цел: %s",
    "Целта е изпълнена: %s",
    "%s — завършено.",
    "Приключение на Delvewright.",
    "Приключението е завършено",
    "Изчакване на групата — %s / %s",
    "Избери своя клас",
    "Избери снаряжението, което ще носиш.",
    "Приливът те връща назад — приключението остана зад теб.",
    "Пътят е запечатан.",
    "Огън",
    "Почини си и запази",
    "Само запази",
    "Връщане след %s",
    "Компас",
    "Следиш: %s",
    "Други отворени задачи: %s — промъкни се (%s) с компаса в ръка, за да смениш.",
    "Все още няма какво да следиш.",
    "Дневник",
    "Как се чете компасът",
    "Стрелката му сочи задачата, която следиш.",
    "Промъкни се (%s) с компаса в ръка, за да следиш следващата отворена задача.",
    "Въртяща се стрелка значи, че няма нищо отворено.",
    "Отворените задачи са на следващите страници.",
    "За вършене",
    "Изпълнена",
    "◆ Следиш тази задача.",
];

const TH: Table = [
    "เป้าหมายใหม่: %s",
    "ทำเป้าหมายสำเร็จ: %s",
    "%s — จบแล้ว",
    "การผจญภัยของ Delvewright",
    "ผจญภัยสำเร็จ",
    "กำลังรอกลุ่ม — %s / %s",
    "เลือกคลาสของคุณ",
    "เลือกอุปกรณ์ที่คุณจะนำติดตัวไป",
    "กระแสน้ำพัดคุณกลับมา — การผจญภัยอยู่ข้างหลังคุณแล้ว",
    "ทางนี้ถูกปิดผนึกไว้",
    "กองไฟ",
    "พักและบันทึก",
    "บันทึกเท่านั้น",
    "กลับมาใน %s",
    "เข็มทิศ",
    "กำลังติดตาม: %s",
    "ภารกิจอื่นที่ยังเปิดอยู่: %s — ถือเข็มทิศแล้วย่อง (%s) เพื่อสลับ",
    "ยังไม่มีอะไรให้ติดตาม",
    "บันทึก",
    "วิธีอ่านเข็มทิศ",
    "เข็มชี้ไปยังภารกิจที่คุณติดตาม",
    "ถือเข็มทิศแล้วย่อง (%s) เพื่อติดตามภารกิจถัดไปที่ยังเปิดอยู่",
    "ถ้าเข็มหมุนไม่หยุด แปลว่าไม่มีภารกิจที่เปิดอยู่",
    "ภารกิจที่ยังเปิดอยู่อยู่ในหน้าถัดไป",
    "สิ่งที่ต้องทำ",
    "เสร็จแล้ว",
    "◆ คุณกำลังติดตามภารกิจนี้",
];

const VI: Table = [
    "Mục tiêu mới: %s",
    "Hoàn thành mục tiêu: %s",
    "%s — hoàn thành.",
    "Một cuộc phiêu lưu Delvewright.",
    "Hoàn thành cuộc phiêu lưu",
    "Đang chờ cả nhóm — %s / %s",
    "Chọn lớp nhân vật của bạn",
    "Chọn trang bị bạn sẽ mang theo.",
    "Thủy triều đẩy bạn trở lại — cuộc phiêu lưu ở phía sau bạn.",
    "Lối đi đã bị niêm phong.",
    "Lửa trại",
    "Nghỉ ngơi và lưu",
    "Chỉ lưu",
    "Quay lại sau %s",
    "La bàn",
    "Đang theo dõi: %s",
    "Nhiệm vụ đang mở khác: %s — cầm la bàn và lén lút (%s) để chuyển.",
    "Chưa có gì để theo dõi.",
    "Nhật ký",
    "Cách đọc la bàn",
    "Kim của nó chỉ tới nhiệm vụ bạn đang theo dõi.",
    "Cầm la bàn và lén lút (%s) để theo dõi nhiệm vụ đang mở tiếp theo.",
    "Kim quay tròn nghĩa là không có gì đang mở.",
    "Các nhiệm vụ đang mở nằm ở những trang sau.",
    "Việc cần làm",
    "Hoàn thành",
    "◆ Bạn đang theo dõi nhiệm vụ này.",
];

const ID: Table = [
    "Tujuan baru: %s",
    "Tujuan selesai: %s",
    "%s — selesai.",
    "Sebuah petualangan Delvewright.",
    "Petualangan selesai",
    "Menunggu rombongan — %s / %s",
    "Pilih kelasmu",
    "Pilih perlengkapan yang akan kamu bawa.",
    "Air pasang membawamu kembali — petualangan ada di belakangmu.",
    "Jalan ini tersegel.",
    "Api unggun",
    "Istirahat dan simpan",
    "Simpan saja",
    "Kembali dalam %s",
    "Kompas",
    "Mengikuti: %s",
    "Tugas terbuka lainnya: %s — menyelinaplah (%s) sambil memegang kompas untuk berganti.",
    "Belum ada yang bisa diikuti.",
    "Jurnal",
    "Membaca kompas",
    "Jarumnya menunjuk tugas yang kamu ikuti.",
    "Menyelinaplah (%s) sambil memegang kompas untuk mengikuti tugas terbuka berikutnya.",
    "Jarum yang berputar berarti tidak ada yang terbuka.",
    "Tugas yang terbuka ada di halaman-halaman berikutnya.",
    "Yang harus dilakukan",
    "Selesai",
    "◆ Kamu mengikuti tugas ini.",
];

const MS: Table = [
    "Objektif baharu: %s",
    "Objektif selesai: %s",
    "%s — selesai.",
    "Sebuah pengembaraan Delvewright.",
    "Pengembaraan selesai",
    "Menunggu kumpulan — %s / %s",
    "Pilih kelas anda",
    "Pilih kelengkapan yang akan anda bawa.",
    "Air pasang membawa anda kembali — pengembaraan berada di belakang anda.",
    "Laluan ini telah dimeterai.",
    "Unggun api",
    "Berehat dan simpan",
    "Simpan sahaja",
    "Kembali dalam %s",
    "Kompas",
    "Mengikuti: %s",
    "Tugas terbuka lain: %s — menyelinap (%s) sambil memegang kompas untuk bertukar.",
    "Belum ada apa-apa untuk diikuti.",
    "Jurnal",
    "Membaca kompas",
    "Jarumnya menunjuk ke tugas yang anda ikuti.",
    "Menyelinap (%s) sambil memegang kompas untuk mengikuti tugas terbuka seterusnya.",
    "Jarum yang berpusing bermakna tiada tugas terbuka.",
    "Tugas terbuka ada di halaman-halaman seterusnya.",
    "Perlu dibuat",
    "Selesai",
    "◆ Anda sedang mengikuti tugas ini.",
];

const AR: Table = [
    "هدف جديد: %s",
    "اكتمل الهدف: %s",
    "%s — اكتملت.",
    "مغامرة من Delvewright.",
    "اكتملت المغامرة",
    "في انتظار الفريق — %s / %s",
    "اختر فئتك",
    "اختر العتاد الذي ستحمله.",
    "المد يعيدك — المغامرة خلفك.",
    "الطريق مغلق.",
    "نار المخيم",
    "استرح واحفظ",
    "احفظ فقط",
    "العودة خلال %s",
    "بوصلة",
    "تتبع: %s",
    "مهام مفتوحة أخرى: %s — تسلّل (%s) والبوصلة في يدك للتبديل.",
    "لا شيء لتتبعه بعد.",
    "اليوميات",
    "قراءة البوصلة",
    "تشير إبرتها إلى المهمة التي تتبعها.",
    "تسلّل (%s) والبوصلة في يدك لتتبع المهمة المفتوحة التالية.",
    "الإبرة الدوّارة تعني أنه لا توجد مهمة مفتوحة.",
    "المهام المفتوحة في الصفحات التالية.",
    "مهمة قائمة",
    "منجزة",
    "◆ أنت تتبع هذه المهمة.",
];

const HE: Table = [
    "יעד חדש: %s",
    "היעד הושלם: %s",
    "%s — הושלמה.",
    "הרפתקה של Delvewright.",
    "ההרפתקה הושלמה",
    "ממתינים לחבורה — %s / %s",
    "בחר את המחלקה שלך",
    "בחר את הציוד שתישא איתך.",
    "הגאות מחזירה אותך — ההרפתקה מאחוריך.",
    "הדרך חתומה.",
    "מדורה",
    "לנוח ולשמור",
    "לשמור בלבד",
    "חזרה בעוד %s",
    "מצפן",
    "עוקב אחר: %s",
    "משימות פתוחות נוספות: %s — התגנב (%s) עם המצפן ביד כדי להחליף.",
    "אין עדיין אחרי מה לעקוב.",
    "יומן",
    "איך לקרוא את המצפן",
    "המחט שלו מצביעה על המשימה שאתה עוקב אחריה.",
    "התגנב (%s) עם המצפן ביד כדי לעקוב אחר המשימה הפתוחה הבאה.",
    "מחט מסתובבת פירושה שאין משימה פתוחה.",
    "המשימות הפתוחות מופיעות בעמודים הבאים.",
    "לביצוע",
    "הושלמה",
    "◆ אתה עוקב אחר משימה זו.",
];

const HI: Table = [
    "नया लक्ष्य: %s",
    "लक्ष्य पूरा हुआ: %s",
    "%s — पूर्ण।",
    "एक Delvewright साहसिक यात्रा।",
    "साहसिक यात्रा पूर्ण",
    "दल की प्रतीक्षा — %s / %s",
    "अपनी श्रेणी चुनें",
    "वह सामान चुनें जो आप साथ ले जाएँगे।",
    "ज्वार आपको वापस मोड़ देता है — यात्रा आपके पीछे रह गई।",
    "यह रास्ता बंद है।",
    "अलाव",
    "विश्राम करें और सहेजें",
    "केवल सहेजें",
    "%s में वापसी",
    "कम्पास",
    "अनुसरण: %s",
    "अन्य खुले कार्य: %s — बदलने के लिए कम्पास हाथ में लेकर दबे पाँव चलें (%s)।",
    "अभी अनुसरण करने को कुछ नहीं है।",
    "डायरी",
    "कम्पास कैसे पढ़ें",
    "इसकी सुई उस कार्य की ओर इशारा करती है जिसका आप अनुसरण कर रहे हैं।",
    "अगले खुले कार्य का अनुसरण करने के लिए कम्पास हाथ में लेकर दबे पाँव चलें (%s)।",
    "घूमती सुई का अर्थ है कि कोई कार्य खुला नहीं है।",
    "खुले कार्य आगे के पन्नों पर हैं।",
    "करना बाकी",
    "पूर्ण",
    "◆ आप इसका अनुसरण कर रहे हैं।",
];

const CA: Table = [
    "Nou objectiu: %s",
    "Objectiu completat: %s",
    "%s — completada.",
    "Una aventura de Delvewright.",
    "Aventura completada",
    "Esperant el grup — %s / %s",
    "Tria la teva classe",
    "Tria l'equipament que t'enduràs.",
    "La marea et fa retrocedir: l'aventura queda enrere.",
    "El pas està segellat.",
    "Foguera",
    "Descansa i desa",
    "Només desa",
    "Tornes en %s",
    "Brúixola",
    "Seguint: %s",
    "Altres tasques obertes: %s — ajup-te (%s) amb la brúixola a la mà per canviar.",
    "Encara no hi ha res a seguir.",
    "Diari",
    "Com llegir la brúixola",
    "La seva agulla assenyala la tasca que segueixes.",
    "Ajup-te (%s) amb la brúixola a la mà per seguir la següent tasca oberta.",
    "Si l'agulla gira, no hi ha res obert.",
    "Les tasques obertes són a les pàgines següents.",
    "Per fer",
    "Feta",
    "◆ Estàs seguint aquesta tasca.",
];

const FIL: Table = [
    "Bagong layunin: %s",
    "Natapos ang layunin: %s",
    "%s — tapos na.",
    "Isang pakikipagsapalaran ng Delvewright.",
    "Tapos na ang pakikipagsapalaran",
    "Naghihintay sa grupo — %s / %s",
    "Piliin ang iyong klase",
    "Piliin ang kagamitang dadalhin mo.",
    "Ibinalik ka ng agos — nasa likuran mo na ang pakikipagsapalaran.",
    "Nakasara ang daan.",
    "Siga",
    "Magpahinga at mag-save",
    "Mag-save lamang",
    "Babalik sa loob ng %s",
    "Kompas",
    "Sinusundan: %s",
    "Iba pang bukas na gawain: %s — yumuko (%s) habang hawak ang kompas para lumipat.",
    "Wala pang masusundan.",
    "Talaarawan",
    "Pagbasa ng kompas",
    "Itinuturo ng karayom nito ang gawaing sinusundan mo.",
    "Yumuko (%s) habang hawak ang kompas para sundan ang susunod na bukas na gawain.",
    "Kapag umiikot ang karayom, walang bukas na gawain.",
    "Nasa mga kasunod na pahina ang mga bukas na gawain.",
    "Gagawin",
    "Tapos na",
    "◆ Sinusundan mo ito.",
];

/// Which Minecraft language file gets which table. Keyed on the client's own file
/// stem ([`crate::mclang::CLIENT_LANGS`]), so a locale variant is an explicit row
/// rather than a prefix guess — `pt_br` and `pt_pt` genuinely differ, and `es_mx`
/// genuinely does not.
///
/// A stem absent here has **no** chrome table and renders English (the honest
/// fallback). English locales (`en_us`, `en_gb`, `en_au`, `en_ca`, `en_nz`) are
/// absent for the same reason and read the canonical text directly.
const TABLES: &[(&str, &Table)] = &[
    ("ar_sa", &AR),
    ("bg_bg", &BG),
    ("ca_es", &CA),
    ("cs_cz", &CS),
    ("da_dk", &DA),
    ("de_at", &DE),
    ("de_ch", &DE),
    ("de_de", &DE),
    ("el_gr", &EL),
    ("es_ar", &ES),
    ("es_cl", &ES),
    ("es_ec", &ES),
    ("es_es", &ES),
    ("es_mx", &ES),
    ("es_uy", &ES),
    ("es_ve", &ES),
    ("fi_fi", &FI),
    ("fil_ph", &FIL),
    ("fr_ca", &FR),
    ("fr_ch", &FR),
    ("fr_fr", &FR),
    ("he_il", &HE),
    ("hi_in", &HI),
    ("hu_hu", &HU),
    ("id_id", &ID),
    ("it_it", &IT),
    ("ja_jp", &JA),
    ("ko_kr", &KO),
    ("ms_my", &MS),
    ("nl_be", &NL),
    ("nl_nl", &NL),
    ("no_no", &NB),
    ("pl_pl", &PL),
    ("pt_br", &PT_BR),
    ("pt_pt", &PT_PT),
    ("ro_ro", &RO),
    ("ru_ru", &RU),
    ("sk_sk", &SK),
    ("sv_se", &SV),
    ("th_th", &TH),
    ("tl_ph", &FIL),
    ("tr_tr", &TR),
    ("uk_ua", &UK),
    ("vi_vn", &VI),
    ("zh_cn", &ZH_HANS),
    ("zh_hk", &ZH_HANT),
    ("zh_tw", &ZH_HANT),
];

/// The chrome table for a Minecraft language stem, or `None` when the compiler
/// ships none (in which case that language's chrome renders English).
fn table(mc_code: &str) -> Option<&'static Table> {
    TABLES.iter().find(|(c, _)| *c == mc_code).map(|(_, t)| *t)
}

/// The chrome entries to write into a language's
/// `assets/delvewright/lang/<mc>.json`. Empty for a language with no table, so the
/// client falls through to `en_us.json` (or to the component's own `fallback` if
/// the player declined the pack) and reads English.
pub fn lang_entries(mc_code: &str) -> BTreeMap<String, String> {
    let Some(t) = table(mc_code) else {
        return BTreeMap::new();
    };
    ALL.iter()
        .map(|c| (c.key.to_string(), t[c.slot].to_string()))
        .collect()
}

/// The canonical-English chrome entries — the `en_us.json` half, always complete.
pub fn english_entries() -> BTreeMap<String, String> {
    ALL.iter()
        .map(|c| (c.key.to_string(), c.en.to_string()))
        .collect()
}

/// How many of [`ALL`] a language really carries, as `(translated, total)`. Stated
/// as a count because a table that binds to nothing must be a number, not an
/// inference from silence.
pub fn coverage(mc_code: &str) -> (usize, usize) {
    (lang_entries(mc_code).len(), ALL.len())
}

/// `DW0186`: a campaign l10n sidecar may not define a key in the reserved chrome
/// namespace. Sits beside the other reserved-channel guards
/// ([`crate::l10n::validate_marker_channel`], [`crate::l10n::validate_tr_sigil`]):
/// chrome is compiler-owned end to end, so a sidecar row under `delvewright.` is
/// either a translator's mistake or an attempt to override text the engine owns —
/// and if it were written into a lang file it would silently replace product
/// chrome for that language. `DW0181` would also flag it as an orphan; this names
/// the actual reason.
pub fn validate_chrome_namespace(sidecars: &BTreeMap<String, L10nDoc>) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    for (lang, doc) in sidecars {
        for key in doc.content.keys() {
            if !key.starts_with(RESERVED_PREFIX) {
                continue;
            }
            d.push(Diagnostic::error(
                codes::CHROME_RESERVED,
                "l10n",
                format!("l10n/{lang}.json#/content/{key}"),
                format!(
                    "`{key}` is in the reserved `{RESERVED_PREFIX}` namespace, which the compiler \
                     owns: those are the engine's own on-screen strings (`New objective: `, \
                     `Choose your class`, …), they ship translated with the compiler, and no \
                     campaign authors or overrides them. Remove `{key}` from \
                     `l10n/{lang}.json` — to change what a campaign's own text says, translate \
                     that campaign's key instead"
                ),
            ));
        }
    }
    d
}

/// The reserved key prefix. No campaign l10n key can begin with it (the key scheme
/// derives a fixed set of kinds) and no vanilla key does either.
pub const RESERVED_PREFIX: &str = "delvewright.";

/// How chrome is rendered for one build: which language's text rides on each
/// component as its `fallback`.
///
/// * The default multi-language build (`delvec build`, spec-0029 v2) uses the
///   canonical English, and the resource pack's language files carry the rest —
///   the client picks. This is the release path.
/// * A `--lang <code>` **bake** ships no language files at all (spec-0029 §4), so
///   the fallback IS what the player reads: the compiler's translation for that
///   language when it has one, its English otherwise. The `translate` key still
///   rides along, harmlessly, and keeps `%s` substitution working — vanilla formats
///   the fallback with the same `with` arguments.
///
/// It also carries **which delve's key space** those components reference. Chrome
/// is engine-owned, but a lang file is not: the client merges every applied pack
/// into one language table, so a globally-keyed chrome row from a delve that
/// declares Chinese answers for a delve that ships English only, and the player
/// reads Chinese chrome wrapped around an English story. Chrome keys are therefore
/// namespaced exactly like campaign keys ([`crate::l10n::pack_key`]) — one delve,
/// one vocabulary, chrome included — and `delvewright.` stays the segment that
/// separates the compiler's strings from the campaign's *within* that namespace.
#[derive(Clone, Debug, Default)]
pub struct Chrome {
    /// The bake's Minecraft language stem, or `None` for the multi-language build.
    baked: Option<&'static Table>,
    /// The delve's [pack namespace](crate::l10n::pack_namespace), prefixed onto
    /// every key this resolver hands an emitter. Empty only for [`Self::default`],
    /// which no emitting consumer may use (see the type's doc).
    ns: String,
}

impl Chrome {
    /// Resolve chrome for a build of `campaign_id`. `language` is `None` for the
    /// default multi-language build and `Some(declared code)` for a `--lang` bake.
    pub fn for_build(campaign_id: &str, language: Option<&str>) -> Self {
        Self {
            baked: language
                .and_then(crate::mclang::mc_lang_code)
                .and_then(table),
            ns: crate::l10n::pack_namespace(campaign_id),
        }
    }

    /// The text this build puts on the component.
    fn text(&self, c: ChromeString) -> &str {
        match self.baked {
            Some(t) => t[c.slot],
            None => c.en,
        }
    }

    /// One chrome string, as the tagged form an emitter lowers into a component:
    /// this delve's pack key over this build's rendition.
    pub fn get(&self, c: ChromeString) -> String {
        tag(&format!("{}{}", self.ns, c.key), self.text(c))
    }

    /// Re-resolve a string that may already carry a chrome tag, for the chrome
    /// defaults the compiler bakes into its **plan** before the build language —
    /// or the delve's namespace — is known (a `close-gate`'s `sealed_hint`, a
    /// bonfire's three dialog strings). A plan-time default carries the bare
    /// chrome key, which is what this looks up; an authored string — which carries
    /// a campaign pack key — passes through untouched, as does any untagged
    /// literal, and so does a chrome tag already bound (its key is namespaced, so
    /// it matches no bare chrome key and rebinding is idempotent).
    pub fn rebind(&self, s: &str) -> String {
        let Some((key, _)) = crate::l10n::untag(s) else {
            return s.to_string();
        };
        match ALL.iter().find(|c| c.key == key) {
            Some(c) => self.get(*c),
            None => s.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholders(s: &str) -> usize {
        s.matches("%s").count()
    }

    /// Every chrome key is unique, reserved-prefixed (so it can collide with
    /// neither a campaign l10n key nor a vanilla one), slot-consistent, and has
    /// English whose `%s` count matches its declared arity.
    #[test]
    fn chrome_strings_are_well_formed() {
        let mut seen = std::collections::BTreeSet::new();
        for (i, c) in ALL.iter().enumerate() {
            assert_eq!(c.slot, i, "`{}` declares the wrong slot", c.key);
            assert!(
                c.key.starts_with("delvewright.ui."),
                "chrome key `{}` is outside the reserved namespace",
                c.key
            );
            assert!(
                c.key.starts_with(RESERVED_PREFIX),
                "`{}` must live under the reserved prefix",
                c.key
            );
            assert!(!c.en.is_empty(), "chrome key `{}` has no English", c.key);
            assert_eq!(
                placeholders(c.en),
                c.args,
                "`{}` declares {} args but its English has {}",
                c.key,
                c.args,
                placeholders(c.en)
            );
            assert!(seen.insert(c.key), "duplicate chrome key `{}`", c.key);
        }
        assert_eq!(ALL.len(), 27, "the chrome inventory changed size");
    }

    /// **The binding that makes `%s` safe.** Every language table renders every
    /// slot, non-empty, with exactly the placeholder count the English declares —
    /// a translation that drops `%s` loses the objective's title on screen, and one
    /// that adds a `%s` renders a substitution vanilla has no argument for.
    #[test]
    fn every_table_is_complete_and_placeholder_faithful() {
        assert!(!TABLES.is_empty(), "no chrome table binds");
        for (code, t) in TABLES {
            assert!(
                crate::mclang::CLIENT_LANGS.contains(code),
                "`{code}` has a chrome table but is not a language the pinned client loads"
            );
            for c in ALL {
                let s = t[c.slot];
                assert!(!s.is_empty(), "`{code}` leaves `{}` empty", c.key);
                assert_eq!(
                    placeholders(s),
                    c.args,
                    "`{code}` renders `{}` with {} placeholder(s), English has {}",
                    c.key,
                    placeholders(s),
                    c.args
                );
            }
        }
    }

    /// The table list is sorted and has no duplicate stem: a second row for one
    /// language would be a silent, order-dependent override.
    #[test]
    fn table_rows_are_sorted_and_unique() {
        let codes: Vec<&str> = TABLES.iter().map(|(c, _)| *c).collect();
        assert!(
            codes.windows(2).all(|w| w[0] < w[1]),
            "TABLES must stay sorted by language stem"
        );
    }

    /// `zh-cn` is fully covered: the owner reads Chinese, and Chinese chrome around
    /// Chinese story is the exact defect this module exists to close.
    #[test]
    fn zh_cn_is_complete() {
        assert_eq!(coverage("zh_cn"), (27, 27));
        assert_eq!(lang_entries("zh_cn")[CLASS_TITLE.key], "选择你的职业");
    }

    /// A language with no table ships no chrome rows — absent, never English text
    /// masquerading as a translation.
    #[test]
    fn untabled_language_ships_no_chrome() {
        assert_eq!(coverage("tlh_aa"), (0, 27));
        assert!(lang_entries("en_us").is_empty());
        assert_eq!(english_entries().len(), 27);
    }

    /// The multi-language build puts English on the component (the pack carries the
    /// rest); a bake puts the baked language on it, falling back to English. Every
    /// key a resolver hands out is the delve's, never the bare chrome key.
    #[test]
    fn build_carries_english_and_bake_carries_its_language() {
        let key = crate::l10n::pack_key("doune-castle", CLASS_TITLE.key);

        let multi = Chrome::for_build("doune-castle", None);
        assert_eq!(multi.get(CLASS_TITLE), tag(&key, CLASS_TITLE.en));
        assert_eq!(
            multi.rebind(&CLASS_TITLE.tagged()),
            tag(&key, CLASS_TITLE.en)
        );

        let zh = Chrome::for_build("doune-castle", Some("zh-cn"));
        assert_eq!(zh.get(CLASS_TITLE), tag(&key, "选择你的职业"));
        assert_eq!(zh.rebind(&CLASS_TITLE.tagged()), tag(&key, "选择你的职业"));

        let klingon = Chrome::for_build("doune-castle", Some("tlh-aa"));
        assert_eq!(klingon.get(CLASS_TITLE), tag(&key, CLASS_TITLE.en));
    }

    /// **One delve, one vocabulary, chrome included.** Two delves resolve the same
    /// chrome string to two keys, so a lang file one of them left applied in the
    /// client answers for nothing the other asks — the engine's own strings are in
    /// the pack too, and a globally-keyed one renders a delve's chrome in a
    /// language that delve does not ship.
    #[test]
    fn two_delves_share_no_chrome_key() {
        let a = Chrome::for_build("doune-castle", None);
        let b = Chrome::for_build("nobodys-cave-island", Some("zh-cn"));
        let mut n = 0;
        for c in ALL {
            let (ta, tb) = (a.get(*c), b.get(*c));
            let (ka, _) = crate::l10n::untag(&ta).expect("chrome is tagged");
            let (kb, _) = crate::l10n::untag(&tb).expect("chrome is tagged");
            assert_ne!(ka, kb, "`{}` is one key for two delves", c.key);
            n += 1;
        }
        assert_eq!(n, ALL.len(), "every chrome string compared");
    }

    /// `rebind` only ever resolves chrome keys: an authored string that reaches it
    /// (a bonfire label the campaign really wrote) passes through unchanged, and so
    /// does a chrome tag already bound to a delve — rebinding is idempotent.
    #[test]
    fn rebind_leaves_authored_strings_alone() {
        let zh = Chrome::for_build("demo", Some("zh-cn"));
        assert_eq!(zh.rebind("Shrine fire"), "Shrine fire");
        let authored = tag(
            &crate::l10n::pack_key("demo", "fx.q.done.0.rest_prompt"),
            "Shrine fire",
        );
        assert_eq!(zh.rebind(&authored), authored);
        let bound = zh.get(CLASS_TITLE);
        assert_eq!(zh.rebind(&bound), bound);
    }

    /// `DW0186`: a sidecar cannot define a chrome key.
    #[test]
    fn sidecar_may_not_shadow_chrome() {
        let mut content = BTreeMap::new();
        content.insert(CLASS_TITLE.key.to_string(), "俺の職業".to_string());
        content.insert("npc.keeper.name".to_string(), "門番".to_string());
        let doc = L10nDoc {
            dsl_version: crate::DSL_VERSION.to_string(),
            campaign_id: crate::ids::CampaignId("demo".to_string()),
            kind: crate::l10n::L10nKind::L10n,
            lang: "ja-jp".to_string(),
            content,
            source: BTreeMap::new(),
        };
        let d = validate_chrome_namespace(&BTreeMap::from([("ja-jp".to_string(), doc)]));
        assert_eq!(d.len(), 1, "exactly the chrome row is flagged");
        assert_eq!(d[0].code, codes::CHROME_RESERVED);
        assert!(d[0].message.contains(CLASS_TITLE.key));
    }
}
