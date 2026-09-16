//! Port of `EarningTypePayMap`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/common/labor/rules/types/EarningTypePayMap.java`.
//!
//! One regular earning type and the premium earning types it escalates to, in
//! level order: index 0 is overtime, index 1 double time. A rule that pays
//! premium *earnings* rather than premium hours distributions looks up the
//! replacement type here.
//!
//! # The list is positional, and may be short
//!
//! `getPremiumEarningTypeIDs().get(premiumLevel)` indexes straight into the
//! list, so a property that configured an overtime type but no double-time one
//! throws `IndexOutOfBoundsException` the first time anybody works past the
//! double-time limit. See
//! [`EarningTypePaySet::premium_earning_type_id`](super::earning_type_pay_set::EarningTypePaySet::premium_earning_type_id)
//! for what this port does instead.

use serde_json::{Map, Value};

/// A regular earning type and its premium escalations. `EarningTypePayMap`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EarningTypePayMap {
    regular_earning_type_id: i32,
    premium_earning_type_ids: Vec<i32>,
    code: Option<String>,
}

impl EarningTypePayMap {
    /// Map a regular earning type onto its premium types, lowest level first.
    pub fn new(regular_earning_type_id: i32, premium_earning_type_ids: Vec<i32>) -> Self {
        Self {
            regular_earning_type_id,
            premium_earning_type_ids,
            code: None,
        }
    }

    /// Attach the optional code. `setCode`.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// `getRegularEarningTypeID()`.
    pub fn regular_earning_type_id(&self) -> i32 {
        self.regular_earning_type_id
    }

    /// The premium types by level — index 0 overtime, index 1 double time.
    /// `getPremiumEarningTypeIDs()`.
    pub fn premium_earning_type_ids(&self) -> &[i32] {
        &self.premium_earning_type_ids
    }

    /// `getCode()`, which Java leaves null when the JSON omits it.
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Read one element of `earningTypePayMapSet`. `setJSONObject`.
    ///
    /// Java's `getInt`/`getJSONArray` throw on a missing or mistyped key; this
    /// reports the failure instead, and the set's parser turns it into the same
    /// refusal to read the parameter at all.
    pub fn from_json(object: &Value) -> Option<Self> {
        let regular_earning_type_id = json_int(object.get("regularEarningTypeID")?)?;

        let mut premium_earning_type_ids = Vec::new();
        for element in object.get("premiumEarningTypeIDs")?.as_array()? {
            premium_earning_type_ids.push(json_int(element)?);
        }

        Some(Self {
            regular_earning_type_id,
            premium_earning_type_ids,
            // `obj.has("code") ? obj.getString("code") : null`.
            code: object
                .get("code")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
    }

    /// `getJSONObject()`.
    ///
    /// A null code is left out: org.json's `put(String, Object)` removes the
    /// key rather than storing a null, so Java's own round trip drops it too.
    pub fn to_json(&self) -> Value {
        let mut object = Map::new();
        object.insert(
            "regularEarningTypeID".to_string(),
            Value::from(self.regular_earning_type_id),
        );
        object.insert(
            "premiumEarningTypeIDs".to_string(),
            Value::from(self.premium_earning_type_ids.clone()),
        );
        if let Some(code) = &self.code {
            object.insert("code".to_string(), Value::from(code.clone()));
        }
        Value::Object(object)
    }
}

/// `JSONArray.getInt`/`JSONObject.getInt`: a number truncated toward zero, or a
/// string that parses as one. The same coercions
/// [`json_ids`](crate::common::json_ids) reproduces.
fn json_int(value: &Value) -> Option<i32> {
    match value {
        Value::Number(number) => number.as_f64().map(|value| value as i32),
        Value::String(text) => text
            .parse::<i32>()
            .ok()
            .or_else(|| text.parse::<f64>().ok().map(|value| value as i32)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Option<EarningTypePayMap> {
        EarningTypePayMap::from_json(&serde_json::from_str(json).unwrap())
    }

    #[test]
    fn a_pay_map_reads_its_regular_type_and_premium_levels() {
        let map = parse(r#"{"regularEarningTypeID":1,"premiumEarningTypeIDs":[2,3]}"#).unwrap();

        assert_eq!(map.regular_earning_type_id(), 1);
        assert_eq!(map.premium_earning_type_ids(), &[2, 3]);
        assert_eq!(map.code(), None);
    }

    #[test]
    fn the_code_is_optional() {
        let map =
            parse(r#"{"regularEarningTypeID":1,"premiumEarningTypeIDs":[],"code":"OT"}"#).unwrap();

        assert_eq!(map.code(), Some("OT"));
    }

    #[test]
    fn a_missing_required_key_does_not_parse() {
        assert!(parse(r#"{"premiumEarningTypeIDs":[2]}"#).is_none());
        assert!(parse(r#"{"regularEarningTypeID":1}"#).is_none());
        assert!(parse(r#"{"regularEarningTypeID":1,"premiumEarningTypeIDs":2}"#).is_none());
    }

    #[test]
    fn getint_coerces_strings_and_decimals() {
        let map = parse(r#"{"regularEarningTypeID":"1","premiumEarningTypeIDs":[2.0]}"#).unwrap();

        assert_eq!(map.regular_earning_type_id(), 1);
        assert_eq!(map.premium_earning_type_ids(), &[2]);
    }

    #[test]
    fn a_pay_map_round_trips_through_json() {
        let map = EarningTypePayMap::new(1, vec![2, 3]).with_code("OT");

        assert_eq!(EarningTypePayMap::from_json(&map.to_json()), Some(map));
    }

    #[test]
    fn a_null_code_is_left_out_of_the_json() {
        let json = EarningTypePayMap::new(1, vec![2, 3]).to_json();

        assert!(json.get("code").is_none());
    }
}
