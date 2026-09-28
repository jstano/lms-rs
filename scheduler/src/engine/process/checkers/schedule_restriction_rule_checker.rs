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
//! [`ScheduleRestrictionRulePort`]. `runScheduleChangeValidationRules` is still **not ported at
//! all** — its return type (`ScheduleChangeValidationResultDTO`) is an entity this crate hasn't
//! modeled, and inventing that shape without reading it would be guessing an API nobody calls yet.
//!
//! `runStrictRestrictions`/`runNonStrictRestrictions` **are** ported as of Phase 3's `autosched`
//! wave (`ScheduleChecker.getFatalConflicts`/`getOverridableDatasetErrors`, their first real
//! callers) — `EmployeeShiftError` now exists (added for that same wave), unblocking
//! `runNonStrictRestrictions`'s return type. Both stay thin wrappers over
//! [`ScheduleRestrictionRulesPort`], for the same reason `canEmployeeWorkShift` is: the underlying
//! `RuleUtils`/`RuleImplFactory`/`RuleSet`/`RuleItem`/`ScheduleRestrictionRuleImpl` dispatch is
//! still out of scope.

use crate::engine::model::employee_data::EmployeeData;
use crate::engine::model::schedule_model::ScheduleModel;
use crate::engine::process::checkers::abstract_can_work_checker::set_notes_for_employee;
use crate::engine::process::checkers::can_work_checker::CanWorkChecker;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::employee_shift_error::EmployeeShiftError;
use crate::entity::schedule_calc_data_set::ScheduleCalcDataSet;

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

/// `RuleUtils.getRuleSet(...)` + iterating `RuleItem`s through `RuleImplFactory` +
/// `ScheduleRestrictionRuleImpl.canEmployeeWorkShift(...)`, for the `ScheduleCalcDataSet`-based
/// overload `runStrictRestrictions`/`runNonStrictRestrictions` use — a separate port from
/// [`ScheduleRestrictionRulePort`] because it dispatches over a different aggregate
/// (`ScheduleCalcDataSet`, not `ScheduleModel`/`EmployeeData`), same as `SchedulesTimeCardCalculatorPort`'s
/// two methods for two different overloads on two different callers.
pub trait ScheduleRestrictionRulesPort {
    /// `runStrictRestrictions(ScheduleCalcDataSet, EmployeeShift)`.
    fn strict_restriction_messages(
        &self,
        data_set: &ScheduleCalcDataSet,
        employee_shift: &EmployeeShift,
    ) -> Vec<String>;

    /// `runNonStrictRestrictions(ScheduleCalcDataSet, EmployeeShift)`.
    fn non_strict_restriction_errors(
        &self,
        data_set: &ScheduleCalcDataSet,
        employee_shift: &EmployeeShift,
    ) -> Vec<EmployeeShiftError>;
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

/// The `ScheduleCalcDataSet`-based half of `ScheduleRestrictionRuleChecker` —
/// `runStrictRestrictions`/`runNonStrictRestrictions`. Java has one Spring-managed
/// `ScheduleRestrictionRuleChecker` bean exposing both this and the `CanWorkChecker` method above;
/// split into a second small type here since the two existing `CanWorkChecker`-only factories
/// (`RegularScheduleCanWorkCheckerFactory`/`VariableCanWorkCheckerFactory`) never construct a
/// dataset-based rule dependency, and `ScheduleChecker` (Phase 3's `autosched` wave, this type's
/// only caller) never needs [`ScheduleRestrictionRulePort`]'s `ScheduleModel`-based check.
pub struct ScheduleRestrictionRules<'a> {
    rules: &'a dyn ScheduleRestrictionRulesPort,
}

impl<'a> ScheduleRestrictionRules<'a> {
    pub fn new(rules: &'a dyn ScheduleRestrictionRulesPort) -> Self {
        Self { rules }
    }

    /// `runStrictRestrictions(ScheduleCalcDataSet, EmployeeShift)`.
    pub fn run_strict_restrictions(
        &self,
        data_set: &ScheduleCalcDataSet,
        employee_shift: &EmployeeShift,
    ) -> Vec<String> {
        self.rules
            .strict_restriction_messages(data_set, employee_shift)
    }

    /// `runNonStrictRestrictions(ScheduleCalcDataSet, EmployeeShift)`.
    pub fn run_non_strict_restrictions(
        &self,
        data_set: &ScheduleCalcDataSet,
        employee_shift: &EmployeeShift,
    ) -> Vec<EmployeeShiftError> {
        self.rules
            .non_strict_restriction_errors(data_set, employee_shift)
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
