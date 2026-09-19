//! Port of `CalculatedAccrualRateRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/earningrate/CalculatedAccrualRateRuleImpl.java`.
//!
//! The family's largest and most stateful rule (230-line spec) — blends an
//! accrual payout's rate across every unapplied hours/cost transaction
//! bucket for the employee, oldest first, falling back to the current job
//! rate for the newest ("last") bucket when it is older than a year or for
//! hours left over once every bucket is exhausted. Caches per-employee
//! bucket state across calls within one run — see the family's "the stateful
//! pair" doc in `mod.rs`.
//!
//! # The bucket-pairing shape
//!
//! `AccrualTransactionDAO.getTransactionsForEmployeeWithUnappliedHours`
//! returns one flat list of transactions across both the hours-accrual and
//! cost-accrual earning types; Java groups them by earning-type id, then
//! pairs an hours transaction with the cost transaction sharing its
//! `asOfDate` (dollars-per-hour buckets, one pair per pay period the accrual
//! ran). Each pair becomes one [`DateRateUnapplied`] bucket: a rate, an
//! unapplied-hours balance, and whether it is the *newest* bucket (by
//! `asOfDate`) — the one bucket close enough to "now" that a stale rate gets
//! replaced by the current job rate instead.
//!
//! # A mismatched pair count triggers a Java bug, reproduced
//!
//! When the hours and cost transaction counts differ, Java's imbalance
//! handling only works in one direction. If there are more hours
//! transactions than cost transactions, it correctly drops the
//! unmatched hours ones (`removeIf` filtering on `costMap.containsKey(asOfDate)`).
//! But if there are *fewer* hours transactions than cost ones, it runs
//! `costMap.keySet().removeIf(key -> !hoursTransactions.contains(key))` —
//! comparing a `LocalDate` key against a `List<AccrualTransaction>` with
//! `List.contains`, which can never be `true` for any element (an
//! `AccrualTransaction` is never `.equals()` to a `LocalDate`). So the
//! predicate `!hoursTransactions.contains(key)` is always `true`, and the
//! branch empties the entire cost map rather than trimming it. Reproduced as
//! written — an hours-transaction-count shortfall leaves every hours
//! transaction with no matching cost transaction, which
//! `hours_transaction_to_bucket`'s `calculate_rate` already handles as its
//! own branch (falls back to the home job rate).
//!
//! Ported cases: `CalculatedAccrualRateRuleImplTest.groovy` (three cases).

use crate::common::numbers::{
    round_accrual_hours, round_currency, round_display_currency, safe_divide_default,
};
use crate::entity::accrual_transaction::AccrualTransaction;
use crate::entity::employee::Employee;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::earningrate::EarningRateRule;
use crate::rules::algorithm::earningrate::config::{
    CALC_ACCRUAL_HOURS_ACCRUAL, COST_ACCRUAL, CalculatedAccrualRateRuleConfig,
};
use crate::rules::params::RuleParams;
use crate::rules::ports::{AccrualTransactionPort, EmployeeEarningPort};
use crate::rules::rule_config::RuleConfig;
use joda_rs::LocalDate;
use std::collections::HashMap;

const RATE_NOTE_TEXT: &str = "Calculated Accrual Rate Rule: ";

/// One dated bucket of unapplied hours and the rate to pay them at.
/// `CalculatedAccrualRateRuleImpl.DateRateUnapplied`.
#[derive(Debug, Clone, Copy)]
struct DateRateUnapplied {
    date: LocalDate,
    rate: f64,
    unapplied: f64,
    last_bucket: bool,
}

/// `CalculatedAccrualRateRuleImpl`.
///
/// `&mut self` for the per-employee bucket cache — see the family's "the
/// stateful pair" doc in `mod.rs`.
pub struct CalculatedAccrualRateRule<A: AccrualTransactionPort, EE: EmployeeEarningPort> {
    accrual_transactions: A,
    employee_earnings: EE,
    /// `dateRateUnappliedByEmployeeId`.
    buckets_by_employee: HashMap<i32, Vec<DateRateUnapplied>>,
}

impl<A: AccrualTransactionPort, EE: EmployeeEarningPort> CalculatedAccrualRateRule<A, EE> {
    /// Build the rule over the ports its accrual transactions and banked-rate
    /// fallback come from.
    pub fn new(accrual_transactions: A, employee_earnings: EE) -> Self {
        Self {
            accrual_transactions,
            employee_earnings,
            buckets_by_employee: HashMap::new(),
        }
    }

