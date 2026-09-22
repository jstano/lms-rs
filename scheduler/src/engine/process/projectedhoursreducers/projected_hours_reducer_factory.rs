//! Port of `com.unifocus.watson.server.scheduler.engine.process.projectedhoursreducers.
//! ProjectedHoursReducerFactory`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! projectedhoursreducers/ProjectedHoursReducerFactory.java` + its Groovy/Spock test
//! (`ProjectedHoursReducerFactoryTest.groovy`), transcribed below. Java lazily builds a
//! `Map<ProjectedHoursReductionMethod, ProjectedHoursReducer>` on first call and caches it on a
//! mutable field — ported as a direct `match` instead, same "don't copy a Java memoized-cache
//! field as a real cache" treatment as `JobData.average_shift_length`
//! (`PARITY_AUDIT.md` finding 4): the map only ever has two fixed entries, so there's nothing
//! the cache buys here that a `match` doesn't already give for free.

use crate::engine::model::job_data::JobData;
use crate::engine::process::projectedhoursreducers::projected_hours_reducer::ProjectedHoursReducer;
use crate::entity::projected_hours_reduction_method::ProjectedHoursReductionMethod;

/// `ProjectedHoursReducerFactory`.
pub struct ProjectedHoursReducerFactory<'a> {
    default_reducer: &'a dyn ProjectedHoursReducer,
    flat_reducer: &'a dyn ProjectedHoursReducer,
    percent_reducer: &'a dyn ProjectedHoursReducer,
}

impl<'a> ProjectedHoursReducerFactory<'a> {
    pub fn new(
        default_reducer: &'a dyn ProjectedHoursReducer,
        flat_reducer: &'a dyn ProjectedHoursReducer,
        percent_reducer: &'a dyn ProjectedHoursReducer,
    ) -> Self {
        Self {
            default_reducer,
            flat_reducer,
            percent_reducer,
        }
    }

    /// `getProjectedHoursReducer(JobData)`.
    pub fn projected_hours_reducer(&self, job_data: &JobData) -> &'a dyn ProjectedHoursReducer {
        match job_data.job().projected_hours_reduction_method() {
            Some(ProjectedHoursReductionMethod::Flat) => self.flat_reducer,
            Some(ProjectedHoursReductionMethod::Percent) => self.percent_reducer,
            None => self.default_reducer,
        }
    }
}

#[cfg(test)]
mod java_parity_tests {
    //! Transcribed from `ProjectedHoursReducerFactoryTest.testGetProjectedHoursReducer`
    //! (`taps/src/junit/.../projectedhoursreducers/ProjectedHoursReducerFactoryTest.groovy`).
    //! The Groovy test asserts reference identity (`== defaultProjectedHoursReducer`); ported as
    //! "invoking the returned reducer marks exactly one spy called" instead, since comparing raw
    //! pointers to a zero-sized `DefaultProjectedHoursReducer` wouldn't reliably distinguish it
    //! from the others.

    use super::*;
    use crate::engine::model::schedule_model::ScheduleModel;
    use crate::entity::assignment::Assignment;
    use crate::entity::assignment_sort_order::AssignmentSortOrder;
    use crate::entity::property::Property;
    use date_range_rs::DateRange;
    use joda_rs::LocalDate;
    use std::cell::Cell;

    #[derive(Default)]
    struct SpyReducer {
        called: Cell<bool>,
    }

    impl ProjectedHoursReducer for SpyReducer {
        fn reduce_projected_hours(&self, _schedule_model: &ScheduleModel, _job_data: &mut JobData) {
            self.called.set(true);
        }
    }

    fn job(reduction_method: Option<ProjectedHoursReductionMethod>) -> Assignment {
        let assignment = Assignment::new(
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
        );

        match reduction_method {
            Some(method) => assignment.with_projected_hours_reduction_method(method),
            None => assignment,
        }
    }

    fn invoke(
        factory: &ProjectedHoursReducerFactory<'_>,
        reduction_method: Option<ProjectedHoursReductionMethod>,
    ) {
        let date = LocalDate::of(2013, 1, 1);
        let range = DateRange::new(date, date);
        let schedule_model = ScheduleModel::new(Property::new(1, range), range);
        let mut job_data = JobData::new(job(reduction_method));

        factory
            .projected_hours_reducer(&job_data)
            .reduce_projected_hours(&schedule_model, &mut job_data);
    }

    #[test]
    fn picks_the_reducer_matching_the_jobs_reduction_method() {
        let default_reducer = SpyReducer::default();
        let flat_reducer = SpyReducer::default();
        let percent_reducer = SpyReducer::default();
        let factory =
            ProjectedHoursReducerFactory::new(&default_reducer, &flat_reducer, &percent_reducer);

        invoke(&factory, None);
        assert!(default_reducer.called.get());
        assert!(!flat_reducer.called.get());
        assert!(!percent_reducer.called.get());

        let default_reducer = SpyReducer::default();
        let flat_reducer = SpyReducer::default();
        let percent_reducer = SpyReducer::default();
        let factory =
            ProjectedHoursReducerFactory::new(&default_reducer, &flat_reducer, &percent_reducer);

        invoke(&factory, Some(ProjectedHoursReductionMethod::Flat));
        assert!(!default_reducer.called.get());
        assert!(flat_reducer.called.get());
        assert!(!percent_reducer.called.get());

        let default_reducer = SpyReducer::default();
        let flat_reducer = SpyReducer::default();
        let percent_reducer = SpyReducer::default();
        let factory =
            ProjectedHoursReducerFactory::new(&default_reducer, &flat_reducer, &percent_reducer);

        invoke(&factory, Some(ProjectedHoursReductionMethod::Percent));
        assert!(!default_reducer.called.get());
        assert!(!flat_reducer.called.get());
        assert!(percent_reducer.called.get());
    }
}
