//! Rule parameters.
//!
//! Every rule is configured by a `Map<String, String>` — `RuleItem.params` in
//! Java, a key/value table joined off the rule item. The keys are `public
//! static String` constants on the rule's own `*RuleConfig` class; the values
//! are always strings, whatever they represent.
//!
//! Java has **no shared parsing helper**: the algorithm packages contain 843
//! separate `Integer.parseInt` / `Boolean.parseBoolean` / `Double.valueOf`
//! calls against `params.get(SomeConfig.SOME_KEY)`. This module is the one
//! place that happens here. That is a deliberate divergence in *shape* only —
//! each accessor reproduces its Java counterpart's behaviour exactly, including
//! the failure modes, which differ between types in ways that matter:
//!
//! | Java | On a missing key | On unparseable text |
//! |---|---|---|
//! | `Boolean.parseBoolean` | `false` | `false` |
//! | `Integer.parseInt` | throws | throws |
//! | `Double.parseDouble` | throws | throws |
//!
//! So [`RuleParams::bool_at`] absorbs anything, while [`RuleParams::int_at`]
//! and [`RuleParams::double_at`] panic where Java throws. Callers that want to
//! handle bad configuration rather than fail the calculation have `try_`
//! variants.

use std::collections::HashMap;

/// The parameters a rule item is configured with. `RuleItem.getParams()`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleParams {
    values: HashMap<String, String>,
}

impl RuleParams {
    /// Empty parameters — the shape a Groovy test passes when it wants a rule
    /// to run entirely on its defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Is a key present?
    pub fn contains(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    /// How many parameters are set.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Are there no parameters at all?
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Set one parameter, returning any previous value.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) -> Option<String> {
        self.values.insert(key.into(), value.into())
    }

    /// The raw string at `key`, if present. `params.get(key)`.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// Fill in any key `defaults` has that these parameters do not.
    ///
    /// `RuleConfig.fixMap` — note it mutates in place, and that existing values
    /// always win. Rules call this first thing in `execute`, which is what lets
    /// a rule item store only the parameters that differ from the defaults.
    pub fn fix(&mut self, defaults: &RuleParams) {
        for (key, value) in &defaults.values {
            self.values
                .entry(key.clone())
                .or_insert_with(|| value.clone());
        }
    }

    /// [`fix`](Self::fix) without mutating. `RuleConfig.fixMapImmutable`.
    #[must_use]
    pub fn fixed(&self, defaults: &RuleParams) -> Self {
        let mut fixed = self.clone();
        fixed.fix(defaults);
        fixed
    }

    /// The boolean at `key`. `Boolean.parseBoolean(params.get(key))`.
    ///
    /// Case-insensitively `"true"`, and `false` for everything else —
    /// including a missing key and unparseable text. Java's
    /// `Boolean.parseBoolean` never throws, not even on `null`, so neither does
    /// this; a typo in a rule parameter silently reads as `false` in both.
    pub fn bool_at(&self, key: &str) -> bool {
        self.get(key)
            .is_some_and(|value| value.eq_ignore_ascii_case("true"))
    }

    /// The integer at `key`. `Integer.parseInt(params.get(key))`.
    ///
    /// # Panics
    ///
    /// If the key is missing or the value is not an integer, matching Java's
    /// unchecked `NumberFormatException`. Use [`try_int_at`](Self::try_int_at)
    /// to handle bad configuration instead of failing the calculation.
    pub fn int_at(&self, key: &str) -> i32 {
        self.try_int_at(key).unwrap_or_else(|| {
            panic!(
                "rule parameter {key:?} is not an integer: {:?}",
                self.get(key)
            )
        })
    }

    /// The integer at `key`, or `None` if absent or unparseable.
    pub fn try_int_at(&self, key: &str) -> Option<i32> {
        self.get(key)?.trim().parse().ok()
    }

    /// The number at `key`. `Double.parseDouble(params.get(key))`.
    ///
    /// # Panics
    ///
    /// If the key is missing or the value is not a number, matching Java's
    /// unchecked exception.
    pub fn double_at(&self, key: &str) -> f64 {
        self.try_double_at(key).unwrap_or_else(|| {
            panic!(
                "rule parameter {key:?} is not a number: {:?}",
                self.get(key)
            )
        })
    }

    /// The number at `key`, or `None` if absent or unparseable.
    pub fn try_double_at(&self, key: &str) -> Option<f64> {
        self.get(key)?.trim().parse().ok()
    }

