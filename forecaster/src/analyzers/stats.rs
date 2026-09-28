//! `Stats.java` — simple sample statistics (mode/min/max/mean/std-deviation) used by
//! `RegressionAnalyzer`'s outlier-rejection pass (`resids` range check).

use crate::analyzers::RowData;

/// `Stats.intervals` — Student's t critical values at 95% confidence for `N - 1` degrees of
/// freedom, `N` in 1..=13 (index 0 unused, matching Java's 1-based `intervals[(int)(N - 1)]`
/// indexing off a 0-based array).
pub const INTERVALS: [f64; 13] = [
    0.0, 12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306, 2.262, 2.228, 2.201, 2.179,
];

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Stats {
    pub minima: f64,
    pub maxima: f64,
    pub range: f64,
    pub sum_x: f64,
    pub mean: f64,
    pub mode: f64,
    pub variance: f64,
    pub std_deviation: f64,
    pub sea_mean: f64,
}

impl Stats {
    pub fn new() -> Self {
        Self::default()
    }

    /// `summarizeStats(RowData sampling)`.
    pub fn summarize_stats(&mut self, sampling: &RowData) {
        let num = sampling.count;
        let mut data = sampling.dataset[..num].to_vec();

        self.calculate_mode(&mut data);
        self.find_min_max(&data);
        self.mat_mean(&data);
        self.mat_std(&data);

        self.sea_mean = if num > 0 {
            self.std_deviation / (num as f64).sqrt()
        } else {
            0.0
        };
    }

    /// `calcInterval(double N)`.
    pub fn calc_interval(&self, n: f64) -> f64 {
        if self.std_deviation != 0.0 && n != 0.0 {
            INTERVALS[(n - 1.0) as usize] * (self.std_deviation / n.sqrt())
        } else {
            f64::MAX
        }
    }

    /// `calculateMode` — sorts `data` in place (matching Java, which mutates the caller's array
    /// too) then takes the midpoint (even count) or middle element (odd count).
    pub fn calculate_mode(&mut self, data: &mut [f64]) {
        let num = data.len();
        let even = num % 2 == 0;

        Self::sort_data(data);

        self.mode = if even {
            let half = num / 2 - 1;
            (data[half] + data[half + 1]) / 2.0
        } else {
            data[num / 2]
        };
    }

    /// `sortData` — Java's hand-rolled insertion sort; a stable ascending sort produces the same
    /// result, so `slice::sort_by` stands in for the literal translation.
    fn sort_data(data: &mut [f64]) {
        if data.len() <= 1 {
            return;
        }
        data.sort_by(|a, b| a.partial_cmp(b).unwrap());
    }

    /// `findMinMax`.
    pub fn find_min_max(&mut self, data: &[f64]) {
        self.minima = data[0];
        self.maxima = data[0];

        for &v in &data[1..] {
            if v < self.minima {
                self.minima = v;
            }
            if v > self.maxima {
                self.maxima = v;
            }
        }

        self.range = self.maxima - self.minima;
    }

    /// `matMean`.
    pub fn mat_mean(&mut self, data: &[f64]) {
        self.sum_x = data.iter().sum();
        self.mean = self.sum_x / data.len() as f64;
    }

    /// `matStd`.
    pub fn mat_std(&mut self, data: &[f64]) {
        let x_sqr: f64 = data.iter().map(|v| v * v).sum();
        let num = data.len();

        self.variance = if num == 1 {
            0.0
        } else {
            (x_sqr - num as f64 * self.mean * self.mean) / (num as f64 - 1.0)
        };

        self.std_deviation = self.variance.abs().sqrt();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_stats_matches_hand_computed_values() {
        // data: 2, 4, 4, 4, 5, 5, 7, 9 -> mean 5, population-style variance per Java's (n-1) denom
        let row = RowData {
            count: 8,
            dataset: vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            weight: 0.0,
        };
        let mut stats = Stats::new();
        stats.summarize_stats(&row);

        assert_eq!(stats.minima, 2.0);
        assert_eq!(stats.maxima, 9.0);
        assert_eq!(stats.range, 7.0);
        assert_eq!(stats.sum_x, 40.0);
        assert_eq!(stats.mean, 5.0);
        // even count of 8 -> midpoint of sorted[3], sorted[4] = (4 + 5) / 2
        assert_eq!(stats.mode, 4.5);
        // sum of squares = 4+16+16+16+25+25+49+81 = 232; variance = (232 - 8*25) / 7 = 32/7
        assert!((stats.variance - 32.0 / 7.0).abs() < 1e-9);
        assert!((stats.std_deviation - (32.0f64 / 7.0).sqrt()).abs() < 1e-9);
    }

    #[test]
    fn summarize_stats_odd_count_uses_middle_element_as_mode() {
        let row = RowData {
            count: 5,
            dataset: vec![9.0, 1.0, 5.0, 3.0, 7.0, 0.0, 0.0],
            weight: 0.0,
        };
        let mut stats = Stats::new();
        stats.summarize_stats(&row);
        assert_eq!(stats.mode, 5.0);
    }

    #[test]
    fn single_sample_has_zero_variance() {
        let row = RowData {
            count: 1,
            dataset: vec![42.0],
            weight: 0.0,
        };
        let mut stats = Stats::new();
        stats.summarize_stats(&row);
        assert_eq!(stats.variance, 0.0);
        assert_eq!(stats.std_deviation, 0.0);
        assert_eq!(stats.sea_mean, 0.0);
    }

    #[test]
    fn calc_interval_uses_the_students_t_table() {
        let mut stats = Stats::new();
        stats.std_deviation = 2.0;
        // N=4 -> intervals[3] = 3.182
        let interval = stats.calc_interval(4.0);
        assert!((interval - 3.182 * (2.0 / 4.0_f64.sqrt())).abs() < 1e-9);
    }

    #[test]
    fn calc_interval_is_max_when_std_deviation_is_zero() {
        let stats = Stats::new();
        assert_eq!(stats.calc_interval(4.0), f64::MAX);
    }
}
