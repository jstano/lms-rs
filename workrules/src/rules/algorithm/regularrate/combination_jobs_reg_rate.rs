//! Port of `CombinationJobsRegRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/regularrate/CombinationJobsRegRateRuleImpl.java`.
//!
//! The family's largest and only genuinely complex rule. For the target date,
//! computes a **daily** candidate rate (only same-date shifts/earnings,
//! grouped by job, keeping groups meeting an hours threshold) and a **weekly**
//! candidate (the whole scheduling week, grouped by job, keeping groups
//! meeting a **days**-worked threshold, pricing each job at whichever status
//! is in force at the *end* of that week rather than at each shift's own
//! date), each candidate limited to rates strictly higher than the target
//! shift's own and optionally required to belong to a separate job, then pays
//! `max(daily, weekly)` — falling back to the target's own status if nothing
//! qualifies either test.
//!
//! # Reaching the property
//!
//! Java: `dataset.getEmployee().getProperty().getCurrentWeek().getDateRangeContainingDate(targetDate)`.
//! This crate's entity graph carries only the property's id, so the week
//! comes through [`PropertyPort`] instead — precisely the mechanism the
//! module doc already worked out, and identical to how
//! [`RegularHoursByWorkWeekRule`](crate::rules::algorithm::regularhoursdistribution::reg_hours_by_work_week::RegularHoursByWorkWeekRule)
//! reaches the same property field for a different purpose.
//!
//! # Grouping is by job, not by job-status identity
//!
//! Java groups by the `EmployeeJobStatus` Hibernate returns, relying on
//! reference/id equality. Every shift or earning for one job on one date (or
//! across one week, for the weekly path) resolves to exactly the same status
//! object — there is at most one status covering a job on a given date — so
//! grouping by `job_id` is the same partition without needing `Hash`/`Eq` on
//! the entity.
//!
//! # `mustBeSeparateJob` compares the **job**, not the status
//!
//! `entry.getKey().getJob().getID() != targetJobId` — so a job whose rate rose
//! mid-week (two different `EmployeeJobStatus` rows, same job) is still
//! "the same job" under this flag, and does not qualify as a higher
//! classification. Confirmed by
//! `CombinationJobsRegRateRuleImplSeparateJobTest`'s "weekly rate path with
//! same job split rate does NOT update shift".
//!
//! Ported cases: `CombinationJobsRegRateRuleImplTest.groovy` (14 cases),
//! `CombinationJobsRegRateRuleImplSeparateJobTest.groovy` (11 cases),
//! `CombinationJobsRegRateRuleImplSplitRateTest.groovy` (4 cases).

use crate::common::enums::shift_type::ShiftType;
use crate::entity::employee::Employee;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_job_status::EmployeeJobStatus;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::regularrate::config::{
    CombinationJobsRegRateRuleConfig, DAYS_THRESHOLD, HIGHER_CLASSIFICATION_MUST_BE_SEPARATE_JOB,
    HOURS_THRESHOLD,
};
use crate::rules::algorithm::regularrate::{RegularRateRule, set_earning_rate, set_regular_rates};
use crate::rules::params::RuleParams;
use crate::rules::ports::{EarningTypePort, PropertyPort};
use crate::rules::rule_config::RuleConfig;
use date_range_rs::DateRange;
use date_range_rs::daterange::weekly_date_range::WeeklyDateRange;
use joda_rs::LocalDate;

/// `CombinationJobsRegRateRuleImpl`.
pub struct CombinationJobsRegRateRule<P: PropertyPort, E: EarningTypePort> {
    property: P,
    earning_types: E,
}

impl<P: PropertyPort, E: EarningTypePort> CombinationJobsRegRateRule<P, E> {
    /// Build the rule over the ports its week lookup and earning-type filter
    /// come from.
    pub fn new(property: P, earning_types: E) -> Self {
        Self {
            property,
            earning_types,
        }
    }

    /// `getTargetWeek(TimeCard, LocalDate)`.
    fn target_week(&self, property_id: i32, target_date: LocalDate) -> DateRange {
        let period_end_date = self.property.period_end_date(property_id);
        WeeklyDateRange::with_end_date(period_end_date).range_containing_date(target_date)
    }

    /// `getMaxRateForDate`.
    fn max_rate_for_date(
        &self,
        target_date: LocalDate,
        dataset: &dyn TimeCard,
        employee: &Employee,
        params: &RuleParams,
        target_status: &EmployeeJobStatus,
        must_be_separate_job: bool,
    ) -> f64 {
        let target_week = self.target_week(employee.property_id(), target_date);

        let daily = self.daily_rate(
            target_date,
            &target_week,
            dataset,
            employee,
            params,
            target_status,
            must_be_separate_job,
        );
        let weekly = self.weekly_rate(
            &target_week,
            dataset,
            employee,
            params,
            target_status,
            must_be_separate_job,
        );

        daily.max(weekly)
    }

    /// `getDailyRate`.
    #[allow(clippy::too_many_arguments)]
    fn daily_rate(
        &self,
        target_date: LocalDate,
        target_week: &DateRange,
        dataset: &dyn TimeCard,
        employee: &Employee,
        params: &RuleParams,
        target_status: &EmployeeJobStatus,
        must_be_separate_job: bool,
    ) -> f64 {
        let hours_threshold = params.double_at(HOURS_THRESHOLD);

        let valid_date = |date: LocalDate| {
            date == target_date
                && target_week.contains_date(date)
                && dataset.is_open_for_editing_on(date)
        };
        let resolve_status =
            |job_id: i32| employee.effective_job_status(target_date, job_id).cloned();

        let groups = hours_by_job(dataset, &self.earning_types, valid_date, resolve_status);

        rate_for_highest_paid(groups, target_status, must_be_separate_job, |hours| {
            hours.iter().sum::<f64>() >= hours_threshold
        })
    }

