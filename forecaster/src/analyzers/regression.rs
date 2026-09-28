//! `RegressionAnalyzer.java` — multiple linear regression against a set of related KBIs, with an
//! outlier-rejection pass (residuals > 4 std deviations get dropped and the regression re-run) and
//! a Durbin-Watson first-difference fallback when residuals show serial correlation.
//!
//! Every data point this class touches is DB-backed in Java (`RowData.loadDependentData`/
//! `loadIndependentData`, `KBIReaderWriter.readKBIValue`/`readKBIOverrideValue`/
//! `readKBIStatData`, `LaborKBIReaderWriter.findSeasonID`/`findEnvironmentID`/
//! `isGlobalEnvironment`) — collapsed into `RegressionLookupPort` per `DATA_MODEL.md` §3. Unlike
//! `PastAverageAnalyzer`, this is confirmed to be *more* I/O-saturated than
//! `PLAN_FORECASTER.md`'s Phase 2 description assumed when it called this a "pure numeric
//! algorithm" alongside `RStats`/`TAESCalculator` — nearly every branch reads through the port.
//! The control flow (which is the actual algorithm) is still ported in full below, verbatim
//! including one confirmed Java quirk (see `INDEPENDENT_VALUE_OVERRIDE_QUIRK`); only the DB reads
//! are deferred to a real Phase 3 port implementation.
//!
//! `LaborKBIReaderWriter` is confirmed (by grep) to be the only concrete `KBIReaderWriter`
//! subtype in this Gradle module, so every `instanceof LaborKBIReaderWriter` / "else" (budget
//! reader/writer) branch in the Java source is dead code here and is not ported, matching the
//! precedent already established in `formula::context`.

use joda_rs::LocalDate;

use crate::analyzers::{RStats, RowData, Stats, DateEnvIdPair, MAXCOL, MAXSAM};
use crate::engine::ForecasterError;
use crate::KbiId;
use crate::kbi::{KbiStatType, StatOpType, StatRelatedKbi};

/// `KBIStatData.java`'s four value columns, as read by `readKBIStatData`.
#[derive(Debug, Clone, Copy, Default)]
pub struct KbiStatSnapshot {
    pub est: Option<f64>,
    pub adj: Option<f64>,
    pub act: Option<f64>,
    pub fst: Option<f64>,
}

/// See the module doc comment.
pub trait RegressionLookupPort {
    /// `Property.getDaysOut()` (Java then adds the constant 7-day pad itself at each call site).
    fn days_out(&self) -> Result<i32, ForecasterError>;
    fn find_season_id(&self, date: LocalDate) -> Result<i32, ForecasterError>;
    fn find_environment_id(&self, kbi_id: KbiId, date: LocalDate, actual_env: bool) -> Result<i32, ForecasterError>;
    fn is_global_environment(&self, date: LocalDate) -> Result<bool, ForecasterError>;
    /// `LaborKBIReaderWriter.ignoreEnvironmentDOW(environmentID)`.
    fn ignore_environment_dow(&self, environment_id: i32) -> Result<bool, ForecasterError>;
    fn min_stat_date(&self, kbi_id: KbiId) -> Result<LocalDate, ForecasterError>;

    /// `RowData.readKBIValue` — `stat_type: None` means the act→adj→est fallback chain;
    /// `Some(t)` reads that one column directly (`KBIStatData.getValue(t)`).
    fn read_kbi_stat_value(
        &self,
        kbi_id: KbiId,
        date: LocalDate,
        stat_type: Option<KbiStatType>,
    ) -> Result<Option<f64>, ForecasterError>;

    /// `KBIReaderWriter.readKBIValue(kbi, date, statType)`.
    fn read_kbi_value(&self, kbi_id: KbiId, date: LocalDate, stat_type: KbiStatType) -> Result<Option<f64>, ForecasterError>;

    /// `KBIReaderWriter.readKBIOverrideValue`.
    fn read_kbi_override_value(&self, kbi_id: KbiId, date: LocalDate) -> Result<Option<f64>, ForecasterError>;

    /// `KBIReaderWriter.readKBIStatData`.
    fn read_kbi_stat_data(&self, kbi_id: KbiId, date: LocalDate) -> Result<KbiStatSnapshot, ForecasterError>;

