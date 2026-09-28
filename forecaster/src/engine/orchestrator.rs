//! `ForecastThread.java` — the top-level driver, ported as a plain function
//! (`run_forecast`) rather than a `Thread`/`Progress`/`IThreadListener` object, per
//! `PLAN_FORECASTER.md`'s Phase 4 scoping. Interruption/progress reporting is left as a TBD hook
//! (nothing in this crate calls `isInterrupted()`/`getProgress().ping()` — there is no cooperative-
//! cancellation infrastructure here to hook them into), same treatment `scheduler` gave Hibernate/
//! session concerns at its own I/O boundary.
//!
//! Error handling: Java's `run()` catches `KBIComputeException`/`Exception`, wraps/logs them, and
//! always calls `fireThreadFinished(success)` in a `finally` block, with `success` staying `false`
//! on any failure. `run_forecast` returns `Result<ForecastOutcome, ForecasterError>` instead —
//! `Err` is the failure case Java modeled as "`success = false`, exception propagated to
//! `Progress`", and `Ok(ForecastOutcome { success: true })` is the only way to reach the end of
//! Java's `try` block. `KBIReaderWriter.close()`'s base implementation is an empty method body (no
//! override in `LaborKBIReaderWriter` either) — there is nothing to close here, so it's not ported.

use joda_rs::LocalDate;

use crate::engine::compute::{compute_kbi_value, java_day_of_week, ComputeContext};
use crate::engine::kbi_list::KbiList;
use crate::engine::ports::{KbiSetPort, MarketSegmentCheckPort, RevenueCenterCheckPort};
use crate::engine::ForecasterError;
use crate::formula::FormulaContext;
use crate::analyzers::{PercentOfBaseLookupPort, RegressionLookupPort, StatisticalDispatch, TaesLookupPort};
use crate::io::{ForecastMode, KbiLoaderPort, KbiReaderWriter, KbiStatReaderPort, KbiStatWriterPort};
use crate::kbi::{Kbi, KbiMode, KbiStatType, KbiType};
use crate::{KbiId, PropertyId, StandardSetId};

/// `ForecastThread`'s constructor arguments (`mode`/`startDate`/`endDate`/`standardSetId`/
/// `kbiIds`), plus `property_id`/`kbi_mode`/`period_start_year` — values `IWatsonData`/`Property`
/// supplied implicitly in Java (`getWatsonData().getProperty()`) that this crate has no session
/// object to derive them from.
pub struct ForecastParams {
    pub mode: ForecastMode,
    pub start_date: LocalDate,
    pub end_date: LocalDate,
    pub standard_set_id: StandardSetId,
    pub property_id: PropertyId,
    pub kbi_mode: KbiMode,
    /// `Collection<Integer> kbiIds` — `null` (no filter) maps to `None`.
    pub kbi_ids: Option<Vec<KbiId>>,
    /// `Property.getPeriodStartDate().year()` — only consulted by `check_revenue_centers`
    /// (`mode == Revenue`)'s 3-year calendar-plan-date rebase.
    pub period_start_year: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForecastOutcome {
    pub success: bool,
}

/// Every port trait `run_forecast` needs, bundled by reference. Mirrors
/// `KBIReaderWriter`/`LaborKBIReaderWriter`'s own constructor dependencies
/// (`IWatsonData`/`Connection`) collapsed into the narrow port traits Phases 1-4 each defined.
pub struct ForecastPorts<'a> {
    pub kbi_set: &'a dyn KbiSetPort,
    pub stat_reader: &'a dyn KbiStatReaderPort,
    pub stat_writer: &'a dyn KbiStatWriterPort,
    pub kbi_loader: &'a dyn KbiLoaderPort,
    pub fny: &'a dyn crate::io::FinancialYearPeriodPort,
    pub formula_ctx: &'a dyn FormulaContext,
    pub stat_dispatch: &'a dyn StatisticalDispatch,
    pub regression: &'a dyn RegressionLookupPort,
    pub taes: &'a dyn TaesLookupPort,
    pub percent_of_base: &'a dyn PercentOfBaseLookupPort,
    pub market_segments: &'a dyn MarketSegmentCheckPort,
    pub revenue_centers: &'a dyn RevenueCenterCheckPort,
}

