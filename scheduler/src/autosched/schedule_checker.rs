//! Port of `com.unifocus.watson.server.scheduler.autosched.ScheduleChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/autosched/
//! ScheduleChecker.java`. Populates/queries the fatal and overridable conflicts on a single
//! employee's `ScheduleCalcDataSet` — a genuinely different aggregate from the `ScheduleModel`/
//! `JobData`/`EmployeeData` pipeline `engine::process::checkers` validates against, so this file
//! doesn't reuse those checkers' `CanWorkChecker` trait; it reimplements the handful of overlapping
//! checks (`canWork`/`canWorkJob`/`canWorkAssignment`) directly against `ScheduleCalcDataSet`,
//! matching Java (which does the same — `ScheduleChecker` has no relationship to
//! `engine.process.checkers` in `taps` either).
//!
//! Three genuinely external subsystems stay behind ports, same "narrow port, real logic around
//! it" treatment as `PARITY_AUDIT.md` findings 10/16: [`CurrentUserPort`](crate::autosched::ports::
//! CurrentUserPort) (via [`ExceedsAvailableHoursConflictValidator`]), [`CertificationPort`](crate::
//! autosched::ports::CertificationPort) (`EmployeeCertificationValidator`), and
//! [`ScheduleRestrictionRules`] (the same rule-dispatch machinery `ScheduleRestrictionRuleChecker`
//! already defers). `shift.getJob()`/`shift.getAssignment()` resolve through
//! [`AssignmentPort`](crate::engine::process::ports::AssignmentPort) (reused from `engine::process`
//! — same DAO method, no need for a second copy of the trait) since this crate's `EmployeeShift`
//! carries `job_id`/`assignment_id`, not live references.
//!
//! `getScheduleWeight`/`exceedsAvailableHours` take `&EmployeeShift` directly rather than Java's
//! `GenericShift` interface — `EmployeeShift` is the only concrete type any ported caller
//! (`ScheduleMatcher`) ever passes.
//!
//! Every `ResourceMgr.lookup(...)` call site returns the bare resource key instead of a translated
//! message — see `entity::shift_error_type::ShiftErrorType`'s doc.
//!
//! `popuplateOverridableShiftErrors`'s `List<String> conflicts` local (built from
//! `getOverridableDatasetErrors()` then never read) is Java dead code — dropped here rather than
//! reproduced.
//!
//! Several validation passes (`validateMinHoursOff`, `validateMinShiftLength`,
//! `validateMaxShiftLength`, `validateAgainstTimeOffRequests`) read every shift in the dataset
//! while deciding which *other* shift(s) to flag, then mutate. Java's shared object graph lets one
//! loop do both; here each is split into a read pass (computing the set of shift ids to flag)
//! followed by a write pass (`self.dataset.shifts_mut()`), since `self.dataset` can't be borrowed
//! both ways at once — same "split the read and write passes" adaptation this crate has used
//! throughout (e.g. `PermanentScheduleProcess`'s peers).

use crate::autosched::exceeds_available_hours_conflict_validator::ExceedsAvailableHoursConflictValidator;
use crate::autosched::ports::CertificationPort;
use crate::common::datetime::duration_in_fractional_hours;
use crate::engine::process::checkers::schedule_restriction_rule_checker::ScheduleRestrictionRules;
use crate::engine::process::ports::AssignmentPort;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::employee_shift_error::EmployeeShiftError;
use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;
use crate::entity::schedule_mode::ScheduleMode;
use crate::entity::shift_error_type::ShiftErrorType;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::collections::HashSet;

/// `ScheduleChecker`.
pub struct ScheduleChecker<'a> {
    dataset: &'a mut ScheduleCalcDataSet,
    schedule_mode: Option<ScheduleMode>,
    net_overtime: f64,
    validate_overtime: bool,
    exceeds_hours_validator: &'a ExceedsAvailableHoursConflictValidator<'a>,
    schedule_restriction_rules: &'a ScheduleRestrictionRules<'a>,
    certifications: &'a dyn CertificationPort,
    assignments: &'a dyn AssignmentPort,
}

