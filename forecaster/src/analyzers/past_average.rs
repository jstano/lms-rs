//! `PastAverageAnalyzer.java` — averages a KBI's actual values over up to `numberDataPoints`
//! historical weeks (or days, for DOW-ignoring environments), choosing which weeks count based on
//! season/environment matching.
//!
//! Every lookup (`isGlobalEnvironment`/`findEnvironmentID`/`hasEnvironmentID`/
//! `ignoreEnvironmentDOW`/`findSeasonID`/`readKBIValue`/`Property.getDaysOut()`) is DB-backed in
//! Java (`LaborKBIReaderWriter`) — collapsed into `PastAverageLookupPort` per `DATA_MODEL.md` §3's
//! "narrow port trait, typed parameters" treatment. The control flow itself (which weeks to skip,
//! how far back to walk, the running total/count) is ported verbatim below and is what's under
//! test here; the port is exercised through an in-memory fake.

use joda_rs::LocalDate;

use crate::engine::ForecasterError;
use crate::KbiId;

/// `PastAverageAnalyzer.MAX_WEEKS`.
const MAX_WEEKS: i32 = 106;

/// See the module doc comment. `date` throughout is always a `statDate` the analyzer itself
/// derived by walking backwards from the requested date — never the requested date itself.
pub trait PastAverageLookupPort {
    /// `Property.getDaysOut()`.
    fn days_out(&self) -> Result<i32, ForecasterError>;

    /// `LaborKBIReaderWriter.isGlobalEnvironment(date)`.
    fn is_global_environment(&self, date: LocalDate) -> Result<bool, ForecasterError>;

    /// `LaborKBIReaderWriter.findEnvironmentID(kbiID, date, actualEnv)`.
    fn find_environment_id(
        &self,
        kbi_id: KbiId,
        date: LocalDate,
        actual_env: bool,
    ) -> Result<i32, ForecasterError>;

    /// `LaborKBIReaderWriter.hasEnvironmentID(kbiID, date, environmentID)`.
    fn has_environment_id(
        &self,
        kbi_id: KbiId,
        date: LocalDate,
        environment_id: i32,
    ) -> Result<bool, ForecasterError>;

    /// `LaborKBIReaderWriter.ignoreEnvironmentDOW(environmentID)`.
    fn ignore_environment_dow(&self, environment_id: i32) -> Result<bool, ForecasterError>;

    /// `LaborKBIReaderWriter.findSeasonID(date)`.
    fn find_season_id(&self, date: LocalDate) -> Result<i32, ForecasterError>;

    /// `PastAverageAnalyzer.readKBIValue` — Java's `LaborKBIReaderWriter` branch: override value,
    /// falling back to adjusted, then estimated (actual is read first but only `LaborKBIReaderWriter`
    /// is ever the concrete type per the plan, so the non-Labor `else` branch is not modeled).
    fn read_kbi_value(&self, kbi_id: KbiId, date: LocalDate) -> Result<Option<f64>, ForecasterError>;
}

pub struct PastAverageAnalyzer;

impl PastAverageAnalyzer {
    /// `calculate(TDate date)`.
    pub fn calculate(
        port: &dyn PastAverageLookupPort,
        kbi_id: KbiId,
        number_data_points: i32,
        date: LocalDate,
    ) -> Result<f64, ForecasterError> {
        let season_id = port.find_season_id(date)?;
        let environment_id = port.find_environment_id(kbi_id, date, false)?;

        if season_id != 0 {
            Self::calculate_seasonal(port, kbi_id, number_data_points, date, season_id, environment_id)
        } else if environment_id != 0 {
            Self::calculate_environmental(port, kbi_id, number_data_points, date, environment_id)
        } else {
            Self::calculate_normal(port, kbi_id, number_data_points, date)
        }
    }

