//! Port of `RegularHoursDistributionRunner`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/calcshift/rules/RegularHoursDistributionRunner.java`.
//!
//! Drives the `regularhoursdistribution` family over a whole time card, one
//! shift at a time:
//!
//! 1. a shift with (persisted) errors, unless it is a schedule, is skipped
//!    entirely — neither branch below touches it;
//! 2. a shift **closed** for editing but whose span crosses the calculation
//!    start date has its regular distributions dated on or after that date
//!    reset to their original hours, and its premium ones dated there
//!    removed — distributions from an earlier period are left alone;
//! 3. a shift **open** for editing has every distribution cleared and the
//!    configured rule re-run, falling back to `REG_HOURS_RHD` on its own
//!    defaults when nothing is configured.
//!
//! # Resolution is the caller's job
//!
//! Java resolves `ruleUtils.getRuleSet(shift.getEmployee(), shift.getJob(),
//! shift.getShiftDate(), REGULAR_HOURS_DISTRIBUTION)` **per shift** — a
//! different employee, job or date can select a different rule set. This
//! crate's `EmployeeShift` does not carry its employee or job assignment (the
//! entity graph is one-way, same reasoning as [`PropertyPort`]), so a fresh
//! per-shift resolution is not reachable from here. [`run_rules`] instead
//! takes one already-resolved rule set for the whole run — faithful to every
//! case in `RegularHoursDistributionRunnerTest.groovy`, which stubs
//! `RuleUtils` unconditionally and so always falls back to the same default
//! regardless of shift.
//!
//! [`run_rules`]: RegularHoursDistributionRunner::run_rules

use crate::entity::employee_shift::EmployeeShift;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::rule_item::RuleItem;
use crate::entity::rule_set::RuleSet;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::regularhoursdistribution::RegularHoursDistributionRule;
use crate::rules::algorithm::regularhoursdistribution::config::RegularHoursOnShiftDateRuleConfig;
use crate::rules::algorithm::regularhoursdistribution::reg_hours_by_day::RegularHoursByDayRule;
use crate::rules::algorithm::regularhoursdistribution::reg_hours_by_work_week::RegularHoursByWorkWeekRule;
use crate::rules::algorithm::regularhoursdistribution::reg_hours_on_shift_date::RegularHoursOnShiftDateRule;
use crate::rules::ports::PropertyPort;
use crate::rules::rule_class::RuleClass;
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;

/// Runs the regular-hours-distribution family over a time card.
/// `RegularHoursDistributionRunner`.
pub struct RegularHoursDistributionRunner<P: PropertyPort> {
    by_day: RegularHoursByDayRule,
    by_work_week: RegularHoursByWorkWeekRule<P>,
}

impl<P: PropertyPort> RegularHoursDistributionRunner<P> {
    /// Build the runner over the port `RegularHoursByWorkWeekRule` needs.
    pub fn new(property: P) -> Self {
        Self {
            by_day: RegularHoursByDayRule,
            by_work_week: RegularHoursByWorkWeekRule::new(property),
        }
    }

    /// `runRules(TimeCard)`.
    pub fn run_rules(&self, time_card: &mut dyn TimeCard, rule_set: Option<&RuleSet>) {
        let calculation_start_date = time_card.calculation_start_date();
        let regular_type_ids = time_card.regular_hours_distribution_type_ids();
        let rule_item = self.rule_item_to_run(rule_set);

        for shift in time_card.shifts_mut() {
            if shift.has_errors()
                && shift.shift_type() != crate::common::enums::shift_type::ShiftType::Schedule
            {
                continue;
            }

            if shift.shift_date() >= calculation_start_date {
                shift.clear_hours_distributions();
                self.run_configured_rule(shift, &rule_item);
            } else if shift_spans_calculation_start_date(shift, calculation_start_date) {
                update_hours_distributions(shift, calculation_start_date, &regular_type_ids);
            }
        }
    }

    /// `runConfiguredRuleForShift` + `runRule`.
    fn run_configured_rule(&self, shift: &mut EmployeeShift, rule_item: &RuleItem) {
        match rule_item.rule_class() {
            RuleClass::RegHoursRhd => RegularHoursOnShiftDateRule.execute(shift, rule_item),
            RuleClass::RegHoursByDayRhd => self.by_day.execute(shift, rule_item),
            RuleClass::RegHoursByWorkWeekRhd => self.by_work_week.execute(shift, rule_item),
            _ => {}
        }
    }

