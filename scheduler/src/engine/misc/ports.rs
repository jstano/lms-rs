//! DAO boundaries `engine::misc` reaches into that no `engine::io` loader also needs — see
//! `engine::io::ports`'s doc for why a DAO shared with `io/` (like `EmployeeDAO`) stays defined
//! there instead of being duplicated here.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/dao/DayOffPlanDAO.java`,
//! `watson/server/labor/calcshift/SchedulesTimeCardCalculator.java`,
//! `watson/server/labor/calcshift/rules/ScheduleLunchRunner.java`.
//!
//! `SchedulesTimeCardCalculatorPort`/`ScheduleLunchRunnerPort` are the same "external subsystem
//! behind a narrow port" treatment as `PARITY_AUDIT.md` findings 10/16, extended to
//! `CalculateDataSet` (Phase 2 step 6): both wrap real time-card/labor-rule engines out of this
//! crate's scope (`SchedulesTimeCardCalculator` computes overtime distribution; `ScheduleLunchRunner`
//! runs the same shape of lunch-placement rules `workrules::rules::algorithm::schedulelunch`
//! already ports for a different subsystem — no cross-crate dependency, per that finding's
//! precedent).

use crate::entity::day_off_plan::DayOffPlan;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;

/// Day-off rotation plans by property. `DayOffPlanDAO`, narrowed to `findAllForProperty`.
/// `DayOffPlanRotator`'s dependency.
pub trait DayOffPlanPort {
    /// `findAllForProperty(Property)`.
    fn find_all_for_property(&self, property_id: i32) -> Vec<DayOffPlan>;
}

/// `SchedulesTimeCardCalculator.calculateOvertimeForScheduleCalcDataSet(ScheduleCalcDataSet)`.
/// `CalculateDataSet.distributeHoursForDataSet`'s dependency.
pub trait SchedulesTimeCardCalculatorPort {
    fn calculate_overtime_for_schedule_calc_data_set(&self, data_set: &mut ScheduleCalcDataSet);
}

/// `ScheduleLunchRunner.runRules(ScheduleCalcDataSet, EmployeeShift)`.
/// `CalculateDataSet.adjustShiftForLunch`'s dependency.
pub trait ScheduleLunchRunnerPort {
    fn run_rules(&self, data_set: &mut ScheduleCalcDataSet, employee_shift: &mut EmployeeShift);
}
