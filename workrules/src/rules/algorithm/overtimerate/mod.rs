//! `RuleType::OvertimeRate` — eight concrete rules behind `OvertimeRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/overtimerate/`,
//! configs at `taps/src/java/com/unifocus/watson/common/labor/rules/algorithm/overtimerate/`.
//!
//! Ported third, after [`regularrate`](crate::rules::algorithm::regularrate)
//! and [`doubletimerate`](crate::rules::algorithm::doubletimerate) — see
//! "Scoping — the rate families" in `PARITY_AUDIT.md`. Adds one port
//! (`EarningTypePort`, already introduced by `regularrate`'s
//! `CombinationJobsRegRateRuleImpl`) beyond what `doubletimerate` needed.
//!
//! `OvertimeRateRuleImpl` is an empty abstract class in Java, folded into the
//! [`OvertimeRateRule`] trait directly, the same choice made for the other
//! two families ported so far.
//!
//! # Refuted: this family does not depend on `RegularRate` having run
//!
//! Of the eight concrete rules, only `FLSAOTRateRuleImpl` reads
//! `shift.getRegRate()` (a value a `RegularRate` rule would have written);
//! `WeightedOTRateRuleImpl` reads `distribution.getBaseRate()`, but that is
//! the same `execute` call chain's own value, not a separate-phase
//! dependency. See "Dependency order between the families" in the scoping
//! section.

pub mod commission_based_ot_rate;
pub mod config;
pub mod flsa_ot_rate;
pub mod flsa_weighted_ot_rate;
pub mod guaranteed_wage_ot_rate;
pub mod home_dept_ot_rate;
pub mod home_job_ot_rate;
pub mod job_ot_rate;
pub mod weighted_ot_rate;

use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;

/// An overtime-rate rule. `OvertimeRateRuleImpl extends RateRuleImpl`.
pub trait OvertimeRateRule {
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
/// `BaseOvertimeRateRuleImpl.setOvertimeRates`.
///
/// Java logs (does not throw or skip the write) when `rate == 0`; as with
/// [`doubletimerate::set_double_time_rates`](crate::rules::algorithm::doubletimerate::set_double_time_rates),
/// there is no logging framework in the rules tree, so the write happens
/// unconditionally.
pub fn set_overtime_rates(distribution: &mut HoursDistribution, rate: f64, rule_item: &RuleItem) {
    distribution.set_premium_rate(rate);
    distribution.set_rate_rule_item_id(Some(rule_item.id()));
}

/// Add a computed overtime top-up onto an earning's existing rate and re-zero
/// its stored dollars. Most (not all — `WeightedOTRateRuleImpl` and
/// `GuaranteedWageOTRateRuleImpl` do their own thing, see their modules) rules
/// in this family do `earning.setRate(roundCurrency(rate + earning.getRate()));
/// setDollars(0.0); calcAndSetTotalDollars();`.
pub fn add_overtime_rate(earning: &mut EmployeeEarning, rate: f64) {
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
    fn set_overtime_rates_writes_premium_rate_and_rule_item() {
        let mut distribution = HoursDistribution::new(1, LocalDate::of(2016, 6, 1), None, 0.0, 0.0);
        let rule_item = RI::new(9, 1, "OT", RuleClass::JobOrr, RuleParams::new());

        set_overtime_rates(&mut distribution, 12.0, &rule_item);

        assert_eq!(distribution.premium_rate(), 12.0);
        assert_eq!(distribution.rate_rule_item_id(), Some(9));
    }

    #[test]
    fn add_overtime_rate_adds_onto_the_existing_rate() {
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

        add_overtime_rate(&mut earning, 10.0);

        assert_eq!(earning.rate(), 15.0);
        assert_eq!(earning.dollars(), 0.0);
    }
}
