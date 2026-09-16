//! Port of `EarningTypePaySet`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/common/labor/rules/types/EarningTypePaySet.java`.
//!
//! The `earningTypePaySet` rule parameter, carried as a JSON string:
//! which earning types a rule treats as regular hours, and which types the
//! premium hours it pays should be booked against. Read by the rules whose
//! config extends `HoursDistributionRuleWithPayMappingsConfig` —
//! `CaliforniaOTHrs` is the first ported one.
//!
//! # An empty pay set silences the earnings half of a rule
//!
//! `HoursDistributionRuleWithPayMappingsConfig.getDefaultValues()` seeds the
//! parameter with a pay set that has **no pay maps at all** — only
//! `premiumLevels`. So [`configured_earning_type_ids`] comes back empty out of
//! the box, and a rule that filters its earnings through it sees none of them.
//! Earnings are opt-in per property; hours distributions are not.
//!
//! # Malformed JSON aborts, unlike an id list
//!
//! `setJSONString` runs `new JSONObject(jsonString)` and `getInt` unguarded, so
//! a corrupt parameter throws out of the rule. That is the choice
//! [`RuleParams::int_at`] makes for a malformed number (divergence 9) and the
//! opposite of [`json_ids::id_list`], which swallows the error and configures
//! the rule with nothing selected. Both spellings live in this tree because
//! both are faithful to their Java. [`from_json_string`] panics accordingly;
//! [`try_from_json_string`] is the checked form, as `try_int_at` is.
//!
//! [`configured_earning_type_ids`]: EarningTypePaySet::configured_earning_type_ids
//! [`from_json_string`]: EarningTypePaySet::from_json_string
//! [`try_from_json_string`]: EarningTypePaySet::try_from_json_string
//! [`RuleParams::int_at`]: crate::rules::params::RuleParams::int_at
//! [`json_ids::id_list`]: crate::common::json_ids::id_list

use crate::rules::types::earning_type_pay_map::EarningTypePayMap;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

/// The earning types a rule pays against. `EarningTypePaySet`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EarningTypePaySet {
    premium_levels: i32,
    default_regular_earning_type_id: i32,
    pay_maps: Vec<EarningTypePayMap>,
}

impl EarningTypePaySet {
    /// An empty pay set — what a rule gets when its property has configured
    /// nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many premium levels the owning config declares. `setPremiumLevels`.
    #[must_use]
    pub fn with_premium_levels(mut self, premium_levels: i32) -> Self {
        self.premium_levels = premium_levels;
        self
    }

    /// `setDefaultRegularEarningTypeID`.
    #[must_use]
    pub fn with_default_regular_earning_type_id(mut self, id: i32) -> Self {
        self.default_regular_earning_type_id = id;
        self
    }

    /// `setEarningTypePayMapSet`.
    #[must_use]
    pub fn with_pay_maps(mut self, pay_maps: Vec<EarningTypePayMap>) -> Self {
        self.pay_maps = pay_maps;
        self
    }

    /// `getPremiumLevels()`.
    pub fn premium_levels(&self) -> i32 {
        self.premium_levels
    }

    /// `getDefaultRegularEarningTypeID()`.
    pub fn default_regular_earning_type_id(&self) -> i32 {
        self.default_regular_earning_type_id
    }

    /// The pay maps. `getEarningTypePayMapSet()`.
    ///
    /// Java holds a `HashSet` of a class that declares neither `equals` nor
    /// `hashCode`, so membership is identity and two pay maps equal by value
    /// both survive. A `Vec` reproduces that; nothing reads the collection as a
    /// set.
    pub fn pay_maps(&self) -> &[EarningTypePayMap] {
        &self.pay_maps
    }

    /// Every earning type named anywhere in the set, regular and premium alike.
    /// `getConfiguredEarningTypeIDs()`.
    ///
    /// A rule uses this to decide which of the card's earnings it is allowed to
    /// touch. Empty by default — see the module note.
    pub fn configured_earning_type_ids(&self) -> HashSet<i32> {
        let mut ids = HashSet::new();
        for pay_map in &self.pay_maps {
            ids.insert(pay_map.regular_earning_type_id());
            ids.extend(pay_map.premium_earning_type_ids());
        }
        ids
    }

