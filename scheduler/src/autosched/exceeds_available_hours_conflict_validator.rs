//! Port of `com.unifocus.watson.server.scheduler.autosched.ExceedsAvailableHoursConflictValidator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/autosched/
//! ExceedsAvailableHoursConflictValidator.java`. `CurrentUser.canAccessItem(SecurityItem.
//! EXCEED_AVAILABLE_HOURS)` is a real permissions check against a live security context — nothing
//! this crate models anywhere — so it's the one call behind [`CurrentUserPort`]; everything else
//! (the weekly-mode gate, the available-hours comparison) is real logic against
//! `ScheduleCalcDataSet`/`Employee`, not stubbed.

use crate::autosched::ports::CurrentUserPort;
use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
use crate::entity::schedule_mode::ScheduleMode;

/// `ExceedsAvailableHoursConflictValidator`.
pub struct ExceedsAvailableHoursConflictValidator<'a> {
    current_user: &'a dyn CurrentUserPort,
}

impl<'a> ExceedsAvailableHoursConflictValidator<'a> {
    pub fn new(current_user: &'a dyn CurrentUserPort) -> Self {
        Self { current_user }
    }

    /// `exceedingHoursShouldCreateFatalConflict(ScheduleCalcDataSet, ScheduleMode)`.
    pub fn exceeding_hours_should_create_fatal_conflict(
        &self,
        data_set: &ScheduleCalcDataSet,
        schedule_mode: Option<ScheduleMode>,
    ) -> bool {
        let in_weekly_schedule_mode = schedule_mode == Some(ScheduleMode::Weekly);
        let user_can_exceed_hours = self.current_user.can_exceed_available_hours();
        let exceeds_available_hours = self.exceeds_available_hours(data_set);

        in_weekly_schedule_mode && exceeds_available_hours && !user_can_exceed_hours
    }

    /// `exceedingHoursShouldCreateConflict(ScheduleCalcDataSet, ScheduleMode)`.
    pub fn exceeding_hours_should_create_conflict(
        &self,
        data_set: &ScheduleCalcDataSet,
        schedule_mode: Option<ScheduleMode>,
    ) -> bool {
        let in_weekly_schedule_mode = schedule_mode == Some(ScheduleMode::Weekly);

        in_weekly_schedule_mode && self.exceeds_available_hours(data_set)
    }

    fn exceeds_available_hours(&self, data_set: &ScheduleCalcDataSet) -> bool {
        let Some(employee) = data_set.employee() else {
            return false;
        };

        data_set.total_distribution_hours() > employee.effective_available_hours()
    }
}

#[cfg(test)]
mod tests {
    //! No Groovy/Java test file was found under `taps/.../autosched/` for this class; cases below
    //! are derived directly from the production method (same shape as `PARITY_AUDIT.md`'s
    //! findings on other undertested `autosched`/`engine` classes).

    use super::*;
    use crate::entity::employee::Employee;
    use crate::entity::employee_shift::EmployeeShift;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::hours_distribution::HoursDistribution;
    use crate::entity::work_class::WorkClass;
    use date_range_rs::DateRange;
    use joda_rs::{LocalDate, LocalTime};

    struct FakeCurrentUser {
        can_exceed: bool,
    }

    impl CurrentUserPort for FakeCurrentUser {
        fn can_exceed_available_hours(&self) -> bool {
            self.can_exceed
        }
    }

    fn data_set_exceeding_hours() -> ScheduleCalcDataSet {
        let date = LocalDate::of(2024, 1, 1);
        let employee = Employee::new(
            1,
            "Employee",
            EmployeeType::Regular,
            Some(10.0),
            WorkClass::new(40.0, true),
            None,
            None,
            None,
            Vec::new(),
            Vec::new(),
        );
        let shift = EmployeeShift::new(1, date, date.at_time(LocalTime::of(9, 0, 0)), 1, None)
            .with_hours_distributions(vec![HoursDistribution::new(date, 12.0, false)]);

        let mut data_set = ScheduleCalcDataSet::new(vec![shift], Vec::new(), Default::default())
            .with_date_range(DateRange::new(date, date));
        data_set.set_employee(employee);
        data_set
    }

    #[test]
    fn fatal_conflict_only_in_weekly_mode_when_exceeding_and_user_cannot_override() {
        let validator =
            ExceedsAvailableHoursConflictValidator::new(&FakeCurrentUser { can_exceed: false });
        let data_set = data_set_exceeding_hours();

        assert!(
            validator.exceeding_hours_should_create_fatal_conflict(
                &data_set,
                Some(ScheduleMode::Weekly)
            )
        );
        assert!(!validator.exceeding_hours_should_create_fatal_conflict(&data_set, None));
    }

    #[test]
    fn fatal_conflict_suppressed_when_user_can_override() {
        let validator =
            ExceedsAvailableHoursConflictValidator::new(&FakeCurrentUser { can_exceed: true });
        let data_set = data_set_exceeding_hours();

        assert!(
            !validator.exceeding_hours_should_create_fatal_conflict(
                &data_set,
                Some(ScheduleMode::Weekly)
            )
        );
    }

    #[test]
    fn overridable_conflict_ignores_the_user_override_permission() {
        let validator =
            ExceedsAvailableHoursConflictValidator::new(&FakeCurrentUser { can_exceed: true });
        let data_set = data_set_exceeding_hours();

        assert!(
            validator.exceeding_hours_should_create_conflict(&data_set, Some(ScheduleMode::Weekly))
        );
    }
}
