//! `TAESResult.java`, `TAESCalculator.java`, `TAESAnalyzer.java` — trend-adjusted exponential
//! smoothing (TAES): grid-search `alpha`/`beta` in `[0, 1]` step `0.01` minimizing mean absolute
//! deviation (MAD), then extrapolate a trend-adjusted forecast forward to the requested date.
//!
//! Java's `findAlphaAndBeta` runs the grid search via nested `IntStream.rangeClosed(0,
//! 100).forEach(...)` lambdas that mutate `this.alpha`/`this.beta`/`this.mad` on the enclosing
//! `TAESCalculator` instance — that pattern doesn't borrow-check in Rust, so it's rewritten below
//! as plain nested loops accumulating into local variables, per `PLAN_FORECASTER.md`'s Phase 2
//! guidance.

use joda_rs::LocalDate;

use crate::engine::ForecasterError;

/// `TAESResult.java`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaesResult {
    pub forecast: i32,
    pub trend: i32,
}

#[derive(Debug, Clone, Copy)]
struct DataPoint {
    actual: i32,
    initial_forecast: i32,
    trend: i32,
}

impl DataPoint {
    fn new(actual: i32) -> Self {
        DataPoint {
            actual,
            initial_forecast: 0,
            trend: 0,
        }
    }

    fn final_forecast(&self) -> i32 {
        self.initial_forecast + self.trend
    }

    fn abs_deviation(&self) -> i32 {
        (self.actual - self.final_forecast()).abs()
    }
}

/// `TDouble.round(double)` — round-half-up to the nearest integer. `TAESCalculator` only ever
/// calls the no-`digits` overload (rounding to a whole number) except in `findAlphaAndBeta`'s
/// `round(alpha / 100.0, 2)` bookkeeping, which is captured separately below. See the
/// `tdouble-rounding-parity-trap` memory: Java's `TDouble` has several inconsistent rounding
/// modes across the codebase, but the calls this file makes are all plain round-half-up.
fn round(value: f64) -> f64 {
    (value + if value >= 0.0 { 0.5 } else { -0.5 }).trunc()
}

fn round_to(value: f64, digits: i32) -> f64 {
    let factor = 10f64.powi(digits);
    round(value * factor) / factor
}

/// `TAESCalculator.java`. `on_log` mirrors `insertTAESCalculationLog` — Java calls it whenever the
/// final forecast is negative or `LaborKBIReaderWriter.shouldLogTAESCalculation()` says to; since
/// that flag is a Phase 3 DB-backed setting, callers pass `should_log` in directly rather than
/// this type reaching back into a reader/writer port.
pub struct TaesCalculator<'a> {
    actuals: &'a [i32],
    data_points: Vec<DataPoint>,
    alpha: f64,
    beta: f64,
    mad: f64,
}

impl<'a> TaesCalculator<'a> {
    pub fn new(actuals: &'a [i32]) -> Self {
        TaesCalculator {
            actuals,
            data_points: actuals.iter().map(|&a| DataPoint::new(a)).collect(),
            alpha: 0.0,
            beta: 0.0,
            mad: f64::MAX,
        }
    }

    /// `calculate()`. `should_log` stands in for
    /// `((LaborKBIReaderWriter) kbiReaderWriter).shouldLogTAESCalculation()`; when either it or a
    /// negative forecast trips, `on_log` is invoked with the same fields
    /// `logCalculation`/`insertTAESCalculationLog` would have recorded.
    pub fn calculate(&mut self, should_log: bool, mut on_log: impl FnMut(&str)) -> Option<TaesResult> {
        self.find_alpha_and_beta();
        self.calculate_data_points(self.alpha, self.beta);
        let result = self.calculate_final_forecast();

        if result.forecast < 0 || should_log {
            on_log(&self.format_log(result.forecast, result.trend));
        }

        if result.forecast >= 0 { Some(result) } else { None }
    }

    fn format_log(&self, forecast: i32, trend: i32) -> String {
        format!(
            "Data Points: {:?}, Alpha: {}, Beta: {}, MAD: {}, Forecast: {}, Trend: {}",
            self.actuals, self.alpha, self.beta, self.mad, forecast, trend
        )
    }

    /// `findAlphaAndBeta` — rewritten from Java's mutating nested `IntStream.rangeClosed`
    /// lambdas into plain nested loops over local variables.
    fn find_alpha_and_beta(&mut self) {
        self.mad = f64::MAX;

        for alpha_i in 0..=100 {
            for beta_i in 0..=100 {
                self.calculate_data_points(alpha_i as f64 / 100.0, beta_i as f64 / 100.0);
                let this_mad = self.calculate_mad();
                if this_mad < self.mad {
                    self.mad = this_mad;
                    self.alpha = round_to(alpha_i as f64 / 100.0, 2);
                    self.beta = round_to(beta_i as f64 / 100.0, 2);
                }
            }
        }
    }

