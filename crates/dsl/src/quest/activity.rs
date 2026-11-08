//! **The activity census** (spec-0131): what the player does on each line of
//! the plan, counted in the act vocabulary of spec-0114, and how long those
//! acts take at the metrics table's price per act.
//!
//! A *line* is one planned quest (stage 4) with its stage-5 expansion. Its
//! activity is the acts its objectives perform ([`Quest::objective_acts`], the
//! one derivation); its minutes are the sum over its objectives of the price
//! of each objective's dearest act. The census is a statement, never a
//! verdict: it refuses nothing, and it is printed with its denominator on
//! every `validate` so a plan whose every line is a conversation is seen as
//! one before the content step closes.

use std::collections::{BTreeMap, BTreeSet};

use crate::envelope::Campaign;
use crate::metrics::{MetricKind, MetricValue, Metrics, Reads};
use crate::quest::{Act, Quest};

/// The table's price of each act, in minutes per objective performing it.
///
/// Read through [`ActivityRates::read`], which reads **every** act's entry —
/// a zero count of an act is still priced at the table's rate, and the price
/// list is printed whole — so `DW0813` names each seed the estimate rests on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityRates {
    minutes: BTreeMap<Act, u32>,
}

impl ActivityRates {
    /// The metrics-table name of an act's price: `pacing.minutes-per-<act>`.
    pub fn metric_name(act: Act) -> String {
        format!("minutes-per-{}", act.as_str())
    }

    /// The price of every act, read from `table` and recorded in `reads`.
    ///
    /// `None` when the table defines no entry for some act — a defect of the
    /// table, which [`Metrics::self_check`] reports — so no estimate is made
    /// from a partial price list.
    pub fn read(table: &Metrics, reads: &mut Reads) -> Option<Self> {
        let mut minutes = BTreeMap::new();
        for act in Act::ALL {
            let entry = table
                .resolve(MetricKind::Pacing, &Self::metric_name(act))
                .ok()?;
            let MetricValue::Count(n) = entry.value(reads) else {
                return None;
            };
            minutes.insert(act, *n);
        }
        Some(Self { minutes })
    }

    /// The price of one act.
    pub fn of(&self, act: Act) -> u32 {
        self.minutes.get(&act).copied().unwrap_or(0)
    }

    /// The price of one objective: its dearest act. An objective performs
    /// its acts in one completion — an `interact` that presents and takes an
    /// item is one use at the thing — so the acts are not summed.
    pub fn objective_minutes(&self, acts: &BTreeSet<Act>) -> u64 {
        acts.iter().map(|a| u64::from(self.of(*a))).max().unwrap_or(0)
    }

    /// The price list as the census line prints it: `talk 2, reach 0, …`, in
    /// act order.
    pub fn line(&self) -> String {
        Act::ALL
            .iter()
            .map(|a| format!("{} {}", a.as_str(), self.of(*a)))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// One line of the plan: a planned quest and what its expansion has the
/// player do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineActivity {
    /// The planned quest's id.
    pub quest: String,
    /// `false` for an optional quest (`mandatory: false`).
    pub mandatory: bool,
    /// Objectives in the stage-5 quest.
    pub objectives: usize,
    /// How many objectives perform each act. An objective performing several
    /// acts is counted under each, so the counts may sum to more than
    /// `objectives`.
    pub acts: BTreeMap<Act, usize>,
    /// The union of the acts the line performs.
    pub performs: BTreeSet<Act>,
    /// The line's activity minutes at the table's rates.
    pub minutes: u64,
}

impl LineActivity {
    /// Whether every objective of the line is a conversation or a walk — the
    /// shape a line has when nothing on it is done: no deed but talk.
    pub fn talk_or_reach_only(&self) -> bool {
        self.objectives > 0
            && self
                .performs
                .iter()
                .all(|a| matches!(a, Act::Talk | Act::Reach))
    }

    /// `quest/x: 3 objective(s) — talk 2, reach 1; 4 min`.
    pub fn line(&self) -> String {
        let acts = if self.acts.is_empty() {
            "no act".to_string()
        } else {
            self.acts
                .iter()
                .map(|(a, n)| format!("{} {n}", a.as_str()))
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "`{}`{}: {} objective(s) — {acts}; about {} min",
            self.quest,
            if self.mandatory { "" } else { " (optional)" },
            self.objectives,
            self.minutes
        )
    }
}

/// The census over every line of the plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityCensus {
    /// Every planned quest that has a stage-5 expansion, in plan order.
    pub lines: Vec<LineActivity>,
    /// Planned quests with no stage-5 quest yet — the state step 3 leaves
    /// (`DW0150`), counted and not priced.
    pub unexpanded: Vec<String>,
    /// Objectives over every line.
    pub objectives: usize,
    /// How many objectives perform each act, over every line.
    pub acts: BTreeMap<Act, usize>,
    /// Activity minutes over every line.
    pub minutes: u64,
    /// Activity minutes over the mandatory lines alone.
    pub mandatory_minutes: u64,
    /// The price list the minutes were computed at.
    pub rates: ActivityRates,
    /// The world's declared `target_minutes`, beside which the estimate reads.
    pub target_minutes: u32,
}

