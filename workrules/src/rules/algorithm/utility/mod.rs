//! Helpers the rule families share.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/utility/`.
//!
//! Java's package holds eleven static utility classes reachable from more than
//! one family. They come across one at a time, with the first family that needs
//! one.

pub mod breaks_and_adjustments_calculator;
pub mod hours_distribution_factory;
