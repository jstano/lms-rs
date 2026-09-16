//! Port of `com.unifocus.watson.common.labor.rules.{RuleConfig,
//! AbstractRuleConfig, PriorityRuleConfig}`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/`.
//!
//! Every rule class has a `*RuleConfig` describing its parameters. At runtime
//! an algorithm uses almost none of it: it news up its own config purely as a
//! namespace of parameter-key constants, calls `fixMap` to backfill defaults,
//! and reads the map. Confirmed by grep — no runner or `*RuleImpl` anywhere
//! calls `getProperties` or `validateProperties`.
//!
//! So what comes across is the parameter keys, [`default_values`], and the
//! constraints inside `validateProperties`. What does not is the l2fprod Swing
//! `Property` construction those constraints were expressed against, the
//! `ResourceMgr` display names, and `getSortOrderMap`.
//!
//! [`default_values`]: RuleConfig::default_values

use crate::rules::params::RuleParams;
use crate::rules::rule_class::RuleClass;

/// The parameter key every [`PriorityRuleConfig`] rule stores its execution
/// order under. `PriorityRuleConfig.PRIORITY`.
pub const PRIORITY: &str = "priority";

/// The priority a rule item takes when nothing says otherwise.
///
/// `RuleItemPriorityComparator` uses this both for rules that are not
/// priority-configurable at all and as `PriorityRuleConfig`'s own default.
pub const DEFAULT_PRIORITY: i32 = 5;

/// Messages explaining why a set of parameters is not valid.
///
/// Stands in for `com.unifocus.tbx.core.ValidationResults`, which is a message
/// list. Empty means valid.
pub type ValidationResults = Vec<String>;

/// The parameter schema for one rule class. `RuleConfig`.
pub trait RuleConfig {
    /// Which rule class this configures. `getRuleClass()`.
    fn rule_class(&self) -> RuleClass;

    /// The value each parameter takes when the rule item does not set it.
    /// `getDefaultValues()`.
    fn default_values(&self) -> RuleParams;

    /// Check a set of parameters, returning a message per problem.
    ///
    /// Java's `validateProperties` reads l2fprod `Property` objects, since it
    /// ran against the configuration UI; the constraints it expresses are real
    /// and are re-stated here against the parameters themselves. Defaults to
    /// "no constraints", which is what `PriorityRuleConfig` and a number of
    /// concrete configs do.
    fn validate(&self, _params: &RuleParams) -> ValidationResults {
        ValidationResults::new()
    }

    /// This rule's execution order within its rule set, if it is
    /// priority-configurable. Otherwise `None`.
    ///
    /// Java models this as a subclass, `PriorityRuleConfig`, and
    /// `RuleItemPriorityComparator` asks `instanceof`. Rust has no `instanceof`,
    /// so the question is asked directly: a config that extends
    /// `PriorityRuleConfig` overrides this, everything else inherits `None` and
    /// is ranked at [`DEFAULT_PRIORITY`].
    fn priority(&self, _params: &RuleParams) -> Option<i32> {
        None
    }

    /// Backfill missing parameters from [`default_values`](Self::default_values).
    ///
    /// `RuleConfig.fixMap`. Every algorithm calls this as the first statement of
    /// `execute`.
    fn fix_map(&self, params: &mut RuleParams) {
        params.fix(&self.default_values());
    }
}

/// Mixin for the configs that Java declares as `extends PriorityRuleConfig`.
///
/// Implement this and the blanket [`priority`](RuleConfig::priority) override
/// below comes with it, along with the `"priority"` default of
/// [`DEFAULT_PRIORITY`].
///
/// ```
/// use workrules::rules::params::RuleParams;
/// use workrules::rules::rule_class::RuleClass;
/// use workrules::rules::rule_config::{priority_default_values, priority_of, RuleConfig};
///
/// struct SomePriorityRule;
///
/// impl RuleConfig for SomePriorityRule {
///     fn rule_class(&self) -> RuleClass { RuleClass::MinutePrr }
///     fn default_values(&self) -> RuleParams { priority_default_values() }
///     fn priority(&self, params: &RuleParams) -> Option<i32> { Some(priority_of(self, params)) }
/// }
/// ```
pub fn priority_default_values() -> RuleParams {
    let mut defaults = RuleParams::new();
    defaults.set(PRIORITY, DEFAULT_PRIORITY.to_string());
    defaults
}

