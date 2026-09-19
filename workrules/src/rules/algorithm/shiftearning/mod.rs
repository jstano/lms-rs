//! `RuleType::ShiftDifferential` — not yet ported.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/`.
//!
//! The largest of the seven families prioritized for scheduling — 23
//! concrete rules with its own `utility` subpackage, closer in scale to
//! `hoursdistribution` than to any family ported so far, and left for last
//! in the prioritized order so it gets its own "Scoping —" pass rather than
//! being folded into another family's port.
//!
//! [`utility::shift_time_window_utility`] arrived early, out of that order —
//! `schedulelunch::LunchStartTimeAndLengthRuleImpl` needs it and nothing
//! else in `shiftearning` yet exists to depend on.

pub mod utility;