    /// Every key/value pair, for iteration and conversion.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

impl<K, V> FromIterator<(K, V)> for RuleParams
where
    K: Into<String>,
    V: Into<String>,
{
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self {
            values: iter
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }
}

/// Build parameters inline, the way a Groovy test table does.
///
/// ```
/// use workrules::rule_params;
///
/// let params = rule_params! { "manualIn" => "false", "inPunchRoundTo" => "15" };
/// assert!(!params.bool_at("manualIn"));
/// assert_eq!(params.int_at("inPunchRoundTo"), 15);
/// ```
#[macro_export]
macro_rules! rule_params {
    () => { $crate::rules::params::RuleParams::new() };
    ($($key:expr => $value:expr),+ $(,)?) => {{
        let mut params = $crate::rules::params::RuleParams::new();
        $(params.set($key, $value);)+
        params
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn a_value_can_be_set_and_read_back() {
        let mut params = RuleParams::new();
        assert!(params.is_empty());
        params.set("inPunchRoundTo", "15");
        assert_eq!(params.get("inPunchRoundTo"), Some("15"));
        assert!(params.contains("inPunchRoundTo"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn a_missing_key_reads_as_none() {
        assert_eq!(RuleParams::new().get("nothing"), None);
    }

    #[test]
    fn fix_fills_only_the_keys_that_are_absent() {
        let defaults = rule_params! { "a" => "1", "b" => "2" };
        let mut params = rule_params! { "a" => "99" };

        params.fix(&defaults);

        assert_eq!(params.get("a"), Some("99"), "an existing value must win");
        assert_eq!(params.get("b"), Some("2"));
    }

    #[test]
    fn fix_on_empty_params_yields_the_defaults() {
        // The shape MinuteRoundingRuleImplTest uses when it passes an empty map.
        let defaults = rule_params! { "inPunchRoundTo" => "15" };
        let mut params = RuleParams::new();

        params.fix(&defaults);

        assert_eq!(params.int_at("inPunchRoundTo"), 15);
    }

    #[test]
    fn fixed_leaves_the_original_alone() {
        let defaults = rule_params! { "b" => "2" };
        let params = rule_params! { "a" => "1" };

        let fixed = params.fixed(&defaults);

        assert!(!params.contains("b"), "the original must not be mutated");
        assert_eq!(fixed.get("b"), Some("2"));
    }

    #[rstest]
    #[case("true", true)]
    #[case("TRUE", true)]
    #[case("True", true)]
    #[case("false", false)]
    #[case("FALSE", false)]
    fn a_boolean_is_case_insensitive_true(#[case] value: &str, #[case] expected: bool) {
        let params = rule_params! { "flag" => value };
        assert_eq!(params.bool_at("flag"), expected);
    }

    #[rstest]
    #[case("yes")]
    #[case("1")]
    #[case("")]
    #[case("  true  ")]
    fn anything_that_is_not_the_word_true_is_false(#[case] value: &str) {
        // Java's Boolean.parseBoolean does not trim, so "  true  " is false.
        let params = rule_params! { "flag" => value };
        assert!(!params.bool_at("flag"));
    }

    #[test]
    fn a_missing_boolean_is_false_rather_than_a_panic() {
        // Boolean.parseBoolean(null) is false in Java; it is the one accessor
        // that absorbs a missing key.
        assert!(!RuleParams::new().bool_at("absent"));
    }

    #[test]
    fn an_integer_parses() {
        let params = rule_params! { "n" => "15", "negative" => "-3" };
        assert_eq!(params.int_at("n"), 15);
        assert_eq!(params.int_at("negative"), -3);
    }

    #[test]
    fn a_number_parses() {
        let params = rule_params! { "d" => "1.5", "whole" => "8" };
        assert_eq!(params.double_at("d"), 1.5);
        assert_eq!(params.double_at("whole"), 8.0);
    }

    #[test]
    fn try_accessors_report_absence_instead_of_panicking() {
        let params = rule_params! { "n" => "oops" };
        assert_eq!(params.try_int_at("n"), None);
        assert_eq!(params.try_int_at("absent"), None);
        assert_eq!(params.try_double_at("n"), None);
    }

    #[test]
    #[should_panic(expected = "is not an integer")]
    fn a_missing_integer_panics_where_java_throws() {
        RuleParams::new().int_at("absent");
    }

    #[test]
    #[should_panic(expected = "is not an integer")]
    fn an_unparseable_integer_panics_where_java_throws() {
        rule_params! { "n" => "fifteen" }.int_at("n");
    }

    #[test]
    #[should_panic(expected = "is not a number")]
    fn a_missing_number_panics_where_java_throws() {
        RuleParams::new().double_at("absent");
    }

    #[test]
    fn the_macro_builds_the_same_thing_as_setting_by_hand() {
        let mut by_hand = RuleParams::new();
        by_hand.set("a", "1");
        assert_eq!(rule_params! { "a" => "1" }, by_hand);
        assert_eq!(rule_params! {}, RuleParams::new());
    }

    #[test]
    fn params_can_be_collected_from_pairs() {
        let params: RuleParams = [("a", "1"), ("b", "2")].into_iter().collect();
        assert_eq!(params.len(), 2);
        assert_eq!(params.get("a"), Some("1"));
    }

    #[test]
    fn iter_yields_every_pair() {
        let params = rule_params! { "a" => "1", "b" => "2" };
        let mut pairs: Vec<_> = params.iter().collect();
        pairs.sort_unstable();
        assert_eq!(pairs, vec![("a", "1"), ("b", "2")]);
    }
}