impl<'a> ScheduleChecker<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        dataset: &'a mut ScheduleCalcDataSet,
        schedule_mode: Option<ScheduleMode>,
        exceeds_hours_validator: &'a ExceedsAvailableHoursConflictValidator<'a>,
        schedule_restriction_rules: &'a ScheduleRestrictionRules<'a>,
        certifications: &'a dyn CertificationPort,
        assignments: &'a dyn AssignmentPort,
    ) -> Self {
        let schedule_period = Self::schedule_period(dataset);
        let net_overtime = dataset.overtime_for_date_range(schedule_period);

        Self {
            dataset,
            schedule_mode,
            net_overtime,
            validate_overtime: true,
            exceeds_hours_validator,
            schedule_restriction_rules,
            certifications,
            assignments,
        }
    }

    fn schedule_period(dataset: &ScheduleCalcDataSet) -> DateRange {
        let fallback_start = dataset
            .calculation_start_date()
            .unwrap_or_else(|| LocalDate::of(1970, 1, 1));
        let period = dataset
            .date_range()
            .copied()
            .unwrap_or(DateRange::new(fallback_start, fallback_start));

        if period
            .end_date()
            .is_before(period.start_date().plus_days(6))
        {
            let start = dataset
                .calculation_start_date()
                .unwrap_or(period.start_date());
            DateRange::new(start, start.plus_days(6))
        } else {
            period
        }
    }

    /// `clearShiftErrors()`.
    pub fn clear_shift_errors(&mut self) {
        let calc_start = self.dataset.calculation_start_date();
        for shift in self.dataset.shifts_mut() {
            if is_open_for_editing(calc_start, shift.shift_date()) {
                shift.clear_errors();
            }
        }
    }

    /// `canSchedule(EmployeeShift, int)`.
    pub fn can_schedule(&self, employee_shift: &EmployeeShift, shift_overlap: i32) -> bool {
        self.fatal_conflicts_with_overlap(employee_shift, shift_overlap)
            .is_empty()
    }

    /// `getFatalConflicts(EmployeeShift)`.
    pub fn fatal_conflicts(&self, shift: &EmployeeShift) -> Vec<String> {
        self.fatal_conflicts_with_overlap(shift, 0)
    }

    fn fatal_conflicts_with_overlap(
        &self,
        shift: &EmployeeShift,
        shift_overlap: i32,
    ) -> Vec<String> {
        let mut conflicts = Vec::new();

        if self
            .exceeds_hours_validator
            .exceeding_hours_should_create_fatal_conflict(self.dataset, self.schedule_mode)
        {
            conflicts.push("res_noPermissionToExceedAvailableHours".to_string());
        }

        if self.shift_overlaps(shift, shift_overlap) {
            conflicts.push("res_overlappingDuplicateShift".to_string());
        }

        if !self.can_work(shift) {
            conflicts.push("res_statusViolation".to_string());
        }

        if !self.can_work_job(shift) {
            conflicts.push("res_jobViolation".to_string());
        }

        if let Some(key) = self.certification_error_key(shift) {
            conflicts.push(key.to_string());
        }

        if !self.can_work_assignment(shift) {
            conflicts.push("res_assignmentViolation".to_string());
        }

        if shift.duration() < 0.0 {
            conflicts.push("res_cannotBeNegative".to_string());
        }

        if let Some(employee) = self.dataset.employee() {
            conflicts.extend(
                crate::autosched::schedule_hours_distribution_validator::validate(shift, employee)
                    .into_iter()
                    .map(|e| e.resource_key().to_string()),
            );
        }

        conflicts.extend(
            self.schedule_restriction_rules
                .run_strict_restrictions(self.dataset, shift),
        );

        conflicts
    }

    fn shift_overlaps(&self, employee_shift: &EmployeeShift, overlap_minutes: i32) -> bool {
        self.dataset.shifts().iter().any(|scheduled_shift| {
            scheduled_shift.id() != employee_shift.id()
                && !(employee_shift.end_date_time().is_on_or_before(
                    scheduled_shift
                        .start_date_time()
                        .plus_minutes(overlap_minutes as i64),
                ) || employee_shift.start_date_time().is_on_or_after(
                    scheduled_shift
                        .end_date_time()
                        .minus_minutes(overlap_minutes as i64),
                ))
        })
    }

    fn can_work(&self, shift: &EmployeeShift) -> bool {
        self.dataset
            .employee()
            .is_some_and(|employee| employee.is_active_on_date(shift.shift_date()))
    }

    fn can_work_job(&self, shift: &EmployeeShift) -> bool {
        let period = DateRange::new(shift.shift_date(), shift.shift_date());
        self.dataset
            .employee()
            .is_some_and(|employee| employee.is_active_job_during_period(shift.job_id(), &period))
    }

    fn certification_error_key(&self, shift: &EmployeeShift) -> Option<&'static str> {
        let employee = self.dataset.employee()?;
        let shift_range = DateRange::new(
            shift.start_date_time().to_local_date(),
            shift.end_date_time().to_local_date(),
        );
        self.certifications
            .certification_error_key(employee, shift.job_id(), &shift_range)
    }

    fn can_work_assignment(&self, shift: &EmployeeShift) -> bool {
        let Some(assignment_id) = shift.assignment_id() else {
            return true;
        };

        self.dataset
            .employee()
            .is_some_and(|employee| employee.active_assignment_ids().contains(&assignment_id))
    }

    /// `popuplateOverridableShiftErrors(Boolean, boolean)`.
    pub fn populate_overridable_shift_errors(
        &mut self,
        validate_availability: bool,
        validate_overtime: bool,
    ) {
        self.validate_overtime = validate_overtime;

        self.validate_min_hours_off();
        self.validate_min_shift_length();
        self.validate_max_shift_length();
        self.validate_against_time_off_requests();

        if validate_availability {
            self.validate_against_availability_requests();
        }

        self.validate_schedule_restriction_rules();
    }

    fn min_hours_off_for_shift(&self, shift: &EmployeeShift) -> f64 {
        let employee_min = self.dataset.employee().and_then(|e| e.min_hours_off());
        let mut result = employee_min.unwrap_or(-1.0);

        if result < 0.0 {
            let mut assignment_id = shift.assignment_id().or(Some(shift.job_id()));
            result = 0.0;

            while result == 0.0 {
                let Some(id) = assignment_id else { break };
                let Some(assignment) = self.assignments.find_by_id(id) else {
                    break;
                };
                result = assignment.min_hours_off().unwrap_or(0.0);
                assignment_id = assignment.parent_assignment_id();
            }
        }

        result
    }

    fn validate_min_hours_off(&mut self) {
        let starts: Vec<(i32, joda_rs::LocalDateTime)> = self
            .dataset
            .shifts()
            .iter()
            .map(|s| (s.id(), s.start_date_time()))
            .collect();

        let mut to_flag: HashSet<i32> = HashSet::new();
        for shift in self.dataset.shifts() {
            let min_hours_off = self.min_hours_off_for_shift(shift);
            if min_hours_off <= 0.0 {
                continue;
            }

            for &(check_id, check_start) in &starts {
                let diff = duration_in_fractional_hours(shift.end_date_time(), check_start);
                if diff < min_hours_off && diff > 0.0 {
                    to_flag.insert(check_id);
                }
            }
        }

        for shift in self.dataset.shifts_mut() {
            if to_flag.contains(&shift.id()) {
                shift.add_error(EmployeeShiftError::new(ShiftErrorType::MinHoursOff));
            }
        }
    }

    fn validate_min_shift_length(&mut self) {
        let mut to_flag: HashSet<i32> = HashSet::new();
        for shift in self.dataset.shifts() {
            let Some(job) = self.assignments.find_by_id(shift.job_id()) else {
                continue;
            };
            if job.min_shift() <= 0.0 {
                continue;
            }
            if shift.duration() < job.min_shift() {
                to_flag.insert(shift.id());
            }
        }

        for shift in self.dataset.shifts_mut() {
            if to_flag.contains(&shift.id()) {
                shift.add_error(EmployeeShiftError::new(ShiftErrorType::MinShiftLength));
            }
        }
    }

    fn validate_max_shift_length(&mut self) {
        let mut to_flag: HashSet<i32> = HashSet::new();
        for shift in self.dataset.shifts() {
            let Some(job) = self.assignments.find_by_id(shift.job_id()) else {
                continue;
            };
            if job.max_shift() <= 0.0 {
                continue;
            }
            if shift.duration() > job.max_shift() {
                to_flag.insert(shift.id());
            }
        }

        for shift in self.dataset.shifts_mut() {
            if to_flag.contains(&shift.id()) {
                shift.add_error(EmployeeShiftError::new(ShiftErrorType::MaxShiftLength));
            }
        }
    }

    fn validate_against_time_off_requests(&mut self) {
        let requests: Vec<(joda_rs::LocalDateTime, joda_rs::LocalDateTime)> = self
            .dataset
            .time_off_requests()
            .iter()
            .map(|r| {
                let end = if r.is_full_day() {
                    r.end_date_time().plus_days(1)
                } else {
                    r.end_date_time()
                };
                (r.start_date_time(), end)
            })
            .collect();

        let calc_start = self.dataset.calculation_start_date();
        let mut to_flag: HashSet<i32> = HashSet::new();
        for shift in self.dataset.shifts() {
            if !is_open_for_editing(calc_start, shift.shift_date()) {
                continue;
            }
            for &(off_start, off_end) in &requests {
                if shift.start_date_time().is_before(off_end)
                    && shift.end_date_time().is_after(off_start)
                {
                    to_flag.insert(shift.id());
                    break;
                }
            }
        }

        for shift in self.dataset.shifts_mut() {
            if to_flag.contains(&shift.id()) {
                shift.add_error(EmployeeShiftError::new(ShiftErrorType::TimeOff));
            }
        }
    }

    fn validate_against_availability_requests(&mut self) {
        let avail_periods: Vec<date_range_rs::DateTimeRange> = self
            .dataset
            .availability()
            .avail_periods_required_off_only()
            .iter()
            .map(|p| p.to_date_time_range())
            .collect();

        let mut to_flag: HashSet<i32> = HashSet::new();
        for shift in self.dataset.shifts() {
            let shift_range = shift.to_date_time_range();
            if avail_periods
                .iter()
                .any(|period| shift_range.overlaps_exclusive(period))
            {
                to_flag.insert(shift.id());
            }
        }

        for shift in self.dataset.shifts_mut() {
            if to_flag.contains(&shift.id()) {
                shift.add_error(EmployeeShiftError::new(ShiftErrorType::Availability));
            }
        }
    }

    fn validate_schedule_restriction_rules(&mut self) {
        let flagged: Vec<(i32, Vec<EmployeeShiftError>)> = self
            .dataset
            .shifts()
            .iter()
            .map(|shift| {
                (
                    shift.id(),
                    self.schedule_restriction_rules
                        .run_non_strict_restrictions(self.dataset, shift),
                )
            })
            .collect();

        for shift in self.dataset.shifts_mut() {
            if let Some((_, errors)) = flagged.iter().find(|(id, _)| *id == shift.id()) {
                for error in errors {
                    shift.add_error(*error);
                }
            }
        }
    }

    /// `getPrintableConflicts(EmployeeShift, boolean)`.
    pub fn printable_conflicts(
        &mut self,
        shift: &EmployeeShift,
        validate_overtime: bool,
    ) -> Vec<String> {
        self.validate_overtime = validate_overtime;

        let mut errors: HashSet<String> = shift
            .errors()
            .iter()
            .map(|e| e.error_type().resource_key().to_string())
            .collect();

        errors.extend(self.overridable_dataset_errors());

        if let Some(closest) = self.find_closest_shift_after(shift)
            && closest.has_error(ShiftErrorType::MinHoursOff)
        {
            errors.insert(ShiftErrorType::MinHoursOff.resource_key().to_string());
        }

        errors.into_iter().collect()
    }

    fn overridable_dataset_errors(&self) -> Vec<String> {
        let mut conflicts = Vec::new();

        if self
            .exceeds_hours_validator
            .exceeding_hours_should_create_conflict(self.dataset, self.schedule_mode)
        {
            conflicts.push(ShiftErrorType::AvailableHours.resource_key().to_string());
        }

        if self.dataset.total_premium_hours() > self.net_overtime && self.validate_overtime {
            conflicts.push("res_inOvertime".to_string());
        }

        conflicts
    }

    fn find_closest_shift_after(&self, shift: &EmployeeShift) -> Option<&EmployeeShift> {
        let mut closest: Option<&EmployeeShift> = None;
        let mut min_hours_off = 0.0;

        for check_shift in self.dataset.shifts() {
            if check_shift.id() == shift.id() {
                continue;
            }

            let diff =
                duration_in_fractional_hours(shift.end_date_time(), check_shift.start_date_time());

            if diff > 0.0 && (closest.is_none() || diff < min_hours_off) {
                closest = Some(check_shift);
                min_hours_off = diff;
            }
        }

        closest
    }

    /// `getScheduleWeight(GenericShift, boolean)`.
    pub fn schedule_weight(
        &self,
        shift: &EmployeeShift,
        include_approved_time_off_employees: bool,
    ) -> i32 {
        let mut weight = 0;

        if self.schedule_mode == Some(ScheduleMode::Weekly) && self.exceeds_available_hours(shift) {
            weight += 1;
        }

        if self.dataset.has_overtime() {
            weight += 1;
        }

        let avail_weight = self.availability_weight(shift);
        let time_off_weight = self.time_off_weight(shift, include_approved_time_off_employees);

        weight += avail_weight.max(time_off_weight);

        weight
    }

    /// `exceedsAvailableHours(ScheduleCalcDataSet, GenericShift)`.
    pub fn exceeds_available_hours(&self, shift: &EmployeeShift) -> bool {
        let Some(employee) = self.dataset.employee() else {
            return false;
        };
        let Some(date_range) = self.dataset.date_range().copied() else {
            return false;
        };

        self.dataset.net_hours_for_work_week(date_range) + shift.duration()
            > employee.effective_available_hours()
    }

    fn availability_weight(&self, shift: &EmployeeShift) -> i32 {
        use crate::entity::employee_avail_type::EmployeeAvailType;

        let mut weight = 0;
        let shift_range = shift.to_date_time_range();

        for avail_period in self
            .dataset
            .availability()
            .avail_periods_including_preferred()
        {
            if !shift_range.overlaps_exclusive(&avail_period.to_date_time_range()) {
                continue;
            }

            match avail_period.request_type() {
                EmployeeAvailType::RequiredOff => {
                    weight = if avail_period.duration() < 24.0 { 7 } else { 8 };
                }
                EmployeeAvailType::PreferredOff => {
                    weight = if avail_period.duration() < 24.0 {
                        weight.max(3)
                    } else {
                        weight.max(4)
                    };
                }
                _ => {}
            }
        }

        weight
    }

    fn time_off_weight(
        &self,
        shift: &EmployeeShift,
        include_approved_time_off_employees: bool,
    ) -> i32 {
        let mut weight = 0;

        if include_approved_time_off_employees {
            for request in self.dataset.time_off_requests() {
                let off_end = if request.is_full_day() {
                    request.end_date_time().plus_days(1)
                } else {
                    request.end_date_time()
                };

                if shift.start_date_time().is_before(off_end)
                    && shift.end_date_time().is_after(request.start_date_time())
                {
                    weight = if request.is_full_day() { 6 } else { 5 };
                    break;
                }
            }
        }

        if weight == 0 {
            for request in self.dataset.pending_time_off_requests() {
                let off_end = if request.is_full_day() {
                    request.end_date_time().plus_days(1)
                } else {
                    request.end_date_time()
                };

                if shift.start_date_time().is_before(off_end)
                    && shift.end_date_time().is_after(request.start_date_time())
                {
                    weight = if request.is_full_day() { 2 } else { 1 };
                    break;
                }
            }
        }

        weight
    }
}