    /// `processEarning`.
    fn process_earning(
        &mut self,
        employee: &Employee,
        earning: &mut EmployeeEarning,
        hours_accrual_id: i32,
        cost_accrual_id: i32,
    ) {
        let employee_id = employee.id();
        let needs_refresh = self
            .buckets_by_employee
            .get(&employee_id)
            .is_none_or(|buckets| buckets.len() == 1);

        if needs_refresh {
            let buckets = self.build_buckets_for_employee(
                employee,
                earning,
                hours_accrual_id,
                cost_accrual_id,
            );
            self.buckets_by_employee.insert(employee_id, buckets);
        }

        self.set_earning_rate_and_note(employee, earning);
    }

    /// `createDateRateUnappliedCacheData`.
    fn build_buckets_for_employee(
        &self,
        employee: &Employee,
        earning: &EmployeeEarning,
        hours_accrual_id: i32,
        cost_accrual_id: i32,
    ) -> Vec<DateRateUnapplied> {
        let transactions = self
            .accrual_transactions
            .transactions_for_employee_with_unapplied_hours(
                employee.id(),
                &[hours_accrual_id, cost_accrual_id],
            );

        let mut hours_transactions: Vec<AccrualTransaction> = transactions
            .iter()
            .copied()
            .filter(|t| t.earning_type_id() == hours_accrual_id)
            .collect();
        let mut cost_by_as_of_date: HashMap<LocalDate, AccrualTransaction> = transactions
            .iter()
            .copied()
            .filter(|t| t.earning_type_id() == cost_accrual_id)
            .map(|t| (t.as_of_date(), t))
            .collect();

        if hours_transactions.len() != cost_by_as_of_date.len() {
            if hours_transactions.len() > cost_by_as_of_date.len() {
                hours_transactions.retain(|t| cost_by_as_of_date.contains_key(&t.as_of_date()));
            } else {
                // A Java bug this port reproduces — see the module doc's
                // "a mismatched pair count triggers a Java bug" section.
                cost_by_as_of_date.clear();
            }
        }

        let last_bucket_date = cost_by_as_of_date.keys().max().copied();

        hours_transactions.sort_by_key(AccrualTransaction::as_of_date);
        hours_transactions
            .into_iter()
            .map(|hours_transaction| {
                self.hours_transaction_to_bucket(
                    employee,
                    earning,
                    hours_transaction,
                    &cost_by_as_of_date,
                    last_bucket_date,
                    hours_accrual_id,
                    cost_accrual_id,
                )
            })
            .collect()
    }

    /// `hoursTransactionToDateRateUnapplied`.
    #[allow(clippy::too_many_arguments)]
    fn hours_transaction_to_bucket(
        &self,
        employee: &Employee,
        earning: &EmployeeEarning,
        hours_transaction: AccrualTransaction,
        cost_by_as_of_date: &HashMap<LocalDate, AccrualTransaction>,
        last_bucket_date: Option<LocalDate>,
        hours_accrual_id: i32,
        cost_accrual_id: i32,
    ) -> DateRateUnapplied {
        let transaction_date = hours_transaction.as_of_date();
        let matching_cost = cost_by_as_of_date.get(&transaction_date).copied();
        let unapplied_hours = hours_transaction.unapplied_hours();

        let mut rate = self.calculate_rate(
            employee,
            earning.earning_date(),
            transaction_date,
            unapplied_hours,
            matching_cost,
        );
        let mut last_bucket = false;

        if last_bucket_date == Some(transaction_date) {
            let current_job_rate =
                employee_job_rate_at(employee, earning.job_id(), earning.earning_date());
            let banked_rate = match matching_cost {
                Some(cost) => safe_divide_default(cost.unapplied_hours(), unapplied_hours),
                None => self
                    .employee_earnings
                    .banked_rate_for_rule(employee.id(), hours_accrual_id, cost_accrual_id)
                    .unwrap_or_else(|| panic!("no banked rate for employee {}", employee.id())),
            };

            rate = if is_older_than_one_year(last_bucket_date.unwrap(), earning.earning_date()) {
                round_display_currency(banked_rate)
            } else {
                current_job_rate.max(round_display_currency(banked_rate))
            };
            last_bucket = true;
        }

        DateRateUnapplied {
            date: transaction_date,
            rate,
            unapplied: unapplied_hours,
            last_bucket,
        }
    }

