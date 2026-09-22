//! Port of `com.unifocus.watson.server.scheduler.engine.process.plannedshiftsorters.
//! PlannedShiftSorter`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/
//! plannedshiftsorters/PlannedShiftSorter.java`.
//!
//! The date→shifts map is an ordered `Vec<(LocalDate, Vec<PlannedShift>)>` here rather than a
//! `Map` — Java's two subclass shapes (`LinkedHashMap` for the by-day sorters, `TreeMap` for
//! `Cascade`) are both "ordered map, iterated in insertion/key order"; a `Vec` of pairs expresses
//! that uniformly without needing two different map types.
//!
//! `nullEnding(...)` (from `com.unifocus.tbx.core.ComparatorUtil`) is a nulls-last comparator
//! wrapper. Java applies it twice — once around "compare by assignment", once inside that around
//! "compare by assignment order" — which collapses to one `Option<i32>` comparison here since
//! [`PlannedShift::assignment_order_no`](crate::entity::planned_shift::PlannedShift::assignment_order_no)
//! already flattens the assignment/assignment-order chain (see that field's doc). `None` sorts
//! **last** to match `nullEnding`, the opposite of `Option`'s derived `Ord` (`None` sorts first)
//! — see [`assignment_order_no_nulls_last`].

use crate::engine::model::job_data::JobData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::entity::planned_shift::PlannedShift;
use joda_rs::LocalDate;
use std::cmp::Ordering;

/// `PlannedShiftSorter`.
pub trait PlannedShiftSorter {
    /// `createPlannedShiftDateMap(ScheduleModel, JobData)`.
    fn create_planned_shift_date_map(
        &self,
        schedule_model: &ScheduleModel,
        job_data: &JobData,
    ) -> Vec<(LocalDate, Vec<PlannedShift>)>;

    /// `flattenPlannedShiftDateMap(Map)`.
    fn flatten_planned_shift_date_map(
        &self,
        map: Vec<(LocalDate, Vec<PlannedShift>)>,
    ) -> Vec<PlannedShift>;

    /// `sortPlannedShifts(ScheduleModel, JobData)`.
    fn sort_planned_shifts(
        &self,
        schedule_model: &ScheduleModel,
        job_data: &JobData,
    ) -> Vec<PlannedShift> {
        let mut map = self.create_planned_shift_date_map(schedule_model, job_data);

        for (_, shifts) in map.iter_mut() {
            perform_planned_shift_sorting_for_single_day(shifts);
        }

        self.flatten_planned_shift_date_map(map)
    }
}

/// `performPlannedShiftSortingForSingleDay(List<PlannedShift>)`.
fn perform_planned_shift_sorting_for_single_day(planned_shifts: &mut [PlannedShift]) {
    planned_shifts.sort_by(|shift1, shift2| {
        let assignment_comparison = assignment_order_no_nulls_last(
            shift1.assignment_order_no(),
            shift2.assignment_order_no(),
        );

        if assignment_comparison == Ordering::Equal {
            shift1.start_date_time().cmp(&shift2.start_date_time())
        } else {
            assignment_comparison
        }
    });
}

/// `nullEnding(PlannedShiftSorter::compareAssignments)` composed with
/// `nullEnding(PlannedShiftSorter::compareAssignmentOrders)` — see module doc.
fn assignment_order_no_nulls_last(a: Option<i32>, b: Option<i32>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(x), Some(y)) => x.cmp(&y),
    }
}
