//! Port of `PriorBalancesRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/earningrate/PriorBalancesRateRuleImpl.java`.
//!
//! Divides a prior accrual period's ending wage balance by its ending hours
//! balance, memoized per employee for the run — the run's first earning for
//! an employee triggers the query, every later one for the same employee
//! reads the cache.
//!
//! `getPriorAccrualBalancesForEmployee` populates the cache with `0.0` for
//! both buckets when Java's query returns no transactions, rather than
//! leaving the employee uncached — so a second earning for the same employee
//! still does not re-query even though the first found nothing. Reproduced:
//! the cache always gets an entry, even an empty-balances one.
//!
//! Ported cases: `PriorBalancesRateRuleImplTest.groovy` (three cases).

use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::earningrate::EarningRateRule;
use crate::rules::algorithm::earningrate::config::{
    COSTS_ACCRUAL, PRIOR_BALANCES_HOURS_ACCRUAL, PriorBalancesRateRuleConfig,
};
use crate::rules::params::RuleParams;
use crate::rules::ports::AccrualTransactionPort;
use crate::rules::rule_config::RuleConfig;
use std::collections::HashMap;

/// `PriorBalancesRateRuleImpl`.
///
/// `&mut self` because of the per-employee cache — see the family's "the
/// stateful pair" doc in `mod.rs`.
pub struct PriorBalancesRateRule<A: AccrualTransactionPort> {
    accrual_transactions: A,
    /// `empToAccrualIDsAndEndingTotalBalance`: employee id to (accrual
    /// earning-type id to ending total balance).
    balances_by_employee: HashMap<i32, HashMap<i32, f64>>,
}

impl<A: AccrualTransactionPort> PriorBalancesRateRule<A> {
    /// Build the rule over the port its prior-period transactions come from.
    pub fn new(accrual_transactions: A) -> Self {
        Self {
            accrual_transactions,
            balances_by_employee: HashMap::new(),
        }
    }

    /// `getPriorAccrualBalancesForEmployee(Employee, int, int)`.
    ///
    /// `populateAccrualEndingBalancesForEmployee` explicitly seeds both ids
    /// at `0.0` when the DAO returns no transactions at all; when it returns
    /// some, the map holds only the ids those transactions' earning types
    /// carry, so a later read of an id neither transaction had is a missing
    /// key — reproduced below by panicking there, the way Java's `Double`
    /// unboxing NPEs on the missing entry.
    fn prior_accrual_balances(
        &mut self,
        employee_id: i32,
        hours_accrual_id: i32,
        costs_accrual_id: i32,
    ) -> &HashMap<i32, f64> {
        self.balances_by_employee
            .entry(employee_id)
            .or_insert_with(|| {
                let transactions = self
                    .accrual_transactions
                    .latest_transactions_prior_to_pay_period(employee_id);

                if transactions.is_empty() {
                    HashMap::from([(hours_accrual_id, 0.0), (costs_accrual_id, 0.0)])
                } else {
                    transactions
                        .into_iter()
                        .map(|transaction| {
                            (
                                transaction.earning_type_id(),
                                transaction.ending_total_balance(),
                            )
                        })
                        .collect()
                }
            })
    }
}

