//! Port of `JobRegRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/regularrate/JobRegRateRuleImpl.java`.
//!
//! The simple case every other rule in the family complicates: the job's own
//! `EmployeeJobStatus.hourlyRate` on the effective date, nothing else.
//!
//! # The two overloads disagree about a missing job status
//!
//! The shift overload guards the lookup and silently writes nothing if there
//! is no job status covering the date. The earning overload does not —
//! `jobStatus.getHourlyRate()` on a null status is an unguarded NPE in Java,
//! reproduced here as a panic. Same rule, two different failure behaviours for
//! the same missing-status condition; see the family-wide finding in
//! `PARITY_AUDIT.md`.
//!
//! Ported cases: `JobRegRateRuleImplTest.groovy` (one case).

use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::regularrate::{RegularRateRule, set_earning_rate, set_regular_rates};

/// `JobRegRateRuleImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct JobRegRateRule;

impl RegularRateRule for JobRegRateRule {
    fn execute_for_shift(
        &self,
        shift: &EmployeeShift,
        distribution: &mut HoursDistribution,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let job_status = dataset
            .employee()
            .and_then(|employee| employee.employee_job_status(shift.job_id(), distribution.date()));

        if let Some(job_status) = job_status {
            set_regular_rates(distribution, job_status.hourly_rate(), rule_item);
        }
    }

    fn execute_for_earning(
        &self,
        earning: &mut EmployeeEarning,
        dataset: &dyn TimeCard,
        _rule_item: &RuleItem,
    ) {
        let hourly_rate = dataset
            .employee()
            .and_then(|employee| {
                employee.employee_job_status(earning.job_id(), earning.earning_date())
            })
            .unwrap_or_else(|| {
                panic!(
                    "no job status for job {} on {}",
                    earning.job_id(),
                    earning.earning_date()
                )
            })
            .hourly_rate();

        set_earning_rate(earning, hourly_rate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::employee::Employee;
    use crate::entity::employee_job_status::EmployeeJobStatus;
    use crate::entity::rule_item::RuleItem;
    use crate::entity::time_card::TimeCardData;
    use crate::rules::params::RuleParams;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::LocalDate;

    fn job_status(job_id: i32, hourly_rate: f64) -> EmployeeJobStatus {
        EmployeeJobStatus::new(
            1,
            100,
            job_id,
            LocalDate::of(1999, 10, 10),
            LocalDate::of(3000, 10, 10),
            EmployeePayType::Hourly,
            hourly_rate,
            false,
        )
    }

    fn rule_item(id: i32) -> RuleItem {
        RuleItem::new(id, 1, "Job rate", RuleClass::JobRrr, RuleParams::new())
    }

    #[test]
    fn job_reg_rate_updates_the_distribution_from_the_job_status() {
        let shift_date = LocalDate::of(2016, 6, 1);
        let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new());
        let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(1, 8.50)]);
        let dataset = TimeCardData::new().with_employee(employee);

        JobRegRateRule.execute_for_shift(&shift, &mut distribution, &dataset, &rule_item(1));

        assert_eq!(distribution.base_rate(), 8.50);
        assert_eq!(distribution.rate_rule_item_id(), Some(1));
    }

    #[test]
    fn no_job_status_leaves_the_distribution_untouched() {
        let shift_date = LocalDate::of(2016, 6, 1);
        let shift = EmployeeShift::new(1, 100, 1, shift_date, ShiftType::Actual, Vec::new());
        let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
        let employee = Employee::new(100, 1, "Alex Kim", Vec::new());
        let dataset = TimeCardData::new().with_employee(employee);

        JobRegRateRule.execute_for_shift(&shift, &mut distribution, &dataset, &rule_item(1));

        assert_eq!(distribution.base_rate(), 0.0);
        assert_eq!(distribution.rate_rule_item_id(), None);
    }

    #[test]
    fn job_reg_rate_prices_a_standalone_earning() {
        let earning_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", vec![job_status(1, 8.50)]);
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning =
            EmployeeEarning::new(1, 100, 1, 5, earning_date, 4.0, 0.0, EarningSource::Rule);

        JobRegRateRule.execute_for_earning(&mut earning, &dataset, &rule_item(1));

        assert_eq!(earning.rate(), 8.50);
        assert_eq!(earning.total_dollars(), 34.0);
    }

    #[test]
    #[should_panic]
    fn a_missing_job_status_panics_on_the_earning_path() {
        let earning_date = LocalDate::of(2016, 6, 1);
        let employee = Employee::new(100, 1, "Alex Kim", Vec::new());
        let dataset = TimeCardData::new().with_employee(employee);
        let mut earning =
            EmployeeEarning::new(1, 100, 1, 5, earning_date, 4.0, 0.0, EarningSource::Rule);

        JobRegRateRule.execute_for_earning(&mut earning, &dataset, &rule_item(1));
    }

    mod java_parity_tests {
        use super::*;

        /// `JobRegRateRuleImplTest`: "job reg rate rule should update hours
        /// distributions and reg rate on shift".
        #[test]
        fn job_reg_rate_rule_should_update_hours_distributions_and_reg_rate_on_shift() {
            let shift_date = LocalDate::of(2016, 6, 1);
            let job_id = 1;
            let base_rate = 8.50;

            let shift = EmployeeShift::new(1, 0, job_id, shift_date, ShiftType::Actual, Vec::new());
            let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);
            let employee = Employee::new(0, 0, "", vec![job_status(job_id, base_rate)]);
            let dataset = TimeCardData::new().with_employee(employee);

            JobRegRateRule.execute_for_shift(&shift, &mut distribution, &dataset, &rule_item(1));

            assert_eq!(distribution.base_rate(), base_rate);
            assert_eq!(distribution.rate_rule_item_id(), Some(1));
        }
    }
}
