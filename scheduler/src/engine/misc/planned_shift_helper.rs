//! Port of `com.unifocus.watson.server.scheduler.engine.misc.PlannedShiftHelper`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/misc/
//! PlannedShiftHelper.java`. Takes `job_id` explicitly (see `PlannedShiftCreator`'s doc for why)
//! instead of re-deriving `regularSchedule.getJob(shiftDate)` and looking up its `JobData` — when
//! that lookup comes back empty (an unreachable path here, since the caller's `job_id` always
//! comes from an already-resolved `JobData`), this treats it as "nothing matches", falling
//! straight to creating a new planned shift, rather than Java's NPE on a null `jobData`.

use crate::engine::misc::planned_shift_creator::PlannedShiftCreator;
use crate::engine::misc::planned_shift_matcher::PlannedShiftMatcher;
use crate::engine::model::regular_schedule::RegularSchedule;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::entity::employee::Employee;
use crate::entity::planned_shift::PlannedShift;
use joda_rs::LocalDate;

/// `PlannedShiftHelper`.
pub struct PlannedShiftHelper<'a> {
    planned_shift_matcher: &'a PlannedShiftMatcher,
    planned_shift_creator: &'a PlannedShiftCreator,
}

impl<'a> PlannedShiftHelper<'a> {
    pub fn new(
        planned_shift_matcher: &'a PlannedShiftMatcher,
        planned_shift_creator: &'a PlannedShiftCreator,
    ) -> Self {
        Self {
            planned_shift_matcher,
            planned_shift_creator,
        }
    }

    /// `findOrCreateMatchingPlannedShift(RegularSchedule, LocalDate, ScheduleModel)`.
    pub fn find_or_create_matching_planned_shift(
        &self,
        schedule_model: &ScheduleModel,
        regular_schedule: &RegularSchedule,
        employee: &Employee,
        job_id: i32,
        shift_date: LocalDate,
    ) -> PlannedShift {
        let job_data = schedule_model
            .job_list()
            .and_then(|job_list| job_list.job_data(job_id));

        let matching_planned_shift = job_data.and_then(|job_data| {
            self.planned_shift_matcher.find_matching_planned_shift(
                regular_schedule,
                employee,
                shift_date,
                job_data,
            )
        });

        if let Some(planned_shift) = matching_planned_shift {
            return planned_shift;
        }

        self.planned_shift_creator
            .create_planned_shift_from_regular_schedule(
                regular_schedule,
                employee,
                job_id,
                shift_date,
            )
    }
}