/// `ForecastThread.run()`.
pub fn run_forecast(params: &ForecastParams, ports: &ForecastPorts) -> Result<ForecastOutcome, ForecasterError> {
    // `if (mode == ForecastMode.ACTUAL) mode = ForecastMode.UPDATE_ACT;`
    let mode = if params.mode == ForecastMode::Actual {
        ForecastMode::UpdateAct
    } else {
        params.mode
    };

    let kbi_set_id = ports.kbi_set.kbi_set_id_for_standard_set(params.standard_set_id)?;
    let kbi_list = KbiList::new(ports.kbi_set.load_kbi_list(params.property_id, kbi_set_id)?);

    let mut reader_writer = KbiReaderWriter::new(mode);

    process_kbis(
        &kbi_list,
        &mut reader_writer,
        mode,
        params.start_date,
        params.end_date,
        params.kbi_ids.as_deref(),
        params.property_id,
        params.kbi_mode,
        ports,
    )?;

    match mode {
        ForecastMode::Rooms => check_market_segments(
            &mut reader_writer,
            &kbi_list,
            ports.stat_reader,
            ports.stat_writer,
            ports.market_segments,
            params.property_id,
            params.start_date,
            params.end_date,
        )?,
        ForecastMode::Revenue => check_revenue_centers(
            &mut reader_writer,
            &kbi_list,
            ports.stat_reader,
            ports.stat_writer,
            ports.revenue_centers,
            params.property_id,
            params.standard_set_id,
            params.start_date,
            params.end_date,
            params.period_start_year,
        )?,
        _ => {}
    }

    Ok(ForecastOutcome { success: true })
}

/// `ForecastThread.processKBIs(TDate startDate, TDate endDate)`.
#[allow(clippy::too_many_arguments)]
fn process_kbis(
    kbi_list: &KbiList,
    reader_writer: &mut KbiReaderWriter,
    mode: ForecastMode,
    start_date: LocalDate,
    end_date: LocalDate,
    kbi_ids: Option<&[KbiId]>,
    property_id: PropertyId,
    kbi_mode: KbiMode,
    ports: &ForecastPorts,
) -> Result<(), ForecasterError> {
    let mut date = start_date;
    while date <= end_date {
        for kbi in kbi_list.iter() {
            if skip_if_kbi_is_not_supposed_to_be_generated(kbi, kbi_ids) {
                continue;
            }

            let record = kbi.record();
            // `ForecastMode.ACTUAL && kbi.getKBIType() == CALCULATED && kbi.getKBIType() ==
            // CALCULATED` is Java's literal (redundant-but-harmless) condition — ported as the
            // single check it actually reduces to.
            let should_process = match mode {
                ForecastMode::Rooms => record.is_rooms_kbi,
                ForecastMode::Revenue => record.is_revenue_center_kbi || record.is_dangling_kbi(),
                ForecastMode::UpdateSys | ForecastMode::UpdateFst | ForecastMode::UpdateAct => {
                    matches!(record.kbi_type, KbiType::Calculated | KbiType::PercentOfBase)
                }
                ForecastMode::Actual => record.kbi_type == KbiType::Calculated,
            };

            if should_process {
                process_kbi(kbi, date, reader_writer, start_date, property_id, kbi_mode, ports)?;
            }
        }
        date = date.plus_days(1);
    }
    Ok(())
}

/// `ForecastThread.skipIfKbiIsNotSupposedToBeGenerated`.
fn skip_if_kbi_is_not_supposed_to_be_generated(kbi: &Kbi, kbi_ids: Option<&[KbiId]>) -> bool {
    match kbi_ids {
        Some(ids) => !ids.contains(&kbi.record().id),
        None => false,
    }
}

/// `ForecastThread.processKBI(KBI kbi, TDate date)`.
fn process_kbi(
    kbi: &Kbi,
    date: LocalDate,
    reader_writer: &mut KbiReaderWriter,
    range_start: LocalDate,
    property_id: PropertyId,
    kbi_mode: KbiMode,
    ports: &ForecastPorts,
) -> Result<(), ForecasterError> {
    let day_of_week = java_day_of_week(date);
    let ctx = ComputeContext {
        property_id,
        kbi_mode,
        stat_reader: ports.stat_reader,
        formula_ctx: ports.formula_ctx,
        stat_dispatch: ports.stat_dispatch,
        regression: ports.regression,
        taes: ports.taes,
        percent_of_base: ports.percent_of_base,
        kbi_loader: ports.kbi_loader,
        fny: ports.fny,
    };

    let value = reader_writer.read_kbi_value(ports.stat_reader, kbi, date, None, day_of_week, range_start, property_id, |stat_type| {
        compute_kbi_value(kbi, date, stat_type, &ctx)
    })?;

    reader_writer.write_kbi_value(ports.stat_writer, ports.stat_reader, kbi, date, value, None, property_id)
}

