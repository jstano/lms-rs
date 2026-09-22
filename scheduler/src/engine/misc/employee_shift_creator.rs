//! Port of `com.unifocus.watson.server.scheduler.engine.misc.EmployeeShiftCreator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/misc/
//! EmployeeShiftCreator.java`. Only `createShift(Employee, PlannedShift, ShiftCategory, int)` is
//! ported — the only overload `PreScheduleProcess` (Phase 2 step 6, this wave's caller) uses;
//! it's inlined directly against `PlannedShift`'s fields rather than also porting the more
//! general `createShift(Employee, Assignment, Assignment, DateTimeRange, ShiftCategory, int)`
//! overload it delegates through in Java, since nothing else calls that one yet either.
//!
//! `createPunch`/the `EmployeeShiftPunch`s it builds, `createdOnDT` (`DTUtil.now()`), and
//! `shiftType`/`source` (fixed constants) are not modeled — no ported call site reads any of them
//! back yet (same treatment as `entity::employee_shift::EmployeeShift`'s other unmodeled
//! fields).
//!
//! The `shiftCategory == null` fallback to `job.getProperty().getDefaultShiftCategory()` is not
//! implemented — `Property.default_shift_category` isn't modeled in this crate, and every call
//! site ported so far (`PreScheduleProcess` always passes `preSchedule.getShiftCategory()`
//! straight through) makes the fallback unreachable. Flagging rather than silently assuming a
//! future caller without a category will hit it safely — it currently just keeps `None`.

use crate::entity::employee::Employee;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::planned_shift::PlannedShift;

/// `EmployeeShiftCreator`.
pub struct EmployeeShiftCreator;

impl EmployeeShiftCreator {
    /// `createShift(Employee, PlannedShift, ShiftCategory, int)`.
    pub fn create_shift(
        &self,
        employee: &Employee,
        planned_shift: PlannedShift,
        shift_category_id: Option<i32>,
        flags: i32,
    ) -> EmployeeShift {
        let date_time_range = planned_shift.to_date_time_range();

        EmployeeShift::new(
            0,
            planned_shift.shift_date(),
            date_time_range.start(),
            planned_shift.job_id(),
            planned_shift.assignment_id(),
        )
        .with_end_date_time(date_time_range.end())
        .with_employee_id(employee.id())
        .with_flags(flags)
        .with_shift_category_id(shift_category_id)
        .with_planned_shift(planned_shift)
    }
}
