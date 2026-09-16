//! The runners that drive each family.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/calcshift/rules/`
//! and `taps/src/java/com/unifocus/watson/server/labor/rules/runner/`.
//!
//! A runner resolves the rule set for a `(employee, job, date, RuleType)`,
//! orders the rule items, and invokes each rule against each target entity —
//! falling back to a hardcoded default rule where nothing is configured.
//!
//! There is no single engine entry point above them:
//! `ActualsTimeCardCalculator` sequences about fourteen families in one fixed
//! pipeline, and the rest are driven from their own subsystems.

pub mod punch_rounding;
