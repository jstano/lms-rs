//! `KBI.load()`'s `setRoomsFlags`/`setRevenueFlags`/`loadDaysOpen` sequence, and
//! `KBIReaderWriter.checkPeriodForDate`/`computePercentOfBaseKBI`'s `FNYPeriod` lookup, on top of
//! `KbiLoaderPort`/`FinancialYearPeriodPort`. The rest of `KBI.load()` (plain `TRecordset` column
//! reads: `id`/`propertyID`/`name`/`code`/`unitID`/`kbiConfigId`/`primary`/`kbiType`) is a
//! straight row-to-`KbiRecord` mapping, left to whatever loads the row in the first place (there is
//! no `TRecordset` equivalent in this crate) rather than ported as a separate function here.

use std::collections::HashMap;

use joda_rs::LocalDate;

use crate::engine::ForecasterError;
use crate::io::{FinancialYearPeriodPort, FnyPeriod, KbiLoaderPort};
use crate::{KbiId, PropertyId, StandardSetId};

/// `KBI.isRoomsKBI`/`isDeparturesKBI`/`isRevenueCenterKBI`/`daysOpen`, as computed by
/// `setRoomsFlags`/`setRevenueFlags`/`loadDaysOpen`.
#[derive(Debug, Clone, PartialEq)]
pub struct KbiFlags {
    pub is_rooms_kbi: bool,
    pub is_departures_kbi: bool,
    pub is_revenue_center_kbi: bool,
    pub days_open: HashMap<i32, bool>,
}

/// `KBI.load()`'s `setRoomsFlags(...)`; `if (!isRoomsKBI) setRevenueFlags(...)`.
pub fn load_kbi_flags(
    port: &dyn KbiLoaderPort,
    kbi_id: KbiId,
    property_id: PropertyId,
    standard_set_id: StandardSetId,
) -> Result<KbiFlags, ForecasterError> {
    let (is_rooms_kbi, is_departures_kbi) = port.rooms_and_departures_flags(kbi_id)?;

    let mut is_revenue_center_kbi = false;
    let mut days_open = HashMap::new();

    if !is_rooms_kbi {
        if let Some(period_id) = port.revenue_center_period_id(kbi_id)? {
            is_revenue_center_kbi = true;
            days_open = port.days_open(property_id, period_id, standard_set_id)?;
        }
    }
    // (kept as nested `if`s rather than `&&`-chained, matching Java's separate
    // `setRoomsFlags`/`setRevenueFlags` call sequence in `KBI.load()`)

    Ok(KbiFlags {
        is_rooms_kbi,
        is_departures_kbi,
        is_revenue_center_kbi,
        days_open,
    })
}

/// `KBIReaderWriter.checkPeriodForDate(TDate, KBI)` — `InvalidDateException` when no financial
/// period covers `date`. Returns the period so callers needing it (e.g. `find_period_for_date`
/// below) don't have to re-fetch it.
pub fn check_period_for_date(port: &dyn FinancialYearPeriodPort, date: LocalDate, kbi_code: &str) -> Result<FnyPeriod, ForecasterError> {
    port.fny_period(date)?.ok_or_else(|| ForecasterError::InvalidDate {
        message: format!("Missing financial periods for date:{date} and code: {kbi_code}"),
    })
}