    /// `calculateRate`.
    fn calculate_rate(
        &self,
        employee: &Employee,
        earning_date: LocalDate,
        transaction_date: LocalDate,
        unapplied_hours: f64,
        matching_cost: Option<AccrualTransaction>,
    ) -> f64 {
        match matching_cost {
            Some(cost) => {
                let rate = round_display_currency(crate::common::numbers::safe_divide(
                    cost.unapplied_hours(),
                    unapplied_hours,
                    4,
                    0.0,
                ));
                if rate == 0.0 {
                    home_job_rate_at(employee, transaction_date)
                } else {
                    rate
                }
            }
            None => home_job_rate_at(employee, earning_date),
        }
    }

    /// `setEarningRateAndNote`.
    fn set_earning_rate_and_note(&mut self, employee: &Employee, earning: &mut EmployeeEarning) {
        let hours_by_rate = self.hours_to_apply_by_rate(employee, earning);
        let rate = blended_rate(&hours_by_rate);
        let note = earning_note(earning, &hours_by_rate);

        earning.set_rate(rate);
        earning.set_note(note);
    }

    /// `getHoursToApplyByRate`. Insertion-ordered by ascending bucket date —
    /// see the note on [`blended_rate`]'s sibling, `earning_note`, about why
    /// order is kept rather than using a plain hash map.
    fn hours_to_apply_by_rate(
        &mut self,
        employee: &Employee,
        earning: &EmployeeEarning,
    ) -> Vec<(f64, f64)> {
        let mut remaining = earning.hours();
        let mut hours_by_rate: Vec<(f64, f64)> = Vec::new();

        if let Some(buckets) = self.buckets_by_employee.get_mut(&employee.id()) {
            let mut buckets: Vec<&mut DateRateUnapplied> = buckets
                .iter_mut()
                .filter(|bucket| bucket.unapplied > 0.0)
                .collect();
            buckets.sort_by_key(|bucket| bucket.date);

            for bucket in buckets {
                if remaining == 0.0 {
                    break;
                }

                let mut rate = bucket.rate;
                if bucket.last_bucket {
                    let current_job_rate =
                        employee_job_rate_at(employee, earning.job_id(), earning.earning_date());
                    if current_job_rate > rate {
                        rate = current_job_rate;
                    }
                }

                let hours_to_apply = remaining.min(bucket.unapplied);
                remaining -= hours_to_apply;
                merge_hours(&mut hours_by_rate, rate, hours_to_apply);
                bucket.unapplied -= hours_to_apply;
            }
        }

        if remaining > 0.0 {
            let rate = employee_job_rate_at(employee, earning.job_id(), earning.earning_date());
            merge_hours(&mut hours_by_rate, rate, remaining);
        }

        hours_by_rate
    }
}

impl<A: AccrualTransactionPort, EE: EmployeeEarningPort> EarningRateRule
    for CalculatedAccrualRateRule<A, EE>
{
    fn execute(
        &mut self,
        dataset: &dyn TimeCard,
        earning: &mut EmployeeEarning,
        params: &RuleParams,
    ) {
        let params = params.fixed(&CalculatedAccrualRateRuleConfig.default_values());
        let hours_accrual_id = params.int_at(CALC_ACCRUAL_HOURS_ACCRUAL);
        let cost_accrual_id = params.int_at(COST_ACCRUAL);

        let employee = dataset
            .employee()
            .expect("no employee on the dataset")
            .clone();
        self.process_earning(&employee, earning, hours_accrual_id, cost_accrual_id);
    }
}

/// `isOlderThanOneYear`.
fn is_older_than_one_year(bucket_date: LocalDate, earning_date: LocalDate) -> bool {
    bucket_date.is_before(earning_date.minus_years(1))
}

/// `homeJobRateAtOnDate`.
fn home_job_rate_at(employee: &Employee, date: LocalDate) -> f64 {
    employee
        .home_employee_job_status(date)
        .map_or(0.0, |status| status.hourly_rate())
}

/// `employeeJobRateAtOnDate`.
fn employee_job_rate_at(employee: &Employee, job_id: i32, date: LocalDate) -> f64 {
    employee
        .employee_job_status(job_id, date)
        .map_or(0.0, |status| round_display_currency(status.hourly_rate()))
}

