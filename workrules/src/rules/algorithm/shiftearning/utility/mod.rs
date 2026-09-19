//! Port of `com.unifocus.watson.server.labor.rules.algorithm.shiftearning.utility`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/utility/`.
//!
//! Shared helpers several `shiftearning` rules use, the same role
//! `hoursdistribution`'s shared-helper set plays for that family. Only
//! [`shift_time_window_utility`] is ported so far — see the family's own
//! module doc for why.

pub mod shift_time_window_utility;