    /// `relatedKBI instanceof StatisticalKBI && ((StatisticalKBI) relatedKBI).isTAESKBI(date)`.
    /// Returns `false` for any non-`StatisticalKBI` related KBI, matching Java's `instanceof` guard.
    fn is_taes_kbi(&self, kbi_id: KbiId, date: LocalDate) -> Result<bool, ForecasterError>;

    /// `LaborKBIReaderWriter.getMode() == ForecastMode.ACTUAL`. `ForecastMode` itself is Phase 4
    /// scope (`DATA_MODEL.md` §2); this one boolean is all `RegressionAnalyzer` needs from it.
    fn is_actual_mode(&self) -> bool;
}

/// The analyzed KBI's fields this analyzer actually reads (`kbi.getId()`/`kbi.isRoomsKBI()`).
#[derive(Debug, Clone, Copy)]
pub struct RegressionKbi {
    pub id: KbiId,
    pub is_rooms_kbi: bool,
}

pub struct RegressionAnalyzer;

impl RegressionAnalyzer {
    /// `calculate(TDate date)`.
    pub fn calculate(
        port: &dyn RegressionLookupPort,
        kbi: RegressionKbi,
        related_kbi_list: &[StatRelatedKbi],
        date: LocalDate,
    ) -> Result<Option<f64>, ForecasterError> {
        let env_id = port.find_environment_id(kbi.id, date, false)?;
        let mut date_env_id_list: Vec<DateEnvIdPair> = (0..MAXCOL - 2).map(|_| DateEnvIdPair::new(env_id)).collect();

        let num_related = related_kbi_list.len();
        let mut use_ind = vec![false; if kbi.is_rooms_kbi { num_related + 1 } else { num_related }];

        let mut ind_count = if kbi.is_rooms_kbi { 1 } else { 0 };
        for related in related_kbi_list {
            if related.op_type == StatOpType::Independent {
                ind_count += 1;
            }
        }

        // "configuration error"
        if ind_count == 0 {
            return Ok(None);
        }

        let mut dependent = RowData::new();
        Self::load_dependent_data(port, kbi.id, date, &mut dependent, &mut date_env_id_list)?;

        let mut ind_vars: Vec<RowData> = Vec::new();
        let mut num_ind = 0usize;
        let mut fcst_val = 0.0f64;

        if kbi.is_rooms_kbi {
            let mut room_independent = RowData::new();
            let num_objects =
                Self::load_independent_data(port, kbi.id, &mut date_env_id_list, Some(KbiStatType::Estimated), &mut room_independent)?;

            use_ind[0] = num_objects > ind_count;

            let stat_data = port.read_kbi_stat_data(kbi.id, date)?;
            if let Some(est) = stat_data.est {
                fcst_val = est;
            }
            room_independent.weight = fcst_val;

            ind_vars.push(room_independent);
            num_ind += 1;
        }

        for related in related_kbi_list {
            if related.op_type != StatOpType::Independent {
                continue;
            }

            let mut related_data = RowData::new();
            let num_objects = Self::load_independent_data(port, related.kbi_id, &mut date_env_id_list, None, &mut related_data)?;

            if num_objects > ind_count {
                use_ind[num_ind] = true;
                related_data.weight = Self::independent_value(port, kbi, related.kbi_id, date)?;
                num_ind += 1;
            }

            ind_vars.push(related_data);
        }

        let mut rstat = RStats::new();
        let mut coefficients = vec![0.0; num_ind + 1];
        let mut out_of_range = false;
        let mut max_objects;
        let mut resids = RowData::new();

        loop {
            let mut num_objects = 0usize;
            for i in 0..MAXSAM {
                if date_env_id_list[i].is_valid() {
                    dependent.dataset[num_objects] = dependent.dataset[i];
                    for j in 0..num_ind {
                        if use_ind[j] {
                            ind_vars[j].dataset[num_objects] = ind_vars[j].dataset[i];
                        }
                    }
                    num_objects += 1;
                }
            }
            max_objects = num_objects;

            if num_objects < num_ind {
                return Ok(None);
            }

            let mut independents = vec![0.0; max_objects * num_ind];
            for i in 0..max_objects {
                for j in 0..num_ind {
                    independents[i * num_ind + j] = ind_vars[j].dataset[i];
                }
            }

            coefficients = vec![0.0; num_ind + 1];
            let mut coef_stand_error = vec![0.0; num_ind + 1];
            let mut y_estimates = vec![0.0; max_objects];
            let mut residuals = vec![0.0; max_objects];

            rstat.multi_regress(
                &independents,
                &dependent,
                num_ind,
                max_objects,
                &mut coefficients,
                &mut y_estimates,
                &mut residuals,
                &mut coef_stand_error,
            );

            if rstat.reg_error == 1.0 || rstat.r_sqr_value < 0.001 || rstat.r_sqr_value > 0.999 {
                return Ok(None);
            }

            if !out_of_range {
                resids = RowData::from_array(&residuals);

                let mut stats = Stats::new();
                stats.summarize_stats(&resids);

                let range = stats.std_deviation * 4.0;

                for i in 0..MAXSAM - 2 {
                    date_env_id_list[i].set_valid(false);
                }

                for j in 0..max_objects {
                    if resids.dataset[j] < 0.0 {
                        resids.dataset[j] *= -1.0;
                    }
                    if resids.dataset[j] < range {
                        date_env_id_list[j].set_valid(true);
                    } else {
                        out_of_range = true;
                    }
                }
            }

            for k in 0..max_objects {
                resids.dataset[k] *= -1.0;
            }

            if !out_of_range && rstat.durbin(&resids) != 0 {
                if rstat.dw * rstat.dw < rstat.r_sqr_value {
                    break;
                }

                let mut mean = 0.0;
                let mut means = vec![0.0; num_ind];
                let mut sumxy = vec![0.0; num_ind];
                let mut sumx2 = vec![0.0; num_ind];

                // `for (int i = maxObjects - 2; i >= 0; i--)` — doesn't run at all when
                // `maxObjects < 2`.
                for i in (0..(max_objects as i64 - 1).max(0)).rev() {
                    let i = i as usize;
                    mean += dependent.dataset[i];
                    let y_prime = dependent.dataset[i + 1] - dependent.dataset[i];

                    for n in 0..num_ind {
                        means[n] += ind_vars[n].dataset[i];
                        let x_prime = ind_vars[n].dataset[i + 1] - ind_vars[n].dataset[i];
                        sumx2[n] += x_prime.powi(2);
                        sumxy[n] += x_prime * y_prime;
                    }
                }

                coefficients[0] = mean / (max_objects as f64 - 1.0);
                for n in 0..num_ind {
                    coefficients[n + 1] = sumxy[n] / sumx2[n];
                    coefficients[0] -= coefficients[n + 1] * (means[n] / (max_objects as f64 - 1.0));
                }

                out_of_range = false;
            }

            if !out_of_range {
                break;
            }
        }

        let mut r_val = fcst_val;
        fcst_val = coefficients[0];

        let mut i = 0usize;
        let mut n = 0usize;

        while n < num_ind {
            if use_ind[n] {
                let cur_coeff = coefficients[n + 1];
                let mut cur_sum = ind_vars[n].weight;

                while i < related_kbi_list.len()
                    && matches!(related_kbi_list[i].op_type, StatOpType::Add | StatOpType::Subtract)
                {
                    let related = related_kbi_list[i];
                    let value = Self::add_subtract_value(port, kbi, related.kbi_id, date)?;

                    match related.op_type {
                        StatOpType::Add => cur_sum += value,
                        StatOpType::Subtract => cur_sum -= value,
                        StatOpType::Independent => unreachable!(),
                    }

                    i += 1;
                }

                if cur_coeff != 0.0 {
                    fcst_val += cur_coeff * cur_sum;
                }
            } else {
                while i < related_kbi_list.len()
                    && matches!(related_kbi_list[i].op_type, StatOpType::Add | StatOpType::Subtract)
                {
                    i += 1;
                }
                if i >= num_related {
                    break;
                }
            }

            if i < related_kbi_list.len() {
                if related_kbi_list[i].op_type == StatOpType::Independent {
                    n += 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        let evaluate = fcst_val as i32;
        if fcst_val > 0.0 && evaluate > 0 {
            r_val = fcst_val;
        }

        Ok(Some(Self::t_double_round(r_val)))
    }

    /// The independent-value lookup inside the main related-KBI loop (`RegressionAnalyzer.calculate`,
    /// the block right after `relatedData.loadIndependentData(...)`).
    ///
    /// `INDEPENDENT_VALUE_OVERRIDE_QUIRK`: in the non-actual-mode, "related KBI is not the
    /// analyzed KBI" branch, Java reads `kbiReaderWriter.readKBIOverrideValue(kbi, date)` — the
    /// *analyzed* KBI's override, not the related KBI's — before falling back to the related KBI's
    /// adjusted/estimated value. That looks like a copy-paste bug (every other branch here is
    /// keyed off whichever KBI is actually being evaluated), but it's what production runs today,
    /// so it's ported verbatim rather than "fixed".
    fn independent_value(
        port: &dyn RegressionLookupPort,
        kbi: RegressionKbi,
        related_kbi_id: KbiId,
        date: LocalDate,
    ) -> Result<f64, ForecasterError> {
        if port.is_actual_mode() {
            let stat_value = if related_kbi_id == kbi.id {
                port.read_kbi_stat_data(kbi.id, date)?.act
            } else {
                port.read_kbi_value(related_kbi_id, date, KbiStatType::Actual)?
            };
            return Ok(stat_value.unwrap_or(0.0));
        }

        if related_kbi_id == kbi.id {
            let stat_value = match port.read_kbi_override_value(kbi.id, date)? {
                Some(v) => Some(v),
                None => {
                    let stat_data = port.read_kbi_stat_data(kbi.id, date)?;
                    stat_data.adj.or(stat_data.est)
                }
            };
            return Ok(stat_value.unwrap_or(0.0));
        }

        // See `INDEPENDENT_VALUE_OVERRIDE_QUIRK` above: `kbi`, not `related_kbi_id`.
        let stat_value = match port.read_kbi_override_value(kbi.id, date)? {
            Some(v) => Some(v),
            None => {
                let mut v = port.read_kbi_value(related_kbi_id, date, KbiStatType::Adjusted)?;
                if v.is_none() && !port.is_taes_kbi(related_kbi_id, date)? {
                    v = port.read_kbi_value(related_kbi_id, date, KbiStatType::Estimated)?;
                }
                v
            }
        };
        Ok(stat_value.unwrap_or(0.0))
    }

    /// The `ADD`/`SUBTRACT` value lookup at the bottom of `calculate` (building `curSum`).
    fn add_subtract_value(
        port: &dyn RegressionLookupPort,
        kbi: RegressionKbi,
        related_kbi_id: KbiId,
        date: LocalDate,
    ) -> Result<f64, ForecasterError> {
        if related_kbi_id == kbi.id {
            if let Some(v) = port.read_kbi_override_value(kbi.id, date)? {
                return Ok(v);
            }
            let stat_data = port.read_kbi_stat_data(related_kbi_id, date)?;
            return Ok(stat_data.est.unwrap_or(0.0));
        }

        let stat_value = port.read_kbi_value(related_kbi_id, date, KbiStatType::Estimated)?;
        Ok(stat_value.unwrap_or(0.0))
    }

    /// `TDouble.round(double)` — round-half-up to the nearest whole number, matching Java's use
    /// here (`Double.valueOf(TDouble.round(rVal))`). See the `tdouble-rounding-parity-trap` memory.
    fn t_double_round(value: f64) -> f64 {
        (value + if value >= 0.0 { 0.5 } else { -0.5 }).trunc()
    }

    /// `RowData.loadDependentData`.
    fn load_dependent_data(
        port: &dyn RegressionLookupPort,
        kbi_id: KbiId,
        date: LocalDate,
        dependent: &mut RowData,
        date_env_id_list: &mut [DateEnvIdPair],
    ) -> Result<usize, ForecasterError> {
        let days_out = port.days_out()? + 7;
        let mut current_date = date.minus_days(days_out as i64);

        let fst_env_id = date_env_id_list[0].env_id();
        let season_id = 0;
        let mut days_back: i64 = -7;

        let limit_date = port.min_stat_date(kbi_id)?;

        if fst_env_id != 0 && port.ignore_environment_dow(fst_env_id)? {
            days_back = -1;
        }

        let mut count = 0usize;
        let len = MAXCOL - 2;
        let mut i = 0usize;

        while i < len {
            if current_date.is_before(limit_date) {
                while i < len {
                    date_env_id_list[i].set_valid(false);
                    i += 1;
                }
                break;
            }

            let mut find_season_id = port.find_season_id(current_date)?;
            let mut find_env_id = port.find_environment_id(kbi_id, current_date, true)?;
            let mut find_global_environment = port.is_global_environment(current_date)?;

            let mut no_data = false;
            while season_id != find_season_id || fst_env_id != find_env_id || find_global_environment {
                current_date = Self::step(current_date, days_back);
                if current_date.is_before(limit_date) {
                    while i < len {
                        date_env_id_list[i].set_valid(false);
                        i += 1;
                    }
                    no_data = true;
                    break;
                }

                find_season_id = port.find_season_id(current_date)?;
                find_env_id = port.find_environment_id(kbi_id, current_date, true)?;
                find_global_environment = port.is_global_environment(current_date)?;
            }

            if no_data {
                break;
            }

            let value = port.read_kbi_stat_value(kbi_id, current_date, None)?;

            match value {
                None => {
                    date_env_id_list[i].set_valid(false);
                    current_date = Self::step(current_date, days_back);
                    continue;
                }
                Some(v) if v == 0.0 => {
                    date_env_id_list[i].set_valid(false);
                    current_date = Self::step(current_date, days_back);
                    continue;
                }
                Some(v) => {
                    dependent.dataset[i] = v;
                    date_env_id_list[i].set_date(current_date);
                    current_date = Self::step(current_date, days_back);
                    count += 1;
                }
            }

            i += 1;
        }

        dependent.count = count;
        Ok(count)
    }

    /// `RowData.loadIndependentData`.
    fn load_independent_data(
        port: &dyn RegressionLookupPort,
        kbi_id: KbiId,
        date_env_id_list: &mut [DateEnvIdPair],
        stat_type: Option<KbiStatType>,
        out: &mut RowData,
    ) -> Result<usize, ForecasterError> {
        let fst_env_id = date_env_id_list[0].env_id();
        let limit_date = port.min_stat_date(kbi_id)?;

        let len = MAXCOL - 2;
        let mut count = 0usize;
        let mut i = 0usize;

        while i < len {
            if date_env_id_list[i].is_valid() {
                let Some(current_date) = date_env_id_list[i].date() else {
                    i += 1;
                    continue;
                };

                if current_date.is_before(limit_date) {
                    while i < len {
                        date_env_id_list[i].set_valid(false);
                        i += 1;
                    }
                    break;
                }

                let env_id = port.find_environment_id(kbi_id, current_date, true)?;
                if env_id != fst_env_id && env_id != 0 {
                    date_env_id_list[i].set_valid(false);
                    i += 1;
                    continue;
                }

                let value = port.read_kbi_stat_value(kbi_id, current_date, stat_type)?;
                match value {
                    None => {
                        date_env_id_list[i].set_valid(false);
                    }
                    Some(v) if v == 0.0 => {
                        date_env_id_list[i].set_valid(false);
                    }
                    Some(v) => {
                        out.dataset[i] = v;
                        count += 1;
                    }
                }
            }

            i += 1;
        }

        out.count = count;
        Ok(count)
    }

    fn step(date: LocalDate, days_back: i64) -> LocalDate {
        if days_back < 0 {
            date.minus_days(-days_back)
        } else {
            date.plus_days(days_back)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A fake port whose "database" is two `HashMap<LocalDate, f64>`s (one per KBI id), built so
    /// every date `load_dependent_data` will visit (starting at `date - 7` and stepping back by 7
    /// days) has a matching value in both maps — a deterministic, end-to-end exercise of
    /// `RegressionAnalyzer::calculate`'s full control flow (compress loop, `RStats`, outlier
    /// rejection) rather than any single branch in isolation.
    struct FakePort {
        dependent_values: HashMap<i64, f64>,
        independent_values: HashMap<i64, f64>,
        min_date: LocalDate,
        env_id: i32,
    }

    fn epoch(date: LocalDate) -> i64 {
        (date - LocalDate::of(1970, 1, 1)).to_days()
    }

    impl RegressionLookupPort for FakePort {
        fn days_out(&self) -> Result<i32, ForecasterError> {
            Ok(0)
        }

        fn find_season_id(&self, _date: LocalDate) -> Result<i32, ForecasterError> {
            Ok(0)
        }

        fn find_environment_id(&self, _kbi_id: KbiId, _date: LocalDate, _actual_env: bool) -> Result<i32, ForecasterError> {
            Ok(self.env_id)
        }

        fn is_global_environment(&self, _date: LocalDate) -> Result<bool, ForecasterError> {
            Ok(false)
        }

        fn ignore_environment_dow(&self, _environment_id: i32) -> Result<bool, ForecasterError> {
            Ok(false)
        }

        fn min_stat_date(&self, _kbi_id: KbiId) -> Result<LocalDate, ForecasterError> {
            Ok(self.min_date)
        }

        fn read_kbi_stat_value(
            &self,
            kbi_id: KbiId,
            date: LocalDate,
            _stat_type: Option<KbiStatType>,
        ) -> Result<Option<f64>, ForecasterError> {
            let map = if kbi_id == KbiId(1) { &self.dependent_values } else { &self.independent_values };
            Ok(map.get(&epoch(date)).copied())
        }

        fn read_kbi_value(&self, _kbi_id: KbiId, _date: LocalDate, _stat_type: KbiStatType) -> Result<Option<f64>, ForecasterError> {
            Ok(None)
        }

        fn read_kbi_override_value(&self, _kbi_id: KbiId, _date: LocalDate) -> Result<Option<f64>, ForecasterError> {
            Ok(None)
        }

        fn read_kbi_stat_data(&self, _kbi_id: KbiId, _date: LocalDate) -> Result<KbiStatSnapshot, ForecasterError> {
            Ok(KbiStatSnapshot::default())
        }

        fn is_taes_kbi(&self, _kbi_id: KbiId, _date: LocalDate) -> Result<bool, ForecasterError> {
            Ok(false)
        }

        fn is_actual_mode(&self) -> bool {
            false
        }
    }

    #[test]
    fn calculate_recovers_a_near_linear_relationship_end_to_end() {
        let date = LocalDate::of(2026, 6, 1);
        let mut dependent_values = HashMap::new();
        let mut independent_values = HashMap::new();

        // 13 weeks of data: independent = i, dependent = 2*i + 10 + small bounded noise so the
        // outlier-rejection pass's range check has real headroom instead of comparing
        // near-zero residuals to a near-zero range.
        let mut stat_date = date.minus_days(7);
        for i in 0..13i64 {
            let independent = i as f64;
            let noise = (i % 3) as f64 - 1.0; // -1, 0, 1 repeating
            let dependent = 2.0 * independent + 10.0 + noise;

            independent_values.insert(epoch(stat_date), independent);
            dependent_values.insert(epoch(stat_date), dependent);

            stat_date = stat_date.minus_days(7);
        }

        let port = FakePort {
            dependent_values,
            independent_values,
            min_date: LocalDate::of(2000, 1, 1),
            env_id: 0,
        };

        let kbi = RegressionKbi { id: KbiId(1), is_rooms_kbi: false };
        let related = [StatRelatedKbi::new(KbiId(2), StatOpType::Independent)];

        let result = RegressionAnalyzer::calculate(&port, kbi, &related, date).unwrap();
        let value = result.expect("regression should produce a forecast");

        // The most recent independent value fed as `weight` is i=0 (the closest stat_date),
        // so the forecast should land close to the fitted line's value at x=0 (~10).
        assert!((value - 10.0).abs() <= 2.0, "expected forecast near 10.0, got {value}");
    }

    #[test]
    fn calculate_returns_none_when_no_independent_kbis_and_not_rooms() {
        let port = FakePort {
            dependent_values: HashMap::new(),
            independent_values: HashMap::new(),
            min_date: LocalDate::of(2000, 1, 1),
            env_id: 0,
        };
        let kbi = RegressionKbi { id: KbiId(1), is_rooms_kbi: false };
        let result = RegressionAnalyzer::calculate(&port, kbi, &[], LocalDate::of(2026, 6, 1)).unwrap();
        assert_eq!(result, None);
    }
}
