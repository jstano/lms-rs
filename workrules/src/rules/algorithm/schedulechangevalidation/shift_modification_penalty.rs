//! Port of `ShiftModificationPenaltyRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/schedulechangevalidation/ShiftModificationPenaltyRuleImpl.java`.
//!
//! A penalty for **moving** a shift without changing its length: same stored net
//! hours (exactly), but a different shift date, start or end *time of day*, or
//! job. Needs both shifts — an added or removed one is the other two rules' business.

use crate::entity::earning_request::EarningRequest;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::rules::algorithm::schedulechangevalidation::config::ShiftModificationPenaltyRuleConfig;
use crate::rules::algorithm::schedulechangevalidation::{
    PenaltySupport, ScheduleChangeValidationRule,
};
use crate::rules::params::RuleParams;
use crate::rules::ports::{CurrentDateTimePort, EarningTypePort};
use crate::rules::rule_config::RuleConfig;

/// `ShiftModificationPenaltyRuleImpl`.
pub struct ShiftModificationPenaltyRule<T: EarningTypePort, C: CurrentDateTimePort> {
    support: PenaltySupport<T, C>,
}

impl<T: EarningTypePort, C: CurrentDateTimePort> ShiftModificationPenaltyRule<T, C> {
    /// Build over the earning-type lookup and the property clock.
    pub fn new(earning_types: T, clock: C) -> Self {
        Self {
            support: PenaltySupport::new(earning_types, clock),
        }
    }
}

/// `ruleValidationShiftChanged`.
fn changed(original: &EmployeeShift, updated: &EmployeeShift) -> bool {
    let time_of_day = |t: Option<joda_rs::LocalDateTime>| t.map(|t| t.to_local_time());
    original.shift_date() != updated.shift_date()
        || time_of_day(original.start_date_time()) != time_of_day(updated.start_date_time())
        || time_of_day(original.end_date_time()) != time_of_day(updated.end_date_time())
        || original.job_id() != updated.job_id()
}

impl<T: EarningTypePort, C: CurrentDateTimePort> ScheduleChangeValidationRule
    for ShiftModificationPenaltyRule<T, C>
{
    fn execute(
        &self,
        original: Option<&EmployeeShift>,
        updated: Option<&EmployeeShift>,
        _rule_item: &RuleItem,
        params: &RuleParams,
        penalty_description: Option<&str>,
    ) -> Option<EarningRequest> {
        let (original, updated) = (original?, updated?);
        let params = params.fixed(&ShiftModificationPenaltyRuleConfig.default_values());
        let duration_change = PenaltySupport::<T, C>::hours_change(Some(original), Some(updated));
        if PenaltySupport::<T, C>::length_falls_in_threshold(Some(original), Some(updated), &params)
            && self
                .support
                .within_notice_threshold(Some(original), Some(updated), &params)
            && duration_change == 0.0
            && changed(original, updated)
        {
            return self.support.create_request(
                Some(original),
                Some(updated),
                &params,
                penalty_description,
            );
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::common::enums::uom::UOM;
    use crate::rule_params;
    use crate::rules::algorithm::schedulechangevalidation::config::EARNING_TYPE;
    use crate::rules::algorithm::schedulechangevalidation::fixtures::*;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::{LocalDate, LocalDateTime};

    fn blank() -> RuleItem {
        RuleItem::new(
            1,
            1,
            "x",
            RuleClass::ShiftModificationSchedValidation,
            Default::default(),
        )
    }
    fn dec_30() -> LocalDateTime {
        LocalDateTime::of(2020, 12, 30, 0, 0, 0)
    }
    fn rule() -> ShiftModificationPenaltyRule<Types, Now> {
        ShiftModificationPenaltyRule::new(Types(UOM::Hours), Now(dec_30()))
    }
    fn params() -> RuleParams {
        rule_params! { EARNING_TYPE => "21" }
    }
    fn at(d: i32, h: i32) -> LocalDateTime {
        LocalDateTime::of(2021, 1, d, h, 0, 0)
    }
    #[allow(clippy::too_many_arguments)]
    fn build(
        net: f64,
        date: i32,
        from: LocalDateTime,
        to: LocalDateTime,
        job: i32,
    ) -> EmployeeShift {
        EmployeeShift::new(
            1,
            11,
            job,
            LocalDate::of(2021, 1, date),
            ShiftType::Schedule,
            Vec::new(),
        )
        .with_times(Some(from), Some(to))
        .with_net_hours(net)
    }

    // ---- java_parity_tests: ShiftModificationPenaltyRuleImplTest.groovy ----

    #[test]
    fn returns_a_request_if_the_start_or_end_changed_with_the_same_duration_otherwise_null() {
        // (original net, updated net, updated date, updated start, updated end, updated job, expect request)
        let rows = [
            (6.0, 8.0, 1, at(1, 9), at(1, 17), 34, false),
            (8.0, 8.0, 1, at(1, 9), at(1, 17), 34, false),
            (8.0, 8.0, 13, at(1, 9), at(1, 17), 34, true),
            (8.0, 8.0, 1, at(1, 12), at(1, 17), 34, true),
            (8.0, 8.0, 1, at(1, 9), at(1, 19), 34, true),
            (8.0, 8.0, 1, at(1, 9), at(1, 19), 35, true),
            (8.0, 8.0, 1, at(1, 9), at(1, 17), 35, true),
        ];
        for (i, (original_net, updated_net, date, from, to, job, want)) in
            rows.into_iter().enumerate()
        {
            let original = build(original_net, 1, at(1, 9), at(1, 17), 34);
            let updated = build(updated_net, date, from, to, job);
            let got = rule().execute(
                Some(&original),
                Some(&updated),
                &blank(),
                &params(),
                Some("temp"),
            );
            assert_eq!(got, want.then(|| expected(1.0)), "row {i}");
        }
    }

    #[test]
    fn returns_null_if_the_original_or_the_updated_shift_is_absent() {
        let shift = build(8.0, 1, at(1, 9), at(1, 12), 34);
        assert!(
            rule()
                .execute(Some(&shift), None, &blank(), &params(), Some("temp"))
                .is_none()
        );
        assert!(
            rule()
                .execute(None, Some(&shift), &blank(), &params(), Some("temp"))
                .is_none()
        );
        assert!(
            rule()
                .execute(None, None, &blank(), &params(), Some("temp"))
                .is_none()
        );
    }
}
