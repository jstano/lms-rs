//! Phase 3 — the Reader/Writer I/O boundary, modeled as narrow port traits
//! (`KbiLoaderPort`, `KbiStatReaderPort`, `KbiStatWriterPort`, `FinancialYearPeriodPort`),
//! following `scheduler::engine::io::ports`. See `../DATA_MODEL.md` and `../PARITY_AUDIT.md`
//! Phase 3 for why `RevenueCenterConfigPort`/`MarketSegmentConfigPort` collapsed into
//! `KbiLoaderPort` and `CalendarPlanPort` stayed deferred to Phase 4.

mod financial_year_period;
mod forecast_mode;
mod kbi_stat_data;
mod loader;
mod ports;
mod reader_writer;

pub use financial_year_period::FnyPeriod;
pub use forecast_mode::ForecastMode;
pub use kbi_stat_data::KbiStatData;
pub use loader::{check_period_for_date, find_period_for_date, load_kbi_flags, KbiFlags};
pub use ports::{FinancialYearPeriodPort, KbiLoaderPort, KbiStatReaderPort, KbiStatWriterPort};
pub use reader_writer::KbiReaderWriter;