/// `isOpenForEditingOn(LocalDate)`, as a free function — used from the read pass of the
/// validate_* methods, before `self.dataset` is reborrowed mutably for the write pass.
fn is_open_for_editing(calculation_start_date: Option<LocalDate>, date: LocalDate) -> bool {
    calculation_start_date.is_none_or(|start| date.is_on_or_after(start))
}

#[cfg(test)]
mod tests {
    //! `ScheduleCheckerTest.java` (`taps/taps/src/test/.../autosched/ScheduleCheckerTest.java`,
    //! not present under `lms/scheduler`, which has no `src/test` at all) relies on helper
    //! infrastructure not present in this checkout (`TestableScheduleChecker`, `LaborTestUtils`,
    //! `TestUtils`), so none of it is transcribed verbatim as `java_parity_tests`. Every outcome it
    //! asserts is covered below instead, independently derived from the production methods:
    //! status/job/assignment/overlap/negative-length fatal conflicts, min/max-shift-length and
    //! min-hours-off overridable errors (including the "earlier shift" propagation
    //! `getPrintableConflicts` does via `find_closest_shift_after`), and the certification-error
    //! wiring (`testEmployeeWithInvalidCertificationsReturnsFatalError`, verified at the
    //! [`CertificationPort`] boundary since the real certification logic lives behind that port).