    /// `getWeeklyRate`.
    fn weekly_rate(
        &self,
        target_week: &DateRange,
        dataset: &dyn TimeCard,
        employee: &Employee,
        params: &RuleParams,
        target_status: &EmployeeJobStatus,
        must_be_separate_job: bool,
    ) -> f64 {
        let days_threshold = params.double_at(DAYS_THRESHOLD);

        let valid_date = |date: LocalDate| {
            target_week.contains_date(date) && dataset.is_open_for_editing_on(date)
        };
        let resolve_status = |job_id: i32| {
            employee
                .last_effective_job_status_for_period(target_week, job_id)
                .cloned()
        };

        let groups = hours_by_job(dataset, &self.earning_types, valid_date, resolve_status);

        rate_for_highest_paid(groups, target_status, must_be_separate_job, |hours| {
            hours.len() as f64 >= days_threshold
        })
    }
}

impl<P: PropertyPort, E: EarningTypePort> RegularRateRule for CombinationJobsRegRateRule<P, E> {
    fn execute_for_shift(
        &self,
        shift: &EmployeeShift,
        distribution: &mut HoursDistribution,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&CombinationJobsRegRateRuleConfig.default_values());

        if !is_valid_shift(shift) || !dataset.is_open_for_editing_on(distribution.date()) {
            return;
        }
        let Some(employee) = dataset.employee() else {
            return;
        };
        let target_status = employee
            .effective_job_status(distribution.date(), shift.job_id())
            .unwrap_or_else(|| {
                panic!(
                    "no job status for job {} on {}",
                    shift.job_id(),
                    distribution.date()
                )
            });
        let must_be_separate_job = params.bool_at(HIGHER_CLASSIFICATION_MUST_BE_SEPARATE_JOB);

        let rate = self.max_rate_for_date(
            distribution.date(),
            dataset,
            employee,
            &params,
            target_status,
            must_be_separate_job,
        );
        set_regular_rates(distribution, rate, rule_item);
    }

    fn execute_for_earning(
        &self,
        earning: &mut EmployeeEarning,
        dataset: &dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&CombinationJobsRegRateRuleConfig.default_values());

        if !dataset.is_open_for_editing_for_earning(earning) {
            return;
        }
        let Some(employee) = dataset.employee() else {
            return;
        };
        let target_status = employee
            .effective_job_status(earning.earning_date(), earning.job_id())
            .unwrap_or_else(|| {
                panic!(
                    "no job status for job {} on {}",
                    earning.job_id(),
                    earning.earning_date()
                )
            });
        let must_be_separate_job = params.bool_at(HIGHER_CLASSIFICATION_MUST_BE_SEPARATE_JOB);

        let rate = self.max_rate_for_date(
            earning.earning_date(),
            dataset,
            employee,
            &params,
            target_status,
            must_be_separate_job,
        );
        set_earning_rate(earning, rate);
    }
}

/// `isValidShift`.
fn is_valid_shift(shift: &EmployeeShift) -> bool {
    shift.shift_type() == ShiftType::Schedule || shift.errors().is_empty()
}

/// `isValidEarning` — `EarnType.REGULAR` or `EarnType.PREMIUM` only.
fn is_valid_earning(earning: &EmployeeEarning, earning_types: &dyn EarningTypePort) -> bool {
    earning_types
        .find_by_id(earning.earning_type_id())
        .is_some_and(|earning_type| {
            earning_type.earn_type().is_regular() || earning_type.earn_type().is_premium()
        })
}

/// `getHoursForJobsFor` — every valid shift and earning's hours, grouped by
/// the job status `resolve_status` picks for it.
fn hours_by_job(
    dataset: &dyn TimeCard,
    earning_types: &dyn EarningTypePort,
    valid_date: impl Fn(LocalDate) -> bool,
    resolve_status: impl Fn(i32) -> Option<EmployeeJobStatus>,
) -> Vec<(EmployeeJobStatus, Vec<f64>)> {
    let mut groups: Vec<(EmployeeJobStatus, Vec<f64>)> = Vec::new();

    for shift in dataset.shifts() {
        if !is_valid_shift(shift) || !valid_date(shift.shift_date()) {
            continue;
        }
        if let Some(status) = resolve_status(shift.job_id()) {
            push_hours(&mut groups, status, shift.worked_hours());
        }
    }

    for earning in dataset.earnings() {
        if !is_valid_earning(earning, earning_types) || !valid_date(earning.earning_date()) {
            continue;
        }
        if let Some(status) = resolve_status(earning.job_id()) {
            push_hours(&mut groups, status, earning.hours());
        }
    }

    groups
}

fn push_hours(
    groups: &mut Vec<(EmployeeJobStatus, Vec<f64>)>,
    status: EmployeeJobStatus,
    hours: f64,
) {
    match groups
        .iter_mut()
        .find(|(s, _)| s.job_id() == status.job_id())
    {
        Some((_, list)) => list.push(hours),
        None => groups.push((status, vec![hours])),
    }
}