    /// The configured rule set's first item, or `defaultRuleItem()`.
    /// `executeRuleItemAgainstShift`.
    fn rule_item_to_run(&self, rule_set: Option<&RuleSet>) -> RuleItem {
        rule_set
            .filter(|rule_set| !rule_set.is_empty())
            .and_then(|rule_set| rule_set.rule_items().first())
            .cloned()
            .unwrap_or_else(default_rule_item)
    }
}

/// `defaultRuleItem()` — `REG_HOURS_RHD` on its own defaults, with no id (Java
/// leaves `new RuleItem()`'s id at its unset `0`).
fn default_rule_item() -> RuleItem {
    RuleItem::new(
        0,
        0,
        "",
        RuleClass::RegHoursRhd,
        RegularHoursOnShiftDateRuleConfig.default_values(),
    )
}

/// Whether a *closed* shift's span still reaches across the calculation start
/// date. `ActualsTimeCardShiftMethods.shiftSpansCalculationStartDate`.
///
/// Java measures against `getCalculationStartDate(employee)` at midnight;
/// here `calculation_start_date` is already the plain field divergence 24
/// carries, so this stays a free function rather than a `TimeCard` method —
/// nothing else needs it, and keeping it free avoids re-borrowing the card
/// while its shifts are being mutated.
fn shift_spans_calculation_start_date(
    shift: &EmployeeShift,
    calculation_start_date: LocalDate,
) -> bool {
    let calculation_start = calculation_start_date.at_start_of_day();
    shift.has_both_times()
        && shift.start_date_time().unwrap() < calculation_start
        && shift.end_date_time().unwrap() > calculation_start
}

/// `updateHoursDistributions(TimeCard).accept(EmployeeShift)`.
///
/// A distribution dated before `calculation_start_date` is not open for
/// editing and is left exactly as it is — neither reset nor removed, matching
/// `partitionShiftHoursDistributionsByRegularType`'s filter running before the
/// partition.
fn update_hours_distributions(
    shift: &mut EmployeeShift,
    calculation_start_date: LocalDate,
    regular_type_ids: &[i32],
) {
    shift.hours_distributions_mut().retain_mut(|distribution| {
        if distribution.date() < calculation_start_date {
            return true;
        }

        if is_regular_type(distribution, regular_type_ids) {
            distribution.reset_hours_to_original_hours();
            true
        } else {
            false
        }
    });
}

