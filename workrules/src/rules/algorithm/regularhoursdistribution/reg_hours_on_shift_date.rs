//! Port of `RegularHoursOnShiftDateRuleImpl`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/regularhoursdistribution/RegularHoursOnShiftDateRuleImpl.java`.
//!
//! The trivial case: one distribution, dated the shift's own date, sized at
//! the shift's net hours. `REG_HOURS_RHD`, the family's default rule.
//!
//! # No real Groovy spec
//!
//! `RegularHoursOnShiftDateRuleImplTest.groovy` and
//! `RegularHoursOnShiftDateRuleConfigTest.groovy` exist, but under the
//! `hoursdistribution` test package rather than `regularhoursdistribution`'s
//! own — evidently misplaced — and every method body in both is empty. Same
//! situation as `hoursdistribution`'s `RegHrsOnlyRuleImpl`: nothing to
//! transcribe, so the tests below are written from the source alone.

use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::rules::algorithm::regularhoursdistribution::RegularHoursDistributionRule;
use crate::rules::algorithm::regularhoursdistribution::config::RegularHoursOnShiftDateRuleConfig;
use crate::rules::algorithm::utility::hours_distribution_factory::create_distribution_of_configured_type;
use crate::rules::rule_config::RuleConfig;

/// `RegularHoursOnShiftDateRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RegularHoursOnShiftDateRule;

impl RegularHoursDistributionRule for RegularHoursOnShiftDateRule {
    fn execute(&self, shift: &mut EmployeeShift, rule_item: &RuleItem) {
        let params = rule_item
            .params()
            .fixed(&RegularHoursOnShiftDateRuleConfig.default_values());

        let distribution = create_distribution_of_configured_type(
            shift.shift_date(),
            shift.property_id(),
            shift.net_hours(),
            &params,
            Some(rule_item.id()),
        );
        shift.add_hours_distribution(distribution);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::hours_distribution_type::HoursDistributionType;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    fn shift() -> EmployeeShift {
        EmployeeShift::new(
            1,
            100,
            200,
            LocalDate::of(2010, 1, 4),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_net_hours(8.0)
    }

    fn rule_item(params: RuleParams) -> RuleItem {
        RuleItem::new(
            7,
            1,
            "Regular hours on shift date",
            RuleClass::RegHoursRhd,
            params,
        )
    }

    #[test]
    fn one_distribution_dated_the_shift_and_sized_at_its_net_hours() {
        let mut shift = shift();

        RegularHoursOnShiftDateRule.execute(&mut shift, &rule_item(RuleParams::new()));

        assert_eq!(shift.hours_distributions().len(), 1);
        let distribution = &shift.hours_distributions()[0];
        assert_eq!(distribution.date(), LocalDate::of(2010, 1, 4));
        assert_eq!(distribution.hours(), 8.0);
        assert_eq!(distribution.original_hours(), 8.0);
        assert_eq!(
            distribution.hours_distribution_type_id(),
            Some(HoursDistributionType::REGULAR_ID)
        );
        assert_eq!(distribution.hours_rule_item_id(), Some(7));
    }

    #[test]
    fn a_configured_bucket_overrides_the_regular_default() {
        let mut shift = shift();
        let params = crate::rule_params! {
            crate::rules::algorithm::single_distribution_type_config::HOURS_DISTRIBUTION_TYPE_ID => "3"
        };

        RegularHoursOnShiftDateRule.execute(&mut shift, &rule_item(params));

        assert_eq!(
            shift.hours_distributions()[0].hours_distribution_type_id(),
            Some(3)
        );
    }
}