/// `getRateForHighestPayedJobWorkedFor`.
fn rate_for_highest_paid(
    groups: Vec<(EmployeeJobStatus, Vec<f64>)>,
    target_status: &EmployeeJobStatus,
    must_be_separate_job: bool,
    threshold_met: impl Fn(&[f64]) -> bool,
) -> f64 {
    groups
        .into_iter()
        .filter(|(_, hours)| threshold_met(hours))
        .filter(|(status, _)| status.hourly_rate() > target_status.hourly_rate())
        .filter(|(status, _)| !must_be_separate_job || status.job_id() != target_status.job_id())
        .max_by(|(a, _), (b, _)| a.hourly_rate().partial_cmp(&b.hourly_rate()).unwrap())
        .map_or(target_status.hourly_rate(), |(status, _)| {
            status.hourly_rate()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earn_type::EarnType;
    use crate::common::enums::earning_source::EarningSource;
    use crate::common::enums::employee_pay_type::EmployeePayType;
    use crate::common::enums::uom::UOM;
    use crate::entity::earning_type::EarningType;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::rule_class::RuleClass;
    use std::collections::HashMap;

    struct FixedPeriodEnd(LocalDate);
    impl PropertyPort for FixedPeriodEnd {
        fn period_end_date(&self, _property_id: i32) -> LocalDate {
            self.0
        }
    }

    struct EarningTypes(HashMap<i32, EarningType>);
    impl EarningTypePort for EarningTypes {
        fn find_by_id(&self, id: i32) -> Option<EarningType> {
            self.0.get(&id).cloned()
        }
    }

    fn regular_earning_types() -> EarningTypes {
        EarningTypes(HashMap::from([(
            5,
            EarningType::new(5, 1, "Regular", EarnType::Regular, UOM::Hours),
        )]))
    }

    fn rule_item(id: i32, params: RuleParams) -> RuleItem {
        RuleItem::new(
            id,
            1,
            "Combination jobs rate",
            RuleClass::CombinationJobRrr,
            params,
        )
    }

    fn job_status(job_id: i32, rate: f64, start: LocalDate, end: LocalDate) -> EmployeeJobStatus {
        EmployeeJobStatus::new(
            1,
            100,
            job_id,
            start,
            end,
            EmployeePayType::Hourly,
            rate,
            false,
        )
    }

    fn shift(date: LocalDate, job_id: i32, worked_hours: f64) -> EmployeeShift {
        EmployeeShift::new(1, 100, job_id, date, ShiftType::Actual, Vec::new())
            .with_worked_hours(worked_hours)
    }

    const PERIOD_END: fn() -> LocalDate = || LocalDate::of(2015, 10, 25);
    const PERIOD_START: fn() -> LocalDate = || LocalDate::of(2015, 10, 19);

    #[test]
    fn one_job_worked_all_week_pays_that_jobs_rate() {
        let employee = Employee::new(
            100,
            1,
            "Test Employee",
            vec![
                job_status(1, 5.0, PERIOD_START(), LocalDate::of(2100, 12, 31)),
                job_status(2, 10.0, PERIOD_START(), LocalDate::of(2100, 12, 31)),
            ],
        );
        let dataset = TimeCardData::new()
            .with_employee(employee)
            .with_shifts(vec![shift(LocalDate::of(2015, 10, 21), 2, 5.0)]);
        let checked_shift = shift(LocalDate::of(2015, 10, 21), 2, 5.0);
        let mut distribution =
            HoursDistribution::new(1, LocalDate::of(2015, 10, 21), None, 0.0, 0.0);
        let rule =
            CombinationJobsRegRateRule::new(FixedPeriodEnd(PERIOD_END()), regular_earning_types());

        rule.execute_for_shift(
            &checked_shift,
            &mut distribution,
            &dataset,
            &rule_item(1, CombinationJobsRegRateRuleConfig.default_values()),
        );

        assert_eq!(distribution.base_rate(), 10.0);
    }

    mod java_parity_tests {
        use super::*;

        fn statuses() -> Vec<EmployeeJobStatus> {
            vec![
                job_status(
                    1,
                    5.0,
                    PERIOD_END().minus_months(12),
                    LocalDate::of(2100, 12, 31),
                ),
                job_status(
                    2,
                    10.0,
                    PERIOD_END().minus_months(12),
                    LocalDate::of(2100, 12, 31),
                ),
                job_status(
                    3,
                    12.0,
                    PERIOD_END().minus_months(12),
                    LocalDate::of(2100, 12, 31),
                ),
            ]
        }

        fn employee() -> Employee {
            Employee::new(100, 1, "Test Employee", statuses())
        }

        fn shift_with_errors(
            date: LocalDate,
            job_id: i32,
            worked_hours: f64,
            has_error: bool,
            shift_type: ShiftType,
        ) -> EmployeeShift {
            let errors = if has_error {
                vec![crate::common::enums::shift_error_type::ShiftErrorType::MissingIn]
            } else {
                Vec::new()
            };
            EmployeeShift::new(1, 100, job_id, date, shift_type, Vec::new())
                .with_worked_hours(worked_hours)
                .with_errors(errors)
        }

        fn default_params() -> RuleParams {
            CombinationJobsRegRateRuleConfig.default_values()
        }

        fn rule() -> CombinationJobsRegRateRule<FixedPeriodEnd, EarningTypes> {
            CombinationJobsRegRateRule::new(FixedPeriodEnd(PERIOD_END()), regular_earning_types())
        }

        /// "when employee works only one job for a week, they are payed at
        /// that job's rate".
        #[test]
        fn when_employee_works_only_one_job_for_a_week_they_are_payed_at_that_jobs_rate() {
            let checked_shift = shift(LocalDate::of(2015, 10, 21), 2, 5.0);
            let dataset = TimeCardData::new()
                .with_employee(employee())
                .with_shifts(vec![shift(LocalDate::of(2015, 10, 21), 2, 5.0)]);
            let mut distribution =
                HoursDistribution::new(1, LocalDate::of(2015, 10, 21), None, 0.0, 0.0);

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &rule_item(1, default_params()),
            );

            assert_eq!(distribution.base_rate(), 10.0);
        }

        /// "when employee has earnings for only one job for a week, they are
        /// payed at that job's rate".
        #[test]
        fn when_employee_has_earnings_for_only_one_job_for_a_week_they_are_payed_at_that_jobs_rate()
        {
            let mut earning = EmployeeEarning::new(
                1,
                100,
                2,
                5,
                LocalDate::of(2015, 10, 21),
                5.0,
                0.0,
                EarningSource::Rule,
            );
            let dataset = TimeCardData::new()
                .with_employee(employee())
                .with_earnings(vec![EmployeeEarning::new(
                    2,
                    100,
                    2,
                    5,
                    LocalDate::of(2015, 10, 21),
                    5.0,
                    0.0,
                    EarningSource::Rule,
                )]);

            rule().execute_for_earning(&mut earning, &dataset, &rule_item(1, default_params()));

            assert_eq!(earning.rate(), 10.0);
        }

        /// "Should not modify an shift in a closed period".
        #[test]
        fn should_not_modify_a_shift_in_a_closed_period() {
            let checked_shift = shift(LocalDate::of(2015, 10, 18), 1, 4.0);
            let mut dataset = TimeCardData::new()
                .with_employee(employee())
                .with_shifts(vec![shift(LocalDate::of(2015, 10, 21), 2, 5.0)]);
            dataset = dataset.with_calculation_start_date(LocalDate::of(2015, 10, 19));
            let mut distribution =
                HoursDistribution::new(1, LocalDate::of(2015, 10, 18), None, 0.0, 0.0);

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &rule_item(1, default_params()),
            );

            assert_eq!(
                distribution.base_rate(),
                0.0,
                "the closed date is never priced"
            );
        }

        /// "shifts with errors are ignored".
        #[test]
        fn shifts_with_errors_are_ignored() {
            let checked_shift =
                shift_with_errors(LocalDate::of(2015, 10, 21), 1, 5.0, true, ShiftType::Actual);
            let dataset = TimeCardData::new()
                .with_employee(employee())
                .with_shifts(vec![
                    shift(LocalDate::of(2015, 10, 21), 1, 5.0),
                    shift(LocalDate::of(2015, 10, 21), 2, 3.0),
                ]);
            let mut distribution =
                HoursDistribution::new(1, LocalDate::of(2015, 10, 21), None, 0.0, 0.0);

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &rule_item(1, default_params()),
            );

            assert_eq!(
                distribution.base_rate(),
                0.0,
                "an errored actual shift is never a valid target"
            );
        }

        /// "scheduled shifts with errors are not ignored".
        #[test]
        fn scheduled_shifts_with_errors_are_not_ignored() {
            let checked_shift = shift_with_errors(
                LocalDate::of(2015, 10, 21),
                1,
                5.0,
                true,
                ShiftType::Schedule,
            );
            let dataset = TimeCardData::new()
                .with_employee(employee())
                .with_shifts(vec![
                    shift(LocalDate::of(2015, 10, 21), 1, 5.0),
                    shift(LocalDate::of(2015, 10, 21), 2, 3.0),
                ]);
            let mut distribution =
                HoursDistribution::new(1, LocalDate::of(2015, 10, 21), None, 0.0, 0.0);
            let params = rule_params! {
                HOURS_THRESHOLD => "4", DAYS_THRESHOLD => "2",
                HIGHER_CLASSIFICATION_MUST_BE_SEPARATE_JOB => "false",
            };

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &rule_item(1, params),
            );

            assert_eq!(
                distribution.base_rate(),
                5.0,
                "job 2's 3 hours miss the 4-hour daily threshold"
            );
        }

        /// "shifts during the week with errors are ignored".
        #[test]
        fn shifts_during_the_week_with_errors_are_ignored() {
            let checked_shift = shift(LocalDate::of(2015, 10, 21), 1, 0.0);
            let dataset = TimeCardData::new()
                .with_employee(employee())
                .with_shifts(vec![
                    shift(LocalDate::of(2015, 10, 21), 1, 5.0),
                    shift_with_errors(LocalDate::of(2015, 10, 21), 2, 4.0, true, ShiftType::Actual),
                ]);
            let mut distribution =
                HoursDistribution::new(1, LocalDate::of(2015, 10, 21), None, 0.0, 0.0);

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &rule_item(1, default_params()),
            );

            assert_eq!(
                distribution.base_rate(),
                5.0,
                "job 2's errored shift never joins the group"
            );
        }

        /// "shifts during the week with errors are not ignored if they are
        /// scheduled".
        #[test]
        fn shifts_during_the_week_with_errors_are_not_ignored_if_they_are_scheduled() {
            let checked_shift = shift(LocalDate::of(2015, 10, 21), 1, 0.0);
            let dataset = TimeCardData::new()
                .with_employee(employee())
                .with_shifts(vec![
                    shift(LocalDate::of(2015, 10, 21), 1, 5.0),
                    shift_with_errors(
                        LocalDate::of(2015, 10, 21),
                        2,
                        4.0,
                        true,
                        ShiftType::Schedule,
                    ),
                ]);
            let mut distribution =
                HoursDistribution::new(1, LocalDate::of(2015, 10, 21), None, 0.0, 0.0);

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &rule_item(1, default_params()),
            );

            assert_eq!(
                distribution.base_rate(),
                10.0,
                "job 2's scheduled shift joins despite the error"
            );
        }

        /// "Should not modify an earning in a closed period".
        #[test]
        fn should_not_modify_an_earning_in_a_closed_period() {
            let mut checked_earning = EmployeeEarning::new(
                1,
                100,
                1,
                5,
                LocalDate::of(2015, 10, 18),
                5.0,
                0.0,
                EarningSource::Rule,
            );
            let mut dataset = TimeCardData::new()
                .with_employee(employee())
                .with_earnings(vec![EmployeeEarning::new(
                    2,
                    100,
                    2,
                    5,
                    LocalDate::of(2015, 10, 18),
                    5.0,
                    0.0,
                    EarningSource::Rule,
                )]);
            dataset = dataset.with_calculation_start_date(LocalDate::of(2015, 10, 19));

            rule().execute_for_earning(
                &mut checked_earning,
                &dataset,
                &rule_item(1, default_params()),
            );

            assert_eq!(checked_earning.rate(), 0.0);
        }

        /// "only earnings of REGULAR and PREMIUM types are accepted".
        #[test]
        fn only_earnings_of_regular_and_premium_types_are_accepted() {
            let earning_types = EarningTypes(HashMap::from([
                (
                    1,
                    EarningType::new(1, 1, "Accrual", EarnType::Accrual, UOM::Hours),
                ),
                (
                    2,
                    EarningType::new(2, 1, "Memo", EarnType::Memo, UOM::Hours),
                ),
                (
                    3,
                    EarningType::new(3, 1, "Earning", EarnType::Earning, UOM::Hours),
                ),
            ]));
            let checked_shift = shift(LocalDate::of(2015, 10, 21), 1, 0.0);
            let dataset = TimeCardData::new()
                .with_employee(employee())
                .with_earnings(vec![
                    EmployeeEarning::new(
                        1,
                        100,
                        2,
                        1,
                        LocalDate::of(2015, 10, 21),
                        5.0,
                        0.0,
                        EarningSource::Rule,
                    ),
                    EmployeeEarning::new(
                        2,
                        100,
                        2,
                        2,
                        LocalDate::of(2015, 10, 21),
                        5.0,
                        0.0,
                        EarningSource::Rule,
                    ),
                    EmployeeEarning::new(
                        3,
                        100,
                        2,
                        3,
                        LocalDate::of(2015, 10, 21),
                        5.0,
                        0.0,
                        EarningSource::Rule,
                    ),
                ]);
            let mut distribution =
                HoursDistribution::new(1, LocalDate::of(2015, 10, 21), None, 0.0, 0.0);
            let rule = CombinationJobsRegRateRule::new(FixedPeriodEnd(PERIOD_END()), earning_types);

            rule.execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &rule_item(1, default_params()),
            );

            assert_eq!(
                distribution.base_rate(),
                5.0,
                "none of the three earning types qualify"
            );
        }

        /// "when shifts for two jobs occur on the same date and the higher
        /// paid one is less than threshold hours, pay the shift at the lower
        /// rate".
        #[test]
        fn a_higher_paid_job_under_the_daily_threshold_does_not_win() {
            let checked_shift = shift(LocalDate::of(2015, 10, 21), 1, 5.0);
            let dataset = TimeCardData::new()
                .with_employee(employee())
                .with_shifts(vec![
                    shift(LocalDate::of(2015, 10, 21), 1, 5.0),
                    shift(LocalDate::of(2015, 10, 21), 2, 3.0),
                ]);
            let mut distribution =
                HoursDistribution::new(1, LocalDate::of(2015, 10, 21), None, 0.0, 0.0);
            let params = rule_params! { DAYS_THRESHOLD => "2" };

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &rule_item(1, params),
            );

            assert_eq!(distribution.base_rate(), 5.0);
        }

        /// "when shifts for two jobs occur on the same date and the higher
        /// paid one is >= threshold hours, pay the shift at the higher rate".
        #[test]
        fn a_higher_paid_job_meeting_the_daily_threshold_wins() {
            let checked_shift = shift(LocalDate::of(2015, 10, 21), 1, 5.0);
            let dataset = TimeCardData::new()
                .with_employee(employee())
                .with_shifts(vec![
                    shift(LocalDate::of(2015, 10, 21), 1, 5.0),
                    shift(LocalDate::of(2015, 10, 21), 2, 4.0),
                ]);
            let mut distribution =
                HoursDistribution::new(1, LocalDate::of(2015, 10, 21), None, 0.0, 0.0);
            let params = rule_params! { DAYS_THRESHOLD => "2" };

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &rule_item(1, params),
            );

            assert_eq!(distribution.base_rate(), 10.0);
        }

        /// "when shifts for two jobs occur on the same date and the checked
        /// shift of the lower pay rate is on another day, pay the shift's
        /// rate.".
        #[test]
        fn the_checked_shift_only_sees_its_own_days_daily_group() {
            let checked_shift = shift(LocalDate::of(2015, 10, 22), 1, 4.0);
            let dataset = TimeCardData::new()
                .with_employee(employee())
                .with_shifts(vec![
                    shift(LocalDate::of(2015, 10, 21), 1, 5.0),
                    shift(LocalDate::of(2015, 10, 21), 2, 4.0),
                    shift(LocalDate::of(2015, 10, 21), 1, 4.0),
                ]);
            let mut distribution =
                HoursDistribution::new(1, LocalDate::of(2015, 10, 22), None, 0.0, 0.0);
            let params = rule_params! { DAYS_THRESHOLD => "2" };

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &rule_item(1, params),
            );

            assert_eq!(distribution.base_rate(), 5.0);
        }

        /// "when shifts for two jobs occur on the same date and the checked
        /// shift of the higher pay rate is less than threshold hours, pay the
        /// higher rate".
        #[test]
        fn the_checked_shift_can_win_on_its_own_status_even_under_threshold() {
            let checked_shift = shift(LocalDate::of(2015, 10, 21), 2, 3.0);
            let dataset = TimeCardData::new()
                .with_employee(employee())
                .with_shifts(vec![
                    shift(LocalDate::of(2015, 10, 21), 1, 5.0),
                    shift(LocalDate::of(2015, 10, 22), 1, 5.0),
                ]);
            let mut distribution =
                HoursDistribution::new(1, LocalDate::of(2015, 10, 21), None, 0.0, 0.0);
            let params = rule_params! { DAYS_THRESHOLD => "2" };

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &rule_item(1, params),
            );

            assert_eq!(
                distribution.base_rate(),
                10.0,
                "target status itself is the fallback, never filtered by threshold"
            );
        }

        /// "when an employee works a higher paid job for more than two days
        /// in a week, they should be paid for all shifts at that rate".
        #[test]
        fn a_higher_paid_job_meeting_the_weekly_days_threshold_wins() {
            let checked_shift = shift(LocalDate::of(2015, 10, 22), 1, 4.0);
            let dataset = TimeCardData::new()
                .with_employee(employee())
                .with_shifts(vec![
                    shift(LocalDate::of(2015, 10, 20), 2, 5.0),
                    shift(LocalDate::of(2015, 10, 21), 2, 4.0),
                    shift(LocalDate::of(2015, 10, 22), 1, 4.0),
                ]);
            let mut distribution =
                HoursDistribution::new(1, LocalDate::of(2015, 10, 22), None, 0.0, 0.0);

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &rule_item(1, default_params()),
            );

            assert_eq!(distribution.base_rate(), 10.0);
        }

        /// "When three jobs with different pays are worked for the week. Pay
        /// either the weekly rate or the higher rate" — all three `when`/
        /// `then` pairs.
        #[test]
        fn three_jobs_in_one_week_each_priced_by_their_own_days_and_daily_thresholds() {
            let shifts = vec![
                shift(LocalDate::of(2015, 10, 20), 1, 8.0),
                shift(LocalDate::of(2015, 10, 21), 2, 8.0),
                shift(LocalDate::of(2015, 10, 22), 2, 8.0),
                shift(LocalDate::of(2015, 10, 23), 2, 8.0),
                shift(LocalDate::of(2015, 10, 24), 3, 3.0),
            ];
            let dataset = TimeCardData::new()
                .with_employee(employee())
                .with_shifts(shifts);
            let params = rule_params! { DAYS_THRESHOLD => "2" };

            let mut checked = shift(LocalDate::of(2015, 10, 20), 1, 8.0);
            let mut distribution =
                HoursDistribution::new(1, LocalDate::of(2015, 10, 20), None, 0.0, 0.0);
            rule().execute_for_shift(
                &checked,
                &mut distribution,
                &dataset,
                &rule_item(1, params.clone()),
            );
            assert_eq!(distribution.base_rate(), 10.0);

            checked = shift(LocalDate::of(2015, 10, 22), 2, 8.0);
            let mut distribution =
                HoursDistribution::new(1, LocalDate::of(2015, 10, 22), None, 0.0, 0.0);
            rule().execute_for_shift(
                &checked,
                &mut distribution,
                &dataset,
                &rule_item(1, params.clone()),
            );
            assert_eq!(distribution.base_rate(), 10.0);

            checked = shift(LocalDate::of(2015, 10, 24), 3, 3.0);
            let mut distribution =
                HoursDistribution::new(1, LocalDate::of(2015, 10, 24), None, 0.0, 0.0);
            rule().execute_for_shift(&checked, &mut distribution, &dataset, &rule_item(1, params));
            assert_eq!(distribution.base_rate(), 12.0);
        }
    }

    mod separate_job_parity_tests {
        use super::*;

        const PAY_PERIOD_START: fn() -> LocalDate = || LocalDate::of(2025, 12, 22);
        const PAY_PERIOD_END: fn() -> LocalDate = || LocalDate::of(2025, 12, 28);
        const RATE_CHANGE_DATE: fn() -> LocalDate = || LocalDate::of(2025, 12, 25);

        fn rule() -> CombinationJobsRegRateRule<FixedPeriodEnd, EarningTypes> {
            CombinationJobsRegRateRule::new(
                FixedPeriodEnd(PAY_PERIOD_END()),
                regular_earning_types(),
            )
        }

        fn item(must_be_separate_job: Option<&str>) -> RuleItem {
            let mut params = CombinationJobsRegRateRuleConfig.default_values();
            match must_be_separate_job {
                Some(value) => {
                    params.set(HIGHER_CLASSIFICATION_MUST_BE_SEPARATE_JOB, value);
                }
                None => {
                    params = rule_params! { HOURS_THRESHOLD => "4", DAYS_THRESHOLD => "2" };
                }
            }
            rule_item(1, params)
        }

        /// "flag false - same job rate increase mid-period updates shift to
        /// higher rate".
        #[test]
        fn flag_false_same_job_rate_increase_mid_period_updates_shift() {
            let employee = Employee::new(
                100,
                1,
                "",
                vec![
                    job_status(
                        1,
                        10.0,
                        PAY_PERIOD_START(),
                        RATE_CHANGE_DATE().minus_days(1),
                    ),
                    job_status(1, 15.0, RATE_CHANGE_DATE(), PAY_PERIOD_END()),
                ],
            );
            let shift_date = LocalDate::of(2025, 12, 23);
            let checked_shift = shift(shift_date, 1, 8.0);
            let dataset = TimeCardData::new()
                .with_employee(employee)
                .with_shifts(vec![
                    shift(shift_date, 1, 8.0),
                    shift(RATE_CHANGE_DATE(), 1, 8.0),
                ]);
            let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &item(Some("false")),
            );

            assert_eq!(distribution.base_rate(), 15.0);
        }

        /// "flag false - different job with higher rate updates shift".
        #[test]
        fn flag_false_different_job_with_higher_rate_updates_shift() {
            let employee = Employee::new(
                100,
                1,
                "",
                vec![
                    job_status(1, 10.0, PAY_PERIOD_START(), PAY_PERIOD_END()),
                    job_status(2, 18.0, PAY_PERIOD_START(), PAY_PERIOD_END()),
                ],
            );
            let shift_date = LocalDate::of(2025, 12, 23);
            let checked_shift = shift(shift_date, 1, 8.0);
            let dataset = TimeCardData::new()
                .with_employee(employee)
                .with_shifts(vec![shift(shift_date, 1, 8.0), shift(shift_date, 2, 4.0)]);
            let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &item(Some("false")),
            );

            assert_eq!(distribution.base_rate(), 18.0);
        }

        /// "flag true - same job rate increase mid-period does NOT update
        /// shift".
        #[test]
        fn flag_true_same_job_rate_increase_mid_period_does_not_update_shift() {
            let employee = Employee::new(
                100,
                1,
                "",
                vec![
                    job_status(
                        1,
                        10.0,
                        PAY_PERIOD_START(),
                        RATE_CHANGE_DATE().minus_days(1),
                    ),
                    job_status(1, 15.0, RATE_CHANGE_DATE(), PAY_PERIOD_END()),
                ],
            );
            let shift_date = LocalDate::of(2025, 12, 23);
            let checked_shift = shift(shift_date, 1, 8.0);
            let dataset = TimeCardData::new()
                .with_employee(employee)
                .with_shifts(vec![
                    shift(shift_date, 1, 8.0),
                    shift(RATE_CHANGE_DATE(), 1, 8.0),
                ]);
            let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &item(Some("true")),
            );

            assert_eq!(distribution.base_rate(), 10.0);
        }

        /// "flag true - different job with higher rate updates shifts for
        /// other job codes".
        #[test]
        fn flag_true_different_job_with_higher_rate_updates_shift() {
            let employee = Employee::new(
                100,
                1,
                "",
                vec![
                    job_status(1, 10.0, PAY_PERIOD_START(), PAY_PERIOD_END()),
                    job_status(2, 18.0, PAY_PERIOD_START(), PAY_PERIOD_END()),
                ],
            );
            let shift_date = LocalDate::of(2025, 12, 23);
            let checked_shift = shift(shift_date, 1, 8.0);
            let dataset = TimeCardData::new()
                .with_employee(employee)
                .with_shifts(vec![shift(shift_date, 1, 8.0), shift(shift_date, 2, 4.0)]);
            let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &item(Some("true")),
            );

            assert_eq!(distribution.base_rate(), 18.0);
        }

        /// "flag true - single job only keeps its own rate".
        #[test]
        fn flag_true_single_job_only_keeps_its_own_rate() {
            let employee = Employee::new(
                100,
                1,
                "",
                vec![
                    job_status(
                        1,
                        10.0,
                        PAY_PERIOD_START(),
                        RATE_CHANGE_DATE().minus_days(1),
                    ),
                    job_status(1, 15.0, RATE_CHANGE_DATE(), PAY_PERIOD_END()),
                ],
            );
            let shift_date = LocalDate::of(2025, 12, 23);
            let checked_shift = shift(shift_date, 1, 8.0);
            let dataset = TimeCardData::new()
                .with_employee(employee)
                .with_shifts(vec![shift(shift_date, 1, 8.0)]);
            let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &item(Some("true")),
            );

            assert_eq!(distribution.base_rate(), 10.0);
        }

        /// "flag true - target job already has highest rate so no update
        /// occurs".
        #[test]
        fn flag_true_target_already_highest_rate() {
            let employee = Employee::new(
                100,
                1,
                "",
                vec![
                    job_status(1, 20.0, PAY_PERIOD_START(), PAY_PERIOD_END()),
                    job_status(2, 12.0, PAY_PERIOD_START(), PAY_PERIOD_END()),
                ],
            );
            let shift_date = LocalDate::of(2025, 12, 23);
            let checked_shift = shift(shift_date, 1, 8.0);
            let dataset = TimeCardData::new()
                .with_employee(employee)
                .with_shifts(vec![shift(shift_date, 1, 8.0), shift(shift_date, 2, 4.0)]);
            let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &item(Some("true")),
            );

            assert_eq!(distribution.base_rate(), 20.0);
        }

        /// "flag true - earning-based execute does NOT update rate for same
        /// job increase".
        #[test]
        fn flag_true_earning_does_not_update_for_same_job_increase() {
            let employee = Employee::new(
                100,
                1,
                "",
                vec![
                    job_status(
                        1,
                        10.0,
                        PAY_PERIOD_START(),
                        RATE_CHANGE_DATE().minus_days(1),
                    ),
                    job_status(1, 15.0, RATE_CHANGE_DATE(), PAY_PERIOD_END()),
                ],
            );
            let earning_date = LocalDate::of(2025, 12, 23);
            let mut earning =
                EmployeeEarning::new(1, 100, 1, 5, earning_date, 8.0, 0.0, EarningSource::Rule);
            let dataset = TimeCardData::new()
                .with_employee(employee)
                .with_earnings(vec![
                    EmployeeEarning::new(1, 100, 1, 5, earning_date, 8.0, 0.0, EarningSource::Rule),
                    EmployeeEarning::new(
                        2,
                        100,
                        1,
                        5,
                        RATE_CHANGE_DATE(),
                        8.0,
                        0.0,
                        EarningSource::Rule,
                    ),
                ]);

            rule().execute_for_earning(&mut earning, &dataset, &item(Some("true")));

            assert_eq!(earning.rate(), 10.0);
        }

        /// "flag true - earning-based execute updates rate for different job
        /// with higher rate".
        #[test]
        fn flag_true_earning_updates_for_different_job_with_higher_rate() {
            let employee = Employee::new(
                100,
                1,
                "",
                vec![
                    job_status(1, 10.0, PAY_PERIOD_START(), PAY_PERIOD_END()),
                    job_status(2, 18.0, PAY_PERIOD_START(), PAY_PERIOD_END()),
                ],
            );
            let earning_date = LocalDate::of(2025, 12, 23);
            let mut earning =
                EmployeeEarning::new(1, 100, 1, 5, earning_date, 8.0, 0.0, EarningSource::Rule);
            let dataset = TimeCardData::new()
                .with_employee(employee)
                .with_earnings(vec![
                    EmployeeEarning::new(1, 100, 1, 5, earning_date, 8.0, 0.0, EarningSource::Rule),
                    EmployeeEarning::new(2, 100, 2, 5, earning_date, 4.0, 0.0, EarningSource::Rule),
                ]);

            rule().execute_for_earning(&mut earning, &dataset, &item(Some("true")));

            assert_eq!(earning.rate(), 18.0);
        }

        /// "absent parameter behaves same as flag false - same job rate
        /// increase updates shift".
        #[test]
        fn absent_parameter_behaves_like_flag_false() {
            let employee = Employee::new(
                100,
                1,
                "",
                vec![
                    job_status(
                        1,
                        10.0,
                        PAY_PERIOD_START(),
                        RATE_CHANGE_DATE().minus_days(1),
                    ),
                    job_status(1, 15.0, RATE_CHANGE_DATE(), PAY_PERIOD_END()),
                ],
            );
            let shift_date = LocalDate::of(2025, 12, 23);
            let checked_shift = shift(shift_date, 1, 8.0);
            let dataset = TimeCardData::new()
                .with_employee(employee)
                .with_shifts(vec![
                    shift(shift_date, 1, 8.0),
                    shift(RATE_CHANGE_DATE(), 1, 8.0),
                ]);
            let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);

            rule().execute_for_shift(&checked_shift, &mut distribution, &dataset, &item(None));

            assert_eq!(distribution.base_rate(), 15.0);
        }

        /// "flag true - weekly rate path with different job updates shift
        /// across multiple days".
        #[test]
        fn flag_true_weekly_path_with_different_job_updates_shift() {
            let employee = Employee::new(
                100,
                1,
                "",
                vec![
                    job_status(1, 10.0, PAY_PERIOD_START(), PAY_PERIOD_END()),
                    job_status(2, 18.0, PAY_PERIOD_START(), PAY_PERIOD_END()),
                ],
            );
            let checked_shift = shift(LocalDate::of(2025, 12, 24), 1, 8.0);
            let dataset = TimeCardData::new()
                .with_employee(employee)
                .with_shifts(vec![
                    shift(LocalDate::of(2025, 12, 22), 2, 8.0),
                    shift(LocalDate::of(2025, 12, 23), 2, 8.0),
                    shift(LocalDate::of(2025, 12, 24), 1, 8.0),
                ]);
            let mut distribution =
                HoursDistribution::new(1, LocalDate::of(2025, 12, 24), None, 0.0, 0.0);

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &item(Some("true")),
            );

            assert_eq!(distribution.base_rate(), 18.0);
        }

        /// "flag true - weekly rate path with same job split rate does NOT
        /// update shift".
        #[test]
        fn flag_true_weekly_path_with_same_job_split_rate_does_not_update() {
            let employee = Employee::new(
                100,
                1,
                "",
                vec![
                    job_status(
                        1,
                        10.0,
                        PAY_PERIOD_START(),
                        RATE_CHANGE_DATE().minus_days(1),
                    ),
                    job_status(1, 15.0, RATE_CHANGE_DATE(), PAY_PERIOD_END()),
                ],
            );
            let checked_shift = shift(LocalDate::of(2025, 12, 23), 1, 8.0);
            let dataset = TimeCardData::new()
                .with_employee(employee)
                .with_shifts(vec![
                    shift(LocalDate::of(2025, 12, 22), 1, 8.0),
                    shift(LocalDate::of(2025, 12, 23), 1, 8.0),
                    shift(LocalDate::of(2025, 12, 25), 1, 8.0),
                    shift(LocalDate::of(2025, 12, 26), 1, 8.0),
                ]);
            let mut distribution =
                HoursDistribution::new(1, LocalDate::of(2025, 12, 23), None, 0.0, 0.0);

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &item(Some("true")),
            );

            assert_eq!(distribution.base_rate(), 10.0);
        }
    }

    mod split_rate_parity_tests {
        use super::*;

        const PAY_PERIOD_START: fn() -> LocalDate = || LocalDate::of(2025, 12, 22);
        const PAY_PERIOD_END: fn() -> LocalDate = || LocalDate::of(2025, 12, 28);
        const RATE_CHANGE_DATE: fn() -> LocalDate = || LocalDate::of(2025, 12, 25);

        fn rule() -> CombinationJobsRegRateRule<FixedPeriodEnd, EarningTypes> {
            CombinationJobsRegRateRule::new(
                FixedPeriodEnd(PAY_PERIOD_END()),
                regular_earning_types(),
            )
        }

        fn split_employee(job1_new_rate: f64) -> Employee {
            Employee::new(
                100,
                1,
                "",
                vec![
                    job_status(
                        1,
                        10.0,
                        PAY_PERIOD_START(),
                        RATE_CHANGE_DATE().minus_days(1),
                    ),
                    job_status(1, job1_new_rate, RATE_CHANGE_DATE(), PAY_PERIOD_END()),
                    job_status(2, 15.0, PAY_PERIOD_START(), PAY_PERIOD_END()),
                ],
            )
        }

        /// "resolves old rate for shift before rate change date with split
        /// statuses".
        #[test]
        fn resolves_old_rate_for_shift_before_rate_change_date() {
            let shift_date = LocalDate::of(2025, 12, 23);
            let checked_shift = shift(shift_date, 1, 8.0);
            let dataset = TimeCardData::new()
                .with_employee(split_employee(12.0))
                .with_shifts(vec![shift(shift_date, 1, 8.0), shift(shift_date, 2, 4.0)]);
            let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &rule_item(1, CombinationJobsRegRateRuleConfig.default_values()),
            );

            assert_eq!(distribution.base_rate(), 15.0);
        }

        /// "resolves new rate for shift on or after rate change date with
        /// split statuses".
        #[test]
        fn resolves_new_rate_for_shift_on_or_after_rate_change_date() {
            let shift_date = RATE_CHANGE_DATE();
            let checked_shift = shift(shift_date, 1, 8.0);
            let dataset = TimeCardData::new()
                .with_employee(split_employee(12.0))
                .with_shifts(vec![shift(shift_date, 1, 8.0), shift(shift_date, 2, 4.0)]);
            let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &rule_item(1, CombinationJobsRegRateRuleConfig.default_values()),
            );

            assert_eq!(distribution.base_rate(), 15.0);
        }

        /// "uses correct split rate when target job has higher rate after
        /// change".
        #[test]
        fn uses_correct_split_rate_when_target_job_has_higher_rate_after_change() {
            let shift_date = RATE_CHANGE_DATE();
            let checked_shift = shift(shift_date, 1, 8.0);
            let dataset = TimeCardData::new()
                .with_employee(split_employee(20.0))
                .with_shifts(vec![shift(shift_date, 1, 8.0), shift(shift_date, 2, 4.0)]);
            let mut distribution = HoursDistribution::new(1, shift_date, None, 0.0, 0.0);

            rule().execute_for_shift(
                &checked_shift,
                &mut distribution,
                &dataset,
                &rule_item(1, CombinationJobsRegRateRuleConfig.default_values()),
            );

            assert_eq!(distribution.base_rate(), 20.0);
        }

        /// "no split produces identical results regardless of flag value".
        #[test]
        fn no_split_produces_identical_results_regardless_of_date() {
            let flat_employee = || {
                Employee::new(
                    100,
                    1,
                    "",
                    vec![
                        job_status(1, 10.0, PAY_PERIOD_START(), PAY_PERIOD_END()),
                        job_status(2, 15.0, PAY_PERIOD_START(), PAY_PERIOD_END()),
                    ],
                )
            };

            let early_date = LocalDate::of(2025, 12, 23);
            let checked_early = shift(early_date, 1, 8.0);
            let dataset_early = TimeCardData::new()
                .with_employee(flat_employee())
                .with_shifts(vec![shift(early_date, 1, 8.0), shift(early_date, 2, 4.0)]);
            let mut distribution_early = HoursDistribution::new(1, early_date, None, 0.0, 0.0);
            rule().execute_for_shift(
                &checked_early,
                &mut distribution_early,
                &dataset_early,
                &rule_item(1, CombinationJobsRegRateRuleConfig.default_values()),
            );

            let late_date = LocalDate::of(2025, 12, 27);
            let checked_late = shift(late_date, 1, 8.0);
            let dataset_late = TimeCardData::new()
                .with_employee(flat_employee())
                .with_shifts(vec![shift(late_date, 1, 8.0), shift(late_date, 2, 4.0)]);
            let mut distribution_late = HoursDistribution::new(1, late_date, None, 0.0, 0.0);
            rule().execute_for_shift(
                &checked_late,
                &mut distribution_late,
                &dataset_late,
                &rule_item(1, CombinationJobsRegRateRuleConfig.default_values()),
            );

            assert_eq!(
                distribution_early.base_rate(),
                distribution_late.base_rate()
            );
        }
    }
}
