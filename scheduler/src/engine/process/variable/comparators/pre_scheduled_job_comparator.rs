//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.comparators.PreScheduledJobComparator`.
//!
//! Ground truth: `taps/.../process/variable/comparators/PreScheduledJobComparator.java`.
//!
//! Java compares two `int`s with subtraction (`preScheduleOrderNo1 - preScheduleOrderNo2`); ported
//! as `i32::cmp` instead, which is behaviorally identical for `Comparator` purposes and avoids the
//! (here harmless, but generally unsound) subtraction-overflow idiom.

use crate::engine::model::job_data::JobData;
use std::cmp::Ordering;

/// `PreScheduledJobComparator`. Callers must only pass `JobData` with `pre_schedule_parameters()`
/// set — same precondition as Java's unchecked `getPreScheduleParameters().getOrderNo()`.
#[derive(Debug, Default, Clone, Copy)]
pub struct PreScheduledJobComparator;

impl PreScheduledJobComparator {
    /// `compare(JobData, JobData)`.
    pub fn compare(&self, job_data1: &JobData, job_data2: &JobData) -> Ordering {
        let order_no1 = job_data1
            .pre_schedule_parameters()
            .expect("PreScheduledJobComparator requires pre_schedule_parameters")
            .order_no();
        let order_no2 = job_data2
            .pre_schedule_parameters()
            .expect("PreScheduledJobComparator requires pre_schedule_parameters")
            .order_no();

        match order_no1.cmp(&order_no2) {
            Ordering::Equal => job_data1
                .job()
                .full_name()
                .to_lowercase()
                .cmp(&job_data2.job().full_name().to_lowercase()),
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::model::pre_schedule_parameters::PreScheduleParameters;
    use crate::entity::assignment::Assignment;

    fn job_data_with(id: i32, name: &str, order_no: i32) -> JobData {
        let assignment = Assignment::new(
            id,
            name,
            false,
            None,
            None,
            None,
            false,
            vec![],
            vec![],
            None,
        );
        let mut job_data = JobData::new(assignment);
        job_data.set_pre_schedule_parameters(Some(PreScheduleParameters::new(order_no, 0)));
        job_data
    }

    #[test]
    fn sorts_by_order_no_first() {
        let job_data1 = job_data_with(1, "zebra", 2);
        let job_data2 = job_data_with(2, "alpha", 1);
        let comparator = PreScheduledJobComparator;

        assert_eq!(
            comparator.compare(&job_data1, &job_data2),
            Ordering::Greater
        );
    }

    #[test]
    fn falls_back_to_full_name_when_order_no_ties() {
        let job_data1 = job_data_with(1, "zebra", 5);
        let job_data2 = job_data_with(2, "Alpha", 5);
        let comparator = PreScheduledJobComparator;

        assert_eq!(
            comparator.compare(&job_data1, &job_data2),
            Ordering::Greater
        );
    }
}

/// Transcribed from `PreScheduledJobComparatorTest.groovy#testCompare`'s `where:` table (5 rows).
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use crate::engine::model::pre_schedule_parameters::PreScheduleParameters;
    use crate::entity::assignment::Assignment;
    use rstest::rstest;

    fn job_data_with(id: i32, name: &str, order_no: i32) -> JobData {
        let assignment = Assignment::new(
            id,
            name,
            false,
            None,
            None,
            None,
            false,
            vec![],
            vec![],
            None,
        );
        let mut job_data = JobData::new(assignment);
        job_data.set_pre_schedule_parameters(Some(PreScheduleParameters::new(order_no, 0)));
        job_data
    }

    #[rstest]
    #[case(1, 1, "AAA", "AAA", Ordering::Equal)]
    #[case(1, 1, "AAA", "BBB", Ordering::Less)]
    #[case(1, 1, "BBB", "AAA", Ordering::Greater)]
    #[case(1, 2, "BBB", "AAA", Ordering::Less)]
    #[case(2, 1, "AAA", "BBB", Ordering::Greater)]
    fn test_compare(
        #[case] order_no1: i32,
        #[case] order_no2: i32,
        #[case] full_name1: &str,
        #[case] full_name2: &str,
        #[case] expected: Ordering,
    ) {
        let job_data1 = job_data_with(1, full_name1, order_no1);
        let job_data2 = job_data_with(2, full_name2, order_no2);
        let comparator = PreScheduledJobComparator;

        assert_eq!(comparator.compare(&job_data1, &job_data2), expected);
    }
}