/// Read a priority-configurable rule's execution order.
///
/// `PriorityRuleConfig.getPriority(Map)`, which calls `fixMap` first so that an
/// unset priority reads as the default rather than throwing.
pub fn priority_of(config: &(impl RuleConfig + ?Sized), params: &RuleParams) -> i32 {
    let fixed = params.fixed(&config.default_values());
    fixed.try_int_at(PRIORITY).unwrap_or(DEFAULT_PRIORITY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_params;

    /// A config with no parameters and no constraints — the common shape.
    struct PlainConfig;

    impl RuleConfig for PlainConfig {
        fn rule_class(&self) -> RuleClass {
            RuleClass::MinutePrr
        }

        fn default_values(&self) -> RuleParams {
            rule_params! { "inPunchRoundTo" => "15" }
        }
    }

    /// Stands in for a Java `extends PriorityRuleConfig`.
    struct PriorityConfig;

    impl RuleConfig for PriorityConfig {
        fn rule_class(&self) -> RuleClass {
            RuleClass::ShortBreakPpr
        }

        fn default_values(&self) -> RuleParams {
            priority_default_values()
        }

        fn priority(&self, params: &RuleParams) -> Option<i32> {
            Some(priority_of(self, params))
        }
    }

    /// A config that re-states a real `validateProperties` constraint.
    struct ValidatingConfig;

    impl RuleConfig for ValidatingConfig {
        fn rule_class(&self) -> RuleClass {
            RuleClass::MinutePrr
        }

        fn default_values(&self) -> RuleParams {
            rule_params! { "roundTo" => "15" }
        }

        fn validate(&self, params: &RuleParams) -> ValidationResults {
            let mut results = ValidationResults::new();
            let round_to = params.try_int_at("roundTo").unwrap_or_default();
            if round_to <= 0 || round_to > 60 {
                results.push("roundTo must be greater than 0 and less than or equal to 60".into());
            }
            results
        }
    }

    #[test]
    fn fix_map_backfills_from_the_configs_defaults() {
        let mut params = RuleParams::new();

        PlainConfig.fix_map(&mut params);

        assert_eq!(params.int_at("inPunchRoundTo"), 15);
    }

    #[test]
    fn fix_map_does_not_overwrite_a_configured_value() {
        let mut params = rule_params! { "inPunchRoundTo" => "5" };

        PlainConfig.fix_map(&mut params);

        assert_eq!(params.int_at("inPunchRoundTo"), 5);
    }

    #[test]
    fn a_plain_config_has_no_priority() {
        // Java asks `instanceof PriorityRuleConfig`; the comparator then ranks
        // it at 5.
        assert_eq!(PlainConfig.priority(&RuleParams::new()), None);
    }

    #[test]
    fn a_priority_config_defaults_to_five() {
        assert_eq!(PriorityConfig.priority(&RuleParams::new()), Some(5));
    }

    #[test]
    fn a_priority_config_reads_a_configured_priority() {
        let params = rule_params! { PRIORITY => "1" };
        assert_eq!(PriorityConfig.priority(&params), Some(1));
    }

    #[test]
    fn reading_a_priority_does_not_mutate_the_callers_parameters() {
        // Java's getPriority calls fixMap, which does mutate. Nothing depends
        // on that side effect, and not mutating is the safer contract.
        let params = RuleParams::new();
        let _ = PriorityConfig.priority(&params);
        assert!(params.is_empty());
    }

    #[test]
    fn a_config_with_no_constraints_validates_anything() {
        assert!(PlainConfig.validate(&RuleParams::new()).is_empty());
    }

    #[test]
    fn a_constraint_reports_one_message_per_problem() {
        assert!(
            ValidatingConfig
                .validate(&rule_params! { "roundTo" => "15" })
                .is_empty()
        );
        assert_eq!(
            ValidatingConfig
                .validate(&rule_params! { "roundTo" => "0" })
                .len(),
            1
        );
        assert_eq!(
            ValidatingConfig
                .validate(&rule_params! { "roundTo" => "61" })
                .len(),
            1
        );
    }

    #[test]
    fn a_config_knows_its_rule_class() {
        assert_eq!(PlainConfig.rule_class(), RuleClass::MinutePrr);
        assert_eq!(
            PlainConfig.rule_class().rule_type(),
            crate::rules::rule_type::RuleType::PunchRounding
        );
    }
}
