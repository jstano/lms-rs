//! The rule algorithms, one module per family.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/`.
//!
//! Thirty-two families, 225 rules. Each family defines its own rule trait —
//! Java's `RuleImpl` is an empty marker and every family declares its own
//! `execute` shape beneath it.

pub mod doubletimerate;
pub mod earningrate;
pub mod hoursdistribution;
pub mod overtimerate;
pub mod punchrounding;
pub mod punchvalidation;
pub mod regularhoursdistribution;
pub mod regularrate;
pub mod single_distribution_type_config;
pub mod utility;
