//! Phase 4 — the orchestrator (`run_forecast`, replacing `ForecastThread`) and
//! `ForecasterError`. See `../DATA_MODEL.md`.

pub mod compute;
mod error;
pub mod kbi_list;
mod orchestrator;
pub mod ports;

pub use compute::{compute_kbi_value, java_day_of_week, ComputeContext};
pub use error::ForecasterError;
pub use kbi_list::KbiList;
pub use orchestrator::{run_forecast, ForecastOutcome, ForecastParams, ForecastPorts};
pub use ports::{CalendarPlan, CalendarPlanDateRange, KbiSetPort, MarketSegmentCheckPort, MarketSegmentRow, RevenueCenterCheckPort, RevenueCenterKbiRow};
