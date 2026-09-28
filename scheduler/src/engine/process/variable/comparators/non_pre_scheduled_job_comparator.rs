//! Port of `com.unifocus.watson.server.scheduler.engine.process.variable.comparators.NonPreScheduledJobComparator`.
//!
//! Ground truth: `taps/.../process/variable/comparators/NonPreScheduledJobComparator.java`.

use crate::engine::model::job_data::JobData;
use std::cmp::Ordering;

/// `NonPreScheduledJobComparator`.
#[derive(Debug, Default, Clone, Copy)]
pub struct NonPreScheduledJobComparator;

impl NonPreScheduledJobComparator {
    /// `compare(JobData, JobData)`.
    pub fn compare(&self, job_data1: &JobData, job_data2: &JobData) -> Ordering {
        job_data1
            .job()
            .full_name()
            .to_lowercase()
            .cmp(&job_data2.job().full_name().to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::assignment::Assignment;

    fn job_data_with_name(id: i32, name: &str) -> JobData {
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
        JobData::new(assignment)
    }

    #[test]
    fn sorts_case_insensitively_by_full_name() {
        let job_data1 = job_data_with_name(1, "zebra");
        let job_data2 = job_data_with_name(2, "Alpha");
        let comparator = NonPreScheduledJobComparator;

        assert_eq!(
            comparator.compare(&job_data1, &job_data2),
            Ordering::Greater
        );
        assert_eq!(comparator.compare(&job_data2, &job_data1), Ordering::Less);
    }
}

/// Transcribed from `NonPreScheduledJobComparatorTest.groovy#testCompare`'s `where:` table (3
/// rows).
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use rstest::rstest;

    fn job_data_with_name(id: i32, name: &str) -> JobData {
        use crate::entity::assignment::Assignment;
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
        JobData::new(assignment)
    }

    #[rstest]
    #[case("AAA", "AAA", Ordering::Equal)]
    #[case("AAA", "BBB", Ordering::Less)]
    #[case("BBB", "AAA", Ordering::Greater)]
    fn test_compare(
        #[case] full_name1: &str,
        #[case] full_name2: &str,
        #[case] expected: Ordering,
    ) {
        let job_data1 = job_data_with_name(1, full_name1);
        let job_data2 = job_data_with_name(2, full_name2);
        let comparator = NonPreScheduledJobComparator;

        assert_eq!(comparator.compare(&job_data1, &job_data2), expected);
    }
}
