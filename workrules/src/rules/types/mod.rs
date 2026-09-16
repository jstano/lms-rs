//! Port of `com.unifocus.watson.common.labor.rules.types`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/types/`.
//!
//! The structured values a rule parameter can hold. Most parameters are a
//! number, a flag or a JSON array of ids (see [`json_ids`]); these are the ones
//! with a shape of their own, serialized into the parameter map as JSON.
//!
//! Types arrive with the rules that read them.
//!
//! [`json_ids`]: crate::common::json_ids

pub mod earning_type_pay_map;
pub mod earning_type_pay_set;
