//! Port of `com.unifocus.watson.server.scheduler.engine.process.projectedhoursreducers.
//! DefaultProjectedHoursReducer`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! projectedhoursreducers/DefaultProjectedHoursReducer.java` + its Groovy/Spock test
//! (`DefaultProjectedHoursReducerTest.groovy`) — a no-op body; the test just confirms
//! `jobData`'s projected hours are left untouched.

use crate::engine::model::job_data::JobData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::projectedhoursreducers::projected_hours_reducer::ProjectedHoursReducer;

/// `DefaultProjectedHoursReducer`.
#[derive(Debug, Default)]
pub struct DefaultProjectedHoursReducer;

impl DefaultProjectedHoursReducer {
    pub fn new() -> Self {
        Self
    }
}

impl ProjectedHoursReducer for DefaultProjectedHoursReducer {
    fn reduce_projected_hours(&self, _schedule_model: &ScheduleModel, _job_data: &mut JobData) {}
}

#[cfg(test)]
mod java_parity_tests {
    //! Transcribed from `DefaultProjectedHoursReducerTest.testReduceProjectedHours`
    //! (`taps/src/junit/.../projectedhoursreducers/DefaultProjectedHoursReducerTest.groovy`).

    use super::*;
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_sort_order::AssignmentSortOrder;
    use crate::entity::property::Property;
    use date_range_rs::DateRange;
    use joda_rs::LocalDate;

    fn job() -> Assignment {
        Assignment::new(
            1,
            "Job",
            false,
            None,
            None,
            None,
            false,
            Vec::<AssignmentSortOrder>::new(),
            Vec::new(),
            None,
        )
    }

    #[test]
    fn leaves_projected_hours_untouched() {
        let date = LocalDate::of(2013, 1, 1);
        let range = DateRange::new(date, date);
        let schedule_model = ScheduleModel::new(Property::new(1, range), range);

        let mut job_data = JobData::new(job());
        job_data
            .projected_hours_mut()
            .add_hours_to_date(date, 100.0);

        DefaultProjectedHoursReducer::new().reduce_projected_hours(&schedule_model, &mut job_data);

        assert_eq!(job_data.projected_hours().hours_for_date(date), 100.0);
    }
}
