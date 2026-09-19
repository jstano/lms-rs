//! Port of `MinDailyHrsRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftadjustment/MinDailyHrsRuleImpl.java`.
//!
//! Tops a day's worked hours up to a configured minimum, chosen from up to
//! three tiers: each tier pairs a "qualify at this many worked hours"
//! threshold with "top up to this many". The tier used is whichever
//! qualifying threshold the day's actual hours are closest to — not simply
//! the highest one met.
//!
//! `WORKED`-type, like `DSTAdjustmentRuleImpl` — built directly rather than
//! through `ShiftAdjustmentRuleImpl.createAdjustment`, and rounded with
//! `TDouble.round(_, 2)` (`round_hours`, the same rounding — see
//! `common/numbers.rs`) rather than `roundRawHours`.
//!
//! No Groovy spec; behaviour tests are written from the Java.

use crate::common::enums::shift_adjust_type::ShiftAdjustType;
use crate::common::numbers::round_hours;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftadjustment::ShiftAdjustmentRule;
use crate::rules::algorithm::shiftadjustment::config::{
    MIN_DAILY_HRS, MIN_DAILY_HRS2, MIN_DAILY_HRS3, MIN_WORKED_HRS, MIN_WORKED_HRS2,
    MIN_WORKED_HRS3, MinDailyHrsRuleConfig,
};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;

/// `MinDailyHrsRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MinDailyHrsRule;

impl ShiftAdjustmentRule for MinDailyHrsRule {
    fn execute(&self, shift: &mut EmployeeShift, dataset: &dyn TimeCard, rule_item: &RuleItem) {
        if shift.has_errors() {
            return;
        }

        let params = rule_item
            .params()
            .fixed(&MinDailyHrsRuleConfig.default_values());
        let min_hours_list = [
            params.double_at(MIN_DAILY_HRS),
            params.double_at(MIN_DAILY_HRS2),
            params.double_at(MIN_DAILY_HRS3),
        ];
        let min_worked_hours_list = [
            params.double_at(MIN_WORKED_HRS),
            params.double_at(MIN_WORKED_HRS2),
            params.double_at(MIN_WORKED_HRS3),
        ];

        let shift_date = shift.shift_date();
        let daily_hrs = dataset
            .shifts_for_period(&DateRange::new(shift_date, shift_date))
            .iter()
            .fold(0.0, |acc, s| round_hours(acc + s.worked_hours()));

        let nearest_index = (0..3)
            .filter(|&i| daily_hrs >= min_worked_hours_list[i])
            .min_by(|&a, &b| {
                (min_worked_hours_list[a] - daily_hrs)
                    .abs()
                    .partial_cmp(&(min_worked_hours_list[b] - daily_hrs).abs())
                    .unwrap()
            })
            .unwrap_or(0);

        if daily_hrs >= min_worked_hours_list[nearest_index]
            && daily_hrs < min_hours_list[nearest_index]
        {
            let adj_hours = round_hours(min_hours_list[nearest_index] - daily_hrs);
            shift.apply_adjustment(adj_hours, ShiftAdjustType::Worked);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    fn rule_item(params: RuleParams) -> RuleItem {
        RuleItem::new(1, 1, "Min daily hrs", RuleClass::MinDailyHrsSad, params)
    }

    fn shift_with_worked_hours(worked_hours: f64) -> EmployeeShift {
        let date = LocalDate::of(2014, 5, 6);
        EmployeeShift::new(1, 100, 1, date, ShiftType::Actual, Vec::new())
            .with_worked_hours(worked_hours)
    }

    fn dataset_with(shift: EmployeeShift) -> TimeCardData {
        TimeCardData::new().with_shifts(vec![shift])
    }

    #[test]
    fn a_day_under_the_single_tier_minimum_is_topped_up() {
        let shift = shift_with_worked_hours(4.0);
        let dataset = dataset_with(shift.clone());
        let mut shift = shift;
        let params = rule_params! {
            MIN_DAILY_HRS => "8.0",
            MIN_WORKED_HRS => "0.0"
        };

        MinDailyHrsRule.execute(&mut shift, &dataset, &rule_item(params));

        assert_eq!(shift.adj_hours(), 4.0);
    }

    #[test]
    fn a_day_already_at_or_over_the_minimum_is_unaffected() {
        let shift = shift_with_worked_hours(8.0);
        let dataset = dataset_with(shift.clone());
        let mut shift = shift;
        let params = rule_params! {
            MIN_DAILY_HRS => "8.0",
            MIN_WORKED_HRS => "0.0"
        };

        MinDailyHrsRule.execute(&mut shift, &dataset, &rule_item(params));

        assert_eq!(shift.adj_hours(), 0.0);
    }

    #[test]
    fn a_day_with_errors_gets_no_adjustment() {
        let shift = shift_with_worked_hours(4.0).with_errors(vec![ShiftErrorType::MissingOut]);
        let dataset = dataset_with(shift.clone());
        let mut shift = shift;
        let params = rule_params! {
            MIN_DAILY_HRS => "8.0",
            MIN_WORKED_HRS => "0.0"
        };

        MinDailyHrsRule.execute(&mut shift, &dataset, &rule_item(params));

        assert_eq!(shift.adj_hours(), 0.0);
    }

    #[test]
    fn the_closest_qualifying_tier_wins() {
        // Two tiers: qualify at 4 -> top up to 6; qualify at 7 -> top up to
        // 10. A day of 7.5 hours qualifies for both; 7 is closer than 4.
        let shift = shift_with_worked_hours(7.5);
        let dataset = dataset_with(shift.clone());
        let mut shift = shift;
        let params = rule_params! {
            MIN_DAILY_HRS => "6.0",
            MIN_WORKED_HRS => "4.0",
            MIN_DAILY_HRS2 => "10.0",
            MIN_WORKED_HRS2 => "7.0"
        };

        MinDailyHrsRule.execute(&mut shift, &dataset, &rule_item(params));

        assert_eq!(shift.adj_hours(), 2.5);
    }
}
