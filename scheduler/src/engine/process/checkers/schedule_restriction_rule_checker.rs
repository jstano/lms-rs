//! Port of `com.unifocus.watson.server.scheduler.engine.process.checkers.
//! ScheduleRestrictionRuleChecker`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/process/checkers/
//! ScheduleRestrictionRuleChecker.java`. Decided with the user ahead of this wave
//! (`PARITY_AUDIT.md` finding 10): `RuleUtils`/`RuleImplFactory`/`RuleSet`/`RuleItem`/
//! `ScheduleRestrictionRuleImpl` are the same shape of rule-dispatch machinery `workrules` ports
//! (which already has a `schedulerestriction` rule family, 5 of 6 rules, done — see
//! `workrules/src/PARITY_AUDIT.md`), but for a rule type not itself in `workrules`'s or
//! `scheduler`'s scope, and the two crates deliberately share no dependency. Only the
//! `CanWorkChecker` method (`canEmployeeWorkShift`) is ported, as a thin wrapper over
//! [`ScheduleRestrictionRulePort`]. The other three public methods on the Java class
//! (`runStrictRestrictions`, `runScheduleChangeValidationRules`, `runNonStrictRestrictions`) are
//! **not ported at all** — their return types (`Set<EmployeeShiftError>`,
//! `ScheduleChangeValidationResultDTO`) are entities this crate hasn't modeled, and inventing
//! that shape without reading them would be guessing an API nobody calls yet. None of the
//! Phase 1 checkers call them.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::abstract_can_work_checker::set_notes_for_employee;
use crate::engine::process::checkers::can_work_checker::CanWorkChecker;
use crate::entity::employee_shift::EmployeeShift;

/// The result of running an employee/shift against a property's `SCHEDULE_RESTRICTION` rule set.
/// `ScheduleRestrictionResult` (only the two fields `canEmployeeWorkShift` reads).
pub struct ScheduleRestrictionCheckResult {
    pub ok: bool,
    pub message: Option<String>,
}

/// `RuleUtils.getRuleSet(...)` + iterating `RuleItem`s through `RuleImplFactory` +
/// `ScheduleRestrictionRuleImpl.canEmployeeWorkShift(...)`, collapsed into one call — nothing
/// ported so far needs the intermediate `RuleSet`/`RuleItem` shape on its own.
pub trait ScheduleRestrictionRulePort {
    fn check(
        &self,
        employee_data: &EmployeeData,
        employee_shift: &EmployeeShift,
    ) -> ScheduleRestrictionCheckResult;
}

/// `ScheduleRestrictionRuleChecker`.
pub struct ScheduleRestrictionRuleChecker<'a> {
    rules: &'a dyn ScheduleRestrictionRulePort,
}

impl<'a> ScheduleRestrictionRuleChecker<'a> {
    pub fn new(rules: &'a dyn ScheduleRestrictionRulePort) -> Self {
        Self { rules }
    }
}

impl CanWorkChecker for ScheduleRestrictionRuleChecker<'_> {
    fn can_employee_work_shift(
        &self,
        schedule_model: &mut ScheduleModel,
        employee_data: &mut EmployeeData,
        employee_shift: &EmployeeShift,
    ) -> bool {
        let result = self.rules.check(employee_data, employee_shift);

        if !result.ok {
            set_notes_for_employee(
                schedule_model,
                employee_data.employee().id(),
                result.message.unwrap_or_default(),
            );
        }

        result.ok
    }
}