impl ActivityCensus {
    /// The census of `c` at `table`'s rates, recording every rate read.
    ///
    /// `None` when the table defines no price for some act, which
    /// [`Metrics::self_check`] reports; the census then has no number to print
    /// and prints nothing rather than an estimate from a partial list.
    pub fn of(c: &Campaign, table: &Metrics, reads: &mut Reads) -> Option<Self> {
        let rates = ActivityRates::read(table, reads)?;
        let by_id: BTreeMap<&str, &Quest> = c
            .quests
            .content
            .quests
            .iter()
            .map(|q| (q.id.as_str(), q))
            .collect();
        let mut census = Self {
            lines: Vec::new(),
            unexpanded: Vec::new(),
            objectives: 0,
            acts: BTreeMap::new(),
            minutes: 0,
            mandatory_minutes: 0,
            rates,
            target_minutes: c.world.content.target_minutes,
        };
        for planned in &c.quest_plan.content.quests {
            let Some(q) = by_id.get(planned.id.as_str()) else {
                census.unexpanded.push(planned.id.to_string());
                continue;
            };
            let mut line = LineActivity {
                quest: planned.id.to_string(),
                mandatory: planned.mandatory,
                objectives: q.objectives.len(),
                acts: BTreeMap::new(),
                performs: BTreeSet::new(),
                minutes: 0,
            };
            for o in &q.objectives {
                let acts = q.objective_acts(o);
                for a in &acts {
                    *line.acts.entry(*a).or_default() += 1;
                    *census.acts.entry(*a).or_default() += 1;
                }
                line.performs.extend(acts.iter().copied());
                line.minutes += census.rates.objective_minutes(&acts);
            }
            census.objectives += line.objectives;
            census.minutes += line.minutes;
            if line.mandatory {
                census.mandatory_minutes += line.minutes;
            }
            census.lines.push(line);
        }
        Some(census)
    }

    /// The lines whose every objective is talk or reach.
    pub fn talk_or_reach_only(&self) -> Vec<&LineActivity> {
        self.lines
            .iter()
            .filter(|l| l.talk_or_reach_only())
            .collect()
    }

    /// The acts over every line as one clause: `talk 16, reach 2`, or `no act`.
    pub fn acts_clause(&self) -> String {
        if self.acts.is_empty() {
            return "no act".to_string();
        }
        self.acts
            .iter()
            .map(|(a, n)| format!("{} {n}", a.as_str()))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The one clause the pacing line (`DW0822`) adds to its route figure: the
    /// activity minutes, their denominator and the price list.
    pub fn pacing_clause(&self) -> String {
        format!(
            "the quests' acts add about {} minute(s) over {} objective(s) on {} line(s) \
             ({}; at {} minutes an objective, by its dearest act){}",
            self.minutes,
            self.objectives,
            self.lines.len(),
            self.acts_clause(),
            self.rates.line(),
            if self.unexpanded.is_empty() {
                String::new()
            } else {
                format!(
                    ", with {} planned quest(s) not yet expanded and not in that total",
                    self.unexpanded.len()
                )
            }
        )
    }

    /// The census as `delvec validate` prints it: a header line, then one
    /// line per planned quest.
    pub fn lines(&self) -> Vec<String> {
        let only = self.talk_or_reach_only();
        let mut out = vec![format!(
            "activity census: {} line(s) over {} objective(s) — {}; about {} min of activity \
             ({} on the mandatory lines) against `target_minutes` {}; {} line(s) talk-or-reach \
             only{}{}",
            self.lines.len(),
            self.objectives,
            self.acts_clause(),
            self.minutes,
            self.mandatory_minutes,
            self.target_minutes,
            only.len(),
            if only.is_empty() {
                String::new()
            } else {
                format!(
                    " ({})",
                    only.iter()
                        .map(|l| format!("`{}`", l.quest))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            },
            if self.unexpanded.is_empty() {
                String::new()
            } else {
                format!(
                    "; {} planned quest(s) awaiting stage 5 ({})",
                    self.unexpanded.len(),
                    self.unexpanded
                        .iter()
                        .map(|q| format!("`{q}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            },
        )];
        out.extend(self.lines.iter().map(|l| format!("  {}", l.line())));
        out
    }

    /// The ledger `validation/activity-census.json` carries. `examined` is the
    /// number of lines read, so a build of a plan with expansions that counted
    /// none reads as a zero binding.
    pub fn to_json(&self) -> serde_json::Value {
        let acts = |m: &BTreeMap<Act, usize>| {
            m.iter()
                .map(|(a, n)| (a.as_str().to_string(), serde_json::json!(n)))
                .collect::<serde_json::Map<_, _>>()
        };
        serde_json::json!({
            "examined": self.lines.len(),
            "lines": self.lines.iter().map(|l| serde_json::json!({
                "quest": l.quest,
                "mandatory": l.mandatory,
                "objectives": l.objectives,
                "acts": acts(&l.acts),
                "minutes": l.minutes,
                "talk_or_reach_only": l.talk_or_reach_only(),
            })).collect::<Vec<_>>(),
            "unexpanded": self.unexpanded,
            "objectives": self.objectives,
            "acts": acts(&self.acts),
            "minutes": self.minutes,
            "mandatory_minutes": self.mandatory_minutes,
            "rates": Act::ALL.iter().map(|a| (a.as_str().to_string(), serde_json::json!(self.rates.of(*a)))).collect::<serde_json::Map<_, _>>(),
            "target_minutes": self.target_minutes,
        })
    }
}
