//! Port of the Java KBI (Key Business Indicator) forecasting engine
//! (`taps/forecaster/.../server/labor/forecaster/engine/`).
//!
//! See `DATA_MODEL.md` for the id strategy, enum translations, and module layout, and
//! `PARITY_AUDIT.md` for the running port log.

pub mod analyzers;
pub mod engine;
pub mod formula;
pub mod io;
pub mod kbi;

/// `KBI.id` — legacy Hibernate `int` primary key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KbiId(pub i32);

/// `KBI.propertyID` / `Property.getId()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PropertyId(pub i32);

/// `KBI.unitID`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnitId(pub i32);

/// `KBI.kbiConfigId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KbiConfigId(pub i32);

/// Standard-set id used by `getKbiSetIdForStandardSetId` / `ForecastThread`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StandardSetId(pub i32);