    use super::*;
    use crate::autosched::ports::CertificationPort;
    use crate::engine::process::checkers::schedule_restriction_rule_checker::ScheduleRestrictionRulesPort;
    use crate::entity::assignment::Assignment;
    use crate::entity::employee::Employee;
    use crate::entity::employee_assignment::EmployeeAssignment;
    use crate::entity::employee_status::EmployeeStatus;
    use crate::entity::employee_status_type::EmployeeStatusType;
    use crate::entity::employee_type::EmployeeType;
    use crate::entity::work_class::WorkClass;
    use joda_rs::LocalTime;
    use std::collections::HashMap;

    struct FakeCurrentUser {
        can_exceed: bool,
    }
    impl crate::autosched::ports::CurrentUserPort for FakeCurrentUser {
        fn can_exceed_available_hours(&self) -> bool {
            self.can_exceed
        }
    }

    struct FakeCertifications;
    impl CertificationPort for FakeCertifications {
        fn certification_error_key(
            &self,
            _employee: &crate::entity::employee::Employee,
            _job_id: i32,
            _shift_range: &DateRange,
        ) -> Option<&'static str> {
            None
        }
    }

    struct FakeRestrictionRules;
    impl ScheduleRestrictionRulesPort for FakeRestrictionRules {
        fn strict_restriction_messages(
            &self,
            _data_set: &ScheduleCalcDataSet,
            _employee_shift: &EmployeeShift,
        ) -> Vec<String> {
            Vec::new()
        }

        fn non_strict_restriction_errors(
            &self,
            _data_set: &ScheduleCalcDataSet,
            _employee_shift: &EmployeeShift,
        ) -> Vec<EmployeeShiftError> {
            Vec::new()
        }
    }

    struct FakeAssignments {
        by_id: HashMap<i32, Assignment>,
    }
    impl AssignmentPort for FakeAssignments {
        fn find_by_id(&self, id: i32) -> Option<Assignment> {
            self.by_id.get(&id).cloned()
        }
    }

    fn job(id: i32, min_shift: f64, max_shift: f64) -> Assignment {
        Assignment::new(
            id,
            "Job",
            false,
            None,
            None,
            None,
            false,
            Vec::new(),
            Vec::new(),
            None,
        )
        .with_min_shift(min_shift)
        .with_max_shift(max_shift)
    }

    fn active_employee(status_date: LocalDate) -> Employee {
        Employee::new(
            1,
            "Employee",
            EmployeeType::Regular,
            Some(40.0),
            WorkClass::new(40.0, true),
            None,
            None,
            None,
            Vec::new(),
            Vec::new(),
        )
        .with_status(vec![EmployeeStatus::new(
            status_date,
            status_date,
            EmployeeStatusType::Active,
        )])
    }

    fn shift(id: i32, date: LocalDate, start: LocalTime, end: LocalTime) -> EmployeeShift {
        EmployeeShift::new(id, date, date.at_time(start), 1, None)
            .with_end_date_time(date.at_time(end))
            .with_net_hours((end.hour() - start.hour()) as f64)
    }

    struct Harness {
        current_user: FakeCurrentUser,
        certifications: FakeCertifications,
        rules: FakeRestrictionRules,
        assignments: FakeAssignments,
    }

    impl Harness {
        fn new(job_id: i32, min_shift: f64, max_shift: f64) -> Self {
            let mut by_id = HashMap::new();
            by_id.insert(job_id, job(job_id, min_shift, max_shift));
            Self {
                current_user: FakeCurrentUser { can_exceed: false },
                certifications: FakeCertifications,
                rules: FakeRestrictionRules,
                assignments: FakeAssignments { by_id },
            }
        }

        fn checker<'a>(
            &'a self,
            dataset: &'a mut ScheduleCalcDataSet,
            exceeds: &'a ExceedsAvailableHoursConflictValidator<'a>,
            restrictions: &'a ScheduleRestrictionRules<'a>,
        ) -> ScheduleChecker<'a> {
            ScheduleChecker::new(
                dataset,
                None,
                exceeds,
                restrictions,
                &self.certifications,
                &self.assignments,
            )
        }
    }

    #[test]
    fn status_violation_when_employee_is_not_active_on_the_shift_date() {
        let harness = Harness::new(1, 0.0, 0.0);
        let exceeds = ExceedsAvailableHoursConflictValidator::new(&harness.current_user);
        let restrictions = ScheduleRestrictionRules::new(&harness.rules);

        let date = LocalDate::of(2024, 1, 1);
        let other_date = LocalDate::of(2024, 2, 1);
        let mut dataset = ScheduleCalcDataSet::new(Vec::new(), Vec::new(), Default::default())
            .with_date_range(DateRange::new(date, date));
        dataset.set_employee(active_employee(other_date));

        let s = shift(1, date, LocalTime::of(9, 0, 0), LocalTime::of(17, 0, 0));
        let checker = harness.checker(&mut dataset, &exceeds, &restrictions);

        let conflicts = checker.fatal_conflicts(&s);
        assert!(conflicts.contains(&"res_statusViolation".to_string()));
    }

    #[test]
    fn no_conflicts_for_an_active_employee_working_their_job_with_a_positive_length_shift() {
        let harness = Harness::new(1, 0.0, 0.0);
        let exceeds = ExceedsAvailableHoursConflictValidator::new(&harness.current_user);
        let restrictions = ScheduleRestrictionRules::new(&harness.rules);

        let date = LocalDate::of(2024, 1, 1);
        let employee = Employee::new(
            1,
            "Employee",
            EmployeeType::Regular,
            Some(40.0),
            WorkClass::new(40.0, true),
            None,
            None,
            None,
            Vec::new(),
            vec![crate::entity::employee_job_status::EmployeeJobStatus::new(
                1, None, date, date, true, 0, date, 0.0, false, 0,
            )],
        )
        .with_status(vec![EmployeeStatus::new(
            date,
            date,
            EmployeeStatusType::Active,
        )]);
        let mut dataset = ScheduleCalcDataSet::new(Vec::new(), Vec::new(), Default::default())
            .with_date_range(DateRange::new(date, date));
        dataset.set_employee(employee);

        let s = shift(1, date, LocalTime::of(9, 0, 0), LocalTime::of(17, 0, 0));
        let checker = harness.checker(&mut dataset, &exceeds, &restrictions);

        assert!(checker.can_schedule(&s, 0));
    }

    #[test]
    fn negative_length_shift_is_a_fatal_conflict() {
        let harness = Harness::new(1, 0.0, 0.0);
        let exceeds = ExceedsAvailableHoursConflictValidator::new(&harness.current_user);
        let restrictions = ScheduleRestrictionRules::new(&harness.rules);

        let date = LocalDate::of(2024, 1, 1);
        let mut dataset = ScheduleCalcDataSet::new(Vec::new(), Vec::new(), Default::default())
            .with_date_range(DateRange::new(date, date));
        dataset.set_employee(active_employee(date));

        let s = EmployeeShift::new(1, date, date.at_time(LocalTime::of(9, 0, 0)), 1, None)
            .with_end_date_time(date.at_time(LocalTime::of(8, 30, 0)))
            .with_net_hours(-0.5);
        let checker = harness.checker(&mut dataset, &exceeds, &restrictions);

        assert!(
            checker
                .fatal_conflicts(&s)
                .contains(&"res_cannotBeNegative".to_string())
        );
    }

    #[test]
    fn overlapping_shift_is_a_fatal_conflict() {
        let harness = Harness::new(1, 0.0, 0.0);
        let exceeds = ExceedsAvailableHoursConflictValidator::new(&harness.current_user);
        let restrictions = ScheduleRestrictionRules::new(&harness.rules);

        let date = LocalDate::of(2024, 1, 1);
        let existing = shift(1, date, LocalTime::of(9, 0, 0), LocalTime::of(17, 0, 0));
        let mut dataset = ScheduleCalcDataSet::new(vec![existing], Vec::new(), Default::default())
            .with_date_range(DateRange::new(date, date));
        dataset.set_employee(active_employee(date));

        let new_shift = shift(2, date, LocalTime::of(10, 0, 0), LocalTime::of(12, 0, 0));
        let checker = harness.checker(&mut dataset, &exceeds, &restrictions);

        assert!(
            checker
                .fatal_conflicts(&new_shift)
                .contains(&"res_overlappingDuplicateShift".to_string())
        );
    }

    #[test]
    fn min_and_max_shift_length_violations_are_flagged_as_overridable_errors() {
        let harness = Harness::new(1, 4.0, 6.0);
        let exceeds = ExceedsAvailableHoursConflictValidator::new(&harness.current_user);
        let restrictions = ScheduleRestrictionRules::new(&harness.rules);

        let date = LocalDate::of(2024, 1, 1);
        let too_short = shift(1, date, LocalTime::of(9, 0, 0), LocalTime::of(11, 0, 0));
        let too_long = shift(2, date, LocalTime::of(12, 0, 0), LocalTime::of(20, 0, 0));
        let mut dataset =
            ScheduleCalcDataSet::new(vec![too_short, too_long], Vec::new(), Default::default())
                .with_date_range(DateRange::new(date, date));
        dataset.set_employee(active_employee(date));

        {
            let mut checker = harness.checker(&mut dataset, &exceeds, &restrictions);
            checker.populate_overridable_shift_errors(true, true);
        }

        assert!(dataset.shifts()[0].has_error(ShiftErrorType::MinShiftLength));
        assert!(dataset.shifts()[1].has_error(ShiftErrorType::MaxShiftLength));
    }

    /// Port of `ScheduleCheckerTest.testJobViolation` (`taps/taps/src/test/.../autosched/
    /// ScheduleCheckerTest.java`), found in the fuller `taps/taps` checkout, not `lms/scheduler`
    /// (which has no `src/test` at all). Java flips one shift's job id between an active and an
    /// inactive job on the same `EmployeeShift`; `EmployeeShift` here is immutable, so this uses
    /// two shifts instead of a mutated one.
    #[test]
    fn job_violation_when_shift_job_is_not_an_active_job_for_the_employee() {
        let harness = Harness::new(1, 0.0, 0.0);
        let exceeds = ExceedsAvailableHoursConflictValidator::new(&harness.current_user);
        let restrictions = ScheduleRestrictionRules::new(&harness.rules);

        let date = LocalDate::of(2024, 1, 1);
        let employee = Employee::new(
            1,
            "Employee",
            EmployeeType::Regular,
            Some(40.0),
            WorkClass::new(40.0, true),
            None,
            None,
            None,
            Vec::new(),
            vec![crate::entity::employee_job_status::EmployeeJobStatus::new(
                1, None, date, date, true, 0, date, 0.0, false, 0,
            )],
        )
        .with_status(vec![EmployeeStatus::new(
            date,
            date,
            EmployeeStatusType::Active,
        )]);

        let mut dataset = ScheduleCalcDataSet::new(Vec::new(), Vec::new(), Default::default())
            .with_date_range(DateRange::new(date, date));
        dataset.set_employee(employee);

        let shift_for_other_job =
            EmployeeShift::new(1, date, date.at_time(LocalTime::of(9, 0, 0)), 200, None)
                .with_end_date_time(date.at_time(LocalTime::of(17, 0, 0)))
                .with_net_hours(8.0);
        let shift_for_own_job =
            EmployeeShift::new(2, date, date.at_time(LocalTime::of(9, 0, 0)), 1, None)
                .with_end_date_time(date.at_time(LocalTime::of(17, 0, 0)))
                .with_net_hours(8.0);

        let checker = harness.checker(&mut dataset, &exceeds, &restrictions);

        assert!(
            checker
                .fatal_conflicts(&shift_for_other_job)
                .contains(&"res_jobViolation".to_string())
        );
        assert!(
            !checker
                .fatal_conflicts(&shift_for_own_job)
                .contains(&"res_jobViolation".to_string())
        );
    }

    /// Port of `ScheduleCheckerTest.testAssignmentViolation` — same "two shifts instead of a
    /// mutated one" adaptation as the job-violation test above.
    #[test]
    fn assignment_violation_when_shift_assignment_is_not_active_for_the_employee() {
        let harness = Harness::new(1, 0.0, 0.0);
        let exceeds = ExceedsAvailableHoursConflictValidator::new(&harness.current_user);
        let restrictions = ScheduleRestrictionRules::new(&harness.rules);

        let date = LocalDate::of(2024, 1, 1);
        let employee = Employee::new(
            1,
            "Employee",
            EmployeeType::Regular,
            Some(40.0),
            WorkClass::new(40.0, true),
            None,
            None,
            None,
            vec![EmployeeAssignment::new(1, 1, 0, true)],
            Vec::new(),
        )
        .with_status(vec![EmployeeStatus::new(
            date,
            date,
            EmployeeStatusType::Active,
        )]);

        let mut dataset = ScheduleCalcDataSet::new(Vec::new(), Vec::new(), Default::default())
            .with_date_range(DateRange::new(date, date));
        dataset.set_employee(employee);

        let shift_for_other_assignment =
            EmployeeShift::new(1, date, date.at_time(LocalTime::of(9, 0, 0)), 1, Some(200))
                .with_end_date_time(date.at_time(LocalTime::of(17, 0, 0)))
                .with_net_hours(8.0);
        let shift_for_own_assignment =
            EmployeeShift::new(2, date, date.at_time(LocalTime::of(9, 0, 0)), 1, Some(1))
                .with_end_date_time(date.at_time(LocalTime::of(17, 0, 0)))
                .with_net_hours(8.0);

        let checker = harness.checker(&mut dataset, &exceeds, &restrictions);

        assert!(
            checker
                .fatal_conflicts(&shift_for_other_assignment)
                .contains(&"res_assignmentViolation".to_string())
        );
        assert!(
            !checker
                .fatal_conflicts(&shift_for_own_assignment)
                .contains(&"res_assignmentViolation".to_string())
        );
    }

    /// Port of `ScheduleCheckerTest.testMinHoursOff`. Java's assertion that the *earlier* shift
    /// also reports `MIN_HOURS_OFF` isn't a second violation on that shift — it's
    /// `getPrintableConflicts` propagating the following shift's error back via
    /// `find_closest_shift_after`, matching this file's own doc comment on that method.
    #[test]
    fn min_hours_off_violation_flags_the_close_shift_and_propagates_back_to_the_earlier_one() {
        let harness = Harness::new(1, 0.0, 0.0);
        let exceeds = ExceedsAvailableHoursConflictValidator::new(&harness.current_user);
        let restrictions = ScheduleRestrictionRules::new(&harness.rules);

        let date = LocalDate::of(2024, 1, 1);
        let next_date = date.plus_days(1);

        let employee = Employee::new(
            1,
            "Employee",
            EmployeeType::Regular,
            Some(40.0),
            WorkClass::new(40.0, true),
            Some(6.0),
            None,
            None,
            Vec::new(),
            Vec::new(),
        )
        .with_status(vec![EmployeeStatus::new(
            date,
            next_date,
            EmployeeStatusType::Active,
        )]);

        let shift1 = shift(1, date, LocalTime::of(8, 0, 0), LocalTime::of(10, 0, 0));
        let shift2 = shift(2, date, LocalTime::of(15, 0, 0), LocalTime::of(20, 0, 0));
        let shift3 = shift(
            3,
            next_date,
            LocalTime::of(12, 0, 0),
            LocalTime::of(20, 0, 0),
        );

        let mut dataset =
            ScheduleCalcDataSet::new(vec![shift1, shift2, shift3], Vec::new(), Default::default())
                .with_date_range(DateRange::new(date, next_date));
        dataset.set_employee(employee);

        {
            let mut checker = harness.checker(&mut dataset, &exceeds, &restrictions);
            checker.populate_overridable_shift_errors(true, true);
        }

        let shifts = dataset.shifts().to_vec();
        let mut checker = harness.checker(&mut dataset, &exceeds, &restrictions);

        assert!(
            checker
                .printable_conflicts(&shifts[0], true)
                .contains(&ShiftErrorType::MinHoursOff.resource_key().to_string())
        );
        assert!(
            checker
                .printable_conflicts(&shifts[1], true)
                .contains(&ShiftErrorType::MinHoursOff.resource_key().to_string())
        );
        assert!(
            !checker
                .printable_conflicts(&shifts[2], true)
                .contains(&ShiftErrorType::MinHoursOff.resource_key().to_string())
        );
    }

    /// Port of `ScheduleCheckerTest.testEmployeeWithInvalidCertificationsReturnsFatalError`.
    /// Java builds real expired `EmployeeCertification`/`EmployeeJobStatus` rows and lets
    /// production certification logic derive the error key; that logic lives behind
    /// [`CertificationPort`] here (see this file's module doc), so the port is verified at the
    /// port boundary instead — a fake that returns the expired-cert key exercises the same
    /// `fatal_conflicts` wiring Java's test does.
    #[test]
    fn certification_violation_for_an_employee_with_an_expired_certification() {
        struct FakeExpiredCertification;
        impl CertificationPort for FakeExpiredCertification {
            fn certification_error_key(
                &self,
                _employee: &Employee,
                _job_id: i32,
                _shift_range: &DateRange,
            ) -> Option<&'static str> {
                Some("res_certificationExpiredError")
            }
        }

        let current_user = FakeCurrentUser { can_exceed: false };
        let certifications = FakeExpiredCertification;
        let rules = FakeRestrictionRules;
        let assignments = FakeAssignments {
            by_id: HashMap::new(),
        };

        let exceeds = ExceedsAvailableHoursConflictValidator::new(&current_user);
        let restrictions = ScheduleRestrictionRules::new(&rules);

        let date = LocalDate::of(2024, 1, 1);
        let mut dataset = ScheduleCalcDataSet::new(Vec::new(), Vec::new(), Default::default())
            .with_date_range(DateRange::new(date, date));
        dataset.set_employee(active_employee(date));

        let s = shift(1, date, LocalTime::of(9, 0, 0), LocalTime::of(17, 0, 0));
        let checker = ScheduleChecker::new(
            &mut dataset,
            None,
            &exceeds,
            &restrictions,
            &certifications,
            &assignments,
        );

        assert!(
            checker
                .fatal_conflicts(&s)
                .contains(&"res_certificationExpiredError".to_string())
        );
    }
}