fn is_regular_type(distribution: &HoursDistribution, regular_type_ids: &[i32]) -> bool {
    distribution
        .hours_distribution_type_id()
        .is_some_and(|id| regular_type_ids.contains(&id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::hours_distribution_type::HoursDistributionType;
    use crate::entity::time_card::TimeCardData;
    use joda_rs::LocalDateTime;

    struct NoProperty;

    impl PropertyPort for NoProperty {
        fn period_end_date(&self, _property_id: i32) -> LocalDate {
            LocalDate::of(2010, 1, 1)
        }
    }

    fn runner() -> RegularHoursDistributionRunner<NoProperty> {
        RegularHoursDistributionRunner::new(NoProperty)
    }

    fn today() -> LocalDate {
        LocalDate::of(2010, 1, 8)
    }

    fn at(date: LocalDate, hour: i32, minute: i32) -> LocalDateTime {
        date.at_time(joda_rs::LocalTime::of(hour, minute, 0))
    }

    fn distribution(
        date: LocalDate,
        type_id: i32,
        hours: f64,
        original_hours: f64,
    ) -> HoursDistribution {
        let mut d = HoursDistribution::new(1, date, Some(type_id), hours, 0.0);
        d.set_original_hours(original_hours);
        d
    }

    fn card_with(shifts: Vec<EmployeeShift>, calculation_start_date: LocalDate) -> TimeCardData {
        TimeCardData::new()
            .with_shifts(shifts)
            .with_calculation_start_date(calculation_start_date)
            .with_hours_distribution_types(vec![
                HoursDistributionType::new(1, "Regular", false),
                HoursDistributionType::new(2, "Overtime", true),
            ])
    }

    #[test]
    fn an_open_shift_in_error_is_skipped_but_a_schedule_shift_in_error_is_not() {
        let error_shift = EmployeeShift::new(1, 1, 1, today(), ShiftType::Actual, Vec::new())
            .with_times(Some(at(today(), 0, 0)), Some(at(today(), 8, 0)))
            .with_errors(vec![
                crate::common::enums::shift_error_type::ShiftErrorType::MissingOut,
            ])
            .with_hours_distributions(vec![distribution(today(), 1, 4.0, 4.0)]);

        let schedule_shift = EmployeeShift::new(2, 1, 1, today(), ShiftType::Schedule, Vec::new())
            .with_times(Some(at(today(), 0, 0)), Some(at(today(), 8, 0)))
            .with_net_hours(8.0)
            .with_errors(vec![
                crate::common::enums::shift_error_type::ShiftErrorType::MissingOut,
            ])
            .with_hours_distributions(vec![distribution(today(), 1, 4.0, 4.0)]);

        let mut card = card_with(vec![error_shift, schedule_shift], today());

        runner().run_rules(&mut card, None);

        assert_eq!(
            card.shifts()[0].hours_distributions().len(),
            1,
            "the errored actual shift is untouched"
        );
        assert_eq!(
            card.shifts()[1].hours_distributions().len(),
            1,
            "the errored schedule shift is still open, so it is cleared and rerun"
        );
        assert_eq!(
            card.shifts()[1].hours_distributions()[0].hours(),
            8.0,
            "the rerun rule wrote a fresh distribution off the shift's net hours, not the old one"
        );
    }

    #[test]
    fn an_open_shift_has_its_distributions_cleared_and_rerun() {
        let shift = EmployeeShift::new(1, 1, 1, today(), ShiftType::Actual, Vec::new())
            .with_times(Some(at(today(), 0, 0)), Some(at(today(), 8, 0)))
            .with_net_hours(8.0)
            .with_hours_distributions(vec![distribution(today(), 1, 4.0, 4.0)]);

        let mut card = card_with(vec![shift], today());

        runner().run_rules(&mut card, None);

        let distributions = card.shifts()[0].hours_distributions();
        assert_eq!(distributions.len(), 1);
        assert_eq!(distributions[0].hours(), 8.0);
        assert_eq!(
            distributions[0].hours_distribution_type_id(),
            Some(HoursDistributionType::REGULAR_ID)
        );
    }

    #[test]
    fn a_closed_shift_not_spanning_the_calculation_start_date_is_left_alone() {
        let far_past = today().minus_days(10);
        let shift = EmployeeShift::new(1, 1, 1, far_past, ShiftType::Actual, Vec::new())
            .with_times(Some(at(far_past, 0, 0)), Some(at(far_past, 8, 0)))
            .with_hours_distributions(vec![distribution(far_past, 1, 4.0, 4.0)]);

        let mut card = card_with(vec![shift], today());

        runner().run_rules(&mut card, None);

        assert_eq!(card.shifts()[0].hours_distributions().len(), 1);
        assert_eq!(card.shifts()[0].hours_distributions()[0].hours(), 4.0);
    }

    #[test]
    fn a_closed_shift_spanning_the_calculation_start_date_resets_regular_hours() {
        let yesterday = today().minus_days(1);
        let shift = EmployeeShift::new(1, 1, 1, yesterday, ShiftType::Actual, Vec::new())
            .with_times(Some(at(yesterday, 20, 0)), Some(at(today(), 8, 0)))
            .with_hours_distributions(vec![
                distribution(yesterday, 1, 4.0, 4.0),
                distribution(today(), 1, 4.0, 6.0),
            ]);

        let mut card = card_with(vec![shift], today());

        runner().run_rules(&mut card, None);

        let distributions = card.shifts()[0].hours_distributions();
        assert_eq!(distributions.len(), 2, "nothing is removed");
        assert_eq!(
            distributions[0].hours(),
            4.0,
            "the earlier-period row is untouched"
        );
        assert_eq!(
            distributions[1].hours(),
            6.0,
            "the open regular row is reset to its original hours"
        );
    }

    #[test]
    fn a_closed_shift_spanning_the_calculation_start_date_removes_open_premium_hours() {
        let yesterday = today().minus_days(1);
        let shift = EmployeeShift::new(1, 1, 1, yesterday, ShiftType::Actual, Vec::new())
            .with_times(Some(at(yesterday, 20, 0)), Some(at(today(), 8, 0)))
            .with_hours_distributions(vec![
                distribution(yesterday, 1, 4.0, 4.0),
                distribution(today(), 1, 4.0, 6.0),
                distribution(today(), 2, 2.0, 0.0),
            ]);

        let mut card = card_with(vec![shift], today());

        runner().run_rules(&mut card, None);

        let distributions = card.shifts()[0].hours_distributions();
        assert_eq!(distributions.len(), 2, "the open premium row is removed");
        assert!(
            distributions
                .iter()
                .all(|d| d.hours_distribution_type_id() != Some(2))
        );
    }
}

/// `RegularHoursDistributionRunnerTest.groovy` — every case.
///
/// **The first case's own assertion cannot fire as written.** Its
/// `ruleImplFactory` is a Spock mock configured so `createRuleImpl` returns a
/// mock of `RegularHoursOnShiftDateRuleImpl` — a mock standing in for the
/// rule itself — and Spock's default for an unstubbed void method is a no-op.
/// So the "cleared" open shift's `execute()` call never runs the real rule,
/// never adds a distribution back, and the spec's
/// `openShifts.every { shift -> shift.hoursDistributions.isEmpty() }` passes
/// **because nothing rebuilt the list**, not because the runner leaves an
/// open shift empty — it does not; a real rule always writes one
/// distribution back. The transcription below wires the real
/// [`RegularHoursOnShiftDateRule`] instead and asserts what actually happens:
/// cleared *and* rerun, one fresh distribution off the shift's net hours.
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use crate::common::enums::shift_error_type::ShiftErrorType;
    use crate::common::enums::shift_type::ShiftType;
    use crate::entity::hours_distribution_type::HoursDistributionType;
    use crate::entity::time_card::TimeCardData;
    use joda_rs::LocalDateTime;

    struct NoProperty;

    impl PropertyPort for NoProperty {
        fn period_end_date(&self, _property_id: i32) -> LocalDate {
            LocalDate::of(2010, 1, 1)
        }
    }

    fn runner() -> RegularHoursDistributionRunner<NoProperty> {
        RegularHoursDistributionRunner::new(NoProperty)
    }

    /// `static today = LocalDate.now()`, pinned.
    fn today() -> LocalDate {
        LocalDate::of(2010, 1, 8)
    }

    fn at(date: LocalDate, hour: i32, minute: i32) -> LocalDateTime {
        date.at_time(joda_rs::LocalTime::of(hour, minute, 0))
    }

    fn distribution(
        date: LocalDate,
        type_id: i32,
        hours: f64,
        original_hours: f64,
    ) -> HoursDistribution {
        let mut d = HoursDistribution::new(1, date, Some(type_id), hours, 0.0);
        d.set_original_hours(original_hours);
        d
    }

    /// The pay group's current pay period always starts `today` — matching
    /// the fixture's `PayGroup.currentPayPeriod() >> ArbitraryDateRange.of(today,
    /// today.plusWeeks(1))`.
    fn card_with(shifts: Vec<EmployeeShift>, calculation_start_date: LocalDate) -> TimeCardData {
        TimeCardData::new()
            .with_shifts(shifts)
            .with_calculation_start_date(calculation_start_date)
            .with_hours_distribution_types(vec![
                HoursDistributionType::new(1, "Regular", false),
                HoursDistributionType::new(2, "Overtime", true),
            ])
    }

    #[test]
    fn run_rules_only_clears_distributions_on_open_shifts_that_are_not_in_error() {
        let shift_in_error = EmployeeShift::new(1, 1, 1, today(), ShiftType::Actual, Vec::new())
            .with_times(Some(at(today(), 0, 0)), Some(at(today(), 8, 0)))
            .with_errors(vec![ShiftErrorType::MissingOut])
            .with_hours_distributions(vec![distribution(today(), 1, 4.0, 4.0)]);
        let closed_no_error = EmployeeShift::new(
            2,
            1,
            1,
            today().minus_days(4),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_times(
            Some(at(today().minus_days(4), 0, 0)),
            Some(at(today().minus_days(4), 8, 0)),
        )
        .with_hours_distributions(vec![distribution(today().minus_days(4), 1, 4.0, 4.0)]);
        let open_shift = EmployeeShift::new(3, 1, 1, today(), ShiftType::Schedule, Vec::new())
            .with_times(Some(at(today(), 0, 0)), Some(at(today(), 8, 0)))
            .with_net_hours(8.0)
            .with_errors(vec![ShiftErrorType::MissingOut])
            .with_hours_distributions(vec![distribution(today(), 1, 4.0, 4.0)]);

        let mut card = card_with(vec![shift_in_error, closed_no_error, open_shift], today());

        runner().run_rules(&mut card, None);

        assert_eq!(
            card.shifts()[0].hours_distributions().len(),
            1,
            "the errored actual shift is skipped, not cleared"
        );
        // The real rule always writes a distribution back — see the module
        // note above on why the Groovy's `isEmpty()` cannot be transcribed
        // literally.
        let open_after = card.shifts()[2].hours_distributions();
        assert_eq!(open_after.len(), 1);
        assert_eq!(open_after[0].hours(), 8.0);
        assert_eq!(
            open_after[0].hours_distribution_type_id(),
            Some(HoursDistributionType::REGULAR_ID)
        );
    }

    #[test]
    fn run_rules_should_only_reset_distributions_on_closed_shifts_with_open_distributions() {
        let closed_far_past = EmployeeShift::new(
            1,
            1,
            1,
            today().minus_days(4),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_times(
            Some(at(today().minus_days(4), 0, 0)),
            Some(at(today().minus_days(4), 8, 0)),
        )
        .with_hours_distributions(vec![distribution(today().minus_days(3), 1, 0.0, 0.0)]);
        let closed_spanning = EmployeeShift::new(
            2,
            1,
            1,
            today().minus_days(1),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_times(
            Some(at(today().minus_days(1), 20, 0)),
            Some(at(today(), 8, 0)),
        )
        .with_hours_distributions(vec![
            distribution(today().minus_days(1), 1, 4.0, 4.0),
            distribution(today(), 1, 4.0, 6.0),
            distribution(today(), 1, 2.0, 0.0),
        ]);
        let open_shift = EmployeeShift::new(3, 1, 1, today(), ShiftType::Schedule, Vec::new())
            .with_times(Some(at(today(), 0, 0)), Some(at(today(), 8, 0)))
            .with_errors(vec![ShiftErrorType::MissingOut])
            .with_hours_distributions(vec![distribution(today(), 1, 4.0, 4.0)]);

        let mut card = card_with(vec![closed_far_past, closed_spanning, open_shift], today());

        runner().run_rules(&mut card, None);

        let open_regular_today: Vec<&HoursDistribution> = card.shifts()[1]
            .hours_distributions()
            .iter()
            .filter(|d| d.date() == today() && d.hours_distribution_type_id() == Some(1))
            .collect();
        assert_eq!(open_regular_today.len(), 2, "both today rows survive");
        assert!(
            open_regular_today
                .iter()
                .all(|d| d.hours() == d.original_hours())
        );
    }

    #[test]
    fn run_rules_should_remove_premium_distributions_on_closed_shifts_with_open_distributions() {
        const REGULAR: i32 = 1;
        const PREMIUM: i32 = 2;

        let closed_far_past = EmployeeShift::new(
            1,
            1,
            1,
            today().minus_days(4),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_times(
            Some(at(today().minus_days(4), 0, 0)),
            Some(at(today().minus_days(4), 8, 0)),
        )
        .with_hours_distributions(vec![distribution(
            today().minus_days(4),
            REGULAR,
            0.0,
            0.0,
        )]);
        let closed_spanning = EmployeeShift::new(
            2,
            1,
            1,
            today().minus_days(1),
            ShiftType::Actual,
            Vec::new(),
        )
        .with_times(
            Some(at(today().minus_days(1), 20, 0)),
            Some(at(today(), 8, 0)),
        )
        .with_hours_distributions(vec![
            distribution(today().minus_days(1), REGULAR, 4.0, 4.0),
            distribution(today(), REGULAR, 4.0, 6.0),
            distribution(today(), PREMIUM, 2.0, 0.0),
        ]);
        let open_shift = EmployeeShift::new(3, 1, 1, today(), ShiftType::Schedule, Vec::new())
            .with_times(Some(at(today(), 0, 0)), Some(at(today(), 8, 0)))
            .with_errors(vec![ShiftErrorType::MissingOut])
            .with_hours_distributions(vec![distribution(today(), REGULAR, 4.0, 4.0)]);

        let mut card = card_with(vec![closed_far_past, closed_spanning, open_shift], today());

        runner().run_rules(&mut card, None);

        let spanning_shift_distributions = card.shifts()[1].hours_distributions();
        assert!(
            spanning_shift_distributions
                .iter()
                .all(|d| d.hours_distribution_type_id() != Some(PREMIUM)),
            "the open premium distribution is removed"
        );
        assert_eq!(
            spanning_shift_distributions
                .iter()
                .filter(|d| d.date() < today())
                .count(),
            1,
            "the earlier-period distribution is kept"
        );
    }
}
