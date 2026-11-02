//! Port of `com.unifocus.watson.server.labor.rules.algorithm.shiftearning.utility`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftearning/utility/`.
//!
//! Shared helpers several `shiftearning` rules use, the same role
//! `hoursdistribution`'s shared-helper set plays for that family. Besides
//! [`shift_time_window_utility`], this holds two helpers that live at the root of
//! Java's `algorithm` package but are used only by this family:
//! [`holiday_matcher`] and [`shift_time_in_period_calculator`].

pub mod holiday_matcher;
pub mod shift_time_in_period_calculator;
pub mod shift_time_window_utility;
