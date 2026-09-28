//! `KBIStatData.java` — the four value columns (`EstValue`/`FstValue`/`AdjValue`/`ActValue`) for
//! one KBI/date, plus the cache/dirty-tracking flags `LaborKBIReaderWriter` reads and writes.

use joda_rs::LocalDate;

use crate::kbi::KbiStatType;
use crate::KbiId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KbiStatData {
    pub kbi_id: KbiId,
    pub date: LocalDate,
    est_value: Option<f64>,
    fst_value: Option<f64>,
    adj_value: Option<f64>,
    act_value: Option<f64>,
    pub in_database: bool,
    pub calculated: bool,
}

impl KbiStatData {
    /// `new KBIStatData(kbiID, date)`.
    pub fn new(kbi_id: KbiId, date: LocalDate) -> Self {
        KbiStatData {
            kbi_id,
            date,
            est_value: None,
            fst_value: None,
            adj_value: None,
            act_value: None,
            in_database: false,
            calculated: false,
        }
    }

    /// `hasData()`.
    pub fn has_data(&self) -> bool {
        self.est_value.is_some() || self.fst_value.is_some() || self.adj_value.is_some() || self.act_value.is_some()
    }

    pub fn est_value(&self) -> Option<f64> {
        self.est_value
    }

    pub fn set_est_value(&mut self, value: Option<f64>) {
        self.est_value = value;
    }

    pub fn fst_value(&self) -> Option<f64> {
        self.fst_value
    }

    pub fn set_fst_value(&mut self, value: Option<f64>) {
        self.fst_value = value;
    }

    pub fn adj_value(&self) -> Option<f64> {
        self.adj_value
    }

    pub fn set_adj_value(&mut self, value: Option<f64>) {
        self.adj_value = value;
    }

    pub fn act_value(&self) -> Option<f64> {
        self.act_value
    }

    pub fn set_act_value(&mut self, value: Option<f64>) {
        self.act_value = value;
    }

    /// `hasBeenEdited()` — true when `fstValue` and `adjValue` are both present and differ.
    pub fn has_been_edited(&self) -> bool {
        match (self.fst_value, self.adj_value) {
            (Some(fst), Some(adj)) => fst != adj,
            _ => false,
        }
    }

    /// `getValue(KBIStatType)`.
    pub fn value(&self, stat_type: KbiStatType) -> Option<f64> {
        match stat_type {
            KbiStatType::Estimated => self.est_value,
            KbiStatType::Forecasted => self.fst_value,
            KbiStatType::Adjusted => self.adj_value,
            KbiStatType::Actual => self.act_value,
        }
    }

    /// `setValue(KBIStatType, Double)`.
    pub fn set_value(&mut self, stat_type: KbiStatType, value: Option<f64>) {
        match stat_type {
            KbiStatType::Estimated => self.est_value = value,
            KbiStatType::Forecasted => self.fst_value = value,
            KbiStatType::Adjusted => self.adj_value = value,
            KbiStatType::Actual => self.act_value = value,
        }
    }

    /// `valuesEqual(KBIStatData)` — Java rounds to 2 decimal places (`Math.round(value * 100.0)`)
    /// before comparing, rather than exact `f64` equality.
    pub fn values_equal(&self, other: &KbiStatData) -> bool {
        Self::same(self.act_value, other.act_value)
            && Self::same(self.adj_value, other.adj_value)
            && Self::same(self.est_value, other.est_value)
            && Self::same(self.fst_value, other.fst_value)
    }

    fn same(a: Option<f64>, b: Option<f64>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => (a * 100.0).round() == (b * 100.0).round(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date() -> LocalDate {
        LocalDate::of(2026, 1, 1)
    }

    #[test]
    fn has_data_false_when_all_columns_empty() {
        assert!(!KbiStatData::new(KbiId(1), date()).has_data());
    }

    #[test]
    fn has_data_true_when_any_column_set() {
        let mut data = KbiStatData::new(KbiId(1), date());
        data.set_adj_value(Some(3.0));
        assert!(data.has_data());
    }

    #[test]
    fn has_been_edited_when_fst_and_adj_differ() {
        let mut data = KbiStatData::new(KbiId(1), date());
        data.set_fst_value(Some(10.0));
        data.set_adj_value(Some(12.0));
        assert!(data.has_been_edited());
    }

    #[test]
    fn has_not_been_edited_when_fst_or_adj_missing() {
        let mut data = KbiStatData::new(KbiId(1), date());
        data.set_fst_value(Some(10.0));
        assert!(!data.has_been_edited());
    }

    #[test]
    fn value_and_set_value_round_trip_per_stat_type() {
        let mut data = KbiStatData::new(KbiId(1), date());
        data.set_value(KbiStatType::Actual, Some(5.0));
        assert_eq!(data.value(KbiStatType::Actual), Some(5.0));
        assert_eq!(data.value(KbiStatType::Forecasted), None);
    }

    #[test]
    fn values_equal_rounds_to_two_decimal_places() {
        let mut a = KbiStatData::new(KbiId(1), date());
        let mut b = KbiStatData::new(KbiId(1), date());
        a.set_act_value(Some(1.001));
        b.set_act_value(Some(1.004));
        assert!(a.values_equal(&b));

        b.set_act_value(Some(1.02));
        assert!(!a.values_equal(&b));
    }
}
