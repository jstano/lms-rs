//! Port of `EarningFixedRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/earningrate/EarningFixedRateRuleImpl.java`.
//!
//! The simplest rule in the whole wave: set the earning's rate to one
//! configured constant.
//!
//! Ported cases: `EarningFixedRateRuleImplTest.groovy` (one case).

use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::earningrate::config::{EarningFixedRateRuleConfig, RATE};
use crate::rules::algorithm::earningrate::{EarningRateRule, set_earning_rate};
use crate::rules::params::RuleParams;
use crate::rules::rule_config::RuleConfig;

/// `EarningFixedRateRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct EarningFixedRateRule;

impl EarningRateRule for EarningFixedRateRule {
    fn execute(
        &mut self,
        _dataset: &dyn TimeCard,
        earning: &mut EmployeeEarning,
        params: &RuleParams,
    ) {
        let params = params.fixed(&EarningFixedRateRuleConfig.default_values());
        let rate = params.double_at(RATE);

        set_earning_rate(earning, rate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use joda_rs::LocalDate;

    #[test]
    fn the_configured_rate_is_set_on_the_earning() {
        let dataset = TimeCardData::new();
        let mut earning = EmployeeEarning::new(
            1,
            100,
            1,
            5,
            LocalDate::of(2016, 6, 1),
            2.0,
            0.0,
            EarningSource::Rule,
        );
        let params = rule_params! { RATE => "4.0" };

        EarningFixedRateRule.execute(&dataset, &mut earning, &params);

        assert_eq!(earning.rate(), 4.0);
        assert_eq!(earning.total_dollars(), 8.0);
    }

    mod java_parity_tests {
        use super::*;

        /// `EarningFixedRateRuleImplTest`: "configured rate should be set on
        /// an earning".
        #[test]
        fn configured_rate_should_be_set_on_an_earning() {
            let dataset = TimeCardData::new();
            let mut earning = EmployeeEarning::new(
                1,
                44,
                1,
                1,
                LocalDate::of(2016, 6, 1),
                2.0,
                0.0,
                EarningSource::Rule,
            );
            let params = rule_params! { RATE => "4.0" };

            EarningFixedRateRule.execute(&dataset, &mut earning, &params);

            assert_eq!(earning.rate(), 4.0);
        }
    }
}
