//! `RowData.java` (data-holding parts only) and `DateEnvIDPair.java`.
//!
//! Java's `RowData` mixes a fixed-size sample buffer with DB-loading methods
//! (`loadDependentData`/`loadIndependentData`, both walking backwards week-by-week through
//! `KBIReaderWriter`/`LaborKBIReaderWriter` lookups). Those loaders are the Reader/Writer I/O
//! boundary (Phase 3), not the numeric core — only the data-holding shape (`count`/`dataset`/
//! `weight`) is ported here; `RegressionAnalyzer`'s port trait (once written) is what will
//! populate a `RowData` the way `loadDependentData`/`loadIndependentData` do today.

use joda_rs::LocalDate;

/// `RegressionAnalyzer.MAXSAM`/`MAXCOL` — the Java constants live on `RegressionAnalyzer`, but
/// `RowData`, `RStats`, and `Stats` all size arrays off them, so they're hoisted here as the one
/// place every analyzer imports them from.
pub const MAXSAM: usize = 13;
pub const MAXCOL: usize = 15;

/// `RowData.java`.
#[derive(Debug, Clone, PartialEq)]
pub struct RowData {
    pub count: usize,
    /// `new double[RegressionAnalyzer.MAXCOL - 2]`, zero-filled.
    pub dataset: Vec<f64>,
    pub weight: f64,
}

impl RowData {
    /// `RowData(KBIReaderWriter kbiReaderWriter, KBI kbi)` — the zero-filled constructor used
    /// before a loader populates `dataset`.
    pub fn new() -> Self {
        RowData {
            count: 0,
            dataset: vec![0.0; MAXCOL - 2],
            weight: 0.0,
        }
    }

    /// `RowData(double[] array)` — builds a `RowData` directly from residuals (used by
    /// `Stats.summarizeStats`/`RegressionAnalyzer.durbin`).
    pub fn from_array(array: &[f64]) -> Self {
        RowData {
            count: array.len(),
            dataset: array.to_vec(),
            weight: 0.0,
        }
    }
}

impl Default for RowData {
    fn default() -> Self {
        Self::new()
    }
}

/// `DateEnvIDPair.java`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DateEnvIdPair {
    date: Option<LocalDate>,
    env_id: i32,
    valid: bool,
}

impl DateEnvIdPair {
    /// `DateEnvIDPair(int envID)`.
    pub fn new(env_id: i32) -> Self {
        DateEnvIdPair {
            date: None,
            env_id,
            valid: false,
        }
    }

    /// `DateEnvIDPair(TDate date, int envID)`.
    pub fn with_date(date: LocalDate, env_id: i32) -> Self {
        DateEnvIdPair {
            date: Some(date),
            env_id,
            valid: true,
        }
    }

    /// `setDate`: also flips `valid` true once `envID >= 0` (which is always true for a
    /// non-negative env id; the Java default-constructed `envID = -1` sentinel is the only value
    /// that keeps this false).
    pub fn set_date(&mut self, date: LocalDate) {
        self.date = Some(date);
        if self.env_id >= 0 {
            self.valid = true;
        }
    }

    pub fn date(&self) -> Option<LocalDate> {
        self.date
    }

    pub fn env_id(&self) -> i32 {
        self.env_id
    }

    pub fn set_valid(&mut self, valid: bool) {
        self.valid = valid;
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_row_data_is_zero_filled_and_sized_maxcol_minus_2() {
        let row = RowData::new();
        assert_eq!(row.dataset.len(), MAXCOL - 2);
        assert!(row.dataset.iter().all(|&v| v == 0.0));
        assert_eq!(row.count, 0);
    }

    #[test]
    fn from_array_copies_values_and_sets_count() {
        let row = RowData::from_array(&[1.0, 2.0, 3.0]);
        assert_eq!(row.count, 3);
        assert_eq!(row.dataset, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn default_env_id_pair_starts_invalid() {
        let pair = DateEnvIdPair::new(5);
        assert!(!pair.is_valid());
        assert_eq!(pair.env_id(), 5);
    }

    #[test]
    fn set_date_marks_valid_for_non_negative_env_id() {
        let mut pair = DateEnvIdPair::new(0);
        assert!(!pair.is_valid());
        pair.set_date(LocalDate::of(2026, 1, 1));
        assert!(pair.is_valid());
    }
}