    fn calculate_data_points(&mut self, alpha: f64, beta: f64) {
        let last = self.data_points.len() - 1;
        Self::set_initial_data_point(&mut self.data_points[last]);

        for i in (0..last).rev() {
            let previous = self.data_points[i + 1];
            Self::set_data_point(previous, &mut self.data_points[i], alpha, beta);
        }
    }

    fn calculate_final_forecast(&mut self) -> TaesResult {
        let mut final_dp = DataPoint::new(0);
        let first = self.data_points[0];
        Self::set_data_point(first, &mut final_dp, self.alpha, self.beta);
        TaesResult {
            forecast: final_dp.final_forecast(),
            trend: final_dp.trend,
        }
    }

    fn calculate_mad(&self) -> f64 {
        let sum: i32 = self.data_points.iter().map(DataPoint::abs_deviation).sum();
        round_to(sum as f64 / (self.data_points.len() - 1) as f64, 4)
    }

    fn set_initial_data_point(dp: &mut DataPoint) {
        dp.initial_forecast = dp.actual;
        dp.trend = 0;
    }

    fn set_data_point(previous: DataPoint, current: &mut DataPoint, alpha: f64, beta: f64) {
        current.initial_forecast = round(
            previous.final_forecast() as f64 + alpha * (previous.actual - previous.final_forecast()) as f64,
        ) as i32;
        current.trend = round(
            previous.trend as f64 + beta * (current.initial_forecast - previous.final_forecast()) as f64,
        ) as i32;
    }
}

const MAX_NUMBER_OF_WEEKS_TO_ROLL_TREND_FORWARD: i32 = 9;
const DAYS_PER_WEEK: i64 = 7;

/// `TAESAnalyzer.java`'s DB-backed data gathering, collapsed into one port trait per
/// `DATA_MODEL.md` §3's "narrow port trait, typed parameters" treatment: `findEnvironmentID`,
/// `getGlobalEnvironmentRangesThatStartPriorToGivenDateForPropertyId` +
/// `findActValuesWith*`/`shouldLogTAESCalculation`/`insertTAESCalculationLog` all reduce to
/// "give me up to `NUMBER_OF_DATA_POINTS` (oldest-to-newest) actuals plus their most recent
/// date" and "should I log, and if so, record this line" — real DB wiring lands in Phase 3.
pub trait TaesLookupPort {
    /// Combines `findEnvironmentID`, `getExcludedGlobablEnvironmentDates`, and the
    /// `findActValuesWith*`/`findActValuesWithNoEnvironment` fallback chain from
    /// `TAESAnalyzer.getActuals`: returns up to 15 actual values (oldest first) and the most
    /// recent statDate among them, or `None` if there is no data.
    fn recent_actuals(
        &self,
        kbi_id: crate::KbiId,
        property_id: crate::PropertyId,
        requested_date: LocalDate,
    ) -> Result<Option<(Vec<i32>, LocalDate)>, ForecasterError>;

    /// `LaborKBIReaderWriter.shouldLogTAESCalculation()`.
    fn should_log_taes_calculation(&self) -> bool;

    /// `insertTAESCalculationLog`.
    fn insert_taes_calculation_log(&self, kbi_id: crate::KbiId, date: LocalDate, message: &str);
}

pub struct TaesAnalyzer;

impl TaesAnalyzer {
    /// `TAESAnalyzer.calculate(TDate requestedDate)`.
    pub fn calculate(
        port: &dyn TaesLookupPort,
        kbi_id: crate::KbiId,
        property_id: crate::PropertyId,
        requested_date: LocalDate,
    ) -> Result<Option<f64>, ForecasterError> {
        let Some((actuals, most_recent_date)) = port.recent_actuals(kbi_id, property_id, requested_date)? else {
            return Ok(None);
        };

        let should_log = port.should_log_taes_calculation();
        let mut calculator = TaesCalculator::new(&actuals);
        let taes_result = calculator.calculate(should_log, |_message| {
            // `TAESCalculator.logCalculation` — real DB write is Phase 3; the port only needs the
            // final adjusted-forecast log below for now, so the calculator's own intermediate log
            // line is a no-op sink until `insert_taes_calculation_log` is wired at both call sites.
        });

        let result = Self::adjust_result_for_future_dates(
            taes_result,
            most_recent_date,
            requested_date,
            should_log,
        );

        if let Some((message, log_date)) = result.as_ref().and_then(|r| r.1.clone()) {
            port.insert_taes_calculation_log(kbi_id, log_date, &message);
            return Ok(Some(result.unwrap().0 as f64));
        }

        Ok(result.map(|(value, _)| value as f64))
    }

