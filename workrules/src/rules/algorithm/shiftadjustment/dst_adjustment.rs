//! Port of `DSTAdjustmentRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftadjustment/DSTAdjustmentRuleImpl.java`.
//!
//! Credits or debits a shift that spans the moment the clock changes for
//! daylight saving: a shift spanning the "spring forward" moment loses
//! `adjustmentLength` minutes of wall-clock time it never actually worked
//! (a `WORKED`-type debit), and one spanning "fall back" gains them back (a
//! credit) — at most one of the two, and only for time-and-attendance
//! calculations (`EmployeeCalculationMode::Ta`); scheduling never applies it.
//!
//! Unlike every `BREAK`-type rule in this family, the stored adjustment here
//! is `WORKED`, so `calcAdjustments`' `adjHours += adj.getAdjHours()` branch
//! applies — no sign inversion to reason about.
//!
//! No Groovy spec; behaviour tests are written from the Java.

use crate::common::enums::employee_calculation_mode::EmployeeCalculationMode;
use crate::common::enums::shift_adjust_type::ShiftAdjustType;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftadjustment::ShiftAdjustmentRule;
use crate::rules::algorithm::shiftadjustment::config::{
    ADJUSTMENT_LENGTH, BACKWARD_DAY, BACKWARD_MONTH, DSTAdjustmentRuleConfig, FORWARD_DAY,
    FORWARD_MONTH, TIME_OF_SHIFT,
};
use crate::rules::rule_config::RuleConfig;
use joda_rs::{LocalDate, LocalTime};

/// `DSTAdjustmentRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DSTAdjustmentRule;

impl ShiftAdjustmentRule for DSTAdjustmentRule {
    fn execute(&self, shift: &mut EmployeeShift, dataset: &dyn TimeCard, rule_item: &RuleItem) {
        if shift.has_errors() || !shift.has_both_times() {
            return;
        }
        if dataset.calculation_mode() != EmployeeCalculationMode::Ta {
            return;
        }

        let params = rule_item
            .params()
            .fixed(&DSTAdjustmentRuleConfig.default_values());
        let backward_month = params.int_at(BACKWARD_MONTH);
        let backward_day = params.int_at(BACKWARD_DAY);
        let forward_month = params.int_at(FORWARD_MONTH);
        let forward_day = params.int_at(FORWARD_DAY);
        let adjustment_length = params.int_at(ADJUSTMENT_LENGTH);
        let time_shift = LocalTime::parse(params.get(TIME_OF_SHIFT).unwrap_or("00:00:00"));

        let adjustment_in_hours = f64::from(adjustment_length) / 60.0;
        let year = shift.shift_date().year();
        let backward_dt = LocalDate::of(year, backward_month, backward_day).at_time(time_shift);
        let forward_dt = LocalDate::of(year, forward_month, forward_day).at_time(time_shift);

        let start = shift
            .start_date_time()
            .expect("has_both_times guarantees a start");
        let end = shift
            .end_date_time()
            .expect("has_both_times guarantees an end");

        if start.is_on_or_before(backward_dt) && end.is_on_or_after(backward_dt) {
            shift.apply_adjustment(adjustment_in_hours, ShiftAdjustType::Worked);
        } else if start.is_on_or_before(forward_dt) && end.is_on_or_after(forward_dt) {
            shift.apply_adjustment(-adjustment_in_hours, ShiftAdjustType::Worked);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDateTime;

    fn rule_item(params: RuleParams) -> RuleItem {
        RuleItem::new(1, 1, "DST adjustment", RuleClass::DstAdjustmentSad, params)
    }

    fn ta_dataset() -> TimeCardData {
        TimeCardData::new().with_calculation_mode(EmployeeCalculationMode::Ta)
    }

    fn shift_spanning(start: LocalDateTime, end: LocalDateTime) -> EmployeeShift {
        EmployeeShift::new(
            1,
            100,
            1,
            start.to_local_date(),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_times(Some(start), Some(end))
    }

    #[test]
    fn a_shift_spanning_the_backward_moment_gains_the_adjustment() {
        let dataset = ta_dataset();
        // Default backward moment: Nov 6, 02:00.
        let mut shift = shift_spanning(
            LocalDateTime::of(2014, 11, 6, 1, 0, 0),
            LocalDateTime::of(2014, 11, 6, 5, 0, 0),
        );

        DSTAdjustmentRule.execute(&mut shift, &dataset, &rule_item(RuleParams::new()));

        assert_eq!(shift.adj_hours(), 1.0);
    }

    #[test]
    fn a_shift_spanning_the_forward_moment_loses_the_adjustment() {
        let dataset = ta_dataset();
        // Default forward moment: Mar 14, 02:00.
        let mut shift = shift_spanning(
            LocalDateTime::of(2014, 3, 14, 1, 0, 0),
            LocalDateTime::of(2014, 3, 14, 5, 0, 0),
        );

        DSTAdjustmentRule.execute(&mut shift, &dataset, &rule_item(RuleParams::new()));

        assert_eq!(shift.adj_hours(), -1.0);
    }

    #[test]
    fn a_shift_not_spanning_either_moment_is_unaffected() {
        let dataset = ta_dataset();
        let mut shift = shift_spanning(
            LocalDateTime::of(2014, 6, 1, 8, 0, 0),
            LocalDateTime::of(2014, 6, 1, 16, 0, 0),
        );

        DSTAdjustmentRule.execute(&mut shift, &dataset, &rule_item(RuleParams::new()));

        assert_eq!(shift.adj_hours(), 0.0);
    }

    #[test]
    fn scheduling_mode_never_applies_the_adjustment() {
        let dataset =
            TimeCardData::new().with_calculation_mode(EmployeeCalculationMode::AutoSchedule);
        let mut shift = shift_spanning(
            LocalDateTime::of(2014, 11, 6, 1, 0, 0),
            LocalDateTime::of(2014, 11, 6, 5, 0, 0),
        );

        DSTAdjustmentRule.execute(&mut shift, &dataset, &rule_item(RuleParams::new()));

        assert_eq!(shift.adj_hours(), 0.0);
    }

    #[test]
    fn a_custom_adjustment_length_scales_the_credit() {
        let dataset = ta_dataset();
        let mut shift = shift_spanning(
            LocalDateTime::of(2014, 11, 6, 1, 0, 0),
            LocalDateTime::of(2014, 11, 6, 5, 0, 0),
        );
        let params = rule_params! { ADJUSTMENT_LENGTH => "30" };

        DSTAdjustmentRule.execute(&mut shift, &dataset, &rule_item(params));

        assert_eq!(shift.adj_hours(), 0.5);
    }
}
