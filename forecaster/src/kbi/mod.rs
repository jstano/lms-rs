//! Phase 2 — the `Kbi` domain enum (`Input`/`Calculated`/`Statistical`/`PercentOfBase`) and
//! supporting enums (`KbiType`, `KbiMode`, `StatOpType`, `KbiStatType`). See `../DATA_MODEL.md`.
//!
//! `ForecastMode` is deliberately not ported here — `DATA_MODEL.md` §2 scopes it to the Phase 4
//! orchestrator (`ForecastThread`'s mode dispatch), not the KBI domain model.

mod domain;
mod kbi_mode;
mod kbi_type;
mod stat_kbi;
mod stat_op_type;
mod stat_type;

pub use domain::{CalculatedKbiData, Kbi, KbiRecord, PercentOfBaseKbiData, StatisticalKbiData};
pub use kbi_mode::KbiMode;
pub use kbi_type::KbiType;
pub use stat_kbi::{StatKbi, StatKbiType, StatRelatedKbi};
pub use stat_op_type::StatOpType;
pub use stat_type::KbiStatType;
