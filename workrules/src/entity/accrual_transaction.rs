//! Port of `com.unifocus.watson.server.hibernate.entity.AccrualTransaction`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/AccrualTransaction.java`.
//!
//! One dated bucket of an accrual balance — hours or cost — for one employee.
//! `CalculatedAccrualRateRuleImpl` and `PriorBalancesRateRuleImpl` are the only
//! two readers in the rules tree, and between them they read four fields off
//! the Java entity's nineteen: `earningType`, `asOfDate`, `unappliedHours` and
//! `endingTotalBalance`. Only those come across.

use joda_rs::LocalDate;

/// One dated accrual bucket. `AccrualTransaction`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccrualTransaction {
    id: i32,
    earning_type_id: i32,
    as_of_date: LocalDate,
    unapplied_hours: f64,
    ending_total_balance: f64,
}

impl AccrualTransaction {
    /// Build a transaction.
    pub fn new(
        id: i32,
        earning_type_id: i32,
        as_of_date: LocalDate,
        unapplied_hours: f64,
        ending_total_balance: f64,
    ) -> Self {
        Self {
            id,
            earning_type_id,
            as_of_date,
            unapplied_hours,
            ending_total_balance,
        }
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// The accrual bucket this transaction belongs to — an hours accrual or a
    /// cost accrual, per whichever earning type a rule's parameters name.
    /// `getEarningType().getID()`.
    pub fn earning_type_id(&self) -> i32 {
        self.earning_type_id
    }

    /// The date this bucket's balance is as of. `getAsOfDate()`.
    pub fn as_of_date(&self) -> LocalDate {
        self.as_of_date
    }

    /// Hours (or cost) in this bucket not yet applied to an earning.
    /// `getUnappliedHours()`.
    pub fn unapplied_hours(&self) -> f64 {
        self.unapplied_hours
    }

    /// The running total balance as of this transaction's date.
    /// `getEndingTotalBalance()`.
    pub fn ending_total_balance(&self) -> f64 {
        self.ending_total_balance
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_transaction_carries_its_bucket_and_balances() {
        let date = LocalDate::of(2016, 6, 1);
        let transaction = AccrualTransaction::new(1, 3, date, 8.0, 150.0);

        assert_eq!(transaction.id(), 1);
        assert_eq!(transaction.earning_type_id(), 3);
        assert_eq!(transaction.as_of_date(), date);
        assert_eq!(transaction.unapplied_hours(), 8.0);
        assert_eq!(transaction.ending_total_balance(), 150.0);
    }
}
