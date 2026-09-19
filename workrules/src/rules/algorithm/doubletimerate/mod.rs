//! `RuleType::DoubleTimeRate` — five concrete rules behind `DoubleTimeRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/doubletimerate/`,
//! configs at `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/doubletimerate/`.
//!
//! The smallest of the four rate families — see "Scoping — the rate families"
//! in `PARITY_AUDIT.md`. Ported second, after [`regularrate`](crate::rules::algorithm::regularrate),
//! per the recommended order: it reuses every port `regularrate` already
//! needed (`AssignmentPort`, `MinWagePort`) plus `PropertyPort` and
//! `TimeCard::flsa_data_map`, and adds nothing new to the entity graph.
//!
//! `DoubleTimeRateRuleImpl` itself is an empty abstract class in Java
//! (`extends RateRuleImpl`, no members) — folded into the
//! [`DoubleTimeRateRule`] trait directly rather than kept as a separate
//! marker, the same choice `regularrate` made for `RegularRateRuleImpl`.

pub mod commission_based_dt_rate;
pub mod config;
pub mod flsa_dt_rate;
pub mod home_dept_dt_rate;
pub mod home_job_dt_rate;
pub mod job_dt_rate;

use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;

/// A double-time-rate rule. `DoubleTimeRateRuleImpl extends RateRuleImpl`.
pub trait DoubleTimeRateRule {
    /// Price a shift's hours distribution. `execute(EmployeeShift,
    /// HoursDistribution, TimeCard, RuleItem)`.
    fn execute_for_shift(
        &self,
        shift: &EmployeeShift,
        distribution: &mut HoursDistribution,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    );

    /// Price a standalone earning. `execute(EmployeeEarning, TimeCard,
    /// RuleItem)`.
    fn execute_for_earning(
        &self,
        earning: &mut EmployeeEarning,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    );
}

/// Write a distribution's premium rate and the rule item that set it.
/// `BaseDoubleTimeRateRuleImpl.setDoubleTimeRates`.
///
/// Java logs (does not throw or skip the write) when `rate == 0`; this crate
/// has no logging framework in the rules tree, so the write happens
/// unconditionally and the zero case is silent, matching every observable
/// effect Java has.
pub fn set_double_time_rates(
    distribution: &mut HoursDistribution,
    rate: f64,
    rule_item: &RuleItem,
) {
    distribution.set_premium_rate(rate);
    distribution.set_rate_rule_item_id(Some(rule_item.id()));
}

/// Add a computed double-time top-up onto an earning's existing rate and
/// re-zero its stored dollars. Every concrete rule in this family does
/// `earning.setRate(roundCurrency(rate + earning.getRate())); setDollars(0.0);
/// calcAndSetTotalDollars();` — the addition (not an overwrite) is what
/// distinguishes this from `regularrate::set_earning_rate`.
pub fn add_double_time_rate(earning: &mut EmployeeEarning, rate: f64) {
    use crate::common::numbers::round_currency;
    earning.set_rate(round_currency(rate + earning.rate()));
    earning.set_dollars(0.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::entity::rule_item::RuleItem as RI;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    #[test]
    fn set_double_time_rates_writes_premium_rate_and_rule_item() {
        let mut distribution = HoursDistribution::new(1, LocalDate::of(2016, 6, 1), None, 0.0, 0.0);
        let rule_item = RI::new(9, 1, "DT", RuleClass::JobDrr, RuleParams::new());

        set_double_time_rates(&mut distribution, 12.0, &rule_item);

        assert_eq!(distribution.premium_rate(), 12.0);
        assert_eq!(distribution.rate_rule_item_id(), Some(9));
    }

    #[test]
    fn set_double_time_rates_still_writes_a_zero_rate() {
        let mut distribution = HoursDistribution::new(1, LocalDate::of(2016, 6, 1), None, 0.0, 0.0);
        let rule_item = RI::new(9, 1, "DT", RuleClass::JobDrr, RuleParams::new());

        set_double_time_rates(&mut distribution, 0.0, &rule_item);

        assert_eq!(distribution.premium_rate(), 0.0);
        assert_eq!(distribution.rate_rule_item_id(), Some(9));
    }

    #[test]
    fn add_double_time_rate_adds_onto_the_existing_rate() {
        let mut earning = EmployeeEarning::new(
            1,
            100,
            1,
            5,
            LocalDate::of(2016, 6, 1),
            4.0,
            0.0,
            EarningSource::Rule,
        );
        earning.set_rate(5.0);

        add_double_time_rate(&mut earning, 10.0);

        assert_eq!(earning.rate(), 15.0);
        assert_eq!(earning.dollars(), 0.0);
    }
}
