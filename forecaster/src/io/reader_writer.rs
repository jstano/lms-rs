//! `KBIReaderWriter`/`LaborKBIReaderWriter`'s mode-dependent Forecast/Adjusted/Actual value
//! read/write selection logic (`readKBIValue`/`writeKBIValue`), on top of `KbiStatReaderPort`/
//! `KbiStatWriterPort`. `LaborKBIReaderWriter` is confirmed (Phase 2's audit note) to be the only
//! concrete `KBIReaderWriter` subtype this engine runs with, so its overrides are ported directly
//! rather than modeling an abstract base + one subclass.
//!
//! `KBI.computeValue(date, statType)` — the actual per-KBI-type compute dispatch
//! (`Kbi::compute_value`, Phase 4 territory since every Java override just delegates to a
//! `KBIReaderWriter` method) — is not implemented yet, so `read_kbi_value` takes it as a `compute`
//! callback rather than calling into `Kbi` directly. The in-memory `kbiValueMap` cache
//! (`getKBIStatData`/`putKBIStatData`) is modeled as a plain `HashMap` field, since this crate has
//! no request-scoped object to own it yet.

use std::collections::HashMap;

use joda_rs::LocalDate;

use crate::engine::ForecasterError;
use crate::io::{ForecastMode, KbiStatData};
use crate::kbi::{Kbi, KbiStatType, KbiType};
use crate::PropertyId;

pub struct KbiReaderWriter {
    forecast_mode: ForecastMode,
    default_stat_type: KbiStatType,
    cache: HashMap<(i32, LocalDate), KbiStatData>,
}

impl KbiReaderWriter {
    /// `new LaborKBIReaderWriter(...)` — `defaultStatType` is derived from `forecastMode` in the
    /// Java constructor's if/else chain (`ForecastMode::default_stat_type`).
    pub fn new(forecast_mode: ForecastMode) -> Self {
        KbiReaderWriter {
            forecast_mode,
            default_stat_type: forecast_mode.default_stat_type(),
            cache: HashMap::new(),
        }
    }

    fn cache_key(kbi: &Kbi, date: LocalDate) -> (i32, LocalDate) {
        (kbi.record().id.0, date)
    }

    /// `getKBIStatData(KBI, TDate)` — cache-only lookup, no DB.
    fn cached(&self, kbi: &Kbi, date: LocalDate) -> Option<&KbiStatData> {
        self.cache.get(&Self::cache_key(kbi, date))
    }

    /// `putKBIStatData`.
    fn put(&mut self, data: KbiStatData) {
        self.cache.insert((data.kbi_id.0, data.date), data);
    }

    /// `LaborKBIReaderWriter.readKBIStatData(KBI, TDate)` — cache-then-DB read, caching the
    /// result. `KbiStatReaderPort::read_kbi_stat_data` is `readKBIStatData(int kbiID, TDate date)`.
    fn read_kbi_stat_data(
        &mut self,
        port: &dyn crate::io::KbiStatReaderPort,
        kbi: &Kbi,
        date: LocalDate,
        property_id: PropertyId,
    ) -> Result<KbiStatData, ForecasterError> {
        if let Some(data) = self.cached(kbi, date) {
            return Ok(*data);
        }

        let data = port.read_kbi_stat_data(kbi.record().id, date, property_id)?;
        self.put(data);
        Ok(data)
    }

