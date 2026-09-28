//! `ForecastMode.java`. `DATA_MODEL.md` §2 originally deferred this enum to Phase 4 (only
//! `RegressionAnalyzer`'s single `is_actual_mode()` dependency was pulled into Phase 2), but
//! `KBIReaderWriter`/`LaborKBIReaderWriter`'s mode-dependent read/write selection logic — this
//! phase's actual subject — switches on every variant directly, so it is ported here instead. See
//! `PARITY_AUDIT.md` Phase 3 for the note.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForecastMode {
    Rooms,
    Revenue,
    UpdateSys,
    UpdateFst,
    UpdateAct,
    Actual,
}

impl ForecastMode {
    pub fn code(self) -> &'static str {
        match self {
            ForecastMode::Rooms => "RMS",
            ForecastMode::Revenue => "REV",
            ForecastMode::UpdateSys => "UPS",
            ForecastMode::UpdateFst => "UPF",
            ForecastMode::UpdateAct => "UPA",
            ForecastMode::Actual => "ACT",
        }
    }

    /// `fromCode(code)`. Returns `None` instead of throwing `IllegalArgumentException`.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "RMS" => Some(ForecastMode::Rooms),
            "REV" => Some(ForecastMode::Revenue),
            "UPS" => Some(ForecastMode::UpdateSys),
            "UPF" => Some(ForecastMode::UpdateFst),
            "UPA" => Some(ForecastMode::UpdateAct),
            "ACT" => Some(ForecastMode::Actual),
            _ => None,
        }
    }

    /// `isUpdate()`.
    pub fn is_update(self) -> bool {
        matches!(self, ForecastMode::UpdateSys | ForecastMode::UpdateFst | ForecastMode::UpdateAct)
    }

    /// `LaborKBIReaderWriter`'s constructor: `defaultStatType` derived from the mode.
    pub fn default_stat_type(self) -> crate::kbi::KbiStatType {
        use crate::kbi::KbiStatType;
        match self {
            ForecastMode::Rooms | ForecastMode::Revenue | ForecastMode::UpdateFst => KbiStatType::Adjusted,
            ForecastMode::Actual | ForecastMode::UpdateAct => KbiStatType::Actual,
            ForecastMode::UpdateSys => KbiStatType::Forecasted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_round_trips_through_from_code() {
        for mode in [
            ForecastMode::Rooms,
            ForecastMode::Revenue,
            ForecastMode::UpdateSys,
            ForecastMode::UpdateFst,
            ForecastMode::UpdateAct,
            ForecastMode::Actual,
        ] {
            assert_eq!(ForecastMode::from_code(mode.code()), Some(mode));
        }
    }

    #[test]
    fn from_code_rejects_unknown_codes() {
        assert_eq!(ForecastMode::from_code("XYZ"), None);
    }

    #[test]
    fn only_update_modes_report_is_update() {
        assert!(ForecastMode::UpdateSys.is_update());
        assert!(ForecastMode::UpdateFst.is_update());
        assert!(ForecastMode::UpdateAct.is_update());
        assert!(!ForecastMode::Rooms.is_update());
        assert!(!ForecastMode::Revenue.is_update());
        assert!(!ForecastMode::Actual.is_update());
    }

    #[test]
    fn default_stat_type_matches_the_constructor_switch() {
        use crate::kbi::KbiStatType;
        assert_eq!(ForecastMode::Rooms.default_stat_type(), KbiStatType::Adjusted);
        assert_eq!(ForecastMode::Revenue.default_stat_type(), KbiStatType::Adjusted);
        assert_eq!(ForecastMode::UpdateFst.default_stat_type(), KbiStatType::Adjusted);
        assert_eq!(ForecastMode::Actual.default_stat_type(), KbiStatType::Actual);
        assert_eq!(ForecastMode::UpdateAct.default_stat_type(), KbiStatType::Actual);
        assert_eq!(ForecastMode::UpdateSys.default_stat_type(), KbiStatType::Forecasted);
    }
}
