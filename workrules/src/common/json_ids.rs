//! Port of `com.unifocus.tbx.core.JSONUtils`'s id-list half.
//!
//! Ground truth:
//! `~/workspace/unifocus/legacy-tbx-10.0/legacy-tbx-core/src/main/java/com/unifocus/tbx/core/JSONUtils.java`.
//!
//! Rules carry multi-select settings — which earning types, which holiday
//! types, which jobs — as a JSON array of ids in a single parameter string:
//! `"[1,2,3]"`. Thirty-six call sites across the tree, all of this one shape.
//!
//! # Bad JSON is empty, not an error
//!
//! `getIdList` catches `JSONException`, prints the stack trace, and returns
//! `Collections.EMPTY_LIST`. So a malformed parameter silently configures the
//! rule with nothing selected rather than aborting the calculation — the
//! opposite of the choice `RuleParams::int_at` makes for a malformed number
//! (divergence 9), and faithful to each.
//!
//! A missing key is the same: `getIdsListForKey` passes `null` through to
//! `new JSONArray(null)`, which throws and is caught.
//!
//! Only the rest of `JSONUtils` — object building, `collectionToJSONArray` —
//! stays out; nothing in the rules tree writes JSON.

use crate::rules::params::RuleParams;

/// The id list a rule parameter holds. `getIdsListForKey(String, Map)`.
///
/// A missing, empty or malformed value is an empty list.
pub fn ids_for_key(key: &str, params: &RuleParams) -> Vec<i32> {
    params.get(key).map(id_list).unwrap_or_default()
}

/// Parse `"[1,2,3]"`. `getIdList(String)`.
///
/// Java reads each element with `JSONArray.getInt`, which coerces a quoted
/// `"1"` and a `1.0` to `1` and throws on anything else — and the throw is
/// caught one frame up, discarding the **whole** list rather than the bad
/// element. That is reproduced: one unparseable entry empties the result.
pub fn id_list(value: &str) -> Vec<i32> {
    let Ok(parsed) = serde_json::from_str::<Vec<serde_json::Value>>(value) else {
        return Vec::new();
    };

    let mut ids = Vec::with_capacity(parsed.len());
    for element in parsed {
        match element_as_int(&element) {
            Some(id) => ids.push(id),
            None => return Vec::new(),
        }
    }
    ids
}

/// `JSONArray.getInt(i)`'s coercions: a number truncated toward zero, or a
/// string that parses as one.
fn element_as_int(element: &serde_json::Value) -> Option<i32> {
    match element {
        serde_json::Value::Number(number) => number.as_f64().map(|value| value as i32),
        serde_json::Value::String(text) => text
            .parse::<i32>()
            .ok()
            .or_else(|| text.parse::<f64>().ok().map(|value| value as i32)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_params;

    #[test]
    fn a_json_array_of_ids_parses() {
        assert_eq!(id_list("[1,2,3]"), vec![1, 2, 3]);
        assert_eq!(id_list("[]"), Vec::<i32>::new());
        assert_eq!(id_list("[7]"), vec![7]);
    }

    #[test]
    fn whitespace_is_not_significant() {
        assert_eq!(id_list("[ 1, 2 , 3 ]"), vec![1, 2, 3]);
    }

    #[test]
    fn json_getint_coerces_strings_and_decimals() {
        assert_eq!(id_list("[\"4\"]"), vec![4]);
        assert_eq!(id_list("[4.0]"), vec![4]);
        assert_eq!(
            id_list("[4.7]"),
            vec![4],
            "truncated toward zero, as Java does"
        );
    }

    #[test]
    fn malformed_json_is_an_empty_list_not_a_panic() {
        // Java prints a stack trace and returns EMPTY_LIST; the rule then runs
        // configured with nothing selected.
        assert_eq!(id_list("not json"), Vec::<i32>::new());
        assert_eq!(id_list("[1,2"), Vec::<i32>::new());
        assert_eq!(id_list(""), Vec::<i32>::new());
    }

    #[test]
    fn one_bad_element_discards_the_whole_list() {
        // getInt throws mid-loop and the catch is outside it, so the partially
        // built list never escapes.
        assert_eq!(id_list("[1,{},3]"), Vec::<i32>::new());
        assert_eq!(id_list("[1,null,3]"), Vec::<i32>::new());
    }

    #[test]
    fn a_missing_key_is_an_empty_list() {
        let params = rule_params! { "holidayTypes" => "[1,2]" };

        assert_eq!(ids_for_key("holidayTypes", &params), vec![1, 2]);
        assert_eq!(ids_for_key("earningTypes", &params), Vec::<i32>::new());
    }
}