    /// `KBIReaderWriter.readKBIValue(KBI kbi, TDate date, KBIStatType statType)`.
    ///
    /// `day_of_week`/`range_start` stand in for `TDate.dayOfWeek()`/`getDates().getStartDate()`
    /// (`TDatePeriod`, not ported — Phase 4's orchestrator owns the actual date range). `compute`
    /// stands in for `kbi.computeValue(date, statType)`, which is itself a nullable `Double` in
    /// Java (`Kbi::compute_value`, Phase 4's `engine::compute::compute_kbi_value`, returns
    /// `Option<f64>` for exactly this reason — `RegressionAnalyzer`/`TAESAnalyzer` can genuinely
    /// return `null`/`None`, e.g. a mis-configured regression with zero independent variables). The
    /// return type here matches: `Option<f64>`, not `f64`, mirroring `readKBIValue`'s own nullable
    /// `Double` return.
    #[allow(clippy::too_many_arguments)]
    pub fn read_kbi_value(
        &mut self,
        port: &dyn crate::io::KbiStatReaderPort,
        kbi: &Kbi,
        date: LocalDate,
        stat_type: Option<KbiStatType>,
        day_of_week: i32,
        range_start: LocalDate,
        property_id: PropertyId,
        compute: impl FnOnce(KbiStatType) -> Result<Option<f64>, ForecasterError>,
    ) -> Result<Option<f64>, ForecasterError> {
        let stat_type = stat_type.unwrap_or(self.default_stat_type);

        let mut result = self.cached(kbi, date).and_then(|data| data.value(stat_type));

        if result.is_none() && self.cached(kbi, date).is_none() {
            let before_range = date < range_start;
            let needs_read = match self.forecast_mode {
                ForecastMode::UpdateSys | ForecastMode::UpdateFst => kbi.is_editable_kbi() || before_range,
                ForecastMode::UpdateAct => kbi.is_editable_kbi() || matches!(kbi, Kbi::Statistical(_)),
                ForecastMode::Revenue => kbi.record().is_rooms_kbi || before_range,
                ForecastMode::Rooms | ForecastMode::Actual => before_range,
            };

            if needs_read {
                let mut data = self.read_kbi_stat_data(port, kbi, date, property_id)?;

                result = match self.forecast_mode {
                    ForecastMode::UpdateSys => data.fst_value(),
                    ForecastMode::UpdateFst | ForecastMode::Revenue | ForecastMode::Rooms => data.adj_value(),
                    ForecastMode::UpdateAct => {
                        if data.act_value().is_none() {
                            data.set_act_value(Some(0.0));
                            self.put(data);
                        }
                        data.act_value()
                    }
                    ForecastMode::Actual => data.act_value(),
                };
            }
        }

        if !kbi.record().is_open(day_of_week) {
            result = Some(0.0);
        }

        if result.is_none() {
            result = if stat_type == KbiStatType::Actual && kbi.record().kbi_type == KbiType::Statistical {
                Some(0.0)
            } else {
                compute(stat_type)?
            };

            self.write_kbi_value_uncached(port, kbi, date, result, Some(stat_type), property_id)?;
        }

        Ok(result)
    }

    /// `KBIReaderWriter.writeKBIValue(KBI, TDate, Double, KBIStatType)`. The "zeroed out" sanity
    /// check in Java (lines 296-323) only ever `System.out.println`s and re-throws into a
    /// `catch (Exception e) { e.printStackTrace(); }` that swallows it — no control-flow or state
    /// effect survives it, so it's not ported (matches this crate's "no logging infra" precedent
    /// elsewhere, e.g. `formula::context`'s dead `@STAT` branch).
    #[allow(clippy::too_many_arguments)]
    pub fn write_kbi_value(
        &mut self,
        stat_writer: &dyn crate::io::KbiStatWriterPort,
        stat_reader: &dyn crate::io::KbiStatReaderPort,
        kbi: &Kbi,
        date: LocalDate,
        value: Option<f64>,
        stat_type: Option<KbiStatType>,
        property_id: PropertyId,
    ) -> Result<(), ForecasterError> {
        self.write_kbi_value_uncached(stat_reader, kbi, date, value, stat_type, property_id)?;
        let data = *self.cached(kbi, date).expect("just written above");
        stat_writer.write_kbi_stat_data(&data, property_id)
    }

