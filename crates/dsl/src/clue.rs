//! **The clue a step's answer rests on** (spec-0143).
//!
//! A step of a puzzle is an act the party performs — an objective, or an
//! environment trigger — whose answer the world does not give away where the
//! act stands: which of four fires is lit first, which lever is the right
//! one. The creator states what tells the player the answer, as a `clues`
//! list on the act, and the engine confirms one thing only: that on the main
//! line every stated clue has been **presented** before the party reaches the
//! step (`DW0863`, `delvec`'s `compiler::clue`). Whether a player can reason
//! from the clue to the answer is judged in play, never by the engine.
//!
//! A clue is a text in the journal or a thing seen on the way:
//!
//! | `by` | What presents it |
//! |---|---|
//! | `hint` | the `hint` of an objective, on its open journal page |
//! | `record` | the `record` of an objective that is over, on its finished journal page |
//! | `sight` | a place the party sees before it reaches the step |
//!
//! An act with no `clues` states no clue and is owed nothing.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{AnchorId, ObjectiveId};

/// One clue an act's answer rests on (spec-0143 §2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "by", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Clue {
    /// **A text clue: an objective's `hint`**, read on its open journal page
    /// from the moment that objective starts. The objective must carry a
    /// `hint` (`DW1136`).
    Hint {
        /// The objective whose hint holds the clue.
        objective: ObjectiveId,
    },
    /// **A text clue: an objective's `record`**, read on its finished journal
    /// page once that objective is over (spec-0137) — a note the party read,
    /// what a talk told it. The objective must carry a `record` (`DW1136`).
    Record {
        /// The objective whose record holds the clue.
        objective: ObjectiveId,
    },
    /// **A visual clue: a place the party sees** on the way to the step — a
    /// mural, a row of statues, the thing itself. Seen is the one sight rule's
    /// answer from a cell the main line stands on before the step.
    Sight {
        /// The anchor of the thing seen; its cell is what the eye must reach.
        of: AnchorId,
    },
}

impl Clue {
    /// The `by` word, for a message.
    pub fn by(&self) -> &'static str {
        match self {
            Clue::Hint { .. } => "hint",
            Clue::Record { .. } => "record",
            Clue::Sight { .. } => "sight",
        }
    }

    /// The clue in the words a message names it by.
    pub fn named(&self) -> String {
        match self {
            Clue::Hint { objective } => format!("the hint of `{}`", objective.as_str()),
            Clue::Record { objective } => format!("the record of `{}`", objective.as_str()),
            Clue::Sight { of } => format!("the sight of `{}`", of.as_str()),
        }
    }
}