    /// `calculateNormal`.
    fn calculate_normal(
        port: &dyn PastAverageLookupPort,
        kbi_id: KbiId,
        number_data_points: i32,
        date: LocalDate,
    ) -> Result<f64, ForecasterError> {
        let days_out = port.days_out()? + 7;

        let mut total = 0.0;
        let mut count = 0;
        let mut i = 1;
        let mut stat_date = date.minus_days(days_out as i64);

        while count < number_data_points && i < MAX_WEEKS {
            if port.is_global_environment(stat_date)? {
                i += 1;
                stat_date = stat_date.minus_days(7);
                continue;
            }
            if port.find_environment_id(kbi_id, stat_date, true)? != 0 {
                i += 1;
                stat_date = stat_date.minus_days(7);
                continue;
            }
            if port.find_season_id(stat_date)? != 0 {
                i += 1;
                stat_date = stat_date.minus_days(7);
                continue;
            }

            if let Some(value) = port.read_kbi_value(kbi_id, stat_date)? {
                total += value;
                count += 1;
            }

            i += 1;
            stat_date = stat_date.minus_days(7);
        }

        Ok(if total != 0.0 && count != 0 {
            total / count as f64
        } else {
            0.0
        })
    }

    /// `calculateEnvironmental`.
    fn calculate_environmental(
        port: &dyn PastAverageLookupPort,
        kbi_id: KbiId,
        number_data_points: i32,
        date: LocalDate,
        environment_id: i32,
    ) -> Result<f64, ForecasterError> {
        let days_out = port.days_out()? + 7;

        let mut total = 0.0;
        let mut count = 0;
        let mut i = 1;
        let days_back: i64 = if port.ignore_environment_dow(environment_id)? { -1 } else { -7 };
        let mut stat_date = date.minus_days(days_out as i64);

        while count < number_data_points && i < MAX_WEEKS {
            if !port.is_global_environment(stat_date)? && port.has_environment_id(kbi_id, stat_date, environment_id)? {
                if let Some(value) = port.read_kbi_value(kbi_id, stat_date)? {
                    total += value;
                    count += 1;
                }
            }

            i += 1;
            stat_date = Self::apply_days_back(stat_date, days_back);
        }

        Ok(if total != 0.0 && count != 0 {
            total / count as f64
        } else {
            0.0
        })
    }

    /// `calculateSeasonal`.
    fn calculate_seasonal(
        port: &dyn PastAverageLookupPort,
        kbi_id: KbiId,
        number_data_points: i32,
        date: LocalDate,
        season_id: i32,
        environment_id: i32,
    ) -> Result<f64, ForecasterError> {
        let days_out = port.days_out()? + 7;

        let mut total = 0.0;
        let mut count = 0;
        let mut i = 1;
        let days_back: i64 = if environment_id != 0 && port.ignore_environment_dow(environment_id)? {
            -1
        } else {
            -7
        };
        let mut stat_date = date.minus_days(days_out as i64);

        while count < number_data_points && i < MAX_WEEKS {
            if !port.is_global_environment(stat_date)?
                && port.find_season_id(stat_date)? == season_id
                && port.has_environment_id(kbi_id, stat_date, environment_id)?
            {
                if let Some(value) = port.read_kbi_value(kbi_id, stat_date)? {
                    total += value;
                    count += 1;
                }
            }

            i += 1;
            stat_date = Self::apply_days_back(stat_date, days_back);
        }

        Ok(if total != 0.0 && count != 0 {
            total / count as f64
        } else {
            0.0
        })
    }