    /// The cache-mutation half of `writeKBIValue`, shared by `read_kbi_value`'s self-write (which
    /// doesn't have a `KbiStatWriterPort` handy — Java's `readKBIValue` calls `writeKBIValue`,
    /// which itself calls `writeKBIStatData`, so this really is one call in Java; split here only
    /// because `read_kbi_value` doesn't take a writer port param) and `write_kbi_value` above.
    fn write_kbi_value_uncached(
        &mut self,
        stat_reader: &dyn crate::io::KbiStatReaderPort,
        kbi: &Kbi,
        date: LocalDate,
        value: Option<f64>,
        stat_type: Option<KbiStatType>,
        property_id: PropertyId,
    ) -> Result<(), ForecasterError> {
        let _ = stat_type.unwrap_or(self.default_stat_type);

        let mut data = match self.cached(kbi, date) {
            Some(data) => *data,
            None => self.read_kbi_stat_data(stat_reader, kbi, date, property_id)?,
        };

        match self.forecast_mode {
            ForecastMode::UpdateFst => {
                if !kbi.is_editable_kbi() {
                    data.set_adj_value(value);
                }
            }
            ForecastMode::UpdateSys => {
                if !kbi.is_editable_kbi() {
                    data.set_fst_value(value);
                }
            }
            ForecastMode::UpdateAct | ForecastMode::Actual => data.set_act_value(value),
            ForecastMode::Revenue => {
                if !kbi.record().is_rooms_kbi {
                    data.set_fst_value(value);
                    data.set_adj_value(value);
                }
            }
            ForecastMode::Rooms => {
                data.set_fst_value(value);
                data.set_adj_value(value);
            }
        }

        self.put(data);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kbi::{KbiType, StatKbi, StatKbiType};
    use crate::{KbiConfigId, KbiId, PropertyId, UnitId};
    use std::cell::RefCell;
    use std::collections::HashMap as Map;

    fn kbi(kbi_type: KbiType, is_rooms: bool) -> Kbi {
        let record = crate::kbi::KbiRecord {
            id: KbiId(1),
            property_id: PropertyId(1),
            name: "Rooms".to_string(),
            code: "RMS".to_string(),
            unit_id: UnitId(1),
            kbi_config_id: KbiConfigId(1),
            kbi_type,
            primary: false,
            is_rooms_kbi: is_rooms,
            is_revenue_center_kbi: false,
            is_departures_kbi: false,
            days_open: Map::new(),
        };
        match kbi_type {
            KbiType::Statistical => Kbi::Statistical(crate::kbi::StatisticalKbiData {
                base: record,
                stat_days: std::array::from_fn(|_| StatKbi::of_type(StatKbiType::Regression)),
            }),
            _ => Kbi::Input(record),
        }
    }

    struct FakePort {
        stat_data: RefCell<HashMap<(i32, LocalDate), KbiStatData>>,
    }

    impl crate::io::KbiStatReaderPort for FakePort {
        fn read_kbi_stat_data(&self, kbi_id: KbiId, date: LocalDate, _property_id: PropertyId) -> Result<KbiStatData, ForecasterError> {
            Ok(self
                .stat_data
                .borrow()
                .get(&(kbi_id.0, date))
                .copied()
                .unwrap_or_else(|| KbiStatData::new(kbi_id, date)))
        }

        fn read_kbi_override_value(&self, _kbi_id: KbiId, _date: LocalDate) -> Result<Option<f64>, ForecasterError> {
            Ok(None)
        }
    }

    fn date() -> LocalDate {
        LocalDate::of(2026, 1, 5)
    }

    #[test]
    fn rooms_mode_reads_adj_value_when_before_range() {
        let mut rw = KbiReaderWriter::new(ForecastMode::Rooms);
        let k = kbi(KbiType::Input, true);
        let mut stored = KbiStatData::new(KbiId(1), date());
        stored.set_adj_value(Some(42.0));
        let mut map = HashMap::new();
        map.insert((1, date()), stored);
        let port = FakePort { stat_data: RefCell::new(map) };

        let before_range = date().plus_days(1);
        let result = rw
            .read_kbi_value(&port, &k, date(), None, 2, before_range, PropertyId(1), |_| {
                panic!("should not need to compute")
            })
            .unwrap();
        assert_eq!(result, Some(42.0));
    }

    #[test]
    fn closed_day_forces_zero_even_with_stored_value() {
        let mut rw = KbiReaderWriter::new(ForecastMode::Rooms);
        let mut k_record = match kbi(KbiType::Input, true) {
            Kbi::Input(r) => r,
            _ => unreachable!(),
        };
        k_record.days_open.insert(2, false);
        let k = Kbi::Input(k_record);

        let mut stored = KbiStatData::new(KbiId(1), date());
        stored.set_adj_value(Some(42.0));
        let mut map = HashMap::new();
        map.insert((1, date()), stored);
        let port = FakePort { stat_data: RefCell::new(map) };

        let result = rw
            .read_kbi_value(&port, &k, date(), None, 2, date().plus_days(1), PropertyId(1), |_| panic!("no compute"))
            .unwrap();
        assert_eq!(result, Some(0.0));
    }

    #[test]
    fn missing_value_falls_through_to_compute_and_caches() {
        let mut rw = KbiReaderWriter::new(ForecastMode::Rooms);
        let k = kbi(KbiType::Input, true);
        let port = FakePort {
            stat_data: RefCell::new(HashMap::new()),
        };

        // date is not before range_start, so no read happens and compute is required.
        let result = rw
            .read_kbi_value(&port, &k, date(), None, 2, date(), PropertyId(1), |stat_type| {
                assert_eq!(stat_type, KbiStatType::Adjusted);
                Ok(Some(7.0))
            })
            .unwrap();
        assert_eq!(result, Some(7.0));

        // second call hits the cache and does not invoke compute again.
        let result2 = rw
            .read_kbi_value(&port, &k, date(), None, 2, date(), PropertyId(1), |_| panic!("cached, should not recompute"))
            .unwrap();
        assert_eq!(result2, Some(7.0));
    }

    #[test]
    fn compute_returning_none_propagates_as_none() {
        let mut rw = KbiReaderWriter::new(ForecastMode::Rooms);
        let k = kbi(KbiType::Statistical, false);
        let port = FakePort {
            stat_data: RefCell::new(HashMap::new()),
        };

        // `RegressionAnalyzer`/`TAESAnalyzer` can genuinely return `None` (e.g. a mis-configured
        // regression with zero independent variables); `readKBIValue`'s own return is nullable
        // `Double` in Java, so this must surface as `None`, not `Some(0.0)`.
        let result = rw
            .read_kbi_value(&port, &k, date(), None, 2, date(), PropertyId(1), |_| Ok(None))
            .unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn actual_stat_type_on_statistical_kbi_short_circuits_to_zero() {
        let mut rw = KbiReaderWriter::new(ForecastMode::Actual);
        let k = kbi(KbiType::Statistical, false);
        let port = FakePort {
            stat_data: RefCell::new(HashMap::new()),
        };

        let result = rw
            .read_kbi_value(&port, &k, date(), Some(KbiStatType::Actual), 2, date(), PropertyId(1), |_| {
                panic!("should short-circuit before compute")
            })
            .unwrap();
        assert_eq!(result, Some(0.0));
    }

    struct FakeWriter {
        written: RefCell<Vec<KbiStatData>>,
    }

    impl crate::io::KbiStatWriterPort for FakeWriter {
        fn write_kbi_stat_data(&self, data: &KbiStatData, _property_id: PropertyId) -> Result<(), ForecasterError> {
            self.written.borrow_mut().push(*data);
            Ok(())
        }
    }

    #[test]
    fn write_kbi_value_in_rooms_mode_sets_fst_and_adj() {
        let mut rw = KbiReaderWriter::new(ForecastMode::Rooms);
        let k = kbi(KbiType::Input, true);
        let reader = FakePort {
            stat_data: RefCell::new(HashMap::new()),
        };
        let writer = FakeWriter { written: RefCell::new(vec![]) };

        rw.write_kbi_value(&writer, &reader, &k, date(), Some(10.0), None, PropertyId(1)).unwrap();

        let written = writer.written.borrow();
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].fst_value(), Some(10.0));
        assert_eq!(written[0].adj_value(), Some(10.0));
    }

    #[test]
    fn write_kbi_value_in_revenue_mode_skips_rooms_kbi() {
        let mut rw = KbiReaderWriter::new(ForecastMode::Revenue);
        let k = kbi(KbiType::Input, true);
        let reader = FakePort {
            stat_data: RefCell::new(HashMap::new()),
        };
        let writer = FakeWriter { written: RefCell::new(vec![]) };

        rw.write_kbi_value(&writer, &reader, &k, date(), Some(10.0), None, PropertyId(1)).unwrap();

        let written = writer.written.borrow();
        assert_eq!(written[0].fst_value(), None);
        assert_eq!(written[0].adj_value(), None);
    }
}
