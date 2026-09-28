//! Phase 2 — statistical analyzers (`RStats`, `RegressionAnalyzer`, `TAESCalculator`/
//! `TAESAnalyzer`, `PastAverageAnalyzer`, `PercentOfBaseKBIAnalyzer`,
//! `StatisticalKBIAnalyzer`). See `../DATA_MODEL.md`.

mod past_average;
mod percent_of_base;
mod regression;
mod row_data;
mod rstats;
mod statistical_kbi_analyzer;
mod stats;
mod taes;

pub use past_average::{PastAverageAnalyzer, PastAverageLookupPort};
pub use percent_of_base::{KbiPercent, PercentOfBaseKbiAnalyzer, PercentOfBaseLookupPort};
pub use regression::{KbiStatSnapshot, RegressionAnalyzer, RegressionKbi, RegressionLookupPort};
pub use row_data::{DateEnvIdPair, RowData, MAXCOL, MAXSAM};
pub use rstats::RStats;
pub use statistical_kbi_analyzer::{StatisticalDispatch, StatisticalDispatchResult, StatisticalKbiAnalyzer};
pub use stats::Stats;
pub use taes::{TaesAnalyzer, TaesCalculator, TaesLookupPort, TaesResult};
