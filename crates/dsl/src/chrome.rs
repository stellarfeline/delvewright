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
//! for text it did not write: a translator answering the same thirteen keys,
//! identically, once per delve, forever. And it would be answering them for
//! **product chrome**: `New objective: `, `Delve Complete`, `Choose your class`
//! are not lines any campaign author wants to write. That eight of them never grew
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
//! checked by a native speaker — which is defensible for thirteen short functional
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

    /// This string as a translation tag over its canonical English.
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
    BONFIRE_TITLE = 10, "delvewright.ui.bonfire.title", 0, crate::stages::BONFIRE_PROMPT_EN;
    /// The bonfire's **rest and save** button when no `rest_label` is authored.
    BONFIRE_REST = 11, "delvewright.ui.bonfire.rest", 0, crate::stages::BONFIRE_REST_LABEL_EN;
    /// The bonfire's **save only** button when no `save_label` is authored.
    BONFIRE_SAVE = 12, "delvewright.ui.bonfire.save", 0, crate::stages::BONFIRE_SAVE_LABEL_EN;
}

/// One language's rendition of [`ALL`], in slot order.
type Table = [&'static str; 13];

// --- the tables ------------------------------------------------------------
// Slot order is ALL's: objective.new, objective.complete, campaign.complete,
// campaign.signature, campaign.banner, lobby.waiting, class.title, class.body,
// boundary.message, gate.sealed, bonfire.title, bonfire.rest, bonfire.save.

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
#[derive(Clone, Debug, Default)]
pub struct Chrome {
    /// The bake's Minecraft language stem, or `None` for the multi-language build.
    baked: Option<&'static Table>,
}

impl Chrome {
    /// Resolve chrome for a build. `language` is `None` for the default
    /// multi-language build and `Some(declared code)` for a `--lang` bake.
    pub fn for_build(language: Option<&str>) -> Self {
        Self {
            baked: language
                .and_then(crate::mclang::mc_lang_code)
                .and_then(table),
        }
    }

    /// The text this build puts on the component.
    fn text(&self, c: ChromeString) -> &str {
        match self.baked {
            Some(t) => t[c.slot],
            None => c.en,
        }
    }

    /// One chrome string, as the tagged form an emitter lowers into a component.
    pub fn get(&self, c: ChromeString) -> String {
        c.tagged_as(self.text(c))
    }

    /// Re-resolve a string that may already carry a chrome tag, for the chrome
    /// defaults the compiler bakes into its **plan** before the build language is
    /// known (a `close-gate`'s `sealed_hint`, a bonfire's three dialog strings).
    /// An authored string — which carries a campaign l10n key — passes through
    /// untouched, as does any untagged literal.
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
        assert_eq!(ALL.len(), 13, "the chrome inventory changed size");
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
        assert_eq!(coverage("zh_cn"), (13, 13));
        assert_eq!(lang_entries("zh_cn")[CLASS_TITLE.key], "选择你的职业");
    }

    /// A language with no table ships no chrome rows — absent, never English text
    /// masquerading as a translation.
    #[test]
    fn untabled_language_ships_no_chrome() {
        assert_eq!(coverage("tlh_aa"), (0, 13));
        assert!(lang_entries("en_us").is_empty());
        assert_eq!(english_entries().len(), 13);
    }

    /// The multi-language build puts English on the component (the pack carries the
    /// rest); a bake puts the baked language on it, falling back to English.
    #[test]
    fn build_carries_english_and_bake_carries_its_language() {
        let multi = Chrome::for_build(None);
        assert_eq!(multi.get(CLASS_TITLE), CLASS_TITLE.tagged());
        assert_eq!(multi.rebind(&CLASS_TITLE.tagged()), CLASS_TITLE.tagged());

        let zh = Chrome::for_build(Some("zh-cn"));
        assert_eq!(zh.get(CLASS_TITLE), tag(CLASS_TITLE.key, "选择你的职业"));
        assert_eq!(
            zh.rebind(&CLASS_TITLE.tagged()),
            tag(CLASS_TITLE.key, "选择你的职业")
        );

        let klingon = Chrome::for_build(Some("tlh-aa"));
        assert_eq!(klingon.get(CLASS_TITLE), CLASS_TITLE.tagged());
    }

    /// `rebind` only ever resolves chrome keys: an authored string that reaches it
    /// (a bonfire label the campaign really wrote) passes through unchanged.
    #[test]
    fn rebind_leaves_authored_strings_alone() {
        let zh = Chrome::for_build(Some("zh-cn"));
        assert_eq!(zh.rebind("Shrine fire"), "Shrine fire");
        let authored = tag("fx.q.done.0.rest_prompt", "Shrine fire");
        assert_eq!(zh.rebind(&authored), authored);
    }

    /// `DW0186`: a sidecar cannot define a chrome key.
    #[test]
    fn sidecar_may_not_shadow_chrome() {
        let mut content = BTreeMap::new();
        content.insert(CLASS_TITLE.key.to_string(), "俺の職業".to_string());
        content.insert("npc.keeper.name".to_string(), "門番".to_string());
        let doc = L10nDoc {
            dsl_version: "0.19.0".to_string(),
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