/// `Map<Double, Double>.merge(rate, hours, Double::sum)` over the
/// insertion-ordered `Vec` [`hours_to_apply_by_rate`](CalculatedAccrualRateRule::hours_to_apply_by_rate) builds.
fn merge_hours(hours_by_rate: &mut Vec<(f64, f64)>, rate: f64, hours: f64) {
    if let Some(entry) = hours_by_rate.iter_mut().find(|(r, _)| *r == rate) {
        entry.1 += hours;
    } else {
        hours_by_rate.push((rate, hours));
    }
}

/// `calculateBlendedRate`.
fn blended_rate(hours_by_rate: &[(f64, f64)]) -> f64 {
    let total_costs = round_currency(hours_by_rate.iter().map(|(rate, hours)| rate * hours).sum());
    let total_hours = round_accrual_hours(hours_by_rate.iter().map(|(_, hours)| hours).sum());
    round_display_currency(safe_divide_default(total_costs, total_hours))
}

/// `createEarningNote`.
fn earning_note(earning: &EmployeeEarning, hours_by_rate: &[(f64, f64)]) -> String {
    let original = original_note_without_rate_text(earning.note());
    let update = hours_by_rate
        .iter()
        .map(|(rate, hours)| {
            // Java's `Double.toString` always shows a decimal point, even for
            // a whole number (`4.0`, not `4`) — Rust's `Display` for `f64`
            // drops it, so `Debug` is used here instead to match.
            format!(
                "{hours:?} hours paid at {:?}",
                round_display_currency(*rate)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");

    format!("{original}{RATE_NOTE_TEXT}{update}")
        .trim()
        .to_string()
}

/// `getOriginalNoteWithoutRateText`.
fn original_note_without_rate_text(note: &str) -> &str {
    match note.find(RATE_NOTE_TEXT) {
        Some(index) => &note[..index],
        None => note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::earning_source::EarningSource;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;

    struct FixedTransactions(Vec<AccrualTransaction>);
    impl AccrualTransactionPort for FixedTransactions {
        fn transactions_for_employee_with_unapplied_hours(
            &self,
            _employee_id: i32,
            _earning_type_ids: &[i32],
        ) -> Vec<AccrualTransaction> {
            self.0.clone()
        }
        fn latest_transactions_prior_to_pay_period(
            &self,
            _employee_id: i32,
        ) -> Vec<AccrualTransaction> {
            Vec::new()
        }
    }

    struct FixedBankedRate(Option<f64>);
    impl EmployeeEarningPort for FixedBankedRate {
        fn save(&mut self, _earning: EmployeeEarning) {}
        fn remove(&mut self, _earning_id: i32) {}
        fn earning_hours_for_period_and_types(
            &self,
            _employee_id: i32,
            _period: &date_range_rs::DateRange,
            _earning_type_ids: &[i32],
        ) -> Option<f64> {
            None
        }
        fn banked_rate_for_rule(
            &self,
            _employee_id: i32,
            _hours_earning_type_id: i32,
            _cost_earning_type_id: i32,
        ) -> Option<f64> {
            self.0
        }
    }

    fn params(hours_id: i32, cost_id: i32) -> RuleParams {
        rule_params! {
            CALC_ACCRUAL_HOURS_ACCRUAL => hours_id.to_string(),
            COST_ACCRUAL => cost_id.to_string()
        }
    }

    mod java_parity_tests {
        use super::*;

        /// `CalculatedAccrualRateRuleImplTest`: "rate and note is properly
        /// set".
        #[test]
        fn rate_and_note_is_properly_set() {
            let pay_period_end_date = LocalDate::of(2020, 6, 15);
            let home_job_rate = 18.75;
            let employee = Employee::new(
                2,
                1,
                "",
                vec![crate::entity::employee_job_status::EmployeeJobStatus::new(
                    1,
                    2,
                    1,
                    pay_period_end_date,
                    pay_period_end_date,
                    crate::common::enums::employee_pay_type::EmployeePayType::Hourly,
                    home_job_rate,
                    true,
                )],
            );
            let dataset = TimeCardData::new().with_employee(employee.clone());
            let hours_id = 3;
            let cost_id = 4;
            let mut earning = EmployeeEarning::new(
                1,
                2,
                1,
                1,
                pay_period_end_date,
                8.0,
                0.0,
                EarningSource::Rule,
            );
            earning.set_note("some note");

            let hours_txn = AccrualTransaction::new(1, hours_id, pay_period_end_date, 8.0, 0.0);
            let cost_txn = AccrualTransaction::new(2, cost_id, pay_period_end_date, 150.0, 0.0);

            let mut rule = CalculatedAccrualRateRule::new(
                FixedTransactions(vec![hours_txn, cost_txn]),
                FixedBankedRate(Some(18.75)),
            );

            rule.execute(&dataset, &mut earning, &params(hours_id, cost_id));

            assert_eq!(earning.rate(), 18.75);
            assert_eq!(
                earning.note(),
                "some noteCalculated Accrual Rate Rule: 8.0 hours paid at 18.75"
            );
        }

        /// `CalculatedAccrualRateRuleImplTest`: "blended rates are properly
        /// set".
        #[test]
        fn blended_rates_are_properly_set() {
            let later = LocalDate::of(2020, 6, 15);
            let earlier = LocalDate::of(2020, 5, 15);
            let employee = Employee::new(2, 1, "", Vec::new());
            let dataset = TimeCardData::new().with_employee(employee.clone());
            let hours_id = 3;
            let cost_id = 4;
            let mut earning =
                EmployeeEarning::new(1, 2, 1, 1, later, 8.0, 0.0, EarningSource::Rule);
            earning.set_note("some note ");

            let transactions = vec![
                AccrualTransaction::new(1, hours_id, earlier, 4.0, 0.0),
                AccrualTransaction::new(2, cost_id, earlier, 40.0, 0.0),
                AccrualTransaction::new(3, hours_id, later, 10.0, 0.0),
                AccrualTransaction::new(4, cost_id, later, 150.0, 0.0),
            ];

            let mut rule = CalculatedAccrualRateRule::new(
                FixedTransactions(transactions),
                FixedBankedRate(Some(18.75)),
            );

            rule.execute(&dataset, &mut earning, &params(hours_id, cost_id));

            assert_eq!(earning.rate(), 12.5);
            assert_eq!(
                earning.note(),
                "some note Calculated Accrual Rate Rule: 4.0 hours paid at 10.0, 4.0 hours paid at 15.0"
            );
        }

        /// `CalculatedAccrualRateRuleImplTest`: "job rate on earning date is
        /// used if no hours / cost transaction pairs exist".
        #[test]
        fn job_rate_on_earning_date_is_used_if_no_hours_cost_transaction_pairs_exist() {
            let pay_period_end_date = LocalDate::of(2020, 6, 15);
            let home_job = 101;
            let secondary_job = 102;
            let home_job_rate = 15.0;
            let secondary_job_rate = 10.0;
            let employee = Employee::new(
                2,
                1,
                "",
                vec![
                    crate::entity::employee_job_status::EmployeeJobStatus::new(
                        1,
                        2,
                        home_job,
                        pay_period_end_date,
                        pay_period_end_date,
                        crate::common::enums::employee_pay_type::EmployeePayType::Hourly,
                        home_job_rate,
                        false,
                    ),
                    crate::entity::employee_job_status::EmployeeJobStatus::new(
                        2,
                        2,
                        secondary_job,
                        pay_period_end_date,
                        pay_period_end_date,
                        crate::common::enums::employee_pay_type::EmployeePayType::Hourly,
                        secondary_job_rate,
                        false,
                    ),
                ],
            );
            let dataset = TimeCardData::new().with_employee(employee.clone());
            let hours_id = 3;
            let cost_id = 4;

            let mut earning = EmployeeEarning::new(
                1,
                2,
                home_job,
                1,
                pay_period_end_date,
                8.0,
                0.0,
                EarningSource::Rule,
            );
            let mut secondary_earning = EmployeeEarning::new(
                2,
                2,
                secondary_job,
                1,
                pay_period_end_date,
                8.0,
                0.0,
                EarningSource::Rule,
            );

            let mut rule = CalculatedAccrualRateRule::new(
                FixedTransactions(Vec::new()),
                FixedBankedRate(Some(18.75)),
            );

            rule.execute(&dataset, &mut earning, &params(hours_id, cost_id));
            assert_eq!(earning.rate(), home_job_rate);
            assert_eq!(
                earning.note(),
                "Calculated Accrual Rate Rule: 8.0 hours paid at 15.0"
            );

            rule.execute(&dataset, &mut secondary_earning, &params(hours_id, cost_id));
            assert_eq!(secondary_earning.rate(), secondary_job_rate);
            assert_eq!(
                secondary_earning.note(),
                "Calculated Accrual Rate Rule: 8.0 hours paid at 10.0"
            );
        }
    }
}