impl<A: AccrualTransactionPort> EarningRateRule for PriorBalancesRateRule<A> {
    fn execute(
        &mut self,
        _dataset: &dyn TimeCard,
        earning: &mut EmployeeEarning,
        params: &RuleParams,
    ) {
        let params = params.fixed(&PriorBalancesRateRuleConfig.default_values());
        let hours_accrual_id = params.int_at(PRIOR_BALANCES_HOURS_ACCRUAL);
        let costs_accrual_id = params.int_at(COSTS_ACCRUAL);

        let employee_id = earning.employee_id();
        let balances = self.prior_accrual_balances(employee_id, hours_accrual_id, costs_accrual_id);
        let hours = *balances.get(&hours_accrual_id).unwrap_or_else(|| {
            panic!("no prior balance cached for hours accrual {hours_accrual_id}")
        });
        let wages = *balances.get(&costs_accrual_id).unwrap_or_else(|| {
            panic!("no prior balance cached for costs accrual {costs_accrual_id}")
        });

        let rate = if hours == 0.0 {
            0.0
        } else {
            crate::common::numbers::round_currency(wages / hours)
        };

        earning.set_rate(rate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::entity::accrual_transaction::AccrualTransaction;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use joda_rs::LocalDate;

    struct FixedTransactions(Vec<AccrualTransaction>);
    impl AccrualTransactionPort for FixedTransactions {
        fn transactions_for_employee_with_unapplied_hours(
            &self,
            _employee_id: i32,
            _earning_type_ids: &[i32],
        ) -> Vec<AccrualTransaction> {
            Vec::new()
        }
        fn latest_transactions_prior_to_pay_period(
            &self,
            _employee_id: i32,
        ) -> Vec<AccrualTransaction> {
            self.0.clone()
        }
    }

    fn params(hours_id: i32, costs_id: i32) -> RuleParams {
        rule_params! {
            PRIOR_BALANCES_HOURS_ACCRUAL => hours_id.to_string(),
            COSTS_ACCRUAL => costs_id.to_string()
        }
    }

    mod java_parity_tests {
        use super::*;

        /// `PriorBalancesRateRuleImplTest`: "Rate on EmployeeEarning is set
        /// to 0 if there are no prior earning balances for the configured
        /// hours and costs accrual".
        #[test]
        fn rate_is_zero_with_no_prior_balances() {
            let dataset = TimeCardData::new();
            let mut earning = EmployeeEarning::new(
                1,
                44,
                1,
                1,
                LocalDate::of(2012, 6, 20),
                2.0,
                0.0,
                EarningSource::Rule,
            );
            let mut rule = PriorBalancesRateRule::new(FixedTransactions(Vec::new()));

            rule.execute(&dataset, &mut earning, &params(1, 2));

            assert_eq!(earning.rate(), 0.0);
        }

        /// `PriorBalancesRateRuleImplTest`: "Rate on EmployeeEarning is
        /// calculated based on prior ending balances for the configured
        /// hours and costs accrual and values from db should be cached".
        #[test]
        fn rate_divides_wages_by_hours_and_caches_the_balances() {
            let dataset = TimeCardData::new();
            let date = LocalDate::of(2012, 6, 20);
            let mut earning =
                EmployeeEarning::new(1, 44, 1, 1, date, 2.0, 0.0, EarningSource::Rule);
            let transactions = vec![
                AccrualTransaction::new(1, 11, date, 0.0, 2.0),
                AccrualTransaction::new(2, 22, date, 0.0, 8.0),
            ];
            let mut rule = PriorBalancesRateRule::new(FixedTransactions(transactions));

            rule.execute(&dataset, &mut earning, &params(11, 22));

            assert_eq!(earning.rate(), 4.0);

            let cached = rule.balances_by_employee.get(&44).unwrap();
            assert_eq!(cached.get(&11), Some(&2.0));
            assert_eq!(cached.get(&22), Some(&8.0));
        }

        /// `PriorBalancesRateRuleImplTest`: "the rule should look for cached
        /// values before getting them from the db".
        #[test]
        fn cached_values_are_used_before_querying_again() {
            let dataset = TimeCardData::new();
            let date = LocalDate::of(2012, 6, 20);
            let mut earning =
                EmployeeEarning::new(1, 44, 1, 1, date, 2.0, 0.0, EarningSource::Rule);
            let mut rule = PriorBalancesRateRule::new(FixedTransactions(Vec::new()));
            rule.balances_by_employee
                .insert(44, HashMap::from([(11, 3.0), (22, 9.0)]));

            rule.execute(&dataset, &mut earning, &params(11, 22));

            assert_eq!(earning.rate(), 3.0);
        }
    }
}