/// `ForecastThread.checkMarketSegments`.
#[allow(clippy::too_many_arguments)]
fn check_market_segments(
    reader_writer: &mut KbiReaderWriter,
    kbi_list: &KbiList,
    stat_reader: &dyn KbiStatReaderPort,
    stat_writer: &dyn KbiStatWriterPort,
    market_segments: &dyn MarketSegmentCheckPort,
    property_id: PropertyId,
    start_date: LocalDate,
    end_date: LocalDate,
) -> Result<(), ForecasterError> {
    for row in market_segments.market_segments(property_id)? {
        let rooms_kbi = kbi_list.get(row.rooms_kbi_id).ok_or_else(|| ForecasterError::DataAccess {
            message: format!("MarketSegment RoomsKBIID {} not found in the loaded KBI list", row.rooms_kbi_id.0),
        })?;
        // `arrivalsKBIID > 0 ? kbiList.getKBI(arrivalsKBIID) : null` — a configured id that isn't
        // in the loaded list also resolves to `null` in Java (a plain `Map` miss), so a missing
        // lookup collapses to `None` exactly like an unconfigured (`<= 0`) id.
        let arrivals_kbi = row.arrivals_kbi_id.and_then(|id| kbi_list.get(id));
        let guests_kbi = row.guests_kbi_id.and_then(|id| kbi_list.get(id));
        let departures_kbi = row.departures_kbi_id.and_then(|id| kbi_list.get(id));

        let mut date = start_date;
        while date <= end_date {
            let yesterday = date.plus_days(-1);

            let rooms = stat_reader.read_kbi_stat_data(row.rooms_kbi_id, date, property_id)?.fst_value().unwrap_or(0.0);

            let mut yrooms = stat_reader.read_kbi_override_value(row.rooms_kbi_id, yesterday)?.unwrap_or(0.0);
            if yrooms == 0.0 {
                yrooms = stat_reader.read_kbi_stat_data(row.rooms_kbi_id, yesterday, property_id)?.fst_value().unwrap_or(yrooms);
            }

            let mut guests = read_fst_value_or_zero(stat_reader, row.guests_kbi_id, date, property_id)?;
            let mut arrivals = read_fst_value_or_zero(stat_reader, row.arrivals_kbi_id, date, property_id)?;
            let mut departs = read_fst_value_or_zero(stat_reader, row.departures_kbi_id, date, property_id)?;

            if arrivals > rooms {
                arrivals = rooms;
                departs = yrooms - rooms + arrivals;
            }

            if departs < 0.0 {
                arrivals += rooms - yrooms;
                departs = yrooms - rooms + arrivals;

                reader_writer.write_kbi_value(stat_writer, stat_reader, rooms_kbi, date, Some(rooms), Some(KbiStatType::Forecasted), property_id)?;

                if let Some(arrivals_kbi) = arrivals_kbi {
                    reader_writer.write_kbi_value(stat_writer, stat_reader, arrivals_kbi, date, Some(arrivals), Some(KbiStatType::Forecasted), property_id)?;
                }
            }

            if guests < rooms {
                if let Some(guests_kbi) = guests_kbi {
                    guests = rooms;
                    reader_writer.write_kbi_value(stat_writer, stat_reader, guests_kbi, date, Some(guests), Some(KbiStatType::Forecasted), property_id)?;
                }
            }

            if let Some(departures_kbi) = departures_kbi {
                reader_writer.write_kbi_value(stat_writer, stat_reader, departures_kbi, date, Some(departs), Some(KbiStatType::Forecasted), property_id)?;
            }

            date = date.plus_days(1);
        }
    }

    Ok(())
}

fn read_fst_value_or_zero(
    stat_reader: &dyn KbiStatReaderPort,
    kbi_id: Option<KbiId>,
    date: LocalDate,
    property_id: PropertyId,
) -> Result<f64, ForecasterError> {
    match kbi_id {
        Some(id) => Ok(stat_reader.read_kbi_stat_data(id, date, property_id)?.fst_value().unwrap_or(0.0)),
        None => Ok(0.0),
    }
}

