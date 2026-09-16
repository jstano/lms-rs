//! Port of `com.unifocus.watson.server.labor.calcshift.CalcDataSetStat`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/calcshift/CalcDataSetStat.java`.
//!
//! The five things a rule may leave on the time card's stat map for a later
//! rule — or a later run — to read back. Half the key; the other half is a
//! [`LocalDate`](joda_rs::LocalDate), which for the three `*_BY_PERIOD_START_DATE`
//! entries is the pay period's start and for the two consecutive-day entries is
//! the dataset start date.
//!
//! A plain `enum` in Java too: no code, no resource key, so it is not a
//! [`coded_enum`](crate::common::coded_enum) and carries no `from_code`.
//!
//! Only `ContractOTHrsRuleImpl` writes the first three;
//! `ConsecutiveDaysCalculator` writes the last two.

/// A statistic a rule caches on the time card. `CalcDataSetStat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CalcDataSetStat {
    /// How far short of the contracted hours the period fell.
    GapToContractByPeriodStartDate,
    /// The period's total distributed hours.
    PeriodTotalHoursByPeriodStartDate,
    /// The period's total earning hours.
    PeriodTotalEarningHoursByPeriodStartDate,
    /// Consecutive days worked counting back from the dataset start date.
    ConsecutiveDaysFromDatasetStartDate,
    /// The same count, with earnings counted as worked days.
    ConsecutiveDaysFromDatasetStartDateIncludingEarnings,
}

#[cfg(test)]
mod tests {
    use super::*;
    use joda_rs::LocalDate;
    use std::collections::HashMap;

    #[test]
    fn a_stat_keys_a_map_together_with_a_date() {
        let mut stats: HashMap<(CalcDataSetStat, LocalDate), f64> = HashMap::new();
        let period_start = LocalDate::of(2010, 1, 3);

        stats.insert(
            (
                CalcDataSetStat::GapToContractByPeriodStartDate,
                period_start,
            ),
            2.5,
        );

        assert_eq!(
            stats.get(&(
                CalcDataSetStat::GapToContractByPeriodStartDate,
                period_start
            )),
            Some(&2.5)
        );
        assert_eq!(
            stats.get(&(
                CalcDataSetStat::GapToContractByPeriodStartDate,
                LocalDate::of(2010, 1, 10)
            )),
            None,
            "the date is part of the key"
        );
    }
}