    /// Each earning type's pay level, regular being zero. `getPayLevelMap()`.
    ///
    /// `CaliforniaOTHrsRuleImpl` assigns this to a field and never reads it; it
    /// is kept because it is this type's API rather than that rule's.
    pub fn pay_level_map(&self) -> HashMap<i32, i32> {
        let mut levels = HashMap::new();
        for pay_map in &self.pay_maps {
            levels.insert(pay_map.regular_earning_type_id(), 0);
            for (level, &id) in pay_map.premium_earning_type_ids().iter().enumerate() {
                levels.insert(id, level as i32 + 1);
            }
        }
        levels
    }

    /// The earning type a regular type's premium hours are booked against at
    /// `premium_level` — 0 overtime, 1 double time.
    /// `getPremiumEarningTypeID(int, int)`.
    ///
    /// # Zero means "not configured"
    ///
    /// Java returns `0` when no pay map claims `regular_earning_type_id`, and
    /// the caller hands that straight to `earningTypeDAO.findByID(0)`, which
    /// finds nothing — so the premium earning is created against no type at
    /// all rather than not created. Reproduced.
    ///
    /// **Divergence:** Java *throws* `IndexOutOfBoundsException` in the other
    /// failure — a pay map that matches but lists fewer premium types than the
    /// level asked for, which is what a property gets for configuring overtime
    /// and not double time. That is a live configuration, not a corrupt
    /// parameter, and it only fails on the days somebody works past the
    /// double-time limit. It returns `0` here, so the two "no premium type for
    /// this level" cases behave alike. Pinned by
    /// `a_level_past_the_configured_premiums_is_zero_not_a_panic`.
    pub fn premium_earning_type_id(
        &self,
        regular_earning_type_id: i32,
        premium_level: usize,
    ) -> i32 {
        self.pay_maps
            .iter()
            .find(|pay_map| pay_map.regular_earning_type_id() == regular_earning_type_id)
            .and_then(|pay_map| pay_map.premium_earning_type_ids().get(premium_level))
            .copied()
            .unwrap_or(0)
    }

    /// Parse the parameter. `setJSONString(String)`.
    ///
    /// # Panics
    ///
    /// If the value is not a JSON object carrying `premiumLevels`,
    /// `defaultRegularEarningTypeID` and `earningTypePayMapSet`, matching
    /// Java's unchecked `JSONException`. Use
    /// [`try_from_json_string`](Self::try_from_json_string) to handle bad
    /// configuration instead of failing the calculation.
    pub fn from_json_string(json: &str) -> Self {
        Self::try_from_json_string(json)
            .unwrap_or_else(|| panic!("rule parameter is not an earning type pay set: {json:?}"))
    }

    /// The pay set the parameter holds, or `None` if it is missing or
    /// malformed.
    pub fn try_from_json_string(json: &str) -> Option<Self> {
        let object: Value = serde_json::from_str(json).ok()?;

        let mut pay_maps = Vec::new();
        for element in object.get("earningTypePayMapSet")?.as_array()? {
            pay_maps.push(EarningTypePayMap::from_json(element)?);
        }

        Some(Self {
            premium_levels: object.get("premiumLevels")?.as_i64()? as i32,
            default_regular_earning_type_id: object.get("defaultRegularEarningTypeID")?.as_i64()?
                as i32,
            pay_maps,
        })
    }