    fn apply_days_back(date: LocalDate, days_back: i64) -> LocalDate {
        if days_back < 0 {
            date.minus_days(-days_back)
        } else {
            date.plus_days(days_back)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    struct FakePort {
        days_out: i32,
        global_environment_dates: Vec<LocalDate>,
        environment_by_date: HashMap<LocalDate, i32>,
        season_by_date: HashMap<LocalDate, i32>,
        ignore_dow_environments: Vec<i32>,
        values_by_date: HashMap<LocalDate, f64>,
        calls: RefCell<Vec<LocalDate>>,
    }

    impl FakePort {
        fn new(days_out: i32) -> Self {
            FakePort {
                days_out,
                global_environment_dates: Vec::new(),
                environment_by_date: HashMap::new(),
                season_by_date: HashMap::new(),
                ignore_dow_environments: Vec::new(),
                values_by_date: HashMap::new(),
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl PastAverageLookupPort for FakePort {
        fn days_out(&self) -> Result<i32, ForecasterError> {
            Ok(self.days_out)
        }

        fn is_global_environment(&self, date: LocalDate) -> Result<bool, ForecasterError> {
            Ok(self.global_environment_dates.contains(&date))
        }

        fn find_environment_id(&self, _kbi_id: KbiId, date: LocalDate, _actual_env: bool) -> Result<i32, ForecasterError> {
            Ok(*self.environment_by_date.get(&date).unwrap_or(&0))
        }

        fn has_environment_id(&self, kbi_id: KbiId, date: LocalDate, environment_id: i32) -> Result<bool, ForecasterError> {
            Ok(self.find_environment_id(kbi_id, date, true)? == environment_id)
        }

        fn ignore_environment_dow(&self, environment_id: i32) -> Result<bool, ForecasterError> {
            Ok(self.ignore_dow_environments.contains(&environment_id))
        }

        fn find_season_id(&self, date: LocalDate) -> Result<i32, ForecasterError> {
            Ok(*self.season_by_date.get(&date).unwrap_or(&0))
        }

        fn read_kbi_value(&self, _kbi_id: KbiId, date: LocalDate) -> Result<Option<f64>, ForecasterError> {
            self.calls.borrow_mut().push(date);
            Ok(self.values_by_date.get(&date).copied())
        }
    }

    #[test]
    fn calculate_normal_averages_the_requested_number_of_weeks() {
        let date = LocalDate::of(2026, 3, 1);
        let mut port = FakePort::new(0);
        // days_out + 7 = 7, so first stat_date = date - 7, then -7 each week.
        let mut stat_date = date.minus_days(7);
        for v in [10.0, 20.0, 30.0] {
            port.values_by_date.insert(stat_date, v);
            stat_date = stat_date.minus_days(7);
        }

        let result = PastAverageAnalyzer::calculate(&port, KbiId(1), 3, date).unwrap();
        assert_eq!(result, 20.0);
    }

    #[test]
    fn calculate_normal_skips_global_environment_weeks() {
        let date = LocalDate::of(2026, 3, 1);
        let mut port = FakePort::new(0);
        let first = date.minus_days(7);
        let second = first.minus_days(7);
        let third = second.minus_days(7);
        port.global_environment_dates.push(first);
        port.values_by_date.insert(second, 40.0);
        port.values_by_date.insert(third, 60.0);

        let result = PastAverageAnalyzer::calculate(&port, KbiId(1), 2, date).unwrap();
        assert_eq!(result, 50.0);
    }

    #[test]
    fn calculate_returns_zero_when_no_data_found() {
        let date = LocalDate::of(2026, 3, 1);
        let port = FakePort::new(0);
        let result = PastAverageAnalyzer::calculate(&port, KbiId(1), 3, date).unwrap();
        assert_eq!(result, 0.0);
    }

    #[test]
    fn calculate_dispatches_to_environmental_when_no_season_but_has_environment() {
        let date = LocalDate::of(2026, 3, 1);
        let mut port = FakePort::new(0);
        port.environment_by_date.insert(date, 5);
        let mut stat_date = date.minus_days(7);
        port.environment_by_date.insert(stat_date, 5);
        port.values_by_date.insert(stat_date, 15.0);
        stat_date = stat_date.minus_days(7);
        port.environment_by_date.insert(stat_date, 5);
        port.values_by_date.insert(stat_date, 25.0);

        let result = PastAverageAnalyzer::calculate(&port, KbiId(1), 2, date).unwrap();
        assert_eq!(result, 20.0);
    }

    #[test]
    fn calculate_environmental_steps_one_day_when_dow_is_ignored() {
        let date = LocalDate::of(2026, 3, 1);
        let mut port = FakePort::new(0);
        port.environment_by_date.insert(date, 9);
        port.ignore_dow_environments.push(9);
        let d1 = date.minus_days(7);
        let d2 = d1.minus_days(1);
        port.environment_by_date.insert(d1, 9);
        port.environment_by_date.insert(d2, 9);
        port.values_by_date.insert(d1, 1.0);
        port.values_by_date.insert(d2, 3.0);

        let result = PastAverageAnalyzer::calculate(&port, KbiId(1), 2, date).unwrap();
        assert_eq!(result, 2.0);
    }
}