/// `ForecastThread.checkRevenueCenters`.
#[allow(clippy::too_many_arguments)]
fn check_revenue_centers(
    reader_writer: &mut KbiReaderWriter,
    kbi_list: &KbiList,
    stat_reader: &dyn KbiStatReaderPort,
    stat_writer: &dyn KbiStatWriterPort,
    revenue_centers: &dyn RevenueCenterCheckPort,
    property_id: PropertyId,
    standard_set_id: StandardSetId,
    start_date: LocalDate,
    end_date: LocalDate,
    period_start_year: i32,
) -> Result<(), ForecasterError> {
    let years = [period_start_year - 1, period_start_year, period_start_year + 1];

    for row in revenue_centers.revenue_center_kbis(property_id, standard_set_id)? {
        let kbi = kbi_list.get(row.kbi_id).ok_or_else(|| ForecasterError::DataAccess {
            message: format!("RevenueCenterPeriod KBIID {} not found in the loaded KBI list", row.kbi_id.0),
        })?;

        if row.calendar_plan_id != 0 {
            if let Some(plan) = revenue_centers.calendar_plan(row.calendar_plan_id)? {
                let calendar_dates = revenue_centers.calendar_plan_dates(row.calendar_plan_id)?;

                let mut date = start_date;
                while date <= end_date {
                    let mut in_closed_range = false;
                    'ranges: for range in &calendar_dates {
                        for &year in &years {
                            if range.contains_in_year(date, year) {
                                in_closed_range = true;
                                break 'ranges;
                            }
                        }
                    }

                    let day_index = (java_day_of_week(date) - 1) as usize;
                    if in_closed_range || !plan.days_open[day_index] {
                        reader_writer.write_kbi_value(stat_writer, stat_reader, kbi, date, Some(0.0), Some(KbiStatType::Forecasted), property_id)?;
                    }

                    date = date.plus_days(1);
                }
            }
        }

        // `RevenueCenterPeriodDay`: zeroes out the *first* date in range matching a closed
        // `DayNo` (`break` inside the Java `for (TDate date = ...)` loop exits after one write,
        // not every matching date in the range) — a Java quirk, ported verbatim rather than
        // "fixed" into zeroing every occurrence.
        for (day_no, is_open) in revenue_centers.revenue_center_period_days(row.revenue_center_period_standard_set_id)? {
            if !is_open {
                let mut date = start_date;
                while date <= end_date {
                    if java_day_of_week(date) == day_no {
                        reader_writer.write_kbi_value(stat_writer, stat_reader, kbi, date, Some(0.0), Some(KbiStatType::Forecasted), property_id)?;
                        break;
                    }
                    date = date.plus_days(1);
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ports::{CalendarPlan, CalendarPlanDateRange, MarketSegmentRow, RevenueCenterKbiRow};
    use crate::io::KbiStatData;
    use crate::kbi::{KbiRecord, KbiType};
    use crate::{KbiConfigId, UnitId};
    use std::cell::RefCell;
    use std::collections::HashMap;

    fn record(id: i32, kbi_type: KbiType, is_rooms: bool, is_revenue: bool) -> KbiRecord {
        KbiRecord {
            id: KbiId(id),
            property_id: PropertyId(1),
            name: format!("K{id}"),
            code: format!("K{id}"),
            unit_id: UnitId(1),
            kbi_config_id: KbiConfigId(1),
            kbi_type,
            primary: false,
            is_rooms_kbi: is_rooms,
            is_revenue_center_kbi: is_revenue,
            is_departures_kbi: false,
            days_open: HashMap::new(),
        }
    }

    struct FakeStore {
        stat_data: RefCell<HashMap<(i32, LocalDate), KbiStatData>>,
        overrides: HashMap<(i32, LocalDate), f64>,
        written: RefCell<Vec<KbiStatData>>,
    }

    impl FakeStore {
        fn new() -> Self {
            FakeStore {
                stat_data: RefCell::new(HashMap::new()),
                overrides: HashMap::new(),
                written: RefCell::new(vec![]),
            }
        }

        fn with_fst(self, kbi_id: i32, date: LocalDate, value: f64) -> Self {
            let mut data = KbiStatData::new(KbiId(kbi_id), date);
            data.set_fst_value(Some(value));
            self.stat_data.borrow_mut().insert((kbi_id, date), data);
            self
        }
    }

    impl KbiStatReaderPort for FakeStore {
        fn read_kbi_stat_data(&self, kbi_id: KbiId, date: LocalDate, _property_id: PropertyId) -> Result<KbiStatData, ForecasterError> {
            Ok(self
                .stat_data
                .borrow()
                .get(&(kbi_id.0, date))
                .copied()
                .unwrap_or_else(|| KbiStatData::new(kbi_id, date)))
        }

        fn read_kbi_override_value(&self, kbi_id: KbiId, date: LocalDate) -> Result<Option<f64>, ForecasterError> {
            Ok(self.overrides.get(&(kbi_id.0, date)).copied())
        }
    }

    impl KbiStatWriterPort for FakeStore {
        fn write_kbi_stat_data(&self, data: &KbiStatData, _property_id: PropertyId) -> Result<(), ForecasterError> {
            self.written.borrow_mut().push(*data);
            self.stat_data.borrow_mut().insert((data.kbi_id.0, data.date), *data);
            Ok(())
        }
    }

    fn kbi_list_with(records: Vec<KbiRecord>) -> KbiList {
        KbiList::new(records.into_iter().map(Kbi::Input).collect())
    }

    struct FakeMarketSegments(Vec<MarketSegmentRow>);
    impl MarketSegmentCheckPort for FakeMarketSegments {
        fn market_segments(&self, _property_id: PropertyId) -> Result<Vec<MarketSegmentRow>, ForecasterError> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn market_segments_caps_arrivals_at_rooms_without_touching_departures_when_nonnegative() {
        // rooms=10 today, yesterday's rooms(fallback FstValue)=8; arrivals read as 15 (> rooms)
        // forces arrivals down to 10 and recomputes departs = yrooms - rooms + arrivals =
        // 8-10+10=8, which is >= 0 so the "departs < 0" rewrite branch never fires.
        let date = LocalDate::of(2026, 1, 5);
        let yesterday = date.plus_days(-1);
        let store = FakeStore::new()
            .with_fst(1, date, 10.0)
            .with_fst(1, yesterday, 8.0)
            .with_fst(3, date, 15.0)
            .with_fst(4, date, 3.0);

        let kbi_list = kbi_list_with(vec![
            record(1, KbiType::Input, true, false),
            record(3, KbiType::Input, false, false),
            record(4, KbiType::Input, false, false),
        ]);

        let market_segments = FakeMarketSegments(vec![MarketSegmentRow {
            rooms_kbi_id: KbiId(1),
            arrivals_kbi_id: Some(KbiId(3)),
            guests_kbi_id: None,
            departures_kbi_id: Some(KbiId(4)),
        }]);

        let mut reader_writer = KbiReaderWriter::new(ForecastMode::Rooms);
        check_market_segments(&mut reader_writer, &kbi_list, &store, &store, &market_segments, PropertyId(1), date, date).unwrap();

        // rooms/arrivals are only re-written inside the `departs < 0.0` branch, which doesn't
        // fire here (recomputed departs = 8-10+10 = 8 >= 0) — but the departures KBI's write at
        // the bottom of `checkMarketSegments` is unconditional whenever a departures KBI exists,
        // so it still gets the recapped value (8.0).
        let written = store.written.borrow();
        assert!(written.iter().all(|d| d.kbi_id != KbiId(1) && d.kbi_id != KbiId(3)));
        let departs_write = written.iter().find(|d| d.kbi_id == KbiId(4)).unwrap();
        assert_eq!(departs_write.fst_value(), Some(8.0));
    }

    #[test]
    fn market_segments_rewrites_rooms_and_arrivals_when_departures_go_negative() {
        // rooms=5, yesterday's rooms=10 (yrooms), arrivals=5, departs stored as -20 so the
        // "departs < 0" branch fires: arrivals += (rooms - yrooms) = 5 + (5-10) = 0, departs =
        // yrooms - rooms + arrivals = 10-5+0=5, and roomsKBI/arrivalsKBI get rewritten.
        let date = LocalDate::of(2026, 1, 5);
        let yesterday = date.plus_days(-1);
        let store = FakeStore::new()
            .with_fst(1, date, 5.0)
            .with_fst(1, yesterday, 10.0)
            .with_fst(3, date, 5.0)
            .with_fst(4, date, -20.0);

        let kbi_list = kbi_list_with(vec![
            record(1, KbiType::Input, true, false),
            record(3, KbiType::Input, false, false),
            record(4, KbiType::Input, false, false),
        ]);

        let market_segments = FakeMarketSegments(vec![MarketSegmentRow {
            rooms_kbi_id: KbiId(1),
            arrivals_kbi_id: Some(KbiId(3)),
            guests_kbi_id: None,
            departures_kbi_id: Some(KbiId(4)),
        }]);

        let mut reader_writer = KbiReaderWriter::new(ForecastMode::Rooms);
        check_market_segments(&mut reader_writer, &kbi_list, &store, &store, &market_segments, PropertyId(1), date, date).unwrap();

        let written = store.written.borrow();
        let rooms_write = written.iter().find(|d| d.kbi_id == KbiId(1)).unwrap();
        let arrivals_write = written.iter().find(|d| d.kbi_id == KbiId(3)).unwrap();
        assert_eq!(rooms_write.fst_value(), Some(5.0));
        assert_eq!(arrivals_write.fst_value(), Some(0.0));
    }

    struct FakeRevenueCenters {
        rows: Vec<RevenueCenterKbiRow>,
        plan: Option<CalendarPlan>,
        dates: Vec<CalendarPlanDateRange>,
        period_days: Vec<(i32, bool)>,
    }

    impl RevenueCenterCheckPort for FakeRevenueCenters {
        fn revenue_center_kbis(&self, _property_id: PropertyId, _standard_set_id: StandardSetId) -> Result<Vec<RevenueCenterKbiRow>, ForecasterError> {
            Ok(self.rows.clone())
        }

        fn calendar_plan(&self, _calendar_plan_id: i32) -> Result<Option<CalendarPlan>, ForecasterError> {
            Ok(self.plan)
        }

        fn calendar_plan_dates(&self, _calendar_plan_id: i32) -> Result<Vec<CalendarPlanDateRange>, ForecasterError> {
            Ok(self.dates.clone())
        }

        fn revenue_center_period_days(&self, _revenue_center_period_standard_set_id: i32) -> Result<Vec<(i32, bool)>, ForecasterError> {
            Ok(self.period_days.clone())
        }
    }

    #[test]
    fn revenue_centers_zeroes_dates_inside_the_closed_calendar_range() {
        // A calendar plan closed Jan 1-10 every year, all days otherwise open; Jan 5 falls inside
        // the closed range and should get zeroed even though its day-of-week is "open".
        let date = LocalDate::of(2026, 1, 5);
        let store = FakeStore::new();

        let kbi_list = kbi_list_with(vec![record(9, KbiType::Input, false, true)]);

        let revenue_centers = FakeRevenueCenters {
            rows: vec![RevenueCenterKbiRow {
                kbi_id: KbiId(9),
                revenue_center_period_standard_set_id: 1,
                calendar_plan_id: 42,
            }],
            plan: Some(CalendarPlan { days_open: [true; 7] }),
            dates: vec![CalendarPlanDateRange {
                start_month: 1,
                start_day: 1,
                end_month: 1,
                end_day: 10,
            }],
            period_days: vec![],
        };

        let mut reader_writer = KbiReaderWriter::new(ForecastMode::Revenue);
        check_revenue_centers(
            &mut reader_writer,
            &kbi_list,
            &store,
            &store,
            &revenue_centers,
            PropertyId(1),
            StandardSetId(1),
            date,
            date,
            2026,
        )
        .unwrap();

        let written = store.written.borrow();
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].fst_value(), Some(0.0));
        assert_eq!(written[0].adj_value(), Some(0.0));
    }

    #[test]
    fn revenue_centers_period_day_quirk_only_zeroes_the_first_matching_date() {
        // `RevenueCenterPeriodDay` with DayNo=2 (Monday, Java's Sun=1..Sat=7 convention) closed;
        // the date range spans two Mondays, but Java's `break` after the first write means only
        // the first Monday gets zeroed — ported verbatim, not "fixed" to zero every occurrence.
        let first_monday = LocalDate::of(2026, 1, 5);
        let second_monday = first_monday.plus_days(7);
        let store = FakeStore::new();

        let kbi_list = kbi_list_with(vec![record(9, KbiType::Input, false, true)]);

        let revenue_centers = FakeRevenueCenters {
            rows: vec![RevenueCenterKbiRow {
                kbi_id: KbiId(9),
                revenue_center_period_standard_set_id: 1,
                calendar_plan_id: 0,
            }],
            plan: None,
            dates: vec![],
            period_days: vec![(2, false)],
        };

        let mut reader_writer = KbiReaderWriter::new(ForecastMode::Revenue);
        check_revenue_centers(
            &mut reader_writer,
            &kbi_list,
            &store,
            &store,
            &revenue_centers,
            PropertyId(1),
            StandardSetId(1),
            first_monday,
            second_monday,
            2026,
        )
        .unwrap();

        let written = store.written.borrow();
        assert_eq!(written.len(), 1, "only the first matching Monday should be zeroed, matching Java's break");
    }
}
