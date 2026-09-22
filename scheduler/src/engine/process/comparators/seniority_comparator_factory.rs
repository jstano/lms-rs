//! Port of `com.unifocus.watson.server.scheduler.engine.process.comparators.
//! SeniorityComparatorFactory`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/comparators/
//! SeniorityComparatorFactory.java`. Java's map lookup can return `null` for an unmapped
//! `JCSortOrderType` (NPEs at the call site); [`JcSortOrderType`] only has the eight variants
//! this factory maps, so the lookup here is exhaustive and infallible instead — see that enum's
//! doc for why the variant set might be incomplete.

use crate::engine::process::comparators::assignment_order_comparator::AssignmentOrderComparator;
use crate::engine::process::comparators::assignment_rank_comparator::AssignmentRankComparator;
use crate::engine::process::comparators::default_seniority_comparator::DefaultSeniorityComparator;
use crate::engine::process::comparators::employee_type_comparator::EmployeeTypeComparator;
use crate::engine::process::comparators::full_time_comparator::FullTimeComparator;
use crate::engine::process::comparators::hire_date_comparator::HireDateComparator;
use crate::engine::process::comparators::job_rank_comparator::JobRankComparator;
use crate::engine::process::comparators::job_seniority_date_comparator::JobSeniorityDateComparator;
use crate::engine::process::comparators::seniority_comparator::SeniorityComparator;
use crate::entity::jc_sort_order_type::JcSortOrderType;

/// `SeniorityComparatorFactory`.
pub struct SeniorityComparatorFactory;

impl SeniorityComparatorFactory {
    /// `getComparator(JCSortOrderType)`.
    pub fn comparator(sort_type: JcSortOrderType) -> &'static dyn SeniorityComparator {
        match sort_type {
            JcSortOrderType::AssignmentOrder => &AssignmentOrderComparator,
            JcSortOrderType::AssignmentRank => &AssignmentRankComparator,
            JcSortOrderType::Default => &DefaultSeniorityComparator,
            JcSortOrderType::EmployeeType => &EmployeeTypeComparator,
            JcSortOrderType::Fulltime => &FullTimeComparator,
            JcSortOrderType::HireDate => &HireDateComparator,
            JcSortOrderType::SkillDate => &JobSeniorityDateComparator,
            JcSortOrderType::SkillRank => &JobRankComparator,
        }
    }
}