    /// Serialize the parameter. `getJSONString()`.
    pub fn to_json_string(&self) -> String {
        let mut object = Map::new();
        object.insert(
            "premiumLevels".to_string(),
            Value::from(self.premium_levels),
        );
        object.insert(
            "defaultRegularEarningTypeID".to_string(),
            Value::from(self.default_regular_earning_type_id),
        );
        object.insert(
            "earningTypePayMapSet".to_string(),
            Value::Array(
                self.pay_maps
                    .iter()
                    .map(EarningTypePayMap::to_json)
                    .collect(),
            ),
        );
        Value::Object(object).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Groovy fixture: earning type 1 escalating to 2 (overtime) and 3
    /// (double time).
    fn pay_set() -> EarningTypePaySet {
        EarningTypePaySet::new()
            .with_premium_levels(2)
            .with_default_regular_earning_type_id(1)
            .with_pay_maps(vec![EarningTypePayMap::new(1, vec![2, 3])])
    }

    #[test]
    fn a_pay_set_round_trips_through_its_parameter_string() {
        let json = pay_set().to_json_string();

        assert_eq!(EarningTypePaySet::from_json_string(&json), pay_set());
    }

    #[test]
    fn the_configured_types_are_the_regular_and_premium_ids_together() {
        assert_eq!(
            pay_set().configured_earning_type_ids(),
            HashSet::from([1, 2, 3])
        );
    }

    #[test]
    fn an_empty_pay_set_configures_no_earning_types() {
        // The default from HoursDistributionRuleWithPayMappingsConfig: a rule
        // filtering earnings through this sees none of them.
        let defaults = EarningTypePaySet::new().with_premium_levels(2);

        assert!(defaults.configured_earning_type_ids().is_empty());
        assert_eq!(defaults.premium_levels(), 2);
    }

    #[test]
    fn the_pay_level_map_numbers_regular_zero_and_premiums_upward() {
        assert_eq!(
            pay_set().pay_level_map(),
            HashMap::from([(1, 0), (2, 1), (3, 2)])
        );
    }

    #[test]
    fn a_premium_level_resolves_to_its_earning_type() {
        assert_eq!(pay_set().premium_earning_type_id(1, 0), 2, "overtime");
        assert_eq!(pay_set().premium_earning_type_id(1, 1), 3, "double time");
    }

    #[test]
    fn an_unmapped_regular_type_is_zero() {
        // Java returns 0 and the caller does findByID(0), which finds nothing —
        // so the premium earning is created against no earning type at all.
        assert_eq!(pay_set().premium_earning_type_id(99, 0), 0);
        assert_eq!(EarningTypePaySet::new().premium_earning_type_id(1, 0), 0);
    }

    #[test]
    fn a_level_past_the_configured_premiums_is_zero_not_a_panic() {
        // Java throws IndexOutOfBoundsException here — a property that
        // configured overtime but not double time dies on the first twelve-hour
        // day. See the divergence on `premium_earning_type_id`.
        let overtime_only = EarningTypePaySet::new()
            .with_premium_levels(2)
            .with_pay_maps(vec![EarningTypePayMap::new(1, vec![2])]);

        assert_eq!(overtime_only.premium_earning_type_id(1, 0), 2);
        assert_eq!(overtime_only.premium_earning_type_id(1, 1), 0);
    }

    #[test]
    fn malformed_json_is_none_in_the_checked_form() {
        assert!(EarningTypePaySet::try_from_json_string("not json").is_none());
        assert!(EarningTypePaySet::try_from_json_string("{}").is_none());
        assert!(
            EarningTypePaySet::try_from_json_string(r#"{"premiumLevels":2}"#).is_none(),
            "earningTypePayMapSet is read with getJSONArray, which throws when absent"
        );
    }

    #[test]
    #[should_panic(expected = "not an earning type pay set")]
    fn malformed_json_panics_in_the_unchecked_form() {
        // Java's setJSONString throws JSONException out of the rule, as
        // Integer.parseInt does for a malformed number (divergence 9).
        let _ = EarningTypePaySet::from_json_string("[]");
    }

    #[test]
    fn duplicate_pay_maps_both_survive() {
        // Java's HashSet is identity-based: EarningTypePayMap declares neither
        // equals nor hashCode.
        let duplicated = EarningTypePaySet::new().with_pay_maps(vec![
            EarningTypePayMap::new(1, vec![2]),
            EarningTypePayMap::new(1, vec![2]),
        ]);

        assert_eq!(duplicated.pay_maps().len(), 2);
        assert_eq!(
            duplicated.configured_earning_type_ids(),
            HashSet::from([1, 2])
        );
    }
}