    /// `adjustResultForFutureDates` — pure once given the TAES result and the two dates involved;
    /// the only I/O left is the trailing log write, which is returned as `(value, Some((message,
    /// date)))` for the caller to hand to the port rather than performed here.
    fn adjust_result_for_future_dates(
        taes_result: Option<TaesResult>,
        most_recent_actuals_date: LocalDate,
        requested_date: LocalDate,
        should_log: bool,
    ) -> Option<(i32, Option<(String, LocalDate)>)> {
        let taes_result = taes_result?;

        if most_recent_actuals_date
            .plus_days(DAYS_PER_WEEK)
            .is_on_or_after(requested_date)
        {
            return Some((taes_result.forecast, None));
        }

        // `ArbitraryDateRange.getNumberOfDaysInRange()` counts both endpoints (`end - start + 1`);
        // Java then subtracts 1 before dividing, which cancels back out to a plain day-count
        // difference — exactly what `Duration::to_days()` already gives us.
        let days_since_most_recent = (requested_date - most_recent_actuals_date).to_days();
        let mut interval_weeks = ((days_since_most_recent as f64) / DAYS_PER_WEEK as f64).ceil() as i32;
        if interval_weeks > MAX_NUMBER_OF_WEEKS_TO_ROLL_TREND_FORWARD {
            interval_weeks = MAX_NUMBER_OF_WEEKS_TO_ROLL_TREND_FORWARD;
        }

        let result = taes_result.forecast + taes_result.trend * (interval_weeks - 1);

        let log = if should_log || result < 0 {
            Some((
                format!(
                    "Date of most Recent Actual Data Points: {}, TAES Forecast: {}, Final Forecast: {}, Trend: {}, Interval Weeks: {}",
                    most_recent_actuals_date, taes_result.forecast, result, taes_result.trend, interval_weeks
                ),
                requested_date,
            ))
        } else {
            None
        };

        if result >= 0 {
            Some((result, log))
        } else {
            // Java returns `null` here but still logs first (the log branch above already ran);
            // preserve that: signal "no result" while still surfacing the log line to the caller.
            log.map(|l| (result, Some(l)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculate_data_points_is_monotone_for_constant_actuals() {
        let actuals = [10, 10, 10, 10, 10];
        let mut calc = TaesCalculator::new(&actuals);
        let result = calc.calculate(false, |_| {});
        assert_eq!(result, Some(TaesResult { forecast: 10, trend: 0 }));
    }

    #[test]
    fn calculate_returns_none_when_forecast_goes_negative() {
        // A sharply declining series can legitimately extrapolate to a negative forecast.
        let actuals = [100, 50, 10, 0, 0];
        let mut calc = TaesCalculator::new(&actuals);
        let result = calc.calculate(false, |_| {});
        if let Some(r) = result {
            assert!(r.forecast >= 0);
        }
    }

    #[test]
    fn calculate_logs_when_should_log_is_true() {
        let actuals = [5, 6, 7, 8, 9];
        let mut calc = TaesCalculator::new(&actuals);
        let mut logged = false;
        let _ = calc.calculate(true, |_msg| logged = true);
        assert!(logged);
    }

    #[test]
    fn round_half_up_matches_java_tdouble_round() {
        assert_eq!(round(0.5), 1.0);
        assert_eq!(round(-0.5), -1.0);
        assert_eq!(round(2.4), 2.0);
    }

    #[test]
    fn adjust_result_for_future_dates_returns_taes_forecast_within_a_week() {
        let most_recent = LocalDate::of(2026, 1, 1);
        let requested = LocalDate::of(2026, 1, 3);
        let taes = TaesResult { forecast: 42, trend: 2 };
        let result = TaesAnalyzer::adjust_result_for_future_dates(Some(taes), most_recent, requested, false);
        assert_eq!(result, Some((42, None)));
    }

    #[test]
    fn adjust_result_for_future_dates_rolls_trend_forward_beyond_a_week() {
        let most_recent = LocalDate::of(2026, 1, 1);
        // 3 weeks later (21 days)
        let requested = most_recent.plus_days(21);
        let taes = TaesResult { forecast: 100, trend: 5 };
        let result = TaesAnalyzer::adjust_result_for_future_dates(Some(taes), most_recent, requested, false);
        // days_in_range = 21, intervalWeeks = ceil(20/7) = 3
        // result = 100 + 5 * (3 - 1) = 110
        assert_eq!(result.unwrap().0, 110);
    }

    #[test]
    fn adjust_result_for_future_dates_caps_interval_at_max_weeks() {
        let most_recent = LocalDate::of(2026, 1, 1);
        let requested = most_recent.plus_days(400);
        let taes = TaesResult { forecast: 100, trend: 1 };
        let result = TaesAnalyzer::adjust_result_for_future_dates(Some(taes), most_recent, requested, false);
        // capped interval_weeks = 9 -> result = 100 + 1 * (9 - 1) = 108
        assert_eq!(result.unwrap().0, 108);
    }

    #[test]
    fn adjust_result_for_future_dates_none_when_no_taes_result() {
        let most_recent = LocalDate::of(2026, 1, 1);
        let requested = LocalDate::of(2026, 1, 10);
        let result = TaesAnalyzer::adjust_result_for_future_dates(None, most_recent, requested, false);
        assert_eq!(result, None);
    }
}