/// `KBIReaderWriter.computePercentOfBaseKBI`'s inline period-lookup: finds the 0-based period
/// index containing `date`, iterating only up to `financial_year_period_count` periods (matching
/// Java's `for (p = 0; p < numPeriods; p++)` bound rather than `FnyPeriod`'s full period array).
/// Returns `None` when there's no `FNYPeriod` for `date` at all (Java returns `0d` from the whole
/// method in that case) or the date falls in none of the first `numPeriods` periods (Java falls
/// through the loop with `periodNo` left at its initial `0`, which the caller then uses as-is —
/// so `None` here should map to period `0`, not to skipping the KBI, at the call site).
pub fn find_period_for_date(port: &dyn FinancialYearPeriodPort, date: LocalDate) -> Result<Option<usize>, ForecasterError> {
    let Some(period) = port.fny_period(date)? else {
        return Ok(None);
    };

    let num_periods = port.financial_year_period_count()? as usize;
    Ok(period
        .periods
        .iter()
        .take(num_periods)
        .position(|p| matches!(p, Some((start, end)) if date >= *start && date <= *end)))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeLoader {
        rooms: (bool, bool),
        revenue_period_id: Option<i32>,
        days_open: HashMap<i32, bool>,
    }

    impl KbiLoaderPort for FakeLoader {
        fn rooms_and_departures_flags(&self, _kbi_id: KbiId) -> Result<(bool, bool), ForecasterError> {
            Ok(self.rooms)
        }

        fn revenue_center_period_id(&self, _kbi_id: KbiId) -> Result<Option<i32>, ForecasterError> {
            Ok(self.revenue_period_id)
        }

        fn days_open(&self, _property_id: PropertyId, _period_id: i32, _standard_set_id: StandardSetId) -> Result<HashMap<i32, bool>, ForecasterError> {
            Ok(self.days_open.clone())
        }

        fn load_kbi_percents(
            &self,
            _kbi_id: KbiId,
            _kbi_config_id: crate::KbiConfigId,
            _kbi_mode_code: &str,
        ) -> Result<Vec<crate::analyzers::KbiPercent>, ForecasterError> {
            Ok(vec![])
        }
    }

    #[test]
    fn rooms_kbi_skips_the_revenue_center_lookup_entirely() {
        let loader = FakeLoader {
            rooms: (true, false),
            revenue_period_id: Some(99),
            days_open: HashMap::new(),
        };
        let flags = load_kbi_flags(&loader, KbiId(1), PropertyId(1), StandardSetId(1)).unwrap();
        assert!(flags.is_rooms_kbi);
        assert!(!flags.is_revenue_center_kbi);
        assert!(flags.days_open.is_empty());
    }

    #[test]
    fn non_rooms_kbi_with_a_revenue_period_loads_days_open() {
        let mut days_open = HashMap::new();
        days_open.insert(1, false);
        let loader = FakeLoader {
            rooms: (false, false),
            revenue_period_id: Some(7),
            days_open: days_open.clone(),
        };
        let flags = load_kbi_flags(&loader, KbiId(1), PropertyId(1), StandardSetId(1)).unwrap();
        assert!(!flags.is_rooms_kbi);
        assert!(flags.is_revenue_center_kbi);
        assert_eq!(flags.days_open, days_open);
    }

    #[test]
    fn non_rooms_non_revenue_kbi_is_dangling() {
        let loader = FakeLoader {
            rooms: (false, false),
            revenue_period_id: None,
            days_open: HashMap::new(),
        };
        let flags = load_kbi_flags(&loader, KbiId(1), PropertyId(1), StandardSetId(1)).unwrap();
        assert!(!flags.is_rooms_kbi);
        assert!(!flags.is_revenue_center_kbi);
    }

    struct FakeFny {
        period: Option<FnyPeriod>,
        period_count: i32,
    }

    impl FinancialYearPeriodPort for FakeFny {
        fn fny_period(&self, _date: LocalDate) -> Result<Option<FnyPeriod>, ForecasterError> {
            Ok(self.period.clone())
        }

        fn financial_year_period_count(&self) -> Result<i32, ForecasterError> {
            Ok(self.period_count)
        }
    }

    #[test]
    fn check_period_for_date_errors_when_no_period_covers_the_date() {
        let port = FakeFny { period: None, period_count: 12 };
        let err = check_period_for_date(&port, LocalDate::of(2026, 1, 1), "RMS").unwrap_err();
        assert!(matches!(err, ForecasterError::InvalidDate { .. }));
    }

    #[test]
    fn find_period_for_date_returns_none_when_no_fny_period_exists() {
        let port = FakeFny { period: None, period_count: 12 };
        assert_eq!(find_period_for_date(&port, LocalDate::of(2026, 1, 1)).unwrap(), None);
    }

    #[test]
    fn find_period_for_date_finds_the_matching_index() {
        let port = FakeFny {
            period: Some(FnyPeriod {
                year: 2026,
                periods: vec![
                    Some((LocalDate::of(2026, 1, 1), LocalDate::of(2026, 1, 31))),
                    Some((LocalDate::of(2026, 2, 1), LocalDate::of(2026, 2, 28))),
                ],
            }),
            period_count: 2,
        };
        assert_eq!(find_period_for_date(&port, LocalDate::of(2026, 2, 15)).unwrap(), Some(1));
    }

    #[test]
    fn find_period_for_date_respects_the_numperiods_bound() {
        let port = FakeFny {
            period: Some(FnyPeriod {
                year: 2026,
                periods: vec![
                    Some((LocalDate::of(2026, 1, 1), LocalDate::of(2026, 1, 31))),
                    Some((LocalDate::of(2026, 2, 1), LocalDate::of(2026, 2, 28))),
                ],
            }),
            period_count: 1,
        };
        assert_eq!(find_period_for_date(&port, LocalDate::of(2026, 2, 15)).unwrap(), None);
    }
}
