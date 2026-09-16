//! Port of `com.unifocus.watson.common.labor.rules.JsonRangeArray`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/JsonRangeArray.java`.
//!
//! Several rules take a banded lookup table as a parameter — "0 to 8 hours pays
//! 1.0, 8 to 12 pays 1.5" and so on. Because rule parameters are all strings,
//! the table travels as JSON: an array of three-element arrays, each
//! `[from, to, value]`.
//!
//! Two behaviours here are worth keeping exactly as Java has them, because
//! rules depend on them:
//!
//! * a lookup that matches no band returns `0.0`, not an error and not the
//!   nearest band;
//! * bands are matched inclusively at *both* ends, so overlapping bands would
//!   be ambiguous — which is why validation rejects them.

use serde_json::Value;

/// How many elements each band must have: `[from, to, value]`.
const DETAILS_LENGTH: usize = 3;

/// Parse `json_range` into its `[from, to, value]` bands.
///
/// Returns `None` if the text is not a JSON array of numeric triples. Java has
/// no such accessor — it reparses inside each public method — but both of those
/// need the same shape, so it is factored out here.
fn parse_bands(json_range: &str) -> Option<Vec<[f64; DETAILS_LENGTH]>> {
    let Value::Array(entries) = serde_json::from_str::<Value>(json_range).ok()? else {
        return None;
    };

    entries
        .iter()
        .map(|entry| {
            let Value::Array(details) = entry else {
                return None;
            };
            if details.len() != DETAILS_LENGTH {
                return None;
            }
            let mut band = [0.0; DETAILS_LENGTH];
            for (slot, detail) in band.iter_mut().zip(details) {
                *slot = detail.as_f64()?;
            }
            Some(band)
        })
        .collect()
}

/// The value of the first band containing `range_value`, or `0.0`.
///
/// `JsonRangeArray.getValueFromJSONRange`. Bands are scanned in order and both
/// ends are inclusive. Java lets a malformed string throw `JSONException` out
/// of this method; here malformed input takes the same path as no match, since
/// every caller reaches it with a value that
/// [`is_valid_json`] has already accepted.
pub fn value_from_json_range(json_range: &str, range_value: f64) -> f64 {
    let Some(bands) = parse_bands(json_range) else {
        return 0.0;
    };

    for [from, to, value] in bands {
        if range_value >= from && range_value <= to {
            return value;
        }
    }

    0.0
}

/// Is `json` a well-formed band table?
///
/// `JsonRangeArray.isValidJson`. Requires: parseable JSON, at least one band,
/// every band exactly three numbers, every band's `from <= to`, and no two
/// bands overlapping. Java swallows `JSONException` into `false`, so any
/// malformed input is simply invalid rather than an error.
pub fn is_valid_json(json: &str) -> bool {
    let Some(bands) = parse_bands(json) else {
        return false;
    };

    if bands.is_empty() {
        return false;
    }

    for (index, [from, to, _]) in bands.iter().enumerate() {
        if from > to {
            return false;
        }
        if ranges_overlap(&bands, index) {
            return false;
        }
    }

    true
}

/// Does the band at `index` overlap any other band?
///
/// `JsonRangeArray.rangesOverlap`. The comparison is inclusive at both ends, so
/// two bands that merely touch — `[0,8]` and `[8,12]` — *do* count as
/// overlapping. That is Java's behaviour and it follows from the lookup being
/// inclusive at both ends too: a value of exactly 8 would otherwise match twice.
fn ranges_overlap(bands: &[[f64; DETAILS_LENGTH]], index: usize) -> bool {
    let [from_check, to_check, _] = bands[index];

    bands
        .iter()
        .enumerate()
        .any(|(other, [from, to, _])| other != index && from_check <= *to && to_check >= *from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    const BANDS: &str = "[[0,8,1.0],[8.01,12,1.5],[12.01,24,2.0]]";

    #[rstest]
    #[case(0.0, 1.0)]
    #[case(4.0, 1.0)]
    #[case(8.0, 1.0)]
    #[case(8.01, 1.5)]
    #[case(12.0, 1.5)]
    #[case(24.0, 2.0)]
    fn a_value_takes_the_value_of_its_band(#[case] range_value: f64, #[case] expected: f64) {
        assert_eq!(value_from_json_range(BANDS, range_value), expected);
    }

    #[rstest]
    #[case(-1.0)]
    #[case(24.01)]
    #[case(8.005)]
    fn a_value_in_no_band_is_zero_rather_than_an_error(#[case] range_value: f64) {
        // Including 8.005, which falls in the gap between two bands. Java
        // returns 0 here too — it does not reach for the nearest band.
        assert_eq!(value_from_json_range(BANDS, range_value), 0.0);
    }

    #[test]
    fn the_first_matching_band_wins() {
        // Overlapping bands are invalid, but the lookup does not check; it
        // returns the first match in document order.
        assert_eq!(value_from_json_range("[[0,10,1.0],[5,15,2.0]]", 7.0), 1.0);
    }

    #[test]
    fn malformed_json_looks_up_as_zero() {
        assert_eq!(value_from_json_range("not json", 1.0), 0.0);
        assert_eq!(value_from_json_range("", 1.0), 0.0);
        assert_eq!(value_from_json_range("[[0,8]]", 1.0), 0.0);
    }

    #[test]
    fn a_well_formed_table_validates() {
        assert!(is_valid_json(BANDS));
        assert!(is_valid_json("[[0,100,1.0]]"));
    }

    #[rstest]
    #[case("", "not JSON at all")]
    #[case("not json", "not JSON at all")]
    #[case("[]", "no bands")]
    #[case("[[0,8]]", "band of two")]
    #[case("[[0,8,1.0,9]]", "band of four")]
    #[case("[[8,0,1.0]]", "from after to")]
    #[case("[[0,10,1.0],[5,15,2.0]]", "overlapping bands")]
    #[case("[[\"a\",8,1.0]]", "non-numeric bound")]
    fn malformed_tables_are_rejected(#[case] json: &str, #[case] why: &str) {
        assert!(!is_valid_json(json), "should have been rejected: {why}");
    }

    #[test]
    fn bands_that_merely_touch_count_as_overlapping() {
        // [0,8] and [8,12] both contain 8, so the lookup would be ambiguous.
        // Java rejects this, and the gapped BANDS constant above is how real
        // rule parameters avoid it.
        assert!(!is_valid_json("[[0,8,1.0],[8,12,1.5]]"));
    }

    #[test]
    fn a_single_band_cannot_overlap_itself() {
        assert!(is_valid_json("[[0,0,1.0]]"));
    }

    #[test]
    fn an_equal_from_and_to_is_a_valid_single_point_band() {
        assert!(is_valid_json("[[5,5,3.0]]"));
        assert_eq!(value_from_json_range("[[5,5,3.0]]", 5.0), 3.0);
        assert_eq!(value_from_json_range("[[5,5,3.0]]", 4.99), 0.0);
    }
}
