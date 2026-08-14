//! The prefab metadata document, as this crate's tools see it.
//!
//! The shape is **not** defined here. It is [`delvewright_dsl::prefab`], and
//! this module is a re-export so that the crate that writes the `.nbt` half of a
//! prefab names the `.json` half by the same path it always did.
//!
//! The definition sits in the DSL crate for one reason: `delvec` is published to
//! crates.io and may only depend on published crates, so that is the only crate
//! every reader of this document can reach. Anywhere else, the compiler would
//! need a copy — which is what it had.

pub use delvewright_dsl::prefab::{
    Anchor, AnchorEdit, Connector, ContractBar, ContractEdge, ContractFace, ContractNoBody,
    ContractSpace, ContractVolume, GeneratedBy, License, PrefabMeta, Region, SpatialContract,
    StructureMeta, UNMEASURED,
};

/// The `lighting` block, which the DSL owns outright: it is the same type the
/// compiler validates a campaign's lighting claims with, so a probe result that
/// this crate's tools write is refused here rather than three tools later.
pub use delvewright_dsl::registry::{Lighting, LightingProfile};
