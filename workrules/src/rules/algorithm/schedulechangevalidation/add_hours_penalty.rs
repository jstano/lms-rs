//! Port of `AddHoursPenaltyRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulechangevalidation/AddHoursPenaltyRuleImpl.java`.
//!
//! A penalty when hours are **added**: the updated shift is longer than the
//! original (an added shift counts as all new hours), by at least
//! `minAdjustmentThreshold` minutes, inside the length and notice windows.

use crate::entity::earning_request::EarningRequest;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::rules::algorithm::schedulechangevalidation::config::{
    AddHoursPenaltyRuleConfig, MIN_ADJUSTMENT_THRESHOLD,
};
use crate::rules::algorithm::schedulechangevalidation::{
    PenaltySupport, ScheduleChangeValidationRule,
};
use crate::rules::params::RuleParams;
use crate::rules::ports::{CurrentDateTimePort, EarningTypePort};
use crate::rules::rule_config::RuleConfig;

/// `AddHoursPenaltyRuleImpl`.
pub struct AddHoursPenaltyRule<T: EarningTypePort, C: CurrentDateTimePort> {
    support: PenaltySupport<T, C>,
}

impl<T: EarningTypePort, C: CurrentDateTimePort> AddHoursPenaltyRule<T, C> {
    /// Build over the earning-type lookup and the property clock.
    pub fn new(earning_types: T, clock: C) -> Self {
        Self {
            support: PenaltySupport::new(earning_types, clock),
        }
    }
}

impl<T: EarningTypePort, C: CurrentDateTimePort> ScheduleChangeValidationRule
    for AddHoursPenaltyRule<T, C>
{
    fn execute(
        &self,
        original: Option<&EmployeeShift>,
        updated: Option<&EmployeeShift>,
        _rule_item: &RuleItem,
        params: &RuleParams,
        penalty_description: Option<&str>,
    ) -> Option<EarningRequest> {
        let params = params.fixed(&AddHoursPenaltyRuleConfig.default_values());
        let change = PenaltySupport::<T, C>::hours_change(original, updated);
        let added_enough = PenaltySupport::<T, C>::net_hours(original)
            < PenaltySupport::<T, C>::net_hours(updated)
            && change * 60.0 >= f64::from(params.int_at(MIN_ADJUSTMENT_THRESHOLD));
        if PenaltySupport::<T, C>::length_falls_in_threshold(original, updated, &params)
            && self
                .support
                .within_notice_threshold(original, updated, &params)
            && added_enough
        {
            return self.support.create_add_remove_request(
                original,
                updated,
                &params,
                change,
                penalty_description,
            );
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::uom::UOM;
    use crate::rule_params;
    use crate::rules::algorithm::schedulechangevalidation::config::{
        EARNING_AMOUNT, EARNING_TYPE, PERCENTAGE_OF_HOURS,
    };
    use crate::rules::algorithm::schedulechangevalidation::fixtures::*;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDateTime;

    fn blank() -> RuleItem {
        RuleItem::new(
            1,
            1,
            "x",
            RuleClass::AddHoursSchedValidation,
            Default::default(),
        )
    }
    fn run(
        now: LocalDateTime,
        original: Option<&EmployeeShift>,
        updated: Option<&EmployeeShift>,
        params: RuleParams,
    ) -> Option<EarningRequest> {
        AddHoursPenaltyRule::new(Types(UOM::Hours), Now(now)).execute(
            original,
            updated,
            &blank(),
            &params,
            Some("temp"),
        )
    }
    fn dec_30() -> LocalDateTime {
        LocalDateTime::of(2020, 12, 30, 0, 0, 0)
    }

    // ---- java_parity_tests: AddHoursPenaltyRuleImplTest.groovy ----

    #[test]
    fn returns_an_earning_request_if_the_adjustment_is_greater_than_the_threshold_otherwise_null() {
        let (original, updated) = (shift(6.0, (9, 0), (15, 0)), shift(8.0, (9, 0), (17, 0)));
        for (threshold, want) in [("0", Some(expected(1.0))), ("121", None)] {
            let params =
                rule_params! { MIN_ADJUSTMENT_THRESHOLD => threshold, EARNING_TYPE => "21" };
            assert_eq!(
                run(dec_30(), Some(&original), Some(&updated), params),
                want,
                "threshold {threshold}"
            );
        }
    }

    #[test]
    fn returns_an_earning_request_between_the_first_and_second_threshold_and_null_if_not() {
        let (original, updated) = (shift(6.0, (9, 0), (15, 0)), shift(8.0, (9, 0), (17, 0)));
        for (days_before, want) in [(5, Some(expected(1.0))), (0, None), (11, None)] {
            let now = LocalDateTime::of(2021, 1, 1, 0, 0, 0).minus_days(days_before);
            let params = rule_params! { EARNING_TYPE => "21" };
            assert_eq!(
                run(now, Some(&original), Some(&updated), params),
                want,
                "{days_before} days before"
            );
        }
    }

    #[test]
    fn returns_an_earning_request_with_the_accurate_amount_when_using_percentage_of_hours() {
        let (original, updated) = (shift(4.0, (9, 0), (13, 0)), shift(8.0, (9, 0), (17, 0)));
        let params = rule_params! { PERCENTAGE_OF_HOURS => "true", EARNING_AMOUNT => "50.0" };
        let result = run(dec_30(), Some(&original), Some(&updated), params).unwrap();
        assert_eq!(result.hours, 2.0);
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn an_added_shift_counts_all_its_hours_as_new() {
        let updated = shift(8.0, (9, 0), (17, 0));
        let params = rule_params! { EARNING_TYPE => "21" };
        assert!(run(dec_30(), None, Some(&updated), params).is_some());
    }

    #[test]
    fn a_shorter_or_unchanged_shift_is_not_penalised_as_an_addition() {
        let (long, short) = (shift(8.0, (9, 0), (17, 0)), shift(6.0, (9, 0), (15, 0)));
        let params = rule_params! { EARNING_TYPE => "21" };
        assert!(run(dec_30(), Some(&long), Some(&short), params.clone()).is_none());
        assert!(run(dec_30(), Some(&long), Some(&long), params).is_none());
    }

    #[test]
    fn a_shift_outside_the_length_window_is_ignored() {
        let (original, updated) = (shift(1.0, (9, 0), (10, 0)), shift(3.0, (9, 0), (12, 0)));
        let params = rule_params! { EARNING_TYPE => "21" };
        assert!(run(dec_30(), Some(&original), Some(&updated), params).is_none());
    }

    #[test]
    fn a_dollars_earning_type_puts_the_amount_in_cost_not_hours() {
        let (original, updated) = (shift(6.0, (9, 0), (15, 0)), shift(8.0, (9, 0), (17, 0)));
        let rule = AddHoursPenaltyRule::new(Types(UOM::Dollars), Now(dec_30()));
        let request = rule
            .execute(
                Some(&original),
                Some(&updated),
                &blank(),
                &rule_params! { EARNING_TYPE => "21", EARNING_AMOUNT => "25" },
                None,
            )
            .unwrap();
        assert_eq!((request.cost, request.hours), (25.0, 0.0));
        // No description given: the earning type's name is the note.
        assert_eq!(request.note, "Penalty");
    }
}
