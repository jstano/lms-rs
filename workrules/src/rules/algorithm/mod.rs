//! The rule algorithms, one module per family.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/`.
//!
//! Thirty-two families, 225 rules. Each family defines its own rule trait —
//! Java's `RuleImpl` is an empty marker and every family declares its own
//! `execute` shape beneath it.

pub mod hoursdistribution;
pub mod punchrounding;
pub mod punchvalidation;
pub mod utility;
